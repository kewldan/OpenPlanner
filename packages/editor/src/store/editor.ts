import type {
  CameraInfo,
  Document,
  FromEngine,
  LengthInfo,
  Options3d,
  RoomInfo,
  SnapKind,
  SunInfo,
  Tool,
  ToolOptions,
  ToolStep,
  View,
  ViewSummary,
} from "@openplanner/engine-client";
import { create } from "zustand";

export type EditorMode =
  | "measure"
  | "replan"
  | "furniture"
  | "electric"
  | "light"
  | "plumbing"
  | "finish";

export type EditorDialog = "palette" | "shortcuts" | "scan" | "compare" | "drawings" | null;

export type EngineStatus =
  | { state: "starting" }
  | { state: "ready"; backend: string; webgpu: string }
  | { state: "failed"; message: string };

/** Курсор на плане после привязки. */
export interface CursorInfo {
  x: number;
  y: number;
  snap: SnapKind | null;
  length: number | null;
  angle: number | null;
}

/** Вычисленные величины модели. */
export interface DerivedInfo {
  rooms: Record<string, RoomInfo>;
  walls: Record<string, number>;
  dimensions: Record<string, number>;
  freeRegions: number;
}

export interface EditorState {
  engine: EngineStatus;
  document: Document | null;
  derived: DerivedInfo;
  canUndo: boolean;
  canRedo: boolean;
  /** Есть несохранённые изменения. */
  dirty: boolean;
  scaleDenominator: number;
  zoomPercent: number;
  lastError: string | null;
  mode: EditorMode;
  /** Открытое модальное окно. */
  dialog: EditorDialog;
  /** Размер холста, CSS-пиксели. */
  viewport: { width: number; height: number };
  selection: string[];
  tool: Tool;
  step: ToolStep;
  toolOptions: ToolOptions;
  cursor: CursorInfo | null;
  /** Элемент, которому нужен ввод текста (новая выноска). */
  editText: string | null;
  /** Что показывает холст: обмер, вариант, сравнение. */
  view: View;
  /** Вид до сравнения — куда вернуться. */
  previousView: View;
  /** Последний открытый вариант. */
  lastVariant: string | null;
  /** Сводка обмера и вариантов. */
  summaries: ViewSummary[];
  /** 3D-вид: настройки, камера и солнце (если включено). */
  three: { options: Options3d; camera: CameraInfo | null; sun: SunInfo | null };
  setMode: (mode: EditorMode) => void;
  setDialog: (dialog: EditorDialog) => void;
  setViewport: (width: number, height: number) => void;
  setEngine: (status: EngineStatus) => void;
  clearEditText: () => void;
  applyEvents: (events: FromEngine[]) => void;
}

const emptyDerived: DerivedInfo = { rooms: {}, walls: {}, dimensions: {}, freeRegions: 0 };

function byId(list: LengthInfo[]): Record<string, number> {
  return Object.fromEntries(list.map((item) => [item.id, item.length]));
}

/** Проекции модели для панелей. Источник истины — движок в воркере. */
export const useEditor = create<EditorState>()((set) => ({
  engine: { state: "starting" },
  document: null,
  derived: emptyDerived,
  canUndo: false,
  canRedo: false,
  dirty: false,
  scaleDenominator: 50,
  zoomPercent: 100,
  lastError: null,
  mode: "measure",
  dialog: null,
  viewport: { width: 0, height: 0 },
  selection: [],
  tool: "select",
  step: "idle",
  toolOptions: { wallThickness: 120, justification: "center" },
  cursor: null,
  editText: null,
  view: { type: "survey" },
  previousView: { type: "survey" },
  lastVariant: null,
  summaries: [],
  three: {
    options: { enabled: false, mode: "orbit", ceiling: true, white: false },
    camera: null,
    sun: null,
  },
  setMode: (mode) => set({ mode }),
  setDialog: (dialog) => set({ dialog }),
  setViewport: (width, height) => set({ viewport: { width, height } }),
  setEngine: (engine) => set({ engine }),
  clearEditText: () => set({ editText: null }),
  applyEvents: (events) =>
    set((state) => events.reduce<Partial<EditorState>>(reduce, { lastError: state.lastError })),
}));

function reduce(acc: Partial<EditorState>, event: FromEngine): Partial<EditorState> {
  switch (event.type) {
    case "documentChanged":
      return { ...acc, document: event.document, lastError: null };
    case "derivedChanged":
      return {
        ...acc,
        derived: {
          rooms: Object.fromEntries(event.rooms.map((room) => [room.id, room])),
          walls: byId(event.walls),
          dimensions: byId(event.dimensions),
          freeRegions: event.freeRegions,
        },
      };
    case "historyChanged":
      return { ...acc, canUndo: event.canUndo, canRedo: event.canRedo, dirty: event.dirty };
    case "viewChanged":
      return { ...acc, scaleDenominator: event.scaleDenominator, zoomPercent: event.zoomPercent };
    case "selectionChanged":
      return { ...acc, selection: event.ids };
    case "toolChanged":
      return { ...acc, tool: event.tool, step: event.step, toolOptions: event.options };
    case "cursor":
      return {
        ...acc,
        cursor: {
          x: event.x,
          y: event.y,
          snap: event.snap ?? null,
          length: event.length ?? null,
          angle: event.angle ?? null,
        },
      };
    case "editText":
      return { ...acc, editText: event.id };
    case "viewModeChanged":
      return {
        ...acc,
        view: event.view,
        mode: event.view.type === "survey" ? "measure" : "replan",
        ...(event.view.type === "variant" ? { lastVariant: event.view.id } : {}),
      };
    case "variantsSummary":
      return { ...acc, summaries: event.views };
    case "view3dChanged":
      return {
        ...acc,
        three: { options: event.options, camera: event.camera, sun: event.sun ?? null },
      };
    case "error":
      return { ...acc, lastError: event.message };
    case "journal":
      return acc;
  }
}
