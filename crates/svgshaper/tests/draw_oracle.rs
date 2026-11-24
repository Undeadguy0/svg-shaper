mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use png::ColorType;
use svgshaper::{ConversionRequest, SourceOutcome, StopFlag, convert};

use common::{TempDir, block_on};

/// SC-007 и сценарии 8, 9 и 11. Без `rsvg-convert` в `PATH` тест пропускается.
#[test]
fn draw_oracle_regions_when_rsvg_is_installed() {
    if !rsvg_on_path() {
        eprintln!("draw_oracle: rsvg-convert нет в PATH, пропуск");
        return;
    }

    let work = TempDir::new();
    check_shape(&render("shape", 64, 64, work.path()));
    check_fill(&render("fill", 64, 64, work.path()));
    check_stroke(&render("stroke", 64, 64, work.path()));
    check_opacity(&render("opacity", 64, 64, work.path()));
    check_gradient(&render("gradient", 64, 64, work.path()));
    assert_no_fonts(&fixture_dir("text"));
    check_text(&render("text", 160, 64, work.path()));
    check_filter(&render("filter", 64, 64, work.path()));
    check_mask(&render("mask", 64, 64, work.path()));
    check_clip(&render("clip", 64, 64, work.path()));
    check_marker(&render("marker", 64, 64, work.path()));
    check_animation(&render("animation", 64, 64, work.path()));
    check_external(work.path());
}

fn rsvg_on_path() -> bool {
    Command::new("rsvg-convert")
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn fixture_dir(group: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/draw")
        .join(group)
}

fn render(group: &str, width: u32, height: u32, dest: &Path) -> Image {
    let svg = fixture_dir(group).join(format!("{group}.svg"));
    let out = dest.join(group);
    fs::create_dir_all(&out).unwrap();
    decode(&convert_svg(&svg, width, height, &out), width, height)
}

fn check_external(dest: &Path) {
    let stage = dest.join("external-src");
    let out = dest.join("external-out");
    fs::create_dir_all(&stage).unwrap();
    fs::create_dir_all(&out).unwrap();
    fs::copy(
        fixture_dir("external").join("external.svg"),
        stage.join("external.svg"),
    )
    .unwrap();
    write_dot(&stage.join("dot.png"));
    let image = decode(
        &convert_svg(&stage.join("external.svg"), 64, 64, &out),
        64,
        64,
    );
    assert!(
        image.at(16, 16)[0] > 200 && image.at(16, 16)[3] == 255,
        "внутренняя фигура external: {}",
        image.near(16, 16)
    );
}

fn convert_svg(svg: &Path, width: u32, height: u32, dest: &Path) -> PathBuf {
    let mut request = ConversionRequest::new(vec![svg.to_path_buf()], width, height).unwrap();
    request.set_converter("rsvg-convert");
    request.set_destination(dest);
    let report = block_on(convert(request, StopFlag::new())).unwrap_or_else(|error| {
        panic!("запрос {svg:?} отклонён: {error}");
    });
    assert_eq!(report.outcomes.len(), 1, "{svg:?}");
    match &report.outcomes[0].outcome {
        SourceOutcome::Succeeded(path) => path.clone(),
        other => panic!("{svg:?}: {other:?}"),
    }
}

fn write_dot(path: &Path) {
    let file = fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(file, 1, 1);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&[255, 0, 0, 255]).unwrap();
}

struct Image {
    width: u32,
    height: u32,
    data: Vec<u8>,
    channels: usize,
}

impl Image {
    fn at(&self, x: u32, y: u32) -> [u8; 4] {
        assert!(
            x < self.width && y < self.height,
            "точка вне холста {x},{y}"
        );
        let index = ((y * self.width + x) as usize) * self.channels;
        let pixel = &self.data[index..index + self.channels];
        match self.channels {
            4 => [pixel[0], pixel[1], pixel[2], pixel[3]],
            3 => [pixel[0], pixel[1], pixel[2], 255],
            _ => panic!("ожидался RGB или RGBA, каналов {}", self.channels),
        }
    }

    fn near(&self, x: u32, y: u32) -> String {
        let mut parts = Vec::new();
        for dy in -2i32..=2 {
            for dx in -2i32..=2 {
                let xx = x as i32 + dx;
                let yy = y as i32 + dy;
                if xx >= 0 && yy >= 0 && (xx as u32) < self.width && (yy as u32) < self.height {
                    parts.push(format!("({xx},{yy})={:?}", self.at(xx as u32, yy as u32)));
                }
            }
        }
        parts.join(" ")
    }
}

