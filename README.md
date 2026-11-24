# svgshaper

Rust-библиотека для конвертации SVG в PNG точного размера (ширина × высота в пикселях)
и тонкий плагин для Bevy 0.20 поверх неё.

- Источники — список конкретных файлов или папка целиком (без вложенности).
- Папка назначения и шаблон имени `{prefix}{name}{suffix}.png`.
- Асинхронное ядро без привязки к runtime: `convert` возвращает `Future`, исполнитель выбирает
  вызывающий код (Bevy, tokio, свой `block_on`).
- Безопасная запись: PNG сначала пишется во временный файл и публикуется переименованием,
  остановка не оставляет недописанных файлов, два одновременных запроса к одному пути не
  портят друг другу результат.
- Подробный отчёт: у каждого источника ровно один итог — `Succeeded(path)`, `Failed(причина)`
  или `Stopped`.

Растеризацию делает внешняя программа [`rsvg-convert`](https://gitlab.gnome.org/GNOME/librsvg)
(librsvg). Она запускается через `std::process::Command` одной программой и списком
аргументов, без shell:

```text
rsvg-convert --width {width} --height {height} --format png --output {temp} {source}
```

## Пример

```rust
use std::path::PathBuf;
use svgshaper::{convert, ConversionRequest, SourceOutcome, StopFlag};

let mut request = ConversionRequest::new(vec![PathBuf::from("icons/logo.svg")], 64, 64)?;
request
    .set_destination("assets/png")
    .set_prefix("icon_")
    .set_suffix("@64");

// Любой исполнитель; здесь — pollster (не зависимость библиотеки, просто для примера).
let report = pollster::block_on(convert(request, StopFlag::new()))?;
for record in &report.outcomes {
    match &record.outcome {
        SourceOutcome::Succeeded(png) => println!("ok: {}", png.display()),
        SourceOutcome::Failed(reason) => println!("ошибка {:?}: {reason:?}", record.source.path),
        SourceOutcome::Stopped => println!("остановлено: {:?}", record.source.path),
    }
}
```

`StopFlag` можно клонировать и вызвать `request()` из другого потока — уже готовые PNG
останутся, остальные источники получат `Stopped`.

### Bevy

```rust
use svgshaper_bevy::{ConversionFinished, PendingConversions, SvgShaperPlugin};

app.add_plugins(SvgShaperPlugin);
// Хост кладёт запросы в ресурс PendingConversions { queue: vec![(request, StopFlag::new())] },
// плагин запускает их в AsyncComputeTaskPool и публикует сообщение ConversionFinished(Result<..>).
```

Плагин не блокирует кадр: система только опрашивает готовность задач.

## Стек

- Rust, edition 2024. MSRV: 1.85 для ядра, 1.97.1 для крейта плагина.
- Ядро: одна зависимость — [`blocking`](https://crates.io/crates/blocking).
- Плагин: `bevy` 0.20 (`default-features = false`).
- Растеризатор: `rsvg-convert` в `PATH` (путь можно переопределить через `set_converter`).

## Сборка и тесты

```sh
cargo build
cargo test -p svgshaper         # ядро: без Bevy, GPU, дисплея и сети
cargo test -p svgshaper-bevy    # плагин: headless-приложение Bevy, без окна
cargo test --workspace          # всё
```

Для `cargo test` системный `rsvg-convert` не нужен: `build.rs` собирает тестовую программу
`svgshaper-test-converter`, которая пишет PNG нужного размера и умеет имитировать задержку и
ошибки. Тест `draw_oracle` (проверка реального рисунка: заливка, обводка, градиенты, текст,
фильтры, маски, обрезка, маркеры, анимация) запускается, если `rsvg-convert` есть в `PATH`,
иначе пропускается. Сейчас тесты рассчитаны на Unix (Linux/macOS).

## Структура

```text
crates/
  svgshaper/                  ядро-библиотека
    src/lib.rs                публичный API и его документация
    src/request.rs            ConversionRequest (размер, шаблон имени, назначение), отчёт, StopFlag
    src/select.rs             раскрытие папок (один уровень), отбор *.svg, снятие повторов
    src/paths.rs              нормализация путей
    src/store.rs              захват конечного пути, временный PNG, публикация rename, чтение IHDR
    src/shell.rs              запуск rsvg-convert без shell
    src/convert.rs            конвейер конвертации
    src/error.rs              RequestError и причины неуспеха источника
    tests/                    интеграционные тесты по пользовательским историям (us1_*, us2_*, us3_*)
    tests/fixtures/draw/      контрольный набор SVG для draw_oracle
  svgshaper-bevy/             плагин Bevy (SvgShaperPlugin)
  svgshaper-test-converter/   тестовая заглушка растеризатора (не продукт)
specs/001-convert-svg-png/    спецификация, план, исследование, модель данных, контракты, задачи
.specify/                     Spec Kit: конституция проекта, шаблоны, скрипты
.grok/skills/                 навыки Spec Kit для ИИ-агента
```

## Статус

Версия 0.1.0, крейты не опубликованы на crates.io (`publish = false`).
Первая фича — `001-convert-svg-png` — реализована: все 33 задачи в
[`specs/001-convert-svg-png/tasks.md`](specs/001-convert-svg-png/tasks.md) отмечены
выполненными, `cargo test --workspace` зелёный (31 интеграционный тест).

Идеи на будущее (пока не запланированы в спецификациях): проверка на Windows, публикация на
crates.io, CI.

## Как это сделано

Проект разработан с ИИ-агентом по методике spec-driven development
([GitHub Spec Kit](https://github.com/github/spec-kit) 1.4.0):

1. **Конституция** — [`.specify/memory/constitution.md`](.specify/memory/constitution.md):
   пять принципов (библиотечное ядро, Rust, обязательные базовые тесты на публичное поведение,
   асинхронное ядро без своего runtime, Bevy только как тонкий плагин) и ворота качества.
2. **Спецификация** — [`spec.md`](specs/001-convert-svg-png/spec.md): пользовательские истории
   P1–P3 с критериями приёмки, функциональные требования, граничные случаи и две сессии
   уточнений (clarify) с записанными решениями.
3. **План и исследование** — [`plan.md`](specs/001-convert-svg-png/plan.md),
   [`research.md`](specs/001-convert-svg-png/research.md),
   [`data-model.md`](specs/001-convert-svg-png/data-model.md), контракты API ядра и плагина в
   [`contracts/`](specs/001-convert-svg-png/contracts/), сценарии проверки в
   [`quickstart.md`](specs/001-convert-svg-png/quickstart.md).
4. **Задачи** — [`tasks.md`](specs/001-convert-svg-png/tasks.md): разбиение по историям,
   тесты пишутся до реализации и сначала падают.
5. **Реализация и проверка** — каждый сценарий из quickstart закрыт интеграционным тестом;
   агент работал по навыкам Spec Kit из `.grok/skills/` (specify, clarify, plan, tasks,
   analyze, implement).

## Лицензия

[GNU General Public License v3.0](LICENSE) (`GPL-3.0-only`).
