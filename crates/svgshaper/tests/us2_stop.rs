mod common;

use std::fs;
use std::thread;
use std::time::{Duration, Instant};

use svgshaper::{SourceOutcome, StopFlag, convert};

use common::{
    TempDir, assert_no_partials, assert_png, block_on, converter_with, request, write_svg,
};

#[test]
fn us2_stop_keeps_the_finished_png() {
    let dir = TempDir::new();
    let a = write_svg(dir.path(), "a.svg");
    let b = write_svg(dir.path(), "b.svg");
    let c = write_svg(dir.path(), "c.svg");
    let converter = converter_with(dir.path(), &["--sleep-if", "b.svg=2000"]);
    let mut req = request(vec![a, b, c], 16, 16);
    req.set_converter(&converter);
    let stop = StopFlag::new();
    let flag = stop.clone();
    let handle = thread::spawn(move || block_on(convert(req, flag)));

    let png = dir.path().join("a.png");
    let deadline = Instant::now() + Duration::from_secs(2);
    while !png.is_file() {
        assert!(Instant::now() < deadline, "a.png не появился за 2 с");
        thread::sleep(Duration::from_millis(10));
    }
    stop.request();
    let report = handle.join().unwrap().unwrap();

    assert_eq!(report.outcomes.len(), 3);
    assert!(report.outcomes[0].source.path.ends_with("a.svg"));
    assert!(matches!(
        report.outcomes[0].outcome,
        SourceOutcome::Succeeded(_)
    ));
    assert!(matches!(report.outcomes[1].outcome, SourceOutcome::Stopped));
    assert!(report.outcomes[1].source.path.ends_with("b.svg"));
    assert!(matches!(report.outcomes[2].outcome, SourceOutcome::Stopped));
    assert!(report.outcomes[2].source.path.ends_with("c.svg"));
    assert_png(&png, 16, 16);
    assert!(!dir.path().join("b.png").exists());
    assert!(!dir.path().join("c.png").exists());
    assert_no_partials(dir.path());
}

#[test]
fn us2_stop_during_replace_keeps_the_previous_file() {
    let dir = TempDir::new();
    let svg = write_svg(dir.path(), "logo.svg");
    let png = dir.path().join("logo.png");
    fs::write(&png, b"KEEP-ME").unwrap();
    let converter = converter_with(dir.path(), &["--sleep-ms", "1500"]);
    let mut req = request(vec![svg], 32, 16);
    req.set_converter(&converter);
    req.set_replace(true);
    let stop = StopFlag::new();
    let flag = stop.clone();
    let handle = thread::spawn(move || block_on(convert(req, flag)));
    thread::sleep(Duration::from_millis(30));
    stop.request();
    let report = handle.join().unwrap().unwrap();
    assert_eq!(report.outcomes.len(), 1);
    assert!(matches!(report.outcomes[0].outcome, SourceOutcome::Stopped));
    assert_eq!(fs::read(&png).unwrap(), b"KEEP-ME");
    assert_no_partials(dir.path());
}
