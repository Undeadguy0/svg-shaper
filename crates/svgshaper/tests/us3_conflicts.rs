mod common;

use std::fs;
use std::sync::{Arc, Barrier};
use std::thread;

use svgshaper::{SourceFailure, SourceOutcome, StopFlag, convert};

use common::{TempDir, assert_png, block_on, converter_with, failure, request, write_svg};

#[test]
fn us3_name_collision_writes_nothing() {
    let dir = TempDir::new();
    let first = write_svg(dir.path(), "a.svg");
    let second = write_svg(dir.path(), "b.svg");
    let mut req = request(vec![first, second], 10, 10);
    req.set_template("same.png");
    let report = block_on(convert(req, StopFlag::new())).unwrap();
    assert_eq!(report.outcomes.len(), 2);
    assert_eq!(failure(&report.outcomes[0]), SourceFailure::NameCollision);
    assert_eq!(failure(&report.outcomes[1]), SourceFailure::NameCollision);
    assert!(!dir.path().join("same.png").exists());
    assert!(!dir.path().join("a.png").exists());
    assert!(!dir.path().join("b.png").exists());
}

#[test]
fn us3_replace_publishes_only_after_success() {
    let dir = TempDir::new();
    let svg = write_svg(dir.path(), "logo.svg");
    let png = dir.path().join("logo.png");
    fs::write(&png, b"OLD-BYTES").unwrap();
    let mut req = request(vec![svg], 18, 14);
    req.set_replace(true);
    let report = block_on(convert(req, StopFlag::new())).unwrap();
    assert!(matches!(
        report.outcomes[0].outcome,
        SourceOutcome::Succeeded(_)
    ));
    let bytes = fs::read(&png).unwrap();
    assert_ne!(bytes, b"OLD-BYTES");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    assert_png(&png, 18, 14);
}

#[test]
fn us3_replace_cannot_draw_keeps_the_previous_file() {
    let dir = TempDir::new();
    let svg = write_svg(dir.path(), "logo.svg");
    let png = dir.path().join("logo.png");
    fs::write(&png, b"OLD-BYTES").unwrap();
    let converter = converter_with(dir.path(), &["--exit-code", "1"]);
    let mut req = request(vec![svg], 18, 14);
    req.set_converter(&converter);
    req.set_replace(true);
    let report = block_on(convert(req, StopFlag::new())).unwrap();
    assert_eq!(failure(&report.outcomes[0]), SourceFailure::CannotDraw);
    assert_eq!(fs::read(&png).unwrap(), b"OLD-BYTES");
}

#[test]
fn us3_overlapping_requests_one_success_and_concurrent_write() {
    let dir = TempDir::new();
    let svg = write_svg(dir.path(), "logo.svg");
    let out = dir.path().join("out");
    let converter = converter_with(dir.path(), &["--sleep-ms", "600"]);
    let barrier = Arc::new(Barrier::new(2));

    let spawn = |barrier: Arc<Barrier>| {
        let svg = svg.clone();
        let out = out.clone();
        let converter = converter.clone();
        thread::spawn(move || {
            barrier.wait();
            let mut req = request(vec![svg], 22, 11);
            req.set_destination(&out);
            req.set_converter(&converter);
            req.set_replace(true);
            block_on(convert(req, StopFlag::new()))
        })
    };
    let first = spawn(Arc::clone(&barrier));
    let second = spawn(barrier);
    let left = first.join().unwrap().unwrap();
    let right = second.join().unwrap().unwrap();

    let mut successes = 0;
    let mut concurrent = 0;
    for report in [&left, &right] {
        assert_eq!(report.outcomes.len(), 1);
        match &report.outcomes[0].outcome {
            SourceOutcome::Succeeded(_) => successes += 1,
            SourceOutcome::Failed(SourceFailure::ConcurrentWrite) => concurrent += 1,
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(successes, 1);
    assert_eq!(concurrent, 1);
    assert_png(&out.join("logo.png"), 22, 11);
}
