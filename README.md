# OpenPlanner

Открытый десктопный планировщик ремонта квартиры. Один план превращается в комплект рабочих чертежей для бригады (обмер, демонтаж, перегородки, мебель, электрика, сантехника, отделка), смету и 3D-вид. Обмер можно сделать по LiDAR-скану с телефона.

Статус: **стадия 0 завершена**, следующая — стадия 1 (обмер). Уже работает:

- окно редактора по дизайну, светлая и тёмная темы;
- холст плана на wgpu (WebGPU или WebGL2) в Web Worker: сдвиг и масштаб;
- документ в Rust-ядре: команды, отмена и повтор;
- файлы `.oplan`: новый, открыть, сохранить, сохранить как; автосохранение и восстановление несохранённых изменений после сбоя;
- палитра команд ⌘K и шпаргалка горячих клавиш ⌘/.

Инструментов черчения пока нет.

## Документация

- [Продуктовая спецификация и стадии](docs/product-spec.md)
- [Архитектура](docs/architecture.md)
- [Журнал решений](docs/decisions.md)

## Стек

Tauri 2 · React + TypeScript · Rust-ядро и wgpu-рендерер в WASM внутри Web Worker · Bun + Vite · Biome · Cargo workspace.

## Требования

- Rust (версия закреплена в `rust-toolchain.toml`, вместе с целью `wasm32-unknown-unknown`)
- [Bun](https://bun.com) 1.4+
- `wasm-bindgen-cli` той же версии, что `wasm-bindgen` в `Cargo.toml`:

  ```bash
  cargo install wasm-bindgen-cli --version 0.2.130 --locked
  ```

- Системные зависимости Tauri для вашей ОС: https://v2.tauri.app/start/prerequisites/

## Запуск

```bash
bun install
```

```bash
bun run build:wasm
```

```bash
bun run dev
```

`bun run dev` открывает десктопное приложение. Только веб-версия в браузере — `bun run dev:web` (http://localhost:1420).

После изменений в Rust-крейтах движка пересоберите WASM: `bun run build:wasm`.

## Проверки

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

```bash
bunx biome check . && bunx tsc -b && bun test
```

E2E по веб-сборке (Playwright, Chromium):

```bash
cd apps/desktop && bunx vite build && bunx playwright test
```

E2E настоящего десктопного окна (Linux: Xvfb, openbox, xdotool, imagemagick, webkit2gtk-driver и `cargo install tauri-driver`):

```bash
cd apps/desktop && bunx tauri build --debug --no-bundle && cd ../.. && bash scripts/desktop-e2e.sh
```

## Структура

```
apps/desktop/        Tauri 2 + Vite-приложение
packages/ui/         дизайн-токены и базовые компоненты
packages/editor/     редактор: каркас, панели, инспектор, холст
packages/engine-client/  клиент WASM-движка и Web Worker
crates/op-model/     документ, команды, отмена/повтор
crates/op-geom/      геометрия
crates/op-render/    рендерер на wgpu
crates/op-engine/    WASM-движок: модель + рендерер
crates/op-format/    формат .oplan (MIT)
crates/op-scan/      распознавание скана (стадия 2)
crates/op-export/    PDF, XLSX, DXF, IFC (стадия 4+)
crates/op-calc/      инженерные расчёты и смета (стадии 7–10)
schema/              JSON Schema формата (MIT)
```

## Лицензия

Приложение — [AGPL-3.0](LICENSE). Формат файла проекта (`crates/op-format`, `schema/`) — MIT, чтобы сторонние инструменты могли свободно читать и писать `.oplan`.
