// Обёртка протокола между UI-потоком и воркером.
// Типы ToEngine/FromEngine генерируются из Rust (crates/op-engine/src/protocol.rs) через tsify.

import type { FromEngine, ToEngine } from "../wasm/op_engine";

export type {
  Command,
  Document,
  EngineEvents,
  FromEngine,
  ProjectMeta,
  ToEngine,
} from "../wasm/op_engine";

/** UI → воркер. */
export type WorkerRequest =
  | {
      kind: "init";
      canvas: OffscreenCanvas;
      width: number;
      height: number;
      devicePixelRatio: number;
    }
  | { kind: "message"; message: ToEngine };

/** Воркер → UI. */
export type WorkerResponse =
  | { kind: "ready"; backend: string }
  | { kind: "events"; events: FromEngine[] }
  | { kind: "fatal"; message: string };
