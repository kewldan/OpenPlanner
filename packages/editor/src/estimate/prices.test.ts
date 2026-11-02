import { describe, expect, test } from "bun:test";
import { measureKey, measurePresets, newPriceItem, today, withOverride, withPrice } from "./prices";

describe("прайс", () => {
  test("ключи правил совпадают с форматом прайса и не повторяются", () => {
    expect(measureKey({ type: "finish", surface: "floor", kind: "tile" })).toBe(
      "finish:floor:tile",
    );
    expect(measureKey({ type: "finish", kind: "tile" })).toBe("finish:*:tile");
    expect(measureKey({ type: "devices", group: "outlet" })).toBe("devices:outlet");
    expect(measureKey({ type: "consumption", name: "Клей" })).toBe("consumption:Клей");
    const keys = measurePresets.map((p) => measureKey(p.measure));
    expect(new Set(keys).size).toBe(keys.length);
  });

  test("новая цена уходит в историю, та же — нет", () => {
    const item = { ...newPriceItem(1), price: 100 };
    const next = withPrice(item, 120, "02.11.2026");
    expect(next.price).toBe(120);
    expect(next.history).toEqual([{ date: "02.11.2026", price: 100 }]);
    expect(withPrice(next, 120, "03.11.2026")).toBe(next);
  });

  test("ручной объём заменяется и снимается", () => {
    const a = withOverride([], "p", "r", 5);
    const b = withOverride(a, "p", "r", 7);
    expect(b).toEqual([{ item: "p", room: "r", quantity: 7 }]);
    expect(withOverride(b, "p", "r", null)).toEqual([]);
    expect(withOverride([], "p", undefined, 1)).toEqual([{ item: "p", quantity: 1 }]);
  });

  test("дата истории", () => {
    expect(today(new Date(2026, 10, 2))).toBe("02.11.2026");
  });
});
