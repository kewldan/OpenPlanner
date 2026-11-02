import { describe, expect, test } from "bun:test";
import type { EngineCall, FromEngine, ToEngine } from "@openplanner/engine-client";
import type { OpenedFile, Platform, SavedFile } from "../platform/types";
import { conflictCopy, type EngineApi, ProjectController } from "./controller";

const HEADER = '{"formatVersion":1}\n';

function setup(
  options: { confirm?: boolean; opened?: OpenedFile; target?: SavedFile | null } = {},
) {
  const journals = new Map<string, string>();
  const written: { bytes: Uint8Array; target: SavedFile }[] = [];
  const sent: ToEngine[] = [];
  const calls: EngineCall["type"][] = [];
  const confirms: string[] = [];
  const stamps = new Map<string, string>();
  let dirty = false;
  let controller: ProjectController | null = null;
  /** События, которые «движок» выдаст во время следующей выгрузки файла. */
  let duringExport: FromEngine[] = [];

  const platform: Platform = {
    openImage: async () => null,
    openScan: async () => null,
    openModel: async () => null,
    openTable: async () => null,
    openDrawing: async () => null,
    fileStamp: async (path) => stamps.get(path) ?? "0:0",
    readProject: async () => new Uint8Array([9]),
    writeExport: async () => {},
    kind: "browser",
    openProject: async () => options.opened ?? null,
    chooseSaveTarget: async (name) =>
      options.target === undefined
        ? { path: `/p/${name}`, name: name.replace(".oplan", "") }
        : options.target,
    writeProject: async (bytes, target) => {
      written.push({ bytes, target });
    },
    saveExport: async () => true,
    journal: {
      locate: async (path, name) => (path ? `${path}.journal` : `recovery/${name}.oplan.journal`),
      read: async (key) => journals.get(key) ?? null,
      reset: async (key, header) => {
        journals.set(key, header);
      },
      append: async (key, text) => {
        journals.set(key, (journals.get(key) ?? "") + text);
      },
      remove: async (key) => {
        journals.delete(key);
      },
    },
    confirm: async (message) => {
      confirms.push(message);
      return options.confirm ?? true;
    },
    alert: async (message) => {
      throw new Error(`неожиданная ошибка: ${message}`);
    },
    setTitle: () => {},
  };

  const engine: EngineApi = {
    send: (message) => {
      sent.push(message);
      if (message.type === "markSaved" || message.type === "newDocument") dirty = false;
    },
    call: (async (call: EngineCall) => {
      calls.push(call.type);
      switch (call.type) {
        case "journalHeader":
          return HEADER;
        case "documentTitle":
          return "Комендантский пр.";
        case "exportFile":
          controller?.onEngineEvents(duringExport);
          duringExport = [];
          return new Uint8Array([1, 2, 3]);
        default:
          return undefined;
      }
    }) as EngineApi["call"],
    isDirty: () => dirty,
  };

  controller = new ProjectController(platform, engine);
  return {
    controller,
    journals,
    written,
    sent,
    calls,
    confirms,
    stamps,
    setDirty: (value: boolean) => {
      dirty = value;
    },
    exportEmits: (events: FromEngine[]) => {
      duringExport = events;
    },
  };
}

const line = (n: number): FromEngine => ({ type: "journal", line: `{"op":"undo","n":${n}}\n` });
const UNTITLED = "recovery/untitled.oplan.journal";

