import { describe, expect, test } from "bun:test";
import type { Material } from "@openplanner/engine-client";
import { formatQuantity, materialOptions, newFinish, newMaterial, replaceFinish } from "./finishes";

describe("отделка", () => {
  test("новая отделка — материал по умолчанию для поверхности", () => {
    const f = newFinish("r1", "walls");
    expect(f.material).toBe("paint-white");
    expect(f.layout.joint).toBe(2);
  });

  test("замена сохраняет порядок перекрытия", () => {
    const a = newFinish("r", "walls");
    const b = newFinish("r", "walls");
    const list = replaceFinish([a, b], { ...a, material: "x" });
    expect(list.map((f) => f.id)).toEqual([a.id, b.id]);
    expect(list[0]?.material).toBe("x");
  });

  test("материалы проекта — первыми и без повторов", () => {
    const lib: Material[] = [
      { ...newMaterial(1), id: "lib" },
      { ...newMaterial(2), id: "same" },
    ];
    const own: Material[] = [{ ...newMaterial(3), id: "same" }];
    expect(materialOptions(lib, own).map((m) => m.id)).toEqual(["same", "lib"]);
  });

  test("количество с упаковками", () => {
    expect(formatQuantity({ name: "Плитка", amount: 120, unit: "шт.", packs: 15 })).toBe(
      "Плитка 120 шт. (15 уп.)",
    );
    expect(formatQuantity({ name: "Покрытие", amount: 5.354, unit: "м²", packs: 0 })).toBe(
      "Покрытие 5,4 м²",
    );
  });
});
