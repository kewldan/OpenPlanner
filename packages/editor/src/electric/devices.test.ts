import { describe, expect, test } from "bun:test";
import type { Circuit, DeviceEntry, Document } from "@openplanner/engine-client";
import {
  deviceEntries,
  deviceGroups,
  lightMarks,
  newCircuit,
  nextCircuitName,
  toggleKeyLight,
} from "./devices";

const entry = (kind: DeviceEntry["kind"], group: string, lighting: boolean): DeviceEntry => ({
  kind,
  name: kind,
  group,
  lighting,
  onWall: true,
  height: 300,
  power: 0,
  flux: 0,
  icon: "M0 0Z",
});

describe("электрика в интерфейсе", () => {
  test("каталог делится на электрику и освещение", () => {
    const items = [
      entry("socket", "Розетки", false),
      entry("switch1", "Выключатели", true),
      entry("spot", "Светильники", true),
    ];
    expect(deviceEntries(items, true).map((i) => i.kind)).toEqual(["switch1", "spot"]);
    expect(deviceGroups(deviceEntries(items, true)).map(([g]) => g)).toEqual([
      "Выключатели",
      "Светильники",
    ]);
  });

  test("клавиша включает и выключает светильник", () => {
    expect(toggleKeyLight([[], ["a"]], 0, "b")).toEqual([["b"], ["a"]]);
    expect(toggleKeyLight([["b"], ["a"]], 1, "a")).toEqual([["b"], []]);
  });

  test("марки светильников и названия групп", () => {
    const doc = {
      devices: [
        { id: "s", kind: "socket" },
        { id: "l1", kind: "spot" },
        { id: "l2", kind: "ceilingLight" },
      ],
    } as unknown as Document;
    expect(lightMarks(doc).map((l) => [l.device.id, l.mark])).toEqual([
      ["l1", "С1"],
      ["l2", "С2"],
    ]);
    const circuits = [{ name: "Гр.1" }, { name: "Гр.2" }] as Circuit[];
    expect(nextCircuitName(circuits)).toBe("Гр.3");
    const lighting = newCircuit([], "lighting");
    expect([lighting.breaker, lighting.section, lighting.rcd]).toEqual([10, 1.5, false]);
  });
});
