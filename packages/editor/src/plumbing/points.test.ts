import { describe, expect, test } from "bun:test";
import type { PlumbingEntry } from "@openplanner/engine-client";
import { pipeSlope, plumbingGroups, withSlope } from "./points";

const entry = (id: string, group: string): PlumbingEntry => ({
  id,
  kind: "outlet",
  system: "cold",
  name: id,
  group,
  height: 500,
  diameter: 20,
  onWall: true,
  icon: "M0 0",
});

describe("сантехника", () => {
  test("разделы каталога в порядке появления", () => {
    const groups = plumbingGroups([
      entry("a", "Выводы"),
      entry("b", "Стояки"),
      entry("c", "Выводы"),
    ]);
    expect(groups.map(([g, items]) => [g, items.length])).toEqual([
      ["Выводы", 2],
      ["Стояки", 1],
    ]);
  });

  test("уклон пересчитывает отметки от первой вершины", () => {
    const points = withSlope(
      [
        { x: 0, y: 0, z: 500 },
        { x: 1000, y: 0, z: 0 },
        { x: 1000, y: 500, z: 0 },
      ],
      0.03,
    );
    expect(points.map((p) => p.z)).toEqual([500, 470, 455]);
    expect(pipeSlope(points)).toBeCloseTo(0.03);
  });

  test("вертикальные участки не считаются в уклоне", () => {
    expect(
      pipeSlope([
        { x: 0, y: 0, z: 500 },
        { x: 0, y: 0, z: 150 },
      ]),
    ).toBeNull();
  });
});
