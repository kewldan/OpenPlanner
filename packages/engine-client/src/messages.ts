// Обёртка протокола между UI-потоком и воркером.
// Типы ToEngine/FromEngine генерируются из Rust (crates/op-engine/src/protocol.rs) через tsify.

import type {
  DrawingFormat,
  DrawingRequest,
  DrawingSetInfo,
  DxfImport,
  DxfInfo,
  FromEngine,
  ScanImport,
  ScanResult,
  TableFormat,
  ToEngine,
} from "../wasm/op_engine";

export type {
  Anchor,
  BillRow,
  CameraAction,
  CameraInfo,
  CameraMode,
  CatalogEntry,
  Circuit,
  CircuitKind,
  CircuitReport,
  Command,
  Device,
  DeviceEntry,
  DeviceGroup,
  DeviceKind,
  Dimension,
  Document,
  DrawingFormat,
  DrawingRequest,
  DrawingSetInfo,
  DrawingSettings,
  DxfImport,
  DxfInfo,
  DxfLayer,
  DxfMode,
  ElectricalReport,
  Element,
  EngineEvents,
  EstimateGroup,
  EstimateLine,
  EstimateReport,
  ExportNames,
  FaceInfo,
  Finish,
  FinishInfo,
  FinishReport,
  FromEngine,
  Furniture,
  HistoryOp,
  IssueKind,
  Justification,
  Layer,
  LayerKind,
  Leader,
  LengthInfo,
  Level,
  LightScene,
  Material,
  MaterialAmount,
  MaterialKind,
  MaterialTotal,
  Measure,
  Node,
  Opening,
  OpeningKind,
  Options3d,
  PanelReport,
  PaperFormat,
  Pipe,
  PipeReport,
  PipeSystem,
  PipeVertex,
  PlumbingEntry,
  PlumbingKind,
  PlumbingPoint,
  PlumbingReport,
  PointerPhase,
  PriceItem,
  PriceKind,
  PricePoint,
  ProjectMeta,
  Purchase,
  Quantity,
  QuantityOverride,
  Room,
  RoomArea,
  RoomInfo,
  RoomLighting,
  RoomSurfaces,
  SavedView,
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
  SheetPreview,
  Side,
  Site,
  SnapKind,
  SunInfo,
  SunTime,
  Surface,
  TableFormat,
  TileLayout,
  TilePattern,
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
  | { type: "documentTitle" }
  | { type: "drawingSet"; request: DrawingRequest }
  | { type: "exportDrawings"; request: DrawingRequest; format: DrawingFormat }
  | { type: "zipFiles"; names: string[]; files: Uint8Array[] }
  | { type: "importFurniture"; bytes: Uint8Array; name: string }
  | { type: "exportEstimate"; format: TableFormat }
  | { type: "exportPrices"; format: TableFormat }
  | { type: "importPrices"; bytes: Uint8Array; date: string }
  | { type: "dxfInfo"; bytes: Uint8Array }
  | { type: "importDxf"; bytes: Uint8Array; name: string; options: DxfImport };

/** Тип результата вызова. */
export interface EngineCallResult {
  exportFile: Uint8Array;
  importFile: undefined;
  addUnderlay: undefined;
  importScan: undefined;
  replayJournal: undefined;
  journalHeader: string;
  documentTitle: string;
  drawingSet: DrawingSetInfo;
  exportDrawings: Uint8Array;
  zipFiles: Uint8Array;
  importFurniture: undefined;
  exportEstimate: Uint8Array;
  exportPrices: Uint8Array;
  importPrices: undefined;
  dxfInfo: DxfInfo;
  importDxf: undefined;
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
