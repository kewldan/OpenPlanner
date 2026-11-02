# CLAUDE.md

Guidance for AI coding agents working in this repository.

## Project

OpenPlanner is an open-source **desktop apartment-renovation planner**. One model of an apartment produces themed working-drawing sheets (survey, demolition, partitions, furniture, electrical, plumbing, finishes), an estimate and a 3D view. Work proceeds in **stages** (no throwaway MVP); Stages 0 (foundation), 1 (survey: walls, openings, rooms, dimensions, leaders, layers, image underlay), 2 (LiDAR scan import with review) 3 (replanning variants: demolition, partitions with materials, comparison), 4 (drawings and export: sheet set with wall elevations, title block, PDF/DXF/SVG/PNG/XLSX) and 5 (3D: orbit, walkthrough, saved views, section cut, sun with shadow map) are done; Stage 6 (furniture) is next.

Key documents (written in Russian — read them before larger changes):

- `docs/product-spec.md` — features and stages.
- `docs/architecture.md` — processes, protocol, model, renderer, file format, testing.
- `docs/decisions.md` — the decision log. **Key architectural decisions belong to the project owner**: propose options and ask; record accepted decisions here before implementing them.

## Language conventions

- Code identifiers: English.
- Code comments, doc comments, docs and **UI strings: Russian.** UI strings go through the message catalog (`packages/editor/src/i18n/ru.ts`, accessed with `t()`), never inline.
- Commit messages: English, Conventional Commits.

## Architecture rules

```
WebView ─ UI thread (React)  ⇄ postMessage ⇄  Web Worker: op-engine (WASM) = model + wgpu renderer → OffscreenCanvas
        └ Tauri IPC → native Rust: file system, journals, dialogs (later: scan, export)
```

- **The Rust core owns the document.** All edits are `op_model::Command`s applied through `History` (reversible, serialisable): generic `put` / `remove` / `putAsset` / `removeAsset` / `batch` plus project-property commands. One user action = one command (usually a `batch`) = one undo step = one journal line. The UI never mutates the model; it sends `ToEngine` messages and applies `FromEngine` events to Zustand stores (`packages/editor/src/store`). Simple property edits from the inspector send `put` with the modified element.
- **Canvas tools live in the engine** (`crates/op-engine/src/tools.rs`): the UI forwards pointer events (CSS px of the canvas) and keys; snapping (`snap.rs`), hit-testing (`hit.rs`), previews (`overlay.rs`) and command building (`edit.rs`) happen next to the geometry. Derived data (wall outlines, room regions, areas) is recomputed in `derived.rs` from the document — rooms are _label points_, their contours are holes of the union of wall outlines (`op-geom`).
- **Scan recognition lives in `crates/op-scan`** (pure Rust, no GPU): readers → samples → `detect` → `assemble` → `plan::apply`. It runs in a second Web Worker (`analyzeScan` from the same WASM module); the engine imports the result with `importScan` as one batch. Tests use synthetic scans (`op_scan::synth`) — never commit third-party scans. Regenerate the e2e fixture with `cargo run -p op-scan --example synthetic_scan -- apps/desktop/e2e/fixtures/apartment.glb`.
- **Variants are a projection, not copies**: elements without `variant` are the shared survey; elements with `variant: <id>` are new in that variant; `Variant.demolished` lists demolished survey elements. The engine works on `op_model::view::project(doc, view)` and adapts every command to the current view (`crates/op-engine/src/variants.rs`) — tools stay view-agnostic. Status (existing/demolish/new) is computed, never stored.
- **Scenes are built on the CPU** (`op_render::scene::Scene`, `draw.rs`) and uploaded by the WASM wrapper; `EngineState` has no GPU dependency, so engine behaviour is tested natively (`crates/op-engine/src/tests.rs`).
- **Protocol types are generated from Rust** with tsify (`crates/op-engine/src/protocol.rs` → `packages/engine-client/wasm/op_engine.d.ts`). Never hand-write duplicates of these types in TS.
- **One renderer** (`crates/op-render`, wgpu) for 2D, point clouds and 3D. It runs in WASM on WebGPU, falling back to WebGL2. Anything relying on compute shaders needs a non-compute path. The 3D mesh is built on the CPU by the engine (`model3d.rs`); 3D coordinates are X right, Y up the plan (plan Y negated), Z up — so the model is not mirrored.
- **Platform differences live behind `Platform`** (`packages/editor/src/platform`): `desktop.ts` (Tauri) and `browser.ts` (web build / e2e). The editor code must not call Tauri APIs directly.
- **Drawings are built by the engine** (`crates/op-engine/src/sheets.rs` → `op_export::Sheet`) and written by `op-export`'s own PDF/SVG/DXF/XLSX writers; the UI only rasterises PNG and saves files via `Platform.saveExport` (native `write_export` accepts export extensions only).
- **`.oplan` bytes are produced and parsed by the engine** (`op-format` in WASM). The native side only reads/writes bytes (`apps/desktop/src-tauri/src/files.rs`) and only accepts `.oplan` / `.oplan.journal` paths.
- **Licensing boundary:** `crates/op-format` and `schema/` are MIT; everything else is AGPL-3.0. `op-format` must not depend on AGPL crates. Dependencies must be AGPL-compatible; GPL-2.0-only is not allowed.
- **Actions/shortcuts:** every user command is an entry in `packages/editor/src/actions/index.ts`; the ⌘K palette, the cheat sheet and keyboard handling are built from it. Shortcuts use `KeyboardEvent.code` so they work in any layout; `mod` = ⌘ on macOS, Ctrl elsewhere.