describe("ProjectController", () => {
  test("запуск без журнала начинает пустой журнал", async () => {
    const s = setup();
    await s.controller.start();
    expect(s.journals.get(UNTITLED)).toBe(HEADER);
    expect(s.confirms).toHaveLength(0);
  });

  test("запуск с журналом предлагает восстановление и воспроизводит его", async () => {
    const s = setup({ confirm: true });
    s.journals.set(UNTITLED, `${HEADER}{"op":"undo"}\n`);
    await s.controller.start();
    expect(s.confirms).toHaveLength(1);
    expect(s.calls).toContain("replayJournal");
    expect(s.journals.get(UNTITLED)).toBe(`${HEADER}{"op":"undo"}\n`);
  });

  test("отказ от восстановления очищает журнал", async () => {
    const s = setup({ confirm: false });
    s.journals.set(UNTITLED, `${HEADER}{"op":"undo"}\n`);
    await s.controller.start();
    expect(s.calls).not.toContain("replayJournal");
    expect(s.journals.get(UNTITLED)).toBe(HEADER);
  });

  test("строки журнала дописываются в журнал проекта", async () => {
    const s = setup();
    await s.controller.start();
    s.controller.onEngineEvents([
      line(1),
      { type: "historyChanged", canUndo: true, canRedo: false, dirty: true },
    ]);
    s.controller.onEngineEvents([line(2)]);
    await s.controller.flushJournal();
    expect(s.journals.get(UNTITLED)).toBe(`${HEADER}{"op":"undo","n":1}\n{"op":"undo","n":2}\n`);
  });

  test("первое сохранение выбирает файл и переносит журнал к нему", async () => {
    const s = setup();
    await s.controller.start();
    s.setDirty(true);
    expect(await s.controller.save()).toBe(true);

    expect(s.written[0]?.target.path).toBe("/p/Комендантский пр.oplan");
    expect(s.sent).toContainEqual({ type: "markSaved" });
    expect(s.journals.has(UNTITLED)).toBe(false);
    expect(s.journals.get("/p/Комендантский пр.oplan.journal")).toBe(HEADER);
    const state = s.controller.store.getState();
    expect(state.path).toBe("/p/Комендантский пр.oplan");
    expect(state.name).toBe("Комендантский пр");
    expect(state.savedAt).not.toBeNull();
  });

  test("строки до выгрузки входят в файл, а не в новый журнал", async () => {
    const s = setup();
    await s.controller.start();
    s.setDirty(true);
    s.controller.onEngineEvents([line(1)]);
    s.exportEmits([line(2)]);
    await s.controller.save();
    s.controller.onEngineEvents([line(3)]);
    await s.controller.flushJournal();
    expect(s.journals.get("/p/Комендантский пр.oplan.journal")).toBe(
      `${HEADER}{"op":"undo","n":3}\n`,
    );
  });

  test("отмена выбора файла ничего не сохраняет", async () => {
    const s = setup({ target: null });
    await s.controller.start();
    expect(await s.controller.save()).toBe(false);
    expect(s.written).toHaveLength(0);
    expect(s.sent).not.toContainEqual({ type: "markSaved" });
  });

  test("новый проект при несохранённых изменениях спрашивает подтверждение", async () => {
    const s = setup({ confirm: false });
    await s.controller.start();
    s.setDirty(true);
    await s.controller.newProject();
    expect(s.confirms).toHaveLength(1);
    expect(s.sent).not.toContainEqual({ type: "newDocument" });
  });

  test("изменения проекта с файлом сохраняются перед открытием другого", async () => {
    const s = setup({
      opened: { path: "/p/Другой.oplan", name: "Другой", bytes: new Uint8Array([9]) },
    });
    await s.controller.start();
    s.setDirty(true);
    await s.controller.save();
    s.setDirty(true);
    await s.controller.openProject();
    expect(s.written).toHaveLength(2);
    expect(s.calls).toContain("importFile");
    expect(s.controller.store.getState().path).toBe("/p/Другой.oplan");
    expect(s.journals.has("/p/Комендантский пр.oplan.journal")).toBe(false);
    expect(s.journals.get("/p/Другой.oplan.journal")).toBe(HEADER);
  });

  test("команда во время запуска выполняется после него, а не теряется", async () => {
    const s = setup({
      opened: { path: "/p/Другой.oplan", name: "Другой", bytes: new Uint8Array([9]) },
    });
    const starting = s.controller.start();
    const opening = s.controller.openProject();
    await Promise.all([starting, opening]);
    expect(s.calls).toContain("importFile");
    expect(s.controller.store.getState().path).toBe("/p/Другой.oplan");
  });

  test("автосохранение не трогает проект без файла", async () => {
    const s = setup();
    await s.controller.start();
    s.setDirty(true);
    await s.controller.autosave();
    expect(s.written).toHaveLength(0);
  });

  test("файл, изменённый на другом устройстве, открывается заново с копией своих правок", async () => {
    const s = setup({ confirm: true });
    await s.controller.start();
    s.setDirty(true);
    await s.controller.save();
    const path = s.written[0]?.target.path ?? "";
    // Файл не меняли — вопроса нет; потом его изменили на другом устройстве.
    await s.controller.checkExternal();
    expect(s.confirms).toHaveLength(0);
    s.stamps.set(path, "2:20");
    s.setDirty(true);
    await s.controller.checkExternal();
    expect(s.confirms).toHaveLength(1);
    // Свои правки — копией рядом, затем открыта версия с диска.
    expect(s.written.at(-1)?.target.name).toMatch(/конфликт/);
    expect(s.calls.filter((c) => c === "importFile")).toHaveLength(1);
  });

  test("гостевой проект открывается только для просмотра и не сохраняется", async () => {
    const s = setup();
    await s.controller.openGuest(new Uint8Array([1]));
    expect(s.sent).toContainEqual({ type: "setReadOnly", readOnly: true });
    expect(s.controller.store.getState().guest).toBe(true);
    s.setDirty(true);
    expect(await s.controller.save()).toBe(false);
    expect(s.written).toHaveLength(0);
  });

  test("копия при конфликте — рядом с файлом", () => {
    const copy = conflictCopy("/d/Кв.oplan", "Кв", new Date(2026, 10, 2, 14, 5));
    expect(copy).toEqual({
      path: "/d/Кв (конфликт 2026-11-02 14-05).oplan",
      name: "Кв (конфликт 2026-11-02 14-05)",
    });
  });
});
