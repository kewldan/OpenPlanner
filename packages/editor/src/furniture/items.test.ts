import { describe, expect, test } from "bun:test";
import type { CatalogEntry } from "@openplanner/engine-client";
import { byCategory, searchCatalog } from "./items";

const entry = (id: string, name: string, category: string, keywords = ""): CatalogEntry => ({
  id,
  name,
  category,
  keywords,
  width: 1000,
  depth: 500,
  height: 800,
  elevation: 0,
  icon: "M0 0Z",
});

const items = [
  entry("bed", "Кровать двуспальная", "Спальня", "матрас"),
  entry("sofa", "Диван", "Гостиная"),
  entry("chair", "Стул", "Кухня и столовая"),
];

describe("каталог мебели", () => {
  test("поиск по названию, разделу и ключевым словам", () => {
    expect(searchCatalog(items, "").length).toBe(3);
    expect(searchCatalog(items, "КРОВ").map((i) => i.id)).toEqual(["bed"]);
    expect(searchCatalog(items, "матрас").map((i) => i.id)).toEqual(["bed"]);
    expect(searchCatalog(items, "кухня стул").map((i) => i.id)).toEqual(["chair"]);
    expect(searchCatalog(items, "шкаф")).toEqual([]);
  });

  test("разделы в порядке каталога", () => {
    expect(byCategory(items).map(([c]) => c)).toEqual(["Спальня", "Гостиная", "Кухня и столовая"]);
  });
});
