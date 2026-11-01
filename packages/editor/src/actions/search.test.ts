import { describe, expect, test } from "bun:test";
import { type Action, actions, searchActions } from "./index";

const ids = (list: Action[]) => list.map((a) => a.id);

describe("поиск команд", () => {
  test("пустой запрос — все команды", () => {
    expect(searchActions("")).toHaveLength(actions.length);
  });

  test("ищет по названию, группе и ключевым словам", () => {
    expect(ids(searchActions("сохр"))).toEqual(["file.save", "file.saveAs"]);
    expect(ids(searchActions("тема тёмн"))).toEqual(["view.themeDark"]);
    expect(ids(searchActions("масштаб"))).toContain("view.zoomToFit");
  });

  test("у всех команд уникальные id и сочетания", () => {
    expect(new Set(ids([...actions])).size).toBe(actions.length);
    const keys = actions.flatMap((a) => (a.keys ?? []).map((k) => JSON.stringify(k)));
    expect(new Set(keys).size).toBe(keys.length);
  });
});
