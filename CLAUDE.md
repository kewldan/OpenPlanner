# CLAUDE.md

Guidance for AI coding agents working in this repository.

## Project

OpenPlanner is an open-source **desktop apartment-renovation planner**. One model of an apartment produces themed working-drawing sheets (survey, demolition, partitions, furniture, electrical, plumbing, finishes), an estimate and a 3D view. Work proceeds in **stages** (no throwaway MVP); Stages 0 (foundation), 1 (survey: walls, openings, rooms, dimensions, leaders, layers, image underlay), 2 (LiDAR scan import with review), 3 (replanning variants: demolition, partitions with materials, comparison), 4 (drawings and export: sheet set with wall elevations, title block, PDF/DXF/SVG/PNG/XLSX) 5 (3D: orbit, walkthrough, saved views, section cut, sun with shadow map) 6 (furniture: catalog, wall snapping, GLB models, layout sheet), 7 (electrical and lighting: points, panel groups with calculations, switch links, scenes, illuminance, four sheets), 8 (plumbing: fixture outlets, risers, manifolds, auto and manual pipes, slope and level checks, pipes and fittings, two sheets), 9 (finishes: room surfaces, material library, tile layout with start optimisation, consumption, floor and ceiling plans, tiles on elevations), 10 (estimate: price list with quantity rules from the model, rates, reserve and price history, manual quantities, grouping by section and room, purchase list, CSV/XLSX import and export, cost per variant) 11 (extensions: photoreal render queue, IFC 4 export, web version with read-only guest links, DXF and glTF import, floors, floor heating, air conditioning and ventilation, cloud-folder sync) and 12 (MCP server inside the desktop app) are done.

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
        └ Tauri IPC → native Rust: file system (.oplan, journals, export files), dialogs
