import { beforeEach, describe, expect, test } from "bun:test";
import { useEditor } from "./editor";

describe("useEditor.applyEvents", () => {
  beforeEach(() => {
    useEditor.setState({ document: null, canUndo: false, canRedo: false, lastError: null });
  });

  test("применяет пачку событий по порядку", () => {
    useEditor.getState().applyEvents([
      {
        type: "documentChanged",
        document: { meta: { title: "Комендантский пр.", address: "" }, defaultCeilingHeight: 2700 },
      },
      { type: "historyChanged", canUndo: true, canRedo: false },
      { type: "viewChanged", scaleDenominator: 25, zoomPercent: 200 },
    ]);
    const state = useEditor.getState();
    expect(state.document?.meta.title).toBe("Комендантский пр.");
    expect(state.canUndo).toBe(true);
    expect(state.zoomPercent).toBe(200);
  });

  test("ошибка сохраняется до следующего изменения документа", () => {
    useEditor.getState().applyEvents([{ type: "error", message: "плохо" }]);
    expect(useEditor.getState().lastError).toBe("плохо");
    useEditor.getState().applyEvents([{ type: "historyChanged", canUndo: false, canRedo: false }]);
    expect(useEditor.getState().lastError).toBe("плохо");
  });
});
