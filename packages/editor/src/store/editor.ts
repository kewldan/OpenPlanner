import type { Document, FromEngine } from "@openplanner/engine-client";
import { create } from "zustand";

export type EditorMode =
  | "measure"
  | "replan"
  | "furniture"
  | "electric"
  | "light"
  | "plumbing"
  | "finish";

export type EditorDialog = "palette" | "shortcuts" | null;

export type EngineStatus =
  | { state: "starting" }
  | { state: "ready"; backend: string; webgpu: string }
  | { state: "failed"; message: string };

export interface EditorState {
  engine: EngineStatus;
  document: Document | null;
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
  setMode: (mode: EditorMode) => void;
  setDialog: (dialog: EditorDialog) => void;
  setViewport: (width: number, height: number) => void;
  setEngine: (status: EngineStatus) => void;
  applyEvents: (events: FromEngine[]) => void;
}

/** Проекции модели для панелей. Источник истины — движок в воркере. */
export const useEditor = create<EditorState>()((set) => ({
  engine: { state: "starting" },
  document: null,
  canUndo: false,
  canRedo: false,
  dirty: false,
  scaleDenominator: 50,
  zoomPercent: 100,
  lastError: null,
  mode: "measure",
  dialog: null,
  viewport: { width: 0, height: 0 },
  setMode: (mode) => set({ mode }),
  setDialog: (dialog) => set({ dialog }),
  setViewport: (width, height) => set({ viewport: { width, height } }),
  setEngine: (engine) => set({ engine }),
  applyEvents: (events) =>
    set((state) => events.reduce<Partial<EditorState>>(reduce, { lastError: state.lastError })),
}));

function reduce(acc: Partial<EditorState>, event: FromEngine): Partial<EditorState> {
  switch (event.type) {
    case "documentChanged":
      return { ...acc, document: event.document, lastError: null };
    case "historyChanged":
      return { ...acc, canUndo: event.canUndo, canRedo: event.canRedo, dirty: event.dirty };
    case "viewChanged":
      return { ...acc, scaleDenominator: event.scaleDenominator, zoomPercent: event.zoomPercent };
    case "error":
      return { ...acc, lastError: event.message };
    case "journal":
      return acc;
  }
}
