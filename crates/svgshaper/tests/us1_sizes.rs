mod common;

use svgshaper::{SourceOutcome, StopFlag, convert};

use common::{TempDir, assert_no_partials, assert_png, block_on, request, write_svg};

#[test]
fn us1_one_svg_is_64_by_32() {
    let dir = TempDir::new();
    let svg = write_svg(dir.path(), "logo.svg");
    let report = block_on(convert(request(vec![svg], 64, 32), StopFlag::new())).unwrap();
    assert_eq!(report.outcomes.len(), 1);
    let png = dir.path().join("logo.png");
    match &report.outcomes[0].outcome {
        SourceOutcome::Succeeded(path) => assert_eq!(path, &png),
        other => panic!("{other:?}"),
    }
    assert_png(&png, 64, 32);
    assert_no_partials(dir.path());
}

#[test]
fn us1_two_svgs_are_128() {
    let dir = TempDir::new();
    let first = write_svg(dir.path(), "a.svg");
    let second = write_svg(dir.path(), "b.svg");
    let report = block_on(convert(
        request(vec![first, second], 128, 128),
        StopFlag::new(),
    ))
    .unwrap();
    assert_eq!(report.outcomes.len(), 2);
    assert!(
        report
            .outcomes
            .iter()
            .all(|record| matches!(record.outcome, SourceOutcome::Succeeded(_)))
    );
    assert_png(&dir.path().join("a.png"), 128, 128);
    assert_png(&dir.path().join("b.png"), 128, 128);
}