## Commands

```bash
bun install                     # also installs Lefthook git hooks
bun run build:wasm              # build the engine (needed after Rust changes it depends on)
bun run dev                     # desktop app (Tauri)
bun run dev:web                 # browser build at http://localhost:1420
```

Checks (CI runs all of them; run the relevant ones before pushing):

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p op-engine --target wasm32-unknown-unknown -- -D warnings
cargo test --workspace
bunx biome check .          # use `bunx biome check --write .` to fix
bunx tsc -b
bun test                    # unit tests under packages/ (see bunfig.toml)
cd apps/desktop && bunx vite build && bunx playwright test   # web e2e: chromium (WebGL2) + webgpu projects
```

Desktop checks (real app; need `bunx tauri build --debug --no-bundle` in `apps/desktop` first):

- `scripts/desktop-selftest.sh <binary>` — launches the app, waits for the engine, prints the GPU backend. `EXPECT_BACKEND=BrowserWebGpu` makes it strict.
- `scripts/desktop-dialogs.sh <binary>` — native Save/Open dialogs, `kill -9`, native recovery prompt (AppleScript / SendKeys / xdotool).
- `scripts/desktop-e2e.sh` — Linux WebDriver e2e of the real window (Xvfb, openbox, xdotool, imagemagick, webkit2gtk-driver, `tauri-driver`).

Other:

- `UPDATE_SCHEMA=1 cargo test -p op-model` — regenerate `schema/model.schema.json` after model changes (a test fails if it is stale).
- `OPENPLANNER_REQUIRE_GPU=1 cargo test -p op-render -- --nocapture` — the offscreen render test must find a GPU adapter (otherwise it is skipped).

## Conventions

- **Rust:** edition 2024, strict clippy via `[workspace.lints]` (no `unwrap`/`expect`/`panic` outside tests, checked numeric casts, `float_cmp`). Justify every `#[allow(...)]` with a comment. `unsafe` is denied workspace-wide; the only exception is the documented WebKitGTK FFI in `apps/desktop/src-tauri/src/linux.rs`.
- **TypeScript:** strict mode with `exactOptionalPropertyTypes` and `noUncheckedIndexedAccess`; Biome for lint and format; CSS Modules plus design tokens (`packages/ui/src/tokens.css`); Base UI for unstyled primitives.
- **Commits:** Conventional Commits; scopes are crate/package names without prefixes (`feat(scan): …`, `fix(editor): …`). Lefthook checks the message and formats staged files.
- **File format changes:** bump `FORMAT_VERSION` in `op-format` and append a migration to `MIGRATIONS` (compilation fails otherwise); regenerate the schema.
- **Tests first-class:** new behaviour gets unit tests; user-visible flows get e2e coverage.

## Clean room (mandatory)

OpenPlanner reimplements **behaviour** of existing planners, never their code, text, icons, layouts or assets.

