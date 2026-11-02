import { describe, expect, test } from "bun:test";
import { formatNumber, t } from "./index";

describe("i18n", () => {
  test("подставляет параметры", () => {
    expect(t("status.scale", { denominator: 50 })).toBe("1:50");
  });

  test("оставляет неизвестные параметры как есть", () => {
    expect(t("status.scale")).toBe("1:{denominator}");
  });

  test("форматирует числа по-русски", () => {
    expect(formatNumber(2720).replace(/\s/g, " ")).toBe("2 720");
  });
});
