import type {
  Circuit,
  CircuitKind,
  Device,
  DeviceEntry,
  DeviceKind,
  Document,
} from "@openplanner/engine-client";
import { sendToEngine } from "../engine";

/** Позиции каталога для режима: «Освещение» — светильники и выключатели, «Электрика» — остальное. */
export function deviceEntries(items: readonly DeviceEntry[], lighting: boolean): DeviceEntry[] {
  return items.filter((i) => i.lighting === lighting);
}

/** Позиции по разделам в порядке каталога. */
export function deviceGroups(items: readonly DeviceEntry[]): [string, DeviceEntry[]][] {
  const groups = new Map<string, DeviceEntry[]>();
  for (const item of items) groups.set(item.group, [...(groups.get(item.group) ?? []), item]);
  return [...groups.entries()];
}

const LIGHTS: readonly DeviceKind[] = ["ceilingLight", "pendant", "spot", "wallLight", "ledStrip"];

export function isLight(kind: DeviceKind): boolean {
  return LIGHTS.includes(kind);
}

/** Светильники документа с марками «С1…» в порядке документа, как на листах. */
export function lightMarks(doc: Document): { device: Device; mark: string }[] {
  return (doc.devices ?? [])
    .filter((d) => isLight(d.kind))
    .map((device, i) => ({ device, mark: `С${i + 1}` }));
}

/** Клавиши выключателя с переключённым светильником. */
export function toggleKeyLight(keys: string[][], key: number, light: string): string[][] {
  return keys.map((lights, i) => {
    if (i !== key) return lights;
    return lights.includes(light) ? lights.filter((l) => l !== light) : [...lights, light];
  });
}

/** Свободное обозначение группы: «Гр.N». */
export function nextCircuitName(circuits: readonly Circuit[]): string {
  let n = circuits.length + 1;
  while (circuits.some((c) => c.name === `Гр.${n}`)) n++;
  return `Гр.${n}`;
}

/** Новая группа розеток по умолчанию. */
export function newCircuit(circuits: readonly Circuit[], kind: CircuitKind = "sockets"): Circuit {
  const lighting = kind === "lighting";
  return {
    id: `circuit-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`,
    name: nextCircuitName(circuits),
    purpose: "",
    kind,
    phases: 1,
    breaker: lighting ? 10 : 16,
    rcd: !lighting,
    section: lighting ? 1.5 : 2.5,
    demand: lighting ? 1 : 0.8,
  };
}

export function setCircuits(circuits: Circuit[]) {
  sendToEngine({ type: "command", command: { type: "setCircuits", circuits } });
}
