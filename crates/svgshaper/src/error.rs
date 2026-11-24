use std::fmt;

/// Запрос отклонён целиком. Новых файлов нет.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestError {
    /// Сторона вне диапазона 1..=16384 или записана не целым числом.
    InvalidSize,
    /// В запросе нет ни одного пути.
    EmptySources,
    /// Префикс, суффикс или шаблон дают пустое имя, `.`, `..` или содержат разделитель пути.
    InvalidName,
    /// Папку назначения нельзя создать.
    DestinationUnavailable,
    /// Программу из поля `converter` нельзя запустить. Проверяется до любой записи.
    ConverterUnavailable,
}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::InvalidSize => "Размер вне диапазона или не целый",
            Self::EmptySources => "Пустой список источников",
            Self::InvalidName => "Недопустимое имя",
            Self::DestinationUnavailable => "Папку назначения нельзя создать",
            Self::ConverterUnavailable => "Средство растеризации нельзя запустить",
        };
        f.write_str(text)
    }
}

impl std::error::Error for RequestError {}

/// Причина неуспеха одного источника. `Stopped` сюда не входит.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFailure {
    /// Пути нет.
    NotFound,
    /// Файл нельзя прочитать.
    Unreadable,
    /// Явный путь не оканчивается на `.svg`.
    NotSvg,
    /// Имя на `.svg` не является файлом, либо ссылка сломана.
    NotAFile,
    /// SVG нельзя нарисовать из самого файла.
    CannotDraw,
    /// Конечный файл уже есть, замена выключена.
    NameTaken,
    /// Несколько источников одного запроса сошлись в один путь.
    NameCollision,
    /// Другой запрос этого процесса уже занимает тот же путь.
    ConcurrentWrite,
    /// Папка или файл назначения не принимают запись.
    WriteFailed,
}

impl fmt::Display for SourceFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::NotFound => "Пути нет",
            Self::Unreadable => "Файл нельзя прочитать",
            Self::NotSvg => "Явный путь не оканчивается на `.svg`",
            Self::NotAFile => "Имя на `.svg` не является файлом, либо ссылка сломана",
            Self::CannotDraw => "SVG нельзя нарисовать из самого файла",
            Self::NameTaken => "Конечный файл уже есть, замена выключена",
            Self::NameCollision => "Несколько источников одного запроса сошлись в один путь",
            Self::ConcurrentWrite => "Другой запрос этого процесса уже занимает тот же путь",
            Self::WriteFailed => "Папка или файл назначения не принимают запись",
        };
        f.write_str(text)
    }
}

impl std::error::Error for SourceFailure {}
