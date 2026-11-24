use std::collections::HashSet;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use crate::error::SourceFailure;
use crate::paths::logical_path;
use crate::request::{Source, is_svg_name, stem_of};

pub(crate) enum Selected {
    Ready(Source),
    Failed(Source, SourceFailure),
}

impl Selected {
    pub(crate) fn source(&self) -> &Source {
        match self {
            Self::Ready(source) | Self::Failed(source, _) => source,
        }
    }
}

/// Раскрыть пути запроса: один уровень папки, повтор того же пути — один источник.
pub(crate) fn select_sources(paths: &[PathBuf]) -> Vec<Selected> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for path in paths {
        push_explicit(path, &mut seen, &mut out);
    }
    out
}

fn push_explicit(path: &Path, seen: &mut HashSet<PathBuf>, out: &mut Vec<Selected>) {
    match classify_explicit(path) {
        Explicit::Directory(dir) => {
            let children = match fs::read_dir(&dir) {
                Ok(reader) => reader,
                Err(_) => {
                    push_unique(
                        Selected::Failed(source_at(path), SourceFailure::Unreadable),
                        seen,
                        out,
                    );
                    return;
                }
            };
            let mut entries: Vec<PathBuf> = children
                .filter_map(|entry| entry.ok().map(|item| item.path()))
                .collect();
            entries.sort();
            for child in entries {
                if let Some(selected) = classify_child(&child) {
                    push_unique(selected, seen, out);
                }
            }
        }
        Explicit::One(selected) => push_unique(selected, seen, out),
    }
}

enum Explicit {
    Directory(PathBuf),
    One(Selected),
}

fn classify_explicit(path: &Path) -> Explicit {
    let svg_name = is_svg_name(path);
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            let reason = if svg_name {
                SourceFailure::NotFound
            } else {
                SourceFailure::NotSvg
            };
            return Explicit::One(Selected::Failed(source_at(path), reason));
        }
        Err(_) => {
            return Explicit::One(Selected::Failed(source_at(path), SourceFailure::Unreadable));
        }
    };

    if meta.file_type().is_symlink() {
        return Explicit::One(classify_symlink(path, svg_name));
    }
    if meta.is_dir() {
        return Explicit::Directory(path.to_path_buf());
    }
    if meta.is_file() {
        return Explicit::One(classify_file(path, svg_name));
    }
    let reason = if svg_name {
        SourceFailure::NotAFile
    } else {
        SourceFailure::NotSvg
    };
    Explicit::One(Selected::Failed(source_at(path), reason))
}

fn classify_symlink(path: &Path, svg_name: bool) -> Selected {
    match fs::metadata(path) {
        Err(_) => Selected::Failed(source_at(path), SourceFailure::NotAFile),
        Ok(meta) if meta.is_dir() => Selected::Failed(source_at(path), SourceFailure::NotAFile),
        Ok(meta) if meta.is_file() => classify_file(path, svg_name),
        Ok(_) => Selected::Failed(source_at(path), SourceFailure::NotAFile),
    }
}

fn classify_file(path: &Path, svg_name: bool) -> Selected {
    if !svg_name {
        return Selected::Failed(source_at(path), SourceFailure::NotSvg);
    }
    match File::open(path) {
        Ok(_) => Selected::Ready(source_at(path)),
        Err(_) => Selected::Failed(source_at(path), SourceFailure::Unreadable),
    }
}

/// Потомок папки. `None` — элемент пропускается и не становится источником.
fn classify_child(path: &Path) -> Option<Selected> {
    if !is_svg_name(path) {
        return None;
    }
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(_) => {
            return Some(Selected::Failed(source_at(path), SourceFailure::NotAFile));
        }
    };
    if meta.file_type().is_symlink() {
        return Some(match fs::metadata(path) {
            Ok(target) if target.is_file() => classify_file(path, true),
            _ => Selected::Failed(source_at(path), SourceFailure::NotAFile),
        });
    }
    if meta.is_dir() || !meta.is_file() {
        return Some(Selected::Failed(source_at(path), SourceFailure::NotAFile));
    }
    Some(classify_file(path, true))
}

fn push_unique(selected: Selected, seen: &mut HashSet<PathBuf>, out: &mut Vec<Selected>) {
    let key = logical_path(&selected.source().path);
    if seen.insert(key) {
        out.push(selected);
    }
}

fn source_at(path: &Path) -> Source {
    Source {
        path: path.to_path_buf(),
        stem: stem_of(path),
    }
}
