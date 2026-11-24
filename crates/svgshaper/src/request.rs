use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{RequestError, SourceFailure};

const MIN_SIDE: i64 = 1;
const MAX_SIDE: i64 = 16384;
const DEFAULT_TEMPLATE: &str = "{prefix}{name}{suffix}.png";

/// Один SVG после раскрытия папок и снятия повторов.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// Путь, как его передал вызывающий код.
    pub path: PathBuf,
    /// `{name}`: имя файла без последнего расширения.
    pub stem: String,
}

/// Итог одного источника. Ровно одно из трёх состояний.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceOutcome {
    /// Целый PNG по этому пути.
    Succeeded(PathBuf),
    /// Нового файла нет. При замене прежний файл не изменён.
    Failed(SourceFailure),
    /// Запрос остановлен. Нового незавершённого файла нет.
    Stopped,
}

/// Итог одного взятого в работу источника.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRecord {
    pub source: Source,
    pub outcome: SourceOutcome,
}

/// Запрос дошёл до обработки источников.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionReport {
    /// Итоги в порядке обработки. Пропущенные не-SVG элементы папки сюда не входят.
    pub outcomes: Vec<SourceRecord>,
}

/// Флаг остановки. Копия делит то же состояние, поэтому `request` можно вызвать из другого потока.
#[derive(Debug, Clone)]
pub struct StopFlag {
    flag: Arc<AtomicBool>,
}

