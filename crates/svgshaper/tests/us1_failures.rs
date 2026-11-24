mod common;

use std::fs;

use svgshaper::{
    ConversionRequest, RequestError, SourceFailure, SourceOutcome, StopFlag, convert,
    parse_pixel_side, try_pixel_side,
};

use common::{
    TempDir, assert_no_partials, assert_png, block_on, converter_with, failure, request, write_svg,
};

#[test]
fn us1_invalid_size_is_rejected_at_construction() {
    assert_eq!(
        ConversionRequest::new(vec![], 0, 32).unwrap_err(),
        RequestError::InvalidSize
    );
    assert_eq!(
        ConversionRequest::new(vec![], 16385, 32).unwrap_err(),
        RequestError::InvalidSize
    );
    assert_eq!(try_pixel_side(0), Err(RequestError::InvalidSize));
    assert_eq!(try_pixel_side(-4), Err(RequestError::InvalidSize));
    assert_eq!(try_pixel_side(16385), Err(RequestError::InvalidSize));
    assert_eq!(parse_pixel_side("1.5"), Err(RequestError::InvalidSize));
    assert_eq!(parse_pixel_side("32.0"), Err(RequestError::InvalidSize));
    assert_eq!(
        ConversionRequest::try_new(vec![], "12.5", "32").unwrap_err(),
        RequestError::InvalidSize
    );
    assert!(ConversionRequest::try_new(vec![], "64", "32").is_ok());
}

#[test]
fn us1_empty_sources_write_nothing() {
    let dir = TempDir::new();
    let dest = dir.path().join("out");
    let mut req = ConversionRequest::new(vec![], 8, 8).unwrap();
    req.set_destination(&dest);
    let err = block_on(convert(req, StopFlag::new())).unwrap_err();
    assert_eq!(err, RequestError::EmptySources);
    assert!(!dest.exists());
}

#[test]
fn us1_not_found_does_not_cancel_success() {
    let dir = TempDir::new();
    let svg = write_svg(dir.path(), "logo.svg");
    let missing = dir.path().join("missing.svg");
    let report = block_on(convert(
        request(vec![svg, missing], 16, 16),
        StopFlag::new(),
    ))
    .unwrap();
    assert_eq!(report.outcomes.len(), 2);
    assert!(matches!(
        report.outcomes[0].outcome,
        SourceOutcome::Succeeded(_)
    ));
    assert_eq!(failure(&report.outcomes[1]), SourceFailure::NotFound);
    assert_png(&dir.path().join("logo.png"), 16, 16);
    assert!(!dir.path().join("missing.png").exists());
}

#[test]
fn us1_explicit_non_svg_is_not_svg() {
    let dir = TempDir::new();
    let notes = dir.path().join("notes.txt");
    fs::write(&notes, b"hello").unwrap();
    let report = block_on(convert(request(vec![notes], 8, 8), StopFlag::new())).unwrap();
    assert_eq!(failure(&report.outcomes[0]), SourceFailure::NotSvg);
    assert!(!dir.path().join("notes.png").exists());
}

#[test]
fn us1_name_taken_leaves_the_old_bytes() {
    let dir = TempDir::new();
    let svg = write_svg(dir.path(), "logo.svg");
    let png = dir.path().join("logo.png");
    fs::write(&png, b"OLD-BYTES").unwrap();
    let report = block_on(convert(request(vec![svg], 8, 8), StopFlag::new())).unwrap();
    assert_eq!(failure(&report.outcomes[0]), SourceFailure::NameTaken);
    assert_eq!(fs::read(&png).unwrap(), b"OLD-BYTES");
}

#[test]
fn us1_exit_code_is_cannot_draw_and_leaves_no_png() {
    let dir = TempDir::new();
    let svg = write_svg(dir.path(), "logo.svg");
    let converter = converter_with(dir.path(), &["--exit-code", "1"]);
    let mut req = request(vec![svg], 8, 8);
    req.set_converter(&converter);
    let report = block_on(convert(req, StopFlag::new())).unwrap();
    assert_eq!(failure(&report.outcomes[0]), SourceFailure::CannotDraw);
    assert!(!dir.path().join("logo.png").exists());
    assert_no_partials(dir.path());
}