- Do not decompile, scrape or automate proprietary planners, and do not parse their closed project formats.
- Draw icons and symbols from scratch (GOST/ESKD, IEC). Use only own, CC0 or license-compatible assets.
- Log new public sources for behaviour specs in `docs/cleanroom/sources.md` (link, date, what was taken — in your own words).

## Gotchas learned the hard way

- **wasm-bindgen CLI must match** the `wasm-bindgen` crate version pinned in `Cargo.toml` (`=0.2.130`); `scripts/build-wasm.sh` checks this.
- **tsify types can vanish from `.d.ts` in dev builds** when their codegen unit is not linked; `op-model` uses `codegen-units = 1` in the dev profile for this reason. If a type is missing, `tsc` fails on its import — do not paper over it.
- **WebKitGTK disables WebGL in workers by default.** The Linux app enables `AllowWebGLInWorkers` at startup and reloads (`linux.rs`). WebKitGTK from Ubuntu has no WebGPU at all.
- **The main window is created in code** (`create: false` in `tauri.conf.json`) so CI can pass WebView2 arguments (`OPENPLANNER_WEBVIEW2_ARGS`); `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` is ignored under Tauri.
- **The canvas element is created inside the effect**: `transferControlToOffscreen()` works once per element, and React StrictMode runs effects twice.
- **Inspector fields do not take document updates while focused**, otherwise a late confirmation of a previous edit overwrites what the user is typing.
- **Ask the engine, not the store, when ordering matters** (e.g. the file name for Save comes from an engine call, which is processed after all earlier commands).
- **Playwright downloads need a UTF-8 locale** (`LANG=C.UTF-8`), or Chromium renames non-ASCII files to `download`.
- **Windows dialog automation** must focus the _enabled_ window of the process (the modal dialog), minimise the runner console, and paste paths via the clipboard (typing gets mangled by autocomplete).
- **`Element` is tagged with `type` in JSON**, not `kind` — `Opening` has its own `kind` field and an internally tagged enum would silently produce duplicate keys. `every_element_survives_json` guards this.
- **Text halos are drawn in a separate pass** under all glyphs; drawing halo and glyph together lets the next glyph's halo erase the previous glyph's right edge («под» turned into «гсд»). Small text averages 4 SDF samples per pixel.
- **Opening a file zooms to fit**, so e2e clicks after `importFile` must use the fitted camera (see `fitView` in `e2e/survey.spec.ts`); a fresh/new project uses the default camera ((4000, 3000) mm at the centre, 0.1 px/mm).
- **In the browser a reopened file has no path**: its journal is not found after a reload. Recovery after reload is tested for untitled projects; file recovery is covered by the desktop tests.
- **Synthetic scan holes must align with the mesh grid** — `synth` splits wall cells at opening edges; otherwise detected widths drift by a cell. Detection precision is ~50 mm horizontally and ~100 mm vertically, refined by fractional emptiness of boundary cells.
- **Disk fills up**: `target/` grows past 20 GB with native, WASM and Tauri builds. Delete `target/*/incremental` when `No space left on device` appears.
- **`pkill -f <pattern>` kills your own shell** when the pattern appears in the command line; kill by PID (`pgrep … | xargs kill`).
- **Playwright in this container** needs `PLAYWRIGHT_CHROMIUM_PATH=/opt/pw-browsers/chromium-*/chrome-linux64/chrome` (the project pins a newer Playwright than the preinstalled browsers).
- CI macOS VMs and GPU-less Windows runners have no hardware WebGPU adapter; Windows gets a software one via `--enable-unsafe-webgpu --ignore-gpu-blocklist`. The selftest reports `backend=`, `webgpu=` (worker) and `main_webgpu=` (main thread) — use them to tell environment limits from bugs.

## Working in this repo

- Prefer extending existing modules over adding parallel ones; keep the dependency direction `ui ← editor ← desktop`, `engine-client` only wraps the worker.
- After changing Rust crates used by the engine, rebuild WASM before running TS checks or e2e (`tsc` reads the generated `.d.ts`).
- Do not commit generated output: `packages/engine-client/wasm/`, `apps/desktop/dist/`, `apps/desktop/src-tauri/gen/`, `test-results/`.
- Keep `docs/` in sync when behaviour or architecture changes; update `docs/decisions.md` for anything a future contributor would otherwise ask "why?" about.
