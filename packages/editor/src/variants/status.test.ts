import { expect, test } from "bun:test";
import type { Document } from "@openplanner/engine-client";
import { demolishedIds, statusOf, visibleIn } from "./status";

const doc = {
  variants: [{ id: "A", name: "Вариант А", demolished: ["w1"] }],
  openings: [{ id: "o1", wall: "w1" }],
} as unknown as Document;

test("статусы элементов в варианте", () => {
  const view = { type: "variant", id: "A" } as const;
  const demolished = demolishedIds(doc, "A");
  expect([...demolished].sort()).toEqual(["o1", "w1"]);
  expect(statusOf(view, demolished, "w1", undefined)).toBe("demolish");
  expect(statusOf(view, demolished, "w2", undefined)).toBe("existing");
  expect(statusOf(view, demolished, "w3", "A")).toBe("new");
  expect(statusOf({ type: "survey" }, demolished, "w1", undefined)).toBeNull();
});

test("видимость: обмер в каждом варианте, новые — только в своём", () => {
  expect(visibleIn({ type: "survey" }, undefined)).toBe(true);
  expect(visibleIn({ type: "survey" }, "A")).toBe(false);
  expect(visibleIn({ type: "variant", id: "A" }, "A")).toBe(true);
  expect(visibleIn({ type: "variant", id: "B" }, "A")).toBe(false);
});
