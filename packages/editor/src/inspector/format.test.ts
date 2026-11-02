import { expect, test } from "bun:test";
import { formatArea, parseNumber } from "./format";

test("площадь с запятой", () => {
  expect(formatArea(11.1744)).toBe("11,17 м²");
});

test("разбор чисел из полей", () => {
  expect(parseNumber("3 500")).toBe(3500);
  expect(parseNumber("2,5")).toBe(2.5);
  expect(parseNumber("")).toBeNull();
  expect(parseNumber("abc")).toBeNull();
});
