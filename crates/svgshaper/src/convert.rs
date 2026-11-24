use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crate::error::{RequestError, SourceFailure};
use crate::paths::logical_path;
use crate::request::{
    ConversionReport, ConversionRequest, SourceOutcome, SourceRecord, StopFlag, output_for,
    validate_request,
};
use crate::select::{Selected, select_sources};
use crate::shell::{resolve_converter, spawn_converter};
use crate::store::{Claim, TempPng, png_ihdr, publish};

enum Step {
    Done(SourceOutcome),
    Halt,
    Abort(RequestError),
}

/// Синхронное тело [`crate::convert`]. Выполняется в пуле `blocking`.
pub(crate) fn run(
    request: ConversionRequest,
    stop: StopFlag,
) -> Result<ConversionReport, RequestError> {
    validate_request(&request)?;
    let selected = select_sources(request.sources());
    for item in &selected {
        if let Selected::Ready(source) = item {
            output_for(&request, source)?;
        }
    }

    let any_ready = selected
        .iter()
        .any(|item| matches!(item, Selected::Ready(_)));
    if any_ready {
        resolve_converter(request.converter())?;
        if let Some(destination) = request.destination() {
            fs::create_dir_all(destination).map_err(|_| RequestError::DestinationUnavailable)?;
        }
    }

    let mut counts: HashMap<PathBuf, usize> = HashMap::new();
    for item in &selected {
        if let Selected::Ready(source) = item {
            let output = output_for(&request, source)?;
            *counts.entry(logical_path(&output.path)).or_insert(0) += 1;
        }
    }
    let collisions: HashSet<PathBuf> = counts
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(path, _)| path)
        .collect();

    let program = if any_ready {
        Some(resolve_converter(request.converter())?)
    } else {
        None
    };

    let mut outcomes = Vec::with_capacity(selected.len());
    let mut halted = false;
    let mut published = false;
    for item in selected {
        if halted || stop.is_requested() {
            halted = true;
            outcomes.push(record(item, SourceOutcome::Stopped));
            continue;
        }
        match item {
            Selected::Failed(source, reason) => {
                outcomes.push(SourceRecord {
                    source,
                    outcome: SourceOutcome::Failed(reason),
                });
            }
            Selected::Ready(source) => {
                let output = output_for(&request, &source)?;
                if collisions.contains(&logical_path(&output.path)) {
                    outcomes.push(SourceRecord {
                        source,
                        outcome: SourceOutcome::Failed(SourceFailure::NameCollision),
                    });
                    continue;
                }
                let program = program
                    .as_ref()
                    .expect("программа проверена для готовых источников");
                match convert_one(
                    &request,
                    &stop,
                    program,
                    &source,
                    &output.path,
                    &output.directory,
                ) {
                    Step::Done(outcome) => {
                        if matches!(outcome, SourceOutcome::Succeeded(_)) {
                            published = true;
                        }
                        outcomes.push(SourceRecord { source, outcome });
                    }
                    Step::Halt => {
                        halted = true;
                        outcomes.push(SourceRecord {
                            source,
                            outcome: SourceOutcome::Stopped,
                        });
                    }
                    Step::Abort(err) if !published => return Err(err),
                    Step::Abort(_) => {
                        outcomes.push(SourceRecord {
                            source,
                            outcome: SourceOutcome::Failed(SourceFailure::CannotDraw),
                        });
                    }
                }
            }
        }
    }

    Ok(ConversionReport { outcomes })
}

fn record(item: Selected, outcome: SourceOutcome) -> SourceRecord {
    let source = match item {
        Selected::Ready(source) | Selected::Failed(source, _) => source,
    };
    SourceRecord { source, outcome }
}

fn convert_one(
    request: &ConversionRequest,
    stop: &StopFlag,
    program: &Path,
    source: &crate::request::Source,
    final_path: &Path,
    directory: &Path,
) -> Step {
    if stop.is_requested() {
        return Step::Halt;
    }
    if let Err(reason) = destination_state(final_path, request.replace()) {
        return Step::Done(SourceOutcome::Failed(reason));
    }
    let _claim = match Claim::try_acquire(final_path) {
        Some(claim) => claim,
        None => return Step::Done(SourceOutcome::Failed(SourceFailure::ConcurrentWrite)),
    };
    if stop.is_requested() {
        return Step::Halt;
    }
    // Повтор после захвата: соседний запрос мог успеть опубликовать файл.
    if let Err(reason) = destination_state(final_path, request.replace()) {
        return Step::Done(SourceOutcome::Failed(reason));
    }

    let temp = match TempPng::create(directory) {
        Ok(temp) => temp,
        Err(reason) => return Step::Done(SourceOutcome::Failed(reason)),
    };
    let mut child = match spawn_converter(
        program,
        request.width(),
        request.height(),
        temp.path(),
        &source.path,
    ) {
        Ok(child) => child,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Step::Abort(RequestError::ConverterUnavailable);
        }
        Err(_) => return Step::Done(SourceOutcome::Failed(SourceFailure::CannotDraw)),
    };

    loop {
        if stop.is_requested() {
            let _ = child.kill();
            let _ = child.wait();
            return Step::Halt;
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if stop.is_requested() {
                    return Step::Halt;
                }
                if !status.success() {
                    return Step::Done(SourceOutcome::Failed(SourceFailure::CannotDraw));
                }
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(15)),
            Err(_) => return Step::Done(SourceOutcome::Failed(SourceFailure::CannotDraw)),
        }
    }

    let Some((width, height)) = png_ihdr(temp.path()) else {
        return Step::Done(SourceOutcome::Failed(SourceFailure::CannotDraw));
    };
    if width != request.width() || height != request.height() {
        return Step::Done(SourceOutcome::Failed(SourceFailure::CannotDraw));
    }
    if stop.is_requested() {
        return Step::Halt;
    }
    if publish(temp.path(), final_path).is_err() {
        return Step::Done(SourceOutcome::Failed(SourceFailure::WriteFailed));
    }
    Step::Done(SourceOutcome::Succeeded(final_path.to_path_buf()))
}

fn destination_state(path: &Path, replace: bool) -> Result<(), SourceFailure> {
    match fs::symlink_metadata(path) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(SourceFailure::WriteFailed),
        Ok(meta) => {
            let is_dir =
                meta.is_dir() || fs::metadata(path).is_ok_and(|followed| followed.is_dir());
            if is_dir {
                Err(SourceFailure::WriteFailed)
            } else if replace {
                Ok(())
            } else {
                Err(SourceFailure::NameTaken)
            }
        }
    }
}
