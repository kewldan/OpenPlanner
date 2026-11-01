// Обёртка протокола между UI-потоком и воркером.
// Типы ToEngine/FromEngine генерируются из Rust (crates/op-engine/src/protocol.rs) через tsify.

import type { FromEngine, ToEngine } from "../wasm/op_engine";

export type {
  Command,
  Document,
  EngineEvents,
  FromEngine,
  HistoryOp,
  ProjectMeta,
  ToEngine,
} from "../wasm/op_engine";

/** Вызов движка, на который приходит ответ. */
export type EngineCall =
  | { type: "exportFile"; generator: string }
  | { type: "importFile"; bytes: Uint8Array }
  | { type: "replayJournal"; text: string }
  | { type: "journalHeader" };

/** Тип результата вызова. */
export interface EngineCallResult {
  exportFile: Uint8Array;
  importFile: undefined;
  replayJournal: undefined;
  journalHeader: string;
}

/** UI → воркер. */
export type WorkerRequest =
  | {
      kind: "init";
      canvas: OffscreenCanvas;
      width: number;
      height: number;
      devicePixelRatio: number;
    }
  | { kind: "message"; message: ToEngine }
  | { kind: "call"; id: number; call: EngineCall };

/** Воркер → UI. */
export type WorkerResponse =
  | { kind: "ready"; backend: string }
  | { kind: "events"; events: FromEngine[] }
  | { kind: "result"; id: number; value: unknown }
  | { kind: "failure"; id: number; message: string }
  | { kind: "fatal"; message: string };