impl StopFlag {
    pub fn new() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Поднять флаг. Между источниками и до переименования ядро его читает.
    pub fn request(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    pub fn is_requested(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}

impl Default for StopFlag {
    fn default() -> Self {
        Self::new()
    }
}

/// Один запрос конвертации.
///
/// Ширина и высота — целые от 1 до 16384. Дробное значение отклоняется при сборке
/// запроса и до [`crate::convert`] не доходит.
#[derive(Debug, Clone)]
pub struct ConversionRequest {
    sources: Vec<PathBuf>,
    width: u32,
    height: u32,
    destination: Option<PathBuf>,
    prefix: String,
    suffix: String,
    template: String,
    replace: bool,
    converter: PathBuf,
}

impl ConversionRequest {
    /// Запрос с размером в уже целых пикселях.
    ///
    /// Пустой список путей здесь допустим: его отклоняет [`crate::convert`]
    /// с [`RequestError::EmptySources`] до записи. Сторона 0 или больше 16384
    /// даёт [`RequestError::InvalidSize`].
    pub fn new(sources: Vec<PathBuf>, width: u32, height: u32) -> Result<Self, RequestError> {
        try_pixel_side(i64::from(width))?;
        try_pixel_side(i64::from(height))?;
        Ok(Self::from_sides(sources, width, height))
    }

    /// Собрать запрос из текстовых сторон. Нецелое (`1.5`, `32.0`) — [`RequestError::InvalidSize`].
    pub fn try_new(sources: Vec<PathBuf>, width: &str, height: &str) -> Result<Self, RequestError> {
        let width = parse_pixel_side(width)?;
        let height = parse_pixel_side(height)?;
        Ok(Self::from_sides(sources, width, height))
    }

    fn from_sides(sources: Vec<PathBuf>, width: u32, height: u32) -> Self {
        Self {
            sources,
            width,
            height,
            destination: None,
            prefix: String::new(),
            suffix: String::new(),
            template: DEFAULT_TEMPLATE.to_string(),
            replace: false,
            converter: PathBuf::from("rsvg-convert"),
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn sources(&self) -> &[PathBuf] {
        &self.sources
    }

    pub fn destination(&self) -> Option<&Path> {
        self.destination.as_deref()
    }

    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    pub fn suffix(&self) -> &str {
        &self.suffix
    }

    pub fn template(&self) -> &str {
        &self.template
    }

    pub fn replace(&self) -> bool {
        self.replace
    }

    pub fn converter(&self) -> &Path {
        &self.converter
    }

    pub fn set_destination(&mut self, destination: impl Into<PathBuf>) -> &mut Self {
        self.destination = Some(destination.into());
        self
    }

    pub fn set_prefix(&mut self, prefix: impl Into<String>) -> &mut Self {
        self.prefix = prefix.into();
        self
    }

    pub fn set_suffix(&mut self, suffix: impl Into<String>) -> &mut Self {
        self.suffix = suffix.into();
        self
    }

    pub fn set_template(&mut self, template: impl Into<String>) -> &mut Self {
        self.template = template.into();
        self
    }

    pub fn set_replace(&mut self, replace: bool) -> &mut Self {
        self.replace = replace;
        self
    }

    pub fn set_converter(&mut self, converter: impl Into<PathBuf>) -> &mut Self {
        self.converter = converter.into();
        self
    }
}

/// Целая сторона PNG. Отрицательное, 0 и значение больше 16384 — ошибка размера.
pub fn try_pixel_side(value: i64) -> Result<u32, RequestError> {
    if (MIN_SIDE..=MAX_SIDE).contains(&value) {
        Ok(u32::try_from(value).expect("сторона помещается в u32"))
    } else {
        Err(RequestError::InvalidSize)
    }
}

/// Разобрать сторону из текста. Дробная запись и нечисловой текст — [`RequestError::InvalidSize`].
pub fn parse_pixel_side(text: &str) -> Result<u32, RequestError> {
    if !is_plain_integer(text) {
        return Err(RequestError::InvalidSize);
    }
    let value: i64 = text.parse().map_err(|_| RequestError::InvalidSize)?;
    try_pixel_side(value)
}

fn is_plain_integer(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
}

pub(crate) fn validate_request(request: &ConversionRequest) -> Result<(), RequestError> {
    try_pixel_side(i64::from(request.width))?;
    try_pixel_side(i64::from(request.height))?;
    if request.sources.is_empty() {
        return Err(RequestError::EmptySources);
    }
    if has_separator(&request.prefix)
        || has_separator(&request.suffix)
        || has_separator(&request.template)
    {
        return Err(RequestError::InvalidName);
    }
    Ok(())
}

pub(crate) struct OutputPath {
    pub directory: PathBuf,
    pub path: PathBuf,
}

pub(crate) fn output_for(
    request: &ConversionRequest,
    source: &Source,
) -> Result<OutputPath, RequestError> {
    let file_name = render_file_name(request, &source.stem)?;
    let directory = match &request.destination {
        Some(dir) => dir.clone(),
        None => source
            .path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf(),
    };
    let path = directory.join(&file_name);
    Ok(OutputPath { directory, path })
}

fn render_file_name(request: &ConversionRequest, stem: &str) -> Result<String, RequestError> {
    if has_separator(&request.prefix)
        || has_separator(&request.suffix)
        || has_separator(&request.template)
    {
        return Err(RequestError::InvalidName);
    }
    let rendered = apply_template(&request.template, &request.prefix, stem, &request.suffix);
    if rendered.is_empty() || rendered == "." || rendered == ".." || has_separator(&rendered) {
        return Err(RequestError::InvalidName);
    }
    let file_name = if rendered.to_ascii_lowercase().ends_with(".png") {
        rendered
    } else {
        format!("{rendered}.png")
    };
    if file_name.is_empty() || file_name == "." || file_name == ".." || has_separator(&file_name) {
        return Err(RequestError::InvalidName);
    }
    Ok(file_name)
}

fn apply_template(template: &str, prefix: &str, name: &str, suffix: &str) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        if let Some(end) = after.find('}') {
            match &after[..end] {
                "prefix" => out.push_str(prefix),
                "name" => out.push_str(name),
                "suffix" => out.push_str(suffix),
                other => {
                    out.push('{');
                    out.push_str(other);
                    out.push('}');
                }
            }
            rest = &after[end + 1..];
        } else {
            out.push_str(&rest[start..]);
            return out;
        }
    }
    out.push_str(rest);
    out
}

fn has_separator(text: &str) -> bool {
    text.contains('/') || text.contains('\\')
}

pub(crate) fn stem_of(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub(crate) fn is_svg_name(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
}
