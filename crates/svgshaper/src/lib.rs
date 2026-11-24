//! Конвертация SVG в PNG заданной ширины и высоты.
//!
//! Ядро — библиотека. Оно не зависит от Bevy, не выбирает асинхронный runtime
//! и не запускает оболочку. Рисунок делает отдельный процесс.
//!
//! # Запрос
//!
//! [`convert`] принимает [`ConversionRequest`] и [`StopFlag`] и возвращает
//! future. Её опрос не блокирует поток вызывающего на время ожидания процесса.
//! Исполнитель — любой: вызывающий код, в том числе плагин Bevy, сам решает,
//! где опрашивать future.
//!
//! Успех — [`ConversionReport`]: у каждого взятого в работу источника ровно один
//! итог [`SourceOutcome`]. Отклонённый запрос — [`RequestError`], новых файлов нет.
//!
//! # Программа по умолчанию
//!
//! Поле `converter` по умолчанию — `rsvg-convert`. Имя без разделителя ищется
//! в `PATH`. Запуск идёт через `std::process::Command`, одной программой и
//! списком аргументов, без `sh` и без `bash`:
//!
//! ```text
//! rsvg-convert --width {width} --height {height} --format png --output {temp} {source}
//! ```
//!
//! `--keep-aspect-ratio` и `--background-color` не передаются. В аргументах
//! только эта программа, ширина, высота, формат `png`, временный файл и один SVG.
//!
//! Если программу нельзя запустить, [`convert`] возвращает
//! [`RequestError::ConverterUnavailable`] до любой записи.
//!
//! # Остановка и запись
//!
//! [`StopFlag::request`] можно вызвать из другого потока. Уже переименованные
//! PNG остаются [`SourceOutcome::Succeeded`]. Текущий незавершённый источник и
//! все ещё не начатые получают [`SourceOutcome::Stopped`]. Временный файл
//! удаляется. Если шла замена, прежний файл остаётся. `Stopped` не смешивается
//! с причинами [`SourceFailure`].
//!
//! Конечный путь на время источника занят в процессной таблице. Второй запрос
//! этого процесса получает [`SourceFailure::ConcurrentWrite`] и процесс не
//! запускает. Совпадение имён внутри одного запроса — [`SourceFailure::NameCollision`]
//! у всех таких источников, файл не пишется.
//!
//! # Граница FR-026
//!
//! Ядро не открывает другие файлы и сетевые адреса ради рисунка и не передаёт
//! их в аргументах. Относительную ссылку, которую откроет сам дочерний процесс,
//! библиотека не запрещает и не обещает, что он её не откроет. Если из-за такого
//! ресурса процесс завершается с ошибкой или PNG получает другой размер,
//! источник неуспешен ([`SourceFailure::CannotDraw`]), новый файл не остаётся.

mod convert;
mod error;
mod paths;
mod request;
mod select;
mod shell;
mod store;

pub use error::{RequestError, SourceFailure};
pub use request::{
    ConversionReport, ConversionRequest, Source, SourceOutcome, SourceRecord, StopFlag,
    parse_pixel_side, try_pixel_side,
};

use std::future::Future;

/// Конвертировать источники запроса в PNG.
///
/// Future нужно опросить. Пока дочерний процесс ещё работает, `poll` возвращает
/// управление вызывающему потоку. Пустой список, недопустимый размер, недопустимое
/// имя, недоступная папка назначения и программа, которую нельзя запустить,
/// завершают future ошибкой [`RequestError`] и ничего не публикуют.
pub fn convert(
    request: ConversionRequest,
    stop: StopFlag,
) -> impl Future<Output = Result<ConversionReport, RequestError>> + Send {
    blocking::unblock(move || convert::run(request, stop))
}
