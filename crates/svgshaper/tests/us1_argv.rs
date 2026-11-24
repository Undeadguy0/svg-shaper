mod common;

use std::fs;

use svgshaper::{StopFlag, convert};

use common::{TempDir, block_on, request, write_svg};

#[test]
fn us1_argv_is_only_the_png_conversion() {
    let dir = TempDir::new();
    let svg = write_svg(dir.path(), "logo.svg");
    let log = dir.path().join("argv.txt");
    unsafe { std::env::set_var("SVGSHAPER_ARGV_LOG", &log) };
    let _guard = ClearArgvLog;
    let report = block_on(convert(request(vec![svg.clone()], 64, 32), StopFlag::new())).unwrap();
    assert_eq!(report.outcomes.len(), 1);

    let text = fs::read_to_string(&log).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert!(
        lines[0].ends_with("svgshaper-test-converter"),
        "argv0 = {}",
        lines[0]
    );
    assert_eq!(
        &lines[1..],
        &[
            "--width",
            "64",
            "--height",
            "32",
            "--format",
            "png",
            "--output",
            lines[8],
            svg.to_str().unwrap(),
        ]
    );
    assert!(lines.iter().all(|line| {
        let name = line.rsplit(['/', '\\']).next().unwrap_or(line);
        name != "sh" && name != "bash" && name != "dash" && *line != "-c"
    }));
    assert!(!lines.contains(&"--keep-aspect-ratio"));
    assert!(!lines.contains(&"--background-color"));
    let inputs: Vec<_> = lines.iter().filter(|line| line.ends_with(".svg")).collect();
    assert_eq!(inputs.len(), 1);
}

struct ClearArgvLog;

impl Drop for ClearArgvLog {
    fn drop(&mut self) {
        unsafe { std::env::remove_var("SVGSHAPER_ARGV_LOG") };
    }
}