fn decode(path: &Path, width: u32, height: u32) -> Image {
    let file = fs::File::open(path).unwrap();
    let mut decoder = png::Decoder::new(file);
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().unwrap();
    let mut data = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut data).unwrap();
    let size = info.buffer_size();
    data.truncate(size);
    assert_eq!(info.width, width, "ширина {path:?}");
    assert_eq!(info.height, height, "высота {path:?}");
    let channels = match info.color_type {
        ColorType::Rgb => 3,
        ColorType::Rgba => 4,
        other => panic!("{path:?}: цвет {other:?}"),
    };
    Image {
        width: info.width,
        height: info.height,
        data,
        channels,
    }
}

fn check_shape(image: &Image) {
    let inside = image.at(32, 32);
    assert!(
        inside[3] == 255 && inside[0] > 200 && inside[1] < 40,
        "фигура внутри: {}",
        image.near(32, 32)
    );
    assert_eq!(image.at(0, 0)[3], 0, "угол фигуры: {}", image.near(0, 0));
}

fn check_fill(image: &Image) {
    let center = image.at(32, 32);
    assert!(
        center[3] == 255 && center[1] > 200,
        "заливка: {}",
        image.near(32, 32)
    );
    assert_eq!(image.at(0, 0)[3], 0, "угол заливки: {}", image.near(0, 0));
}

fn check_stroke(image: &Image) {
    assert_eq!(
        image.at(32, 21)[3],
        255,
        "точка на контуре: {}",
        image.near(32, 21)
    );
    assert_eq!(
        image.at(32, 32)[3],
        0,
        "пустая середина контура: {}",
        image.near(32, 32)
    );
}

fn check_opacity(image: &Image) {
    let alpha = image.at(32, 32)[3];
    assert!(
        (80..=200).contains(&alpha),
        "полупрозрачность {alpha}: {}",
        image.near(32, 32)
    );
}

fn check_gradient(image: &Image) {
    let left = image.at(12, 32);
    let right = image.at(52, 32);
    assert_ne!(
        (left[0], left[1], left[2]),
        (right[0], right[1], right[2]),
        "градиент слева {} справа {}",
        image.near(12, 32),
        image.near(52, 32)
    );
}

fn check_text(image: &Image) {
    let ink = image.data.chunks(image.channels).any(|pixel| {
        let alpha = if image.channels == 4 { pixel[3] } else { 255 };
        alpha > 100 && pixel.iter().take(3).any(|channel| *channel < 250)
    });
    assert!(ink, "в области текста нет непрозрачных точек");
}

fn check_filter(image: &Image) {
    assert!(
        image.at(32, 32)[3] > 200,
        "фильтр внутри фигуры: {}",
        image.near(32, 32)
    );
    assert!(
        image.at(16, 32)[3] > 0,
        "ореол за краем фигуры: {}",
        image.near(16, 32)
    );
}

fn check_mask(image: &Image) {
    assert_eq!(
        image.at(16, 32)[3],
        255,
        "видимая часть маски: {}",
        image.near(16, 32)
    );
    assert_eq!(
        image.at(48, 32)[3],
        0,
        "скрытая часть маски: {}",
        image.near(48, 32)
    );
}

fn check_clip(image: &Image) {
    assert_eq!(
        image.at(16, 32)[3],
        255,
        "видимая часть обрезки: {}",
        image.near(16, 32)
    );
    assert_eq!(
        image.at(48, 32)[3],
        0,
        "скрытая часть обрезки: {}",
        image.near(48, 32)
    );
}

fn check_marker(image: &Image) {
    assert!(
        image.at(40, 36)[3] > 200,
        "маркер на конце линии: {}",
        image.near(40, 36)
    );
    assert_eq!(
        image.at(8, 8)[3],
        0,
        "пустое место рядом с маркером: {}",
        image.near(8, 8)
    );
}

fn check_animation(image: &Image) {
    assert_eq!(
        image.at(12, 32)[3],
        255,
        "начальный вид анимации: {}",
        image.near(12, 32)
    );
    assert_eq!(
        image.at(44, 32)[3],
        0,
        "конечное положение анимации не должно быть нарисовано: {}",
        image.near(44, 32)
    );
}

fn assert_no_fonts(dir: &Path) {
    for entry in fs::read_dir(dir).unwrap() {
        let name = entry.unwrap().file_name();
        let name = name.to_string_lossy().to_ascii_lowercase();
        for extension in [".ttf", ".otf", ".woff", ".woff2", ".ttc"] {
            assert!(
                !name.ends_with(extension),
                "рядом с текстовым SVG лежит файл шрифта {name}"
            );
        }
    }
}
