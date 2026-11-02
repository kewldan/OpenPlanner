import { describe, expect, test } from "bun:test";
import { drawingRequest, pngNames, rasterSize, sheetDate } from "./export";

describe("экспорт чертежей", () => {
  test("дата основной надписи — месяц и год", () => {
    expect(sheetDate(new Date(2026, 10, 2))).toBe("11.26");
    expect(sheetDate(new Date(2030, 0, 31))).toBe("01.30");
  });

  test("запрос без варианта — только обмер", () => {
    const now = new Date(2026, 10, 2);
    expect(drawingRequest(null, now)).toEqual({ date: "11.26" });
    expect(drawingRequest("v1", now)).toEqual({ variant: "v1", date: "11.26" });
  });

  test("PNG: один лист — файл, несколько — архив", () => {
    const sheet = { title: "Обмерный план" };
    expect(pngNames({ name: "Кв — Обмер", sheets: [sheet] as never }).file).toBe("Кв — Обмер.png");
    const many = pngNames({
      name: "Кв — Вариант А",
      sheets: [sheet, { title: "План демонтажа" }] as never,
    });
    expect(many.file).toBe("Кв — Вариант А — PNG.zip");
    expect(many.entries).toEqual(["01 Обмерный план.png", "02 План демонтажа.png"]);
  });

  test("A3 в 150 dpi", () => {
    expect(rasterSize({ width: 420, height: 297 })).toEqual({ width: 2480, height: 1754 });
  });
});
