use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use crate::error::RequestError;

/// Найти программу. Имя без разделителя ищется в `PATH`, путь проверяется как файл.
pub(crate) fn resolve_converter(converter: &Path) -> Result<PathBuf, RequestError> {
    let program = if is_explicit_path(converter) {
        converter.to_path_buf()
    } else {
        search_path(converter).ok_or(RequestError::ConverterUnavailable)?
    };
    if !executable_file(&program) {
        return Err(RequestError::ConverterUnavailable);
    }
    Ok(program)
}

fn is_explicit_path(path: &Path) -> bool {
    path.is_absolute() || path.components().count() != 1
}

fn search_path(name: &Path) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn executable_file(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// `program --width W --height H --format png --output TEMP SOURCE`.
/// Без оболочки, без `--keep-aspect-ratio` и без `--background-color`.
pub(crate) fn spawn_converter(
    program: &Path,
    width: u32,
    height: u32,
    temp: &Path,
    source: &Path,
) -> io::Result<Child> {
    Command::new(program)
        .arg("--width")
        .arg(width.to_string())
        .arg("--height")
        .arg(height.to_string())
        .arg("--format")
        .arg("png")
        .arg("--output")
        .arg(temp)
        .arg(source)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
}
