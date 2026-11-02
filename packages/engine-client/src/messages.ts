// Обёртка протокола между UI-потоком и воркером.
// Типы ToEngine/FromEngine генерируются из Rust (crates/op-engine/src/protocol.rs) через tsify.

import type { FromEngine, ScanImport, ScanResult, ToEngine } from "../wasm/op_engine";

export type {
  Anchor,
  Command,
  Dimension,
  Document,
  Element,
  EngineEvents,
  FromEngine,
  HistoryOp,
  IssueKind,
  Justification,
  Layer,
  LayerKind,
  Leader,
  LengthInfo,
  MaterialAmount,
  Node,
  Opening,
  OpeningKind,
  PointerPhase,
  ProjectMeta,
  Room,
  RoomArea,
  RoomInfo,
  ScanAnswer,
  ScanFailure,
  ScanFormat,
  ScanImport,
  ScanImportMode,
  ScanIssue,
  ScanOpening,
  ScanParams,
  ScanRaster,
  ScanResult,
  ScanWall,
  Side,
  SnapKind,
  ToEngine,
  Tool,
  ToolOptions,
  ToolStep,
  Underlay,
  Variant,
  View,
  ViewSummary,
  Wall,
  WallMaterial,
} from "../wasm/op_engine";

/** Вызов движка, на который приходит ответ. */
export type EngineCall =
  | { type: "exportFile"; generator: string }
  | { type: "importFile"; bytes: Uint8Array }
  | { type: "addUnderlay"; bytes: Uint8Array; name: string }
  | { type: "importScan"; request: ScanImport; source?: Uint8Array }
  | { type: "replayJournal"; text: string }
  | { type: "journalHeader" }
  | { type: "documentTitle" };

/** Тип результата вызова. */
export interface EngineCallResult {
  exportFile: Uint8Array;
  importFile: undefined;
  addUnderlay: undefined;
  importScan: undefined;
  replayJournal: undefined;
  journalHeader: string;
  documentTitle: string;
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
  | { kind: "ready"; backend: string; webgpu: string }
  | { kind: "events"; events: FromEngine[] }
  | { kind: "result"; id: number; value: unknown }
  | { kind: "failure"; id: number; message: string }
  | { kind: "fatal"; message: string };

/** UI → воркер распознавания. */
export interface ScanWorkerRequest {
  bytes: Uint8Array;
}

/** Воркер распознавания → UI. */
export type ScanWorkerResponse =
  | { kind: "progress"; stage: number; count: number }
  | { kind: "result"; result: ScanResult }
  | { kind: "error"; message: string };
