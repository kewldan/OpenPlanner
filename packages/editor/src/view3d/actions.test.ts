import { describe, expect, test } from "bun:test";
import { dateOfDay, dayOfDate, dayOfYear, formatHour, nextViewName } from "./actions";

describe("3D-вид", () => {
  test("день года совпадает с движком (год невисокосный)", () => {
    expect(dayOfYear(1, 1)).toBe(1);
    expect(dayOfYear(3, 21)).toBe(80);
    expect(dayOfYear(12, 31)).toBe(365);
    expect(dayOfYear(2, 30)).toBe(59);
  });

  test("дата и день года переводятся туда и обратно", () => {
    expect(dateOfDay(172, 2026)).toBe("2026-06-21");
    expect(dayOfDate("2026-06-21")).toBe(172);
    expect(dateOfDay(1, 2026)).toBe("2026-01-01");
    expect(dayOfDate("")).toBeNull();
  });

  test("время и названия ракурсов", () => {
    expect(formatHour(13.5)).toBe("13:30");
    expect(formatHour(7.25)).toBe("07:15");
    expect(nextViewName(["Ракурс 1"], "Ракурс")).toBe("Ракурс 2");
  });
});
