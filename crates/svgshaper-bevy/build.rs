use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let source = manifest_dir.join("../svgshaper-test-converter/src/main.rs");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let dest = out_dir.join("svgshaper-test-converter");
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let output = Command::new(rustc)
        .arg("--edition=2024")
        .arg("-o")
        .arg(&dest)
        .arg(&source)
        .output()
        .expect("запуск rustc для svgshaper-test-converter");
    if !output.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        panic!("не удалось собрать svgshaper-test-converter");
    }
    println!(
        "cargo:rustc-env=SVGSHAPER_TEST_CONVERTER={}",
        dest.display()
    );
    println!("cargo:rerun-if-changed={}", source.display());
}
