<div align="center">

# 📐 OpenPlanner

**An open-source desktop planner for apartment renovation — from a tape measure (or a LiDAR scan) to a complete set of working drawings and a cost estimate.**

[![CI](https://github.com/kewldan/OpenPlanner/actions/workflows/ci.yml/badge.svg)](https://github.com/kewldan/OpenPlanner/actions/workflows/ci.yml)
![License: AGPL-3.0](https://img.shields.io/badge/app-AGPL--3.0-blue)
![File format: MIT](https://img.shields.io/badge/.oplan%20format-MIT-green)
![Platforms](https://img.shields.io/badge/macOS%20·%20Windows%20·%20Linux-desktop-lightgrey)
![Stage](https://img.shields.io/badge/stage%202%20scan-done-brightgreen)

<img src="docs/images/survey.png" alt="OpenPlanner survey: walls, doors, windows, rooms with areas, dimensions and a leader" width="860">

</div>

---

Most room planners stop at a pretty 3D picture. OpenPlanner is built around what a renovation crew actually needs: **one model of the apartment** that turns into themed drawing sheets — survey, demolition, new partitions, furniture, electrical, plumbing, finishes — plus an estimate whose quantities come straight from the model. Exporting your own project will never be paywalled. 🆓

> 🚧 **Status: Stage 2 (scan) is complete.** Import a LiDAR scan (Polycam GLB/OBJ/PLY/ZIP or Apple RoomPlan data), review the doubtful spots and get walls, openings and rooms with areas in one step — on top of the Stage 1 survey tools. Next up: replanning and design variants (Stage 3). See the [roadmap](#-roadmap).

## ✨ Highlights

|                                      |                                                                                                                                                                                                                                                                                                                                                                             |
| ------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 📡 **Scan to plan**                  | Drop in a LiDAR scan — Polycam GLB, OBJ, PLY, ZIP or Apple RoomPlan room data. OpenPlanner finds the floor, ceiling, walls (with measured thickness where both faces were scanned), doors and windows, asks about doubtful spots with a confidence score, and imports the plan with room areas as a single undoable step. Recognition runs locally, in a background worker. |
| 📏 **Survey tools**                  | Wall chains with snapping (ends, midpoints, perpendicular, axes, grid) or exact lengths and angles from the keyboard, mitered corners and T-junctions, doors / windows / arches, node-anchored dimensions, leaders, layers you can hide or lock, and an image underlay you calibrate with one known distance.                                                               |
| 📐 **Rooms that compute themselves** | Click inside a closed contour (or let _Find rooms_ do it): area and perimeter are measured along the wall faces and stay correct when you move, split or extend walls.                                                                                                                                                                                                      |
| 🦀 **Rust core, GPU canvas**         | The document model and the renderer are Rust compiled to WebAssembly and run in a Web Worker. The plan is drawn with **wgpu** — WebGPU where the platform provides it, WebGL2 otherwise — so the UI thread stays responsive.                                                                                                                                                |
| 🖥️ **Native desktop app**            | Built on **Tauri 2**: small binaries, native file dialogs, real `.oplan` files on disk. macOS is the primary target; Windows and Linux are first-class in CI.                                                                                                                                                                                                               |
| 🛟 **Crash-proof by design**         | Every edit is appended to a journal next to your file. Kill the process, pull the plug — on the next open OpenPlanner offers to restore your changes, undo history included. Projects with a file are also autosaved every 30 s.                                                                                                                                            |
| 💾 **Open file format**              | `.oplan` is a plain ZIP with a JSON model, a published [JSON Schema](schema/) and versioned migrations. The format crate and schema are **MIT-licensed**, so other tools can read and write projects freely.                                                                                                                                                                |
| ⌨️ **Keyboard-first**                | A command palette (<kbd>⌘K</kbd>), a shortcut cheat sheet (<kbd>⌘/</kbd>), and shortcuts that work in any keyboard layout.                                                                                                                                                                                                                                                  |
| ↩️ **Real undo**                     | Every change is a reversible command in the Rust core. The “unsaved” marker is tracked precisely — undo back to the saved state and it clears.                                                                                                                                                                                                                              |
| 🌗 **Dense, calm UI**                | A compact Figma/Linear-style interface with light and dark themes. The drawing canvas always stays paper-white.                                                                                                                                                                                                                                                             |
| 🧪 **Tested on real windows**        | CI launches the actual desktop app on macOS, Windows and Linux, drives the **native** Save/Open dialogs, kills the process and verifies recovery, and checks rendered pixels on Metal, D3D12 and Vulkan.                                                                                                                                                                    |

<p align="center">
  <img src="docs/images/crash-recovery.png" alt="Native recovery prompt after the app was killed" width="760"><br>
  <sub>After <code>kill -9</code>: the native prompt offers to restore the edits from the journal.</sub>
</p>

## 🗺️ Roadmap

Development goes in stages. Each stage ships fully working; there is no throwaway MVP.

| Stage                     | Scope                                                                                                 | Status     |
| ------------------------- | ----------------------------------------------------------------------------------------------------- | ---------- |
| 0 · Foundation            | App shell, engine, `.oplan` files, autosave and recovery, command palette, CI on 3 OSes               | ✅ Done    |
| 1 · Survey                | Walls with snapping and numeric input, openings, rooms with areas, dimensions, layers, image underlay | ✅ Done    |
| 2 · Scan                  | Import a Polycam LiDAR scan → walls, openings and rooms, with a review step                           | ✅ Done    |
| 3 · Replanning            | Existing / demolish / new status, partitions, design variants and comparison                          | 🔜 Next    |
| 4 · Drawings              | Sheet set with title block, vector PDF, DXF by layers                                                 | 📋 Planned |
| 5 · 3D                    | Orbit, walkthrough, section cut, sun                                                                  | 📋 Planned |
| 6 · Furniture             | Catalog, drag and drop, GLB import                                                                    | 📋 Planned |
| 7 · Electrical & lighting | Outlets, switches, panel groups, cable runs, illuminance                                              | 📋 Planned |
| 8 · Plumbing              | Fixtures, outlets, auto-routing, slope checks                                                         | 📋 Planned |
| 9 · Finishes              | Surfaces, tile layout with cuts, wall elevations                                                      | 📋 Planned |
| 10 · Estimate             | Quantities from the model, price lists, XLSX                                                          | 📋 Planned |
| 11 · Extensions           | Photoreal render, IFC, web version, floors, sync                                                      | 📋 Planned |

Full details: [product spec](docs/product-spec.md) (in Russian).

## 🏗️ How it fits together

```
┌──────────────────────────── WebView (WKWebView · WebView2 · WebKitGTK) ────────────────────────────┐
│  UI thread: React + TypeScript              Web Worker: op-engine (Rust → WASM)                     │
│  panels, inspector, ⌘K palette    ──cmds──▶ document model · commands · undo · autosave journal     │
│  Zustand projections of the model ◀─events─ wgpu renderer (WebGPU | WebGL2) → OffscreenCanvas       │
└───────────────────────────────────────────────┬─────────────────────────────────────────────────────┘
                                                │ Tauri IPC
┌───────────────────────────────────────────────┴─────────────────────────────────────────────────────┐
│  Tauri 2 (native Rust): file system, atomic .oplan writes, journals, dialogs; later scan & export  │
└─────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

- The **Rust core is the source of truth.** The UI sends commands and renders projections.
- **TypeScript types are generated from Rust** (tsify), so the protocol can't drift.
- The same crates compile to **WASM** (worker, future web version) and **native** (Tauri, tests).

More in [architecture](docs/architecture.md) and the [decision log](docs/decisions.md) (both in Russian).

## 🧪 Try a build

Every CI run publishes the desktop app as an artifact: open a green run on the [Actions page](https://github.com/kewldan/OpenPlanner/actions/workflows/ci.yml) and download **OpenPlanner-macOS** (Apple Silicon `.app`), **OpenPlanner-Windows** (`.exe`, needs WebView2 — built into Windows 10/11) or **OpenPlanner-Linux** (binary, needs WebKitGTK 4.1).

The macOS build is not notarised yet. After unzipping, either right-click the app → **Open**, or run:

```bash
xattr -dr com.apple.quarantine OpenPlanner.app
```

## 🚀 Getting started

**Prerequisites**

- 🦀 Rust — the toolchain is pinned in `rust-toolchain.toml` (includes `wasm32-unknown-unknown`)
- 🥟 [Bun](https://bun.com) 1.4+
- 🔧 `wasm-bindgen-cli`, matching the `wasm-bindgen` version in `Cargo.toml`:

  ```bash
  cargo install wasm-bindgen-cli --version 0.2.130 --locked
  ```

- 🧩 Tauri system dependencies for your OS: <https://v2.tauri.app/start/prerequisites/>

**Run**

```bash
bun install
```

```bash
bun run build:wasm
```

```bash
bun run dev
```

`bun run dev` opens the desktop app. For the browser build only, use `bun run dev:web` (<http://localhost:1420>). Rebuild the engine with `bun run build:wasm` after changing Rust crates it depends on.

## 🧪 Testing

| What                                        | Command                                                                                                         |
| ------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| 🦀 Rust: format, lints, tests               | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` |
| 🟦 TypeScript: lint, types, unit tests      | `bunx biome check . && bunx tsc -b && bun test`                                                                 |
| 🦀 Engine scenarios (pointer + keyboard)    | `cargo test -p op-engine`                                                                                       |
| 🌐 E2E on the web build (WebGL2 + WebGPU)   | `cd apps/desktop && bunx vite build && bunx playwright test`                                                    |
| 🖥️ E2E on the real Linux desktop window     | `bash scripts/desktop-e2e.sh` (needs a built app, Xvfb, openbox, xdotool, `tauri-driver`)                       |
| 🗂️ Native dialogs + crash recovery (any OS) | `bash scripts/desktop-dialogs.sh <path to app binary>`                                                          |

CI runs all of the above on every push, plus a selftest of the packaged app on **macOS 26, Windows and Linux** and an offscreen render check on **Metal, D3D12 and Vulkan**.

## 📁 Repository layout

```
apps/desktop/          Tauri 2 app (src-tauri) + Vite entry, e2e tests
packages/ui/           design tokens and base components (CSS Modules, Base UI)
packages/editor/       the editor: shell, inspector, canvas, commands, project files
packages/engine-client/  worker + typed client for the WASM engine
crates/op-model/       document, commands, undo/redo, journal ops
crates/op-geom/        geometry: wall joints, unions, room regions
crates/op-render/      wgpu renderer (+ offscreen rendering for tests)
crates/op-engine/      WASM engine: model + renderer in the worker
crates/op-format/      .oplan read/write, migrations, journal          (MIT)
crates/op-scan/        LiDAR scan readers and recognition
crates/op-export/      PDF, XLSX, DXF, IFC                             (stage 4+)
crates/op-calc/        engineering calculations and estimates          (stages 7–10)
schema/                JSON Schema of the file format                  (MIT)
scripts/               WASM build, desktop selftests and drivers
docs/                  product spec, architecture, decision log
```

## 🛠️ Tech stack

**Tauri 2** · **React 19** + **TypeScript** · **Rust** + **wgpu** (WebGPU / WebGL2) · **wasm-bindgen** + **tsify** · **Zustand** · **Base UI** · **CSS Modules** · **Bun** + **Vite** · **Biome** · **Playwright** · **Lefthook**

## 🤝 Contributing

Contributions are welcome — please read [CONTRIBUTING.md](CONTRIBUTING.md) first. In short:

- 🧼 **Clean room.** OpenPlanner reimplements _behaviour_, never code, text, icons or assets of proprietary planners. No decompiling or scraping of other products.
- ✍️ Commits follow [Conventional Commits](https://www.conventionalcommits.org/) and are signed off (DCO).
- 🧭 Key architectural decisions are recorded in [`docs/decisions.md`](docs/decisions.md) before they are implemented.

## 📄 License

- The application is licensed under **[AGPL-3.0](LICENSE)**.
- The project file format — [`crates/op-format`](crates/op-format) and [`schema/`](schema) — is licensed under **MIT**, so any tool can read and write `.oplan` files.
