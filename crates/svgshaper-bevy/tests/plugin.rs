use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bevy::prelude::*;

use svgshaper::{ConversionRequest, SourceOutcome, StopFlag};
use svgshaper_bevy::{ConversionFinished, PendingConversions, SvgShaperPlugin};

#[derive(Resource, Default)]
struct Collected(Vec<Result<svgshaper::ConversionReport, svgshaper::RequestError>>);

fn collect(mut reader: MessageReader<ConversionFinished>, mut collected: ResMut<Collected>) {
    for message in reader.read() {
        collected.0.push(message.0.clone());
    }
}

#[test]
fn plugin_publishes_one_report_without_blocking() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, SvgShaperPlugin));
    app.init_resource::<Collected>();
    app.add_systems(Update, collect);
    app.update();

    let dir = temp_dir();
    let svg = dir.join("logo.svg");
    fs::write(&svg, b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>").unwrap();
    let converter = wrapper(&dir, &["--sleep-ms", "300"]);
    let mut request = ConversionRequest::new(vec![svg], 24, 18).unwrap();
    request.set_converter(&converter);
    app.insert_resource(PendingConversions {
        queue: vec![(request, StopFlag::new())],
    });

    let started = Instant::now();
    app.update();
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(100),
        "кадр с постановкой задачи занял {elapsed:?}"
    );

    let deadline = Instant::now() + Duration::from_secs(3);
    while app.world().resource::<Collected>().0.len() != 1 {
        assert!(Instant::now() < deadline, "итог плагина не появился за 3 с");
        app.update();
        std::thread::sleep(Duration::from_millis(15));
    }
    app.update();
    let collected = &app.world().resource::<Collected>().0;
    assert_eq!(collected.len(), 1);
    let report = collected[0].as_ref().unwrap();
    let png = dir.join("logo.png");
    match &report.outcomes[0].outcome {
        SourceOutcome::Succeeded(path) => assert_eq!(path, &png),
        other => panic!("{other:?}"),
    }
    assert_eq!(png_size(&png), Some((24, 18)));
    let _ = fs::remove_dir_all(&dir);
}

fn temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("svgshaper-bevy-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn wrapper(dir: &Path, extra: &[&str]) -> PathBuf {
    let bin = PathBuf::from(env!("SVGSHAPER_TEST_CONVERTER"));
    let script = dir.join("wrapper.sh");
    let extras = extra
        .iter()
        .map(|arg| format!("'{}'", arg.replace('\'', "'\\''")))
        .collect::<Vec<_>>()
        .join(" ");
    let body = format!(
        "#!/bin/sh\nexec '{}' {extras} \"$@\"\n",
        bin.display().to_string().replace('\'', "'\\''")
    );
    fs::write(&script, body).unwrap();
    let mut perms = fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script, perms).unwrap();
    script
}

fn png_size(path: &Path) -> Option<(u32, u32)> {
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
