use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

use crate::error::SourceFailure;
use crate::paths::logical_path;

static CLAIMS: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

/// Захват конечного пути на время одного источника. Снимается при drop.
pub(crate) struct Claim {
    key: PathBuf,
}

impl Claim {
    pub(crate) fn try_acquire(path: &Path) -> Option<Self> {
        let key = logical_path(path);
        let mut guard = CLAIMS.lock().unwrap_or_else(|err| err.into_inner());
        if guard.insert(key.clone()) {
            Some(Self { key })
        } else {
            None
        }
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        let mut guard = CLAIMS.lock().unwrap_or_else(|err| err.into_inner());
        guard.remove(&self.key);
    }
}

/// Временный PNG в папке назначения. Пока значение живо, файл удаляется.
pub(crate) struct TempPng {
    path: PathBuf,
}

impl TempPng {
    pub(crate) fn create(directory: &Path) -> Result<Self, SourceFailure> {
        let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let path = directory.join(format!(".svgshaper-{}-{seq}.png.part", std::process::id()));
        File::create(&path).map_err(|_| SourceFailure::WriteFailed)?;
        Ok(Self { path })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempPng {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub(crate) fn publish(temp: &Path, final_path: &Path) -> Result<(), SourceFailure> {
    fs::rename(temp, final_path).map_err(|_| SourceFailure::WriteFailed)
}

/// Ширина и высота из IHDR. `None`, если это не PNG или кусок короче заголовка.
pub(crate) fn png_ihdr(path: &Path) -> Option<(u32, u32)> {
    let mut file = File::open(path).ok()?;
    let mut buf = [0u8; 24];
    file.read_exact(&mut buf).ok()?;
    const SIG: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
    if buf[0..8] != SIG || &buf[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(buf[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(buf[20..24].try_into().ok()?);
    Some((width, height))
}
