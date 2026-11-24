use std::path::{Component, Path, PathBuf};

/// Абсолютный путь без `.` и `..`. Ссылки не раскрываются: файла может ещё не быть.
pub(crate) fn logical_path(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut out = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}
