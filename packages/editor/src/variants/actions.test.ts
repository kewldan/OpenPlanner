import { expect, test } from "bun:test";
import { nextVariantName } from "./actions";

test("названия вариантов идут по алфавиту и не повторяются", () => {
  expect(nextVariantName([])).toBe("Вариант А");
  expect(nextVariantName(["Вариант А"])).toBe("Вариант Б");
  expect(nextVariantName(["Вариант Б"])).toBe("Вариант А");
});