```

- **The Rust core owns the document.** All edits are `op_model::Command`s applied through `History` (reversible, serialisable): generic `put` / `remove` / `putAsset` / `removeAsset` / `batch` plus project-property commands. One user action = one command (usually a `batch`) = one undo step = one journal line. The UI never mutates the model; it sends `ToEngine` messages and applies `FromEngine` events to Zustand stores (`packages/editor/src/store`). Simple property edits from the inspector send `put` with the modified element.
- **Canvas tools live in the engine** (`crates/op-engine/src/tools.rs`): the UI forwards pointer events (CSS px of the canvas) and keys; snapping (`snap.rs`), hit-testing (`hit.rs`), previews (`overlay.rs`) and command building (`edit.rs`) happen next to the geometry. Derived data (wall outlines, room regions, areas) is recomputed in `derived.rs` from the document — rooms are _label points_, their contours are holes of the union of wall outlines (`op-geom`).
- **Scan recognition lives in `crates/op-scan`** (pure Rust, no GPU): readers → samples → `detect` → `assemble` → `plan::apply`. It runs in a second Web Worker (`analyzeScan` from the same WASM module); the engine imports the result with `importScan` as one batch. Tests use synthetic scans (`op_scan::synth`) — never commit third-party scans. Regenerate the e2e fixture with `cargo run -p op-scan --example synthetic_scan -- apps/desktop/e2e/fixtures/apartment.glb`.
- **Variants are a projection, not copies**: elements without `variant` are the shared survey; elements with `variant: <id>` are new in that variant; `Variant.demolished` lists demolished survey elements. The engine works on `op_model::view::project(doc, view)` and adapts every command to the current view (`crates/op-engine/src/variants.rs`) — tools stay view-agnostic. Status (existing/demolish/new) is computed, never stored.
- **Scenes are built on the CPU** (`op_render::scene::Scene`, `draw.rs`) and uploaded by the WASM wrapper; `EngineState` has no GPU dependency, so engine behaviour is tested natively (`crates/op-engine/src/tests.rs`).
- **Protocol types are generated from Rust** with tsify (`crates/op-engine/src/protocol.rs` → `packages/engine-client/wasm/op_engine.d.ts`). Never hand-write duplicates of these types in TS.
- **One renderer** (`crates/op-render`, wgpu) for 2D, point clouds and 3D. It runs in WASM on WebGPU, falling back to WebGL2. Anything relying on compute shaders needs a non-compute path. The 3D mesh is built on the CPU by the engine (`model3d.rs`); 3D coordinates are X right, Y up the plan (plan Y negated), Z up — so the model is not mirrored.
- **Furniture** is an element (`Furniture`) like walls: projected per variant, demolishable in a variant. The catalog, plan symbols, 3D parts and wall snapping live in `crates/op-engine/src/furniture.rs` — symbols and shapes are drawn procedurally from the item size (clean room); local axes: width along X, back on −Y. Own GLB models are stored under `assets/furniture/` (the `.oplan` reader only accepts `assets/` and `scan/`).
- **Electrical points** (`Device`) follow the same pattern (`crates/op-engine/src/electric.rs`); panel groups and lighting scenes are document lists replaced whole by `setCircuits` / `setScenes`. Switch keys reference light ids: deleting a light or copying it into a variant must keep `keys` and scenes consistent (`edit.rs`, `variants.rs`). Engineering formulas are pure functions in `crates/op-calc` with unit tests; the engine only gathers inputs and emits the `electrical` report.
- **Plumbing** (`PlumbingPoint`, `Pipe`) follows the device pattern (`crates/op-engine/src/plumbing.rs`): outlets sit exactly on the wall face (`electric::snap_to_face_at` with inset 0), pipes are 3D polylines whose vertex `z` the engine computes (water at the start level, drain with the normal slope). `routePipes` replaces only `auto` pipes; outlets made by `fixtureOutlets` reference their fixture (`fixture`), remapped on variant copy.
- **Finishes are a document list** (`finishes`, `materials`, replaced whole by `setFinishes` / `setMaterials`), keyed by room id; a single wall is the face index of the room contour — the same letters as on the elevations (`crates/op-engine/src/surfaces.rs`, shared by `finish.rs` and `sheets/elevations.rs`). Wall finishes of a room override each other in list order within their height band. Tile layout is `op_geom::tiling`; consumption norms are `op_calc::finish`. Deleting a room or a variant drops its finishes; copying a variant copies them.
- **3D view state is window state, not document**: `Options3d` and the camera live in `EngineState` (`view3d.rs`, `camera3d.rs`); only the site orientation (`site`) and saved views (`views`) are project properties (`setSite`, `setViews`). In 3D the plan tools are disabled; the UI maps mouse, wheel and WASD to `camera3d` messages.
- **Platform differences live behind `Platform`** (`packages/editor/src/platform`): `desktop.ts` (Tauri) and `browser.ts` (web build / e2e). The editor code must not call Tauri APIs directly.
- **The estimate is computed, never stored**: the document keeps only the price list (`prices`, `setPrices`) and manual quantities (`overrides`, keyed by price item + room, `setOverrides`). `crates/op-engine/src/estimate.rs` turns each item's `Measure` into lines per room (areas, points, furniture, finishes) or one shared line (demolition, partitions, openings, cable, pipes) for the current view and emits `estimate` after every edit; `ViewSummary.cost` prices every variant. Quantity math is `op_calc::estimate`; CSV and XLSX reading are `op_export::csv` / `op_export::xlsx::read`. `Measure` has a one-line token form (`finish:floor:tile`) shared by the CSV/XLSX price files and `measureKey` in `packages/editor/src/estimate/prices.ts` — keep them in sync.
- **Floors are a projection like variants**: elements carry `level` (none = the implicit first floor); walls and openings follow their nodes. Always project with `op_model::view::project_level(doc, variant, level)`; the engine tags new elements with the current level in `EngineState::adapt`. Plan, tools and per-trade reports use the current level; summaries, the estimate and IFC iterate `view::levels(doc)`; 3D stacks lower floors under the current one.
- **Climate** points are `DeviceKind`s (`is_climate()`); floor heating zones are a document list (`heating`, `setHeating`). A thermostat's load in the panel calc is the sum of its zones (`electric::power`). Norms live in `op_calc::climate`, the report in `crates/op-engine/src/climate.rs`.
- **Render** is a CPU path tracer (`op_render::trace`) run in `render.worker.ts`: the engine encodes scene + camera (`renderInput`), the worker returns PNG (`traceImage`), shots are stored as `assets/renders/*.png` + `renders`. Camera rays see faces one-sided (like the rasteriser), shadow rays two-sided.
- **Guest mode**: `#guest=<base64url .oplan without assets>` opens the web build read-only (`setReadOnly` makes the engine reject every edit); the project controller skips the journal and saving. Cloud-folder sync compares `Platform.fileStamp` on focus and before autosave.
- **MCP** lives in the engine (`crates/op-engine/src/mcp.rs`: JSON-RPC, tool schemas and handlers over `EngineState`, natively tested). The desktop binary with the `mcp` argument is the stdio bridge to the running app (TCP 127.0.0.1 + token in `<app data>/mcp.json`); the window relays lines to the engine (`packages/editor/src/mcp/relay.ts`) and handles `save_project` / `open_project` itself. New engine features should get an MCP tool or be reachable through `apply_command`.
- **Drawings are built by the engine** (`crates/op-engine/src/sheets.rs` → `op_export::Sheet`) and written by `op-export`'s own PDF/SVG/DXF/XLSX writers; the UI only rasterises PNG and saves files via `Platform.saveExport` (native `write_export` accepts export extensions only, now including `.csv`). Price files are picked with `Platform.openTable` (native `read_table`: `.csv` / `.xlsx` only).
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

Checks (CI runs them on every push; run the relevant ones before pushing):

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

Desktop checks (real app; need `bunx tauri build --debug --no-bundle` in `apps/desktop` first). CI runs only the selftest on every push; dialogs, the Linux window e2e, WebGPU in WebView2 and GPU frames live in `.github/workflows/desktop-checks.yml` (manual and weekly):

- `scripts/desktop-selftest.sh <binary>` — launches the app, waits for the engine, prints the GPU backend. `EXPECT_BACKEND=BrowserWebGpu` makes it strict.
- `scripts/desktop-dialogs.sh <binary>` — native Save/Open dialogs, `kill -9`, native recovery prompt (AppleScript / SendKeys / xdotool).
- `scripts/desktop-mcp.py <binary>` — MCP through the real app: the bridge launches it, an agent draws a flat, exports PDF/IFC and saves.
- `scripts/desktop-e2e.sh` — Linux WebDriver e2e of the real window (Xvfb, openbox, xdotool, imagemagick, webkit2gtk-driver, `tauri-driver`).

Other:

- `bun run build:web` in `apps/desktop` — the static web version (release WASM, relative paths).
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
- **E2E shortcuts on a Mac:** `mod` is ⌘ there, so specs pressing `Control+…` fail locally (they pass on Linux CI). Write new specs with `ControlOrMeta+…`.
- **Biome treats only `use` + an uppercase letter as a hook name** (`use3dKeys` trips `useHookAtTopLevel`).
- **Send whole option objects from an up-to-date base**: `set3d` sends all 3D options at once, so the UI updates the store optimistically first — otherwise quick successive clicks overwrite each other with stale values.
- **glam 0.34 has no `DMat4::look_at_rh` / `perspective_rh`**; `op_render::three` has its own f64 helpers with wgpu's 0–1 depth.
- **DXF output is cp1251**: read it as bytes in tests, not as UTF-8.
- **`bun run build:wasm` needs `~/.cargo/bin` on `PATH`** (the script looks for `wasm-bindgen`); otherwise it fails and the old `.d.ts` stays — `tsc` then reports missing new types.
- **`pkill -f <pattern>` kills your own shell** when the pattern appears in the command line; kill by PID (`pgrep … | xargs kill`).
- **Playwright in this container** needs `PLAYWRIGHT_CHROMIUM_PATH=/opt/pw-browsers/chromium-*/chrome-linux64/chrome` (the project pins a newer Playwright than the preinstalled browsers).
- CI macOS VMs and GPU-less Windows runners have no hardware WebGPU adapter; Windows gets a software one via `--enable-unsafe-webgpu --ignore-gpu-blocklist`. The selftest reports `backend=`, `webgpu=` (worker) and `main_webgpu=` (main thread) — use them to tell environment limits from bugs.

## Working in this repo

- Prefer extending existing modules over adding parallel ones; keep the dependency direction `ui ← editor ← desktop`, `engine-client` only wraps the worker.
- After changing Rust crates used by the engine, rebuild WASM before running TS checks or e2e (`tsc` reads the generated `.d.ts`).
- Do not commit generated output: `packages/engine-client/wasm/`, `apps/desktop/dist/`, `apps/desktop/src-tauri/gen/`, `test-results/`.
- Keep `docs/` in sync when behaviour or architecture changes; update `docs/decisions.md` for anything a future contributor would otherwise ask "why?" about.
