mod common;

use std::fs;
use std::os::unix::fs::symlink;

use svgshaper::{SourceFailure, SourceOutcome, StopFlag, convert};

use common::{TempDir, block_on, failure, request, write_svg};

#[test]
fn us2_symlink_to_directory_is_not_opened() {
    let dir = TempDir::new();
    let real = dir.path().join("real");
    fs::create_dir(&real).unwrap();
    write_svg(&real, "a.svg");
    let link = dir.path().join("via-link");
    symlink(&real, &link).unwrap();

    let report = block_on(convert(
        request(vec![link.clone()], 16, 16),
        StopFlag::new(),
    ))
    .unwrap();
    assert_eq!(report.outcomes.len(), 1);
    assert!(report.outcomes[0].source.path.ends_with("via-link"));
    assert_eq!(failure(&report.outcomes[0]), SourceFailure::NotAFile);
    assert!(matches!(
        report.outcomes[0].outcome,
        SourceOutcome::Failed(SourceFailure::NotAFile)
    ));
    assert!(!real.join("a.png").exists());
    assert!(!dir.path().join("a.png").exists());
    assert!(!link.join("a.png").exists());
}
