import { beforeEach, describe, expect, test } from "bun:test";
import type { Document } from "@openplanner/engine-client";
import { useEditor } from "./editor";

const document: Document = {
  meta: { title: "Комендантский пр.", address: "" },
  defaultCeilingHeight: 2700,
  layers: [],
  nodes: [],
  walls: [],
  openings: [],
  rooms: [],
  dimensions: [],
  leaders: [],
  underlays: [],
};

describe("useEditor.applyEvents", () => {
  beforeEach(() => {
    useEditor.setState({ document: null, canUndo: false, canRedo: false, lastError: null });
  });

  test("применяет пачку событий по порядку", () => {
    useEditor.getState().applyEvents([
      { type: "documentChanged", document },
      { type: "historyChanged", canUndo: true, canRedo: false, dirty: true },
      { type: "viewChanged", scaleDenominator: 25, zoomPercent: 200 },
    ]);
    const state = useEditor.getState();
    expect(state.document?.meta.title).toBe("Комендантский пр.");
    expect(state.canUndo).toBe(true);
    expect(state.zoomPercent).toBe(200);
    expect(state.dirty).toBe(true);
  });

  test("ошибка сохраняется до следующего изменения документа", () => {
    useEditor.getState().applyEvents([{ type: "error", message: "плохо" }]);
    expect(useEditor.getState().lastError).toBe("плохо");
    useEditor
      .getState()
      .applyEvents([{ type: "historyChanged", canUndo: false, canRedo: false, dirty: false }]);
    expect(useEditor.getState().lastError).toBe("плохо");
  });

  test("вычисленные величины индексируются по элементам", () => {
    useEditor.getState().applyEvents([
      {
        type: "derivedChanged",
        rooms: [{ id: "r", closed: true, area: 12.5, perimeter: 14 }],
        walls: [{ id: "w", length: 3000 }],
        dimensions: [],
        freeRegions: 1,
      },
      { type: "selectionChanged", ids: ["w"] },
      { type: "cursor", x: 1, y: 2, snap: "endpoint" },
    ]);
    const state = useEditor.getState();
    expect(state.derived.rooms.r?.area).toBe(12.5);
    expect(state.derived.walls.w).toBe(3000);
    expect(state.selection).toEqual(["w"]);
    expect(state.cursor).toEqual({ x: 1, y: 2, snap: "endpoint", length: null, angle: null });
  });

  test("режим «Мебель» не сбрасывается правками, другие режимы следуют за видом", () => {
    useEditor.setState({ mode: "furniture" });
    useEditor
      .getState()
      .applyEvents([{ type: "viewModeChanged", view: { type: "variant", id: "v" } }]);
    expect(useEditor.getState().mode).toBe("furniture");
    useEditor.setState({ mode: "measure" });
    useEditor
      .getState()
      .applyEvents([{ type: "viewModeChanged", view: { type: "variant", id: "v" } }]);
    expect(useEditor.getState().mode).toBe("replan");
  });
});
