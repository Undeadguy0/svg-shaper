mod common;

use std::fs;

use svgshaper::{SourceOutcome, StopFlag, convert};

use common::{TempDir, assert_no_partials, assert_png, block_on, request, write_svg};

#[test]
fn us2_folder_converts_only_immediate_svgs() {
    let dir = TempDir::new();
    let folder = dir.path().join("icons");
    fs::create_dir(&folder).unwrap();
    write_svg(&folder, "a.svg");
    write_svg(&folder, "b.SVG");
    fs::write(folder.join("notes.txt"), b"notes").unwrap();
    let nested = folder.join("nested");
    fs::create_dir(&nested).unwrap();
    write_svg(&nested, "c.svg");

    let report = block_on(convert(
        request(vec![folder.clone()], 16, 8),
        StopFlag::new(),
    ))
    .unwrap();
    assert_eq!(report.outcomes.len(), 2);
    assert!(report.outcomes.iter().all(|record| {
        matches!(record.outcome, SourceOutcome::Succeeded(_))
            && (record.source.path.ends_with("a.svg") || record.source.path.ends_with("b.SVG"))
    }));
    assert_png(&folder.join("a.png"), 16, 8);
    assert_png(&folder.join("b.png"), 16, 8);
    assert!(!nested.join("c.png").exists());
    assert!(!folder.join("notes.png").exists());
    assert_no_partials(dir.path());
}

#[test]
fn us2_empty_folder_has_zero_outcomes() {
    let dir = TempDir::new();
    let folder = dir.path().join("empty");
    fs::create_dir(&folder).unwrap();
    let report = block_on(convert(
        request(vec![folder.clone()], 16, 8),
        StopFlag::new(),
    ))
    .unwrap();
    assert!(report.outcomes.is_empty());
    assert_eq!(fs::read_dir(&folder).unwrap().count(), 0);
}

#[test]
fn us2_directory_path_inside_the_source_list() {
    let dir = TempDir::new();
    let extra = write_svg(dir.path(), "extra.svg");
    let folder = dir.path().join("icons");
    fs::create_dir(&folder).unwrap();
    write_svg(&folder, "a.svg");
    write_svg(&folder, "b.SVG");
    fs::write(folder.join("notes.txt"), b"notes").unwrap();
    let nested = folder.join("nested");
    fs::create_dir(&nested).unwrap();
    write_svg(&nested, "c.svg");

    let report = block_on(convert(
        request(vec![extra, folder.clone()], 12, 12),
        StopFlag::new(),
    ))
    .unwrap();
    assert_eq!(report.outcomes.len(), 3);
    assert!(
        report
            .outcomes
            .iter()
            .all(|record| matches!(record.outcome, SourceOutcome::Succeeded(_)))
    );
    assert_png(&dir.path().join("extra.png"), 12, 12);
    assert_png(&folder.join("a.png"), 12, 12);
    assert_png(&folder.join("b.png"), 12, 12);
    assert!(!nested.join("c.png").exists());
}

#[test]
fn us2_same_path_twice_is_one_source() {
    let dir = TempDir::new();
    let svg = write_svg(dir.path(), "logo.svg");
    let report = block_on(convert(
        request(vec![svg.clone(), svg], 9, 7),
        StopFlag::new(),
    ))
    .unwrap();
    assert_eq!(report.outcomes.len(), 1);
    assert!(matches!(
        report.outcomes[0].outcome,
        SourceOutcome::Succeeded(_)
    ));
    assert_png(&dir.path().join("logo.png"), 9, 7);
}
