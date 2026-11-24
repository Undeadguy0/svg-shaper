mod common;

use std::fs;

use svgshaper::{RequestError, SourceOutcome, StopFlag, convert};

use common::{TempDir, assert_png, block_on, request, write_svg};

#[test]
fn us3_prefix_suffix_and_destination() {
    let dir = TempDir::new();
    let icons = dir.path().join("icons");
    fs::create_dir(&icons).unwrap();
    let svg = write_svg(&icons, "logo.svg");
    let out = dir.path().join("out");
    let mut req = request(vec![svg], 20, 10);
    req.set_destination(&out);
    req.set_prefix("ico_");
    req.set_suffix("_2x");

    let report = block_on(convert(req, StopFlag::new())).unwrap();
    let png = out.join("ico_logo_2x.png");
    assert_eq!(report.outcomes.len(), 1);
    match &report.outcomes[0].outcome {
        SourceOutcome::Succeeded(path) => assert_eq!(path, &png),
        other => panic!("{other:?}"),
    }
    assert!(out.is_dir());
    assert_png(&png, 20, 10);
    assert!(!icons.join("logo.png").exists());
    assert!(!icons.join("ico_logo_2x.png").exists());
}

#[test]
fn us3_template_gains_one_png_suffix() {
    let dir = TempDir::new();
    let svg = write_svg(dir.path(), "logo.svg");
    let mut req = request(vec![svg], 8, 8);
    req.set_template("{name}");
    let report = block_on(convert(req, StopFlag::new())).unwrap();
    let png = dir.path().join("logo.png");
    assert!(matches!(
        report.outcomes[0].outcome,
        SourceOutcome::Succeeded(_)
    ));
    assert_png(&png, 8, 8);
    assert!(!dir.path().join("logo.png.png").exists());
}

#[test]
fn us3_template_ending_in_png_is_not_doubled() {
    let dir = TempDir::new();
    let svg = write_svg(dir.path(), "logo.svg");
    let mut lower = request(vec![svg.clone()], 8, 6);
    lower.set_template("{name}.png");
    block_on(convert(lower, StopFlag::new())).unwrap();
    assert_png(&dir.path().join("logo.png"), 8, 6);
    assert!(!dir.path().join("logo.png.png").exists());

    let upper_dir = TempDir::new();
    let upper_svg = write_svg(upper_dir.path(), "logo.svg");
    let mut upper = request(vec![upper_svg], 8, 6);
    upper.set_template("{name}.PNG");
    let report = block_on(convert(upper, StopFlag::new())).unwrap();
    match &report.outcomes[0].outcome {
        SourceOutcome::Succeeded(path) => assert_eq!(path, &upper_dir.path().join("logo.PNG")),
        other => panic!("{other:?}"),
    }
    assert!(!upper_dir.path().join("logo.PNG.png").exists());
    assert!(!upper_dir.path().join("logo.png").exists());
}

#[test]
fn us3_separator_in_prefix_is_invalid_name_before_write() {
    let dir = TempDir::new();
    let svg = write_svg(dir.path(), "logo.svg");
    let out = dir.path().join("out");
    let mut req = request(vec![svg], 8, 8);
    req.set_destination(&out);
    req.set_prefix("a/b");
    let error = block_on(convert(req, StopFlag::new())).unwrap_err();
    assert_eq!(error, RequestError::InvalidName);
    assert!(!out.exists());
    assert!(!dir.path().join("logo.png").exists());
}

#[test]
fn us3_destination_under_a_file_is_unavailable() {
    let dir = TempDir::new();
    let svg = write_svg(dir.path(), "logo.svg");
    let blocker = dir.path().join("blocker");
    fs::write(&blocker, b"not-a-directory").unwrap();
    let mut req = request(vec![svg], 8, 8);
    req.set_destination(blocker.join("out"));
    let error = block_on(convert(req, StopFlag::new())).unwrap_err();
    assert_eq!(error, RequestError::DestinationUnavailable);
    assert!(!dir.path().join("logo.png").exists());
    assert!(!blocker.join("out").exists());
}
