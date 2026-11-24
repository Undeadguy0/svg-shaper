mod common;

use std::fs;
use std::os::unix::fs::{FileTypeExt, PermissionsExt, symlink};

use svgshaper::{SourceFailure, SourceOutcome, StopFlag, convert};

use common::{TempDir, assert_png, block_on, failure, record_named, request, write_svg};

#[test]
fn us1_unreadable_broken_link_and_uppercase_svg() {
    let dir = TempDir::new();
    let ok = write_svg(dir.path(), "ok.svg");
    let secret = write_svg(dir.path(), "secret.svg");
    let mut perms = fs::metadata(&secret).unwrap().permissions();
    perms.set_mode(0);
    fs::set_permissions(&secret, perms).unwrap();

    let broken = dir.path().join("broken.svg");
    symlink("missing-target", &broken).unwrap();
    let fifo = dir.path().join("notfile.svg");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let upper = write_svg(dir.path(), "logo.SVG");

    let report = block_on(convert(
        request(
            vec![ok, secret, broken, dir.path().join("notfile.svg"), upper],
            10,
            12,
        ),
        StopFlag::new(),
    ))
    .unwrap();

    assert!(matches!(
        record_named(&report.outcomes, "ok.svg").outcome,
        SourceOutcome::Succeeded(_)
    ));
    assert_eq!(
        failure(record_named(&report.outcomes, "secret.svg")),
        SourceFailure::Unreadable
    );
    assert_eq!(
        failure(record_named(&report.outcomes, "broken.svg")),
        SourceFailure::NotAFile
    );
    assert_eq!(
        failure(record_named(&report.outcomes, "notfile.svg")),
        SourceFailure::NotAFile
    );
    assert!(matches!(
        record_named(&report.outcomes, "logo.SVG").outcome,
        SourceOutcome::Succeeded(_)
    ));
    assert_png(&dir.path().join("ok.png"), 10, 12);
    assert_png(&dir.path().join("logo.png"), 10, 12);
    assert!(!dir.path().join("secret.png").exists());
    assert!(!dir.path().join("broken.png").exists());
    assert!(fs::metadata(&fifo).unwrap().file_type().is_fifo());
}
