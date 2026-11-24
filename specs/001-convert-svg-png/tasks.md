# Tasks: Конвертация SVG в PNG

**Input**: Design documents from `/specs/001-convert-svg-png/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Constitution**: Принципы I–V. Ядро — библиотека на Rust. Файловый `cargo test`
не требует Bevy, GPU, дисплей, сеть и системный `rsvg-convert`. Ожидание
процесса не блокирует поток вызывающего. Плагин Bevy — отдельный крейт.

**Tests**: Базовые тесты обязательны по принципу III. В каждой истории тесты
пишутся до реализации и сначала должны падать. `cargo test -p svgshaper`
запускает `svgshaper-test-converter` и проверяет файлы, имена, остановку и
конфликты. Проверка областей SC-007 — `draw_oracle`: она идёт, когда
`rsvg-convert` есть в `PATH`, и пропускается, когда его нет.

**Organization**: Задачи сгруппированы по пользовательским историям, чтобы каждую
можно было реализовать и проверить отдельно.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Можно выполнять параллельно (другие файлы, нет зависимости от незавершённых задач)
- **[Story]**: История, к которой относится задача (US1, US2, US3)
- В описании указан точный путь к файлу

## Path Conventions

- Ядро: `crates/svgshaper/`
- Тестовая программа: `crates/svgshaper-test-converter/`
- Плагин: `crates/svgshaper-bevy/`
- Интеграционные тесты ядра: `crates/svgshaper/tests/`

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Каркас workspace без доменной логики

- [X] T001 Create the workspace manifest in `Cargo.toml` with members `crates/svgshaper` and `crates/svgshaper-test-converter`, edition 2024
- [X] T002 [P] Create `crates/svgshaper/Cargo.toml` (`rust-version` 1.85, edition 2024, dependency `blocking`, no Bevy) and module stubs in `crates/svgshaper/src/lib.rs`
- [X] T003 [P] Create `crates/svgshaper-test-converter/Cargo.toml` and `crates/svgshaper-test-converter/src/main.rs`. It accepts `--width`, `--height`, `--format png`, `--output`, and one source path, then writes a PNG whose IHDR matches that width and height. `--sleep-ms N` waits before writing. `--exit-code N` exits with that code and does not write a valid PNG when N is not 0. When `SVGSHAPER_ARGV_LOG` is a path, the program writes its argv there, one argument per line. The program is not a product and does not read the SVG

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Типы, запуск процесса и публикация файла, без которых нельзя собрать ни одну историю

**⚠️ CRITICAL**: Пользовательские истории не начинаются, пока эта фаза не завершена

- [X] T004 [P] Implement `RequestError` and source failure reasons in `crates/svgshaper/src/error.rs`. Distinguish `NotFound` («Пути нет»), `Unreadable` («Файл нельзя прочитать»), `NotSvg` («Явный путь не оканчивается на `.svg`»), `NotAFile` («Имя на `.svg` не является файлом, либо ссылка сломана»), `CannotDraw` («SVG нельзя нарисовать из самого файла»), `NameTaken` («Конечный файл уже есть, замена выключена»), `NameCollision` («Несколько источников одного запроса сошлись в один путь»), `ConcurrentWrite` («Другой запрос этого процесса уже занимает тот же путь»), `WriteFailed` («Папка или файл назначения не принимают запись»). Request errors: `InvalidSize`, `EmptySources`, `InvalidName`, `DestinationUnavailable`, `ConverterUnavailable`. `Stopped` is not a `Failed` reason
- [X] T005 [P] Implement argv and spawn in `crates/svgshaper/src/shell.rs` with `std::process::Command` and no `sh -c`. Default program is `rsvg-convert`. Argv is `rsvg-convert --width {width} --height {height} --format png --output {temp} {source}`. Do not pass `--keep-aspect-ratio` or `--background-color`. Arguments contain only that program, width, height, format `png`, the temp path, and one SVG. Do not add a sandbox: a relative link opened by the child is outside the library guarantee (FR-026)
- [X] T006 Implement the in-process path claim, temp file, IHDR read, and atomic rename in `crates/svgshaper/src/store.rs` after the error types from T004 exist. IHDR rule: «Поля IHDR: Ровно запрошенные, иначе файл не публикуется». Temp path: «Лежит в папке назначения». Claim is a process-wide `Mutex<HashSet<PathBuf>>` held until rename or rejection. A single output that cannot be written is `WriteFailed`; failure to create the destination directory itself stays `DestinationUnavailable`
- [X] T007 Implement `ConversionRequest`, `Source`, and `StopFlag` in `crates/svgshaper/src/request.rs` and export them from `crates/svgshaper/src/lib.rs`. `width` is «Целое от 1 до 16384 включительно», `height` is «То же», `prefix` is «По умолчанию пустая строка», `suffix` is «По умолчанию пустая строка», `template` is «По умолчанию `{prefix}{name}{suffix}.png`», `replace` is «По умолчанию `false`», `converter` is «По умолчанию `rsvg-convert`. Ищется в `PATH`, если это не путь», `stem` is «Имя файла без последнего расширения»

**Checkpoint**: Типы, argv и публикация файла собираются. Обхода источников ещё нет

---

## Phase 3: User Story 1 - Конвертация выбранных SVG в PNG заданного размера (Priority: P1) 🎯 MVP

**Goal**: Явные пути к SVG становятся PNG запрошенной ширины и высоты рядом с исходником, под именем без расширения и с окончанием `.png`

**Independent Test**: Положить один SVG и запросить конвертацию с шириной 64 и высотой 32. Рядом появляется один PNG 64×32 с именем исходника и расширением `.png`

### Tests for User Story 1

- [X] T008 [P] [US1] Write a failing test for one SVG at 64×32 and two SVGs at 128×128 in `crates/svgshaper/tests/us1_sizes.rs`, spawning `svgshaper-test-converter` via the public `converter` field
- [X] T009 [P] [US1] Write a failing test for `InvalidSize`, `EmptySources`, `NotFound` beside one success, `NotSvg`, `NameTaken` when `replace` is false, and `CannotDraw` when the test converter is started with `--exit-code 1` in `crates/svgshaper/tests/us1_failures.rs`. `InvalidSize` covers a side of 0, a side of 16385, and a non-integer rejected at construction of the request. `CannotDraw` leaves no new PNG
- [X] T027 [P] [US1] Write a failing test in `crates/svgshaper/tests/us1_sources.rs`: an unreadable file is `Unreadable`, a broken symlink and a non-file whose name ends in `.svg` are `NotAFile`, and an explicit `logo.SVG` is converted as SVG. A readable SVG in the same request still succeeds
- [X] T028 [P] [US1] Write a failing test in `crates/svgshaper/tests/us1_converter.rs`: `converter` points at a program that cannot be spawned, the result is `ConverterUnavailable`, and no PNG is created (FR-030)
- [X] T029 [P] [US1] Write a failing test in `crates/svgshaper/tests/us1_argv.rs` that sets `SVGSHAPER_ARGV_LOG` and checks the spawned arguments: program, `--width`, `--height`, `--format`, `png`, `--output`, one source path. The log has no `sh`, no `--keep-aspect-ratio`, no `--background-color`, and no second input
- [X] T032 [P] [US1] Write a failing test in `crates/svgshaper/tests/us1_nonblocking.rs`: the test converter is started with `--sleep-ms 300`, and polling the `convert` future returns `Pending` in under 100 ms while that process is still asleep

### Implementation for User Story 1

- [X] T010 [US1] Reject a bad request before any write in `crates/svgshaper/src/request.rs`: «размер вне диапазона или не целый» returns `InvalidSize`. `width` and `height` are integers, so a fractional value is rejected when the request is built and never reaches `convert`. «Пустой список отклоняет весь запрос до записи» returns `EmptySources`
- [X] T011 [US1] Resolve explicit file sources and the default output path in `crates/svgshaper/src/select.rs`. `destination` `None` writes each PNG beside its SVG. The default template with empty prefix and suffix yields `{name}.png`. Compare the `.svg` suffix case-insensitively, so `logo.SVG` is an SVG. Map a non-`.svg` explicit path to `NotSvg`, a missing path to `NotFound`, an unreadable file to `Unreadable`, and a non-file or broken symlink to `NotAFile`
- [X] T012 [US1] Implement sequential `convert` for explicit files in `crates/svgshaper/src/convert.rs` using `crates/svgshaper/src/shell.rs` and `crates/svgshaper/src/store.rs`. A program that cannot be spawned returns `ConverterUnavailable` before any source is written. A non-zero exit or an IHDR that is not the requested size deletes the temp file and returns `CannotDraw`. Do not inspect pixels for a dropped filter, mask, clip, or marker. `replace` false and an existing final file return `NameTaken` without opening that file for write. Waiting for the child must not block the caller thread: `poll` returns while `--sleep-ms` is still running
- [X] T013 [US1] Export `convert` from `crates/svgshaper/src/lib.rs` and make `us1_sizes.rs`, `us1_failures.rs`, `us1_sources.rs`, `us1_converter.rs`, `us1_argv.rs`, and `us1_nonblocking.rs` pass under `cargo test -p svgshaper`

**Checkpoint**: История 1 проверяется отдельно и уже отдаёт PNG заданного размера

---

## Phase 4: User Story 2 - Все SVG в папке без вложенности (Priority: P2)

**Goal**: Папка даёт PNG только для SVG на своём уровне. Остановка оставляет уже целые PNG и помечает остальные источники как `Stopped`

**Independent Test**: В папке лежат два SVG, один текстовый файл и вложенная папка с ещё одним SVG. После запроса появляются ровно два PNG, а вложенный SVG не конвертируется

### Tests for User Story 2

- [X] T014 [P] [US2] Write a failing test in `crates/svgshaper/tests/us2_folder.rs` for `a.svg`, `b.SVG`, `notes.txt`, and `nested/c.svg` producing only two PNGs, for an empty folder producing zero sources, for a directory path inside the source list, and for the same path listed twice producing one source
- [X] T015 [P] [US2] Write a failing test in `crates/svgshaper/tests/us2_stop.rs`. The test converter uses `--sleep-ms` so the second process is still running when `StopFlag` fires. After the first of three PNGs is complete, the first PNG stays, the second and third outcomes are `Stopped`, and no partial PNG remains. A second case replaces an existing PNG: when stop fires before that rename, the previous file stays and that source is `Stopped`
- [X] T030 [P] [US2] Write a failing test in `crates/svgshaper/tests/us2_symlink.rs`: a symlink to a directory is not opened, and an SVG inside the symlink target is not converted

### Implementation for User Story 2

- [X] T016 [US2] Implement one-level selection in `crates/svgshaper/src/select.rs`: «Папка и путь-папка: только непосредственные потомки, чьё имя оканчивается на `.svg` без учёта регистра.» «Подпапки и ссылки на папки не открываются.» «Прочие потомки папки пропускаются и не становятся источниками.» «Повтор того же пути в запросе схлопывается в один источник»
- [X] T017 [US2] Honor `StopFlag` in `crates/svgshaper/src/convert.rs`: between sources and before rename, `Child::kill` the running process, delete the temp file, mark that source and every not-yet-started source `Stopped`, and keep already renamed PNGs as `Succeeded`. If the interrupted source was replacing an existing file, that previous file stays. «`Stopped` не смешивается с этими причинами.» Make `us2_folder.rs`, `us2_stop.rs`, and `us2_symlink.rs` pass

**Checkpoint**: Истории 1 и 2 проходят независимо

---

## Phase 5: User Story 3 - Папка назначения и шаблон имени (Priority: P3)

**Goal**: Все PNG одного запроса можно положить в заданную папку и назвать шаблоном из префикса, исходного имени и суффикса. Совпадения имён и одновременные запросы не портят чужой файл

**Independent Test**: Сконвертировать `logo.svg` в папку `out` с префиксом `ico_`, суффиксом `_2x` и шаблоном по умолчанию. Появляется `out/ico_logo_2x.png` запрошенного размера

### Tests for User Story 3

- [X] T018 [P] [US3] Write a failing test in `crates/svgshaper/tests/us3_names.rs` for creating `out/ico_logo_2x.png`, appending a single `.png` when the template has none, not doubling an existing `.png`, rejecting a prefix that contains a path separator with `InvalidName` before any write, and returning `DestinationUnavailable` when the destination cannot be created
- [X] T019 [P] [US3] Write a failing test in `crates/svgshaper/tests/us3_conflicts.rs` for `NameCollision` writing neither file, `replace` true replacing a file only after success, `replace` true plus `CannotDraw` (test converter `--exit-code 1`) leaving the previous file in place, and two overlapping requests to one path yielding one success and one `ConcurrentWrite`
- [X] T031 [P] [US3] Write a failing test in `crates/svgshaper/tests/us3_write.rs`: the destination directory exists, one output path is occupied by a directory so that file cannot be written (`WriteFailed`), and another source in the same request still succeeds

### Implementation for User Story 3

- [X] T020 [US3] Implement output names in `crates/svgshaper/src/request.rs`: «Подстановки только `{prefix}`, `{name}`, `{suffix}`. Прочий текст шаблона буквальный.» «Если имя не оканчивается на `.png` без учёта регистра, добавляется `.png`. Уже оканчивающееся на `.png` не удваивается.» «`path` остаётся внутри `directory`.» Reject «префикс, суффикс или шаблон дают пустое имя, `.`, `..` или содержат разделитель пути» with `InvalidName` before writing
- [X] T021 [US3] Apply destination, replace, and collisions in `crates/svgshaper/src/convert.rs` and `crates/svgshaper/src/store.rs`. Create the destination before writing, or return `DestinationUnavailable`. A single output that cannot be written is `WriteFailed` and does not cancel the other sources. «Два источника одного запроса с одним `path`: оба неуспешны, «совпадение имён», файл не пишется.» When `replace` is true, rename over the existing file only after a complete PNG; on `CannotDraw` the previous file stays. The second in-process request for a claimed path gets `ConcurrentWrite` and does not spawn. Make `us3_names.rs`, `us3_conflicts.rs`, and `us3_write.rs` pass

**Checkpoint**: Все три истории функциональны по отдельности

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Контрольный рисунок, документация публичного API и плагин Bevy. Плагин и рисунок не блокируют файловый MVP

- [X] T033 Add `crates/svgshaper/tests/fixtures/draw/` and a test `crates/svgshaper/tests/draw_oracle.rs` for SC-007. Each fixture is one SVG (shape, stroke, fill, opacity, gradient, text, filter, mask, clip, marker, animation). The text fixture uses `font-family: sans-serif` and has no font file beside it. The animation fixture's checked pixel is the initial appearance. The test calls public `convert` with `converter` set to `rsvg-convert`. If that program is not on `PATH`, the test skips and passes. If it is on `PATH`, every region check in SC-007 passes and each PNG has the requested size
- [X] T022 [P] Create `crates/svgshaper-bevy/Cargo.toml` (`rust-version` 1.97.1, `bevy` 0.20 with `default-features = false`, dependency on `svgshaper`) and add the member to `Cargo.toml`
- [X] T023 [P] Document the public API on `crates/svgshaper/src/lib.rs`: `convert`, `ConversionRequest`, `ConversionReport`, `RequestError`, source outcomes, `StopFlag`, the default `rsvg-convert` argv, `ConverterUnavailable` before any write, and the FR-026 boundary that a relative link opened by the child is outside the library guarantee
- [X] T024 Write a failing headless test in `crates/svgshaper-bevy/tests/plugin.rs`: one `add_plugins(SvgShaperPlugin)`, one request using `svgshaper-test-converter`, one core report event, no window and no GPU. The test does not wait synchronously for the whole batch on the main thread
- [X] T025 Implement `SvgShaperPlugin` in `crates/svgshaper-bevy/src/lib.rs`: `Plugin::build` only registers the report event, a system spawns `svgshaper::convert` on a Bevy 0.20 task pool off the main thread, and `bevy::prelude::Commands` publishes the finished `Result`. The plugin does not assemble `std::process::Command`
- [X] T026 Run `cargo test -p svgshaper` and `cargo test -p svgshaper-bevy` against `specs/001-convert-svg-png/quickstart.md`. The file scenarios pass without `rsvg-convert`. `draw_oracle` passes when `rsvg-convert` is on `PATH` and is skipped when it is not

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: начинается сразу
- **Foundational (Phase 2)**: после Setup и блокирует все истории
- **User Stories (Phase 3–5)**: после Foundational, по приоритету P1 → P2 → P3. Тесты разных файлов можно писать параллельно. `request.rs`, `select.rs`, `convert.rs` и `store.rs` правятся по очереди: US1, затем US2, затем US3
- **Polish (Phase 6)**: после историй, которые должны войти в поставку. Файловый MVP — Phase 3. Приёмка рисунка (T033) и плагин входят в полную поставку, но не в файловый MVP

### User Story Dependencies

- **User Story 1 (P1)**: после Foundational. От US2 и US3 не зависит
- **User Story 2 (P2)**: после Foundational. Встраивается в `convert` из US1 и проверяется своими тестами папки и остановки
- **User Story 3 (P3)**: после Foundational. Встраивается в имя, папку назначения и захват пути. Проверяется тестами `out/ico_logo_2x.png`, шаблона и конфликтов

### Within Each User Story

- Тесты истории пишутся раньше реализации и сначала падают
- Отбор источников и имена — до конвейера `convert`
- История считается готовой, когда её тесты зелёные и предыдущие истории не сломаны

### Parallel Opportunities

- T002 и T003 параллельно после T001
- T004 и T005 параллельно; T006 после T004
- T008, T009, T027, T028, T029 и T032 параллельно
- T014, T015 и T030 параллельно
- T018, T019 и T031 параллельно
- T022, T023 и T033 параллельно: разные файлы
- После Foundational тесты следующих историй можно писать, пока реализация US1 ещё идёт. Правки одних и тех же `.rs` файлов ядра не параллелить

---

## Parallel Example: User Story 1

```bash
Task: "Write a failing test for one SVG at 64×32 and two SVGs at 128×128 in crates/svgshaper/tests/us1_sizes.rs"
Task: "Write a failing test for InvalidSize, EmptySources, NotFound, NotSvg, and NameTaken in crates/svgshaper/tests/us1_failures.rs"
Task: "Write a failing test in crates/svgshaper/tests/us1_sources.rs"
Task: "Write a failing test in crates/svgshaper/tests/us1_converter.rs"
Task: "Write a failing test in crates/svgshaper/tests/us1_argv.rs"
Task: "Write a failing test in crates/svgshaper/tests/us1_nonblocking.rs"
```

## Parallel Example: User Story 2

```bash
Task: "Write a failing test in crates/svgshaper/tests/us2_folder.rs"
Task: "Write a failing test in crates/svgshaper/tests/us2_stop.rs"
Task: "Write a failing test in crates/svgshaper/tests/us2_symlink.rs"
```

## Parallel Example: User Story 3

```bash
Task: "Write a failing test in crates/svgshaper/tests/us3_names.rs"
Task: "Write a failing test in crates/svgshaper/tests/us3_conflicts.rs"
Task: "Write a failing test in crates/svgshaper/tests/us3_write.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Завершить Phase 1: Setup
2. Завершить Phase 2: Foundational
3. Завершить Phase 3: User Story 1
4. **STOP and VALIDATE**: `cargo test -p svgshaper us1`
5. Явные SVG уже конвертируются в PNG заданного размера

### Incremental Delivery

1. Setup + Foundational
2. User Story 1 → отдельные файлы и точный размер
3. User Story 2 → папка без вложенности и остановка
4. User Story 3 → папка назначения, шаблон имени, замена и одновременная запись
5. Polish → контрольный рисунок SC-007, когда есть `rsvg-convert`, документация API, плагин Bevy и прогон quickstart

### Parallel Team Strategy

1. Вместе закрыть Setup и Foundational
2. После этого один человек ведёт US1 (`convert.rs`), другой готовит тесты US2 и US3
3. Правки `crates/svgshaper/src/convert.rs` и `crates/svgshaper/src/select.rs` не брать в параллель: их последовательно занимают US1, затем US2, затем US3

---

## Notes

- [P] означает другие файлы и отсутствие зависимости от незавершённой задачи
- Метка истории есть только у задач Phase 3–5
- Файловый `cargo test` не требует системный `rsvg-convert`. `draw_oracle` пропускается, если программы нет в `PATH`
- Плагин Bevy и контрольный рисунок не блокируют файловый MVP
- T027–T033 закрывают уточнение 2026-11-18 и стоят в фазе своей истории
