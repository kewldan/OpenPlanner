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

export type EngineStatus =
  | { state: "starting" }
  | { state: "ready"; backend: string }
  | { state: "failed"; message: string };

export interface EditorState {
  engine: EngineStatus;
  document: Document | null;
  canUndo: boolean;
  canRedo: boolean;
  scaleDenominator: number;
  zoomPercent: number;
  lastError: string | null;
  mode: EditorMode;
  setMode: (mode: EditorMode) => void;
  setEngine: (status: EngineStatus) => void;
  applyEvents: (events: FromEngine[]) => void;
}

/** Проекции модели для панелей. Источник истины — движок в воркере. */
export const useEditor = create<EditorState>()((set) => ({
  engine: { state: "starting" },
  document: null,
  canUndo: false,
  canRedo: false,
  scaleDenominator: 50,
  zoomPercent: 100,
  lastError: null,
  mode: "measure",
  setMode: (mode) => set({ mode }),
  setEngine: (engine) => set({ engine }),
  applyEvents: (events) =>
    set((state) => events.reduce<Partial<EditorState>>(reduce, { lastError: state.lastError })),
}));

function reduce(acc: Partial<EditorState>, event: FromEngine): Partial<EditorState> {
  switch (event.type) {
    case "documentChanged":
      return { ...acc, document: event.document, lastError: null };
    case "historyChanged":
      return { ...acc, canUndo: event.canUndo, canRedo: event.canRedo };
    case "viewChanged":
      return { ...acc, scaleDenominator: event.scaleDenominator, zoomPercent: event.zoomPercent };
    case "error":
      return { ...acc, lastError: event.message };
  }
}
