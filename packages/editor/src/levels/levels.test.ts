import { describe, expect, test } from "bun:test";
import type { Document } from "@openplanner/engine-client";
import { FIRST_LEVEL, levelList } from "./levels";

describe("этажи", () => {
  test("первый этаж неявный, остальные — по отметке", () => {
    const doc = {
      levels: [
        { id: "b", name: "Мансарда", elevation: 6000 },
        { id: "a", name: "Этаж 2", elevation: 3000 },
      ],
    } as unknown as Document;
    expect(levelList(doc).map((l) => l.name)).toEqual([FIRST_LEVEL, "Этаж 2", "Мансарда"]);
    expect(levelList(doc)[0]?.id).toBeNull();
  });
});
