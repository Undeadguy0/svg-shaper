mod common;

use std::fs;

use svgshaper::{SourceFailure, SourceOutcome, StopFlag, convert};

use common::{TempDir, assert_png, block_on, failure, record_named, request, write_svg};

#[test]
fn us3_directory_at_output_path_is_write_failed_and_sibling_succeeds() {
    let dir = TempDir::new();
    let dest = dir.path().join("out");
    fs::create_dir(&dest).unwrap();
    fs::create_dir(dest.join("logo.png")).unwrap();
    let logo = write_svg(dir.path(), "logo.svg");
    let other = write_svg(dir.path(), "other.svg");
    let mut req = request(vec![logo, other], 15, 9);
    req.set_destination(&dest);

    let report = block_on(convert(req, StopFlag::new())).unwrap();
    assert_eq!(
        failure(record_named(&report.outcomes, "logo.svg")),
        SourceFailure::WriteFailed
    );
    assert!(matches!(
        record_named(&report.outcomes, "other.svg").outcome,
        SourceOutcome::Succeeded(_)
    ));
    assert!(dest.join("logo.png").is_dir());
    assert_png(&dest.join("other.png"), 15, 9);
}
