#![allow(dead_code)]

use std::fs;
use std::future::Future;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use svgshaper::{ConversionRequest, SourceOutcome, SourceRecord};

static WRAPPER_SEQ: AtomicU64 = AtomicU64::new(1);

pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("svgshaper-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn converter_bin() -> PathBuf {
    PathBuf::from(env!("SVGSHAPER_TEST_CONVERTER"))
}

pub fn converter_with(dir: &Path, extra: &[&str]) -> PathBuf {
    let seq = WRAPPER_SEQ.fetch_add(1, Ordering::Relaxed);
    let script = dir.join(format!("wrapper-{seq}.sh"));
    let extras = extra
        .iter()
        .map(|arg| shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" ");
    let body = format!(
        "#!/bin/sh\nexec {} {extras} \"$@\"\n",
        shell_quote(&converter_bin().to_string_lossy())
    );
    fs::write(&script, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).unwrap();
    }
    script
}

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

pub fn write_svg(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>").unwrap();
    path
}

pub fn request(sources: Vec<PathBuf>, width: u32, height: u32) -> ConversionRequest {
    let mut request = ConversionRequest::new(sources, width, height).unwrap();
    request.set_converter(converter_bin());
    request
}

pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    drive(future.as_mut())
}

pub fn drive<F: Future>(mut future: Pin<&mut F>) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut context = Context::from_waker(&waker);
    let started = Instant::now();
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => {
                if started.elapsed() > Duration::from_secs(30) {
                    panic!("future не завершилась за 30 с");
                }
                thread::park_timeout(Duration::from_millis(20));
            }
        }
    }
}

struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

pub fn png_size(path: &Path) -> Option<(u32, u32)> {
    let mut file = fs::File::open(path).ok()?;
    let mut buf = [0u8; 24];
    file.read_exact(&mut buf).ok()?;
    if buf[0..8] != [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'] || &buf[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(buf[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(buf[20..24].try_into().ok()?);
    Some((width, height))
}

pub fn assert_png(path: &Path, width: u32, height: u32) {
    assert_eq!(
        png_size(path),
        Some((width, height)),
        "IHDR {path:?} не равен {width}x{height}"
    );
}

pub fn assert_no_partials(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            assert_no_partials(&path);
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        assert!(
            !name.ends_with(".png.part"),
            "остался временный файл {path:?}"
        );
    }
}

pub fn failure(record: &SourceRecord) -> svgshaper::SourceFailure {
    match &record.outcome {
        SourceOutcome::Failed(reason) => *reason,
        other => panic!("ожидался неуспех, получено {other:?}"),
    }
}

pub fn record_named<'a>(records: &'a [SourceRecord], name: &str) -> &'a SourceRecord {
    records
        .iter()
        .find(|record| record.source.path.ends_with(name))
        .unwrap_or_else(|| panic!("нет источника {name}"))
}
