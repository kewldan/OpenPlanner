import type { Finish, Material, MaterialKind, Quantity, Surface } from "@openplanner/engine-client";
import { sendToEngine } from "../engine";
import { formatDecimal } from "../inspector/format";

/** Материал по умолчанию для новой отделки поверхности. */
export const defaultMaterial: Record<Surface, string> = {
  floor: "porcelain-600x600",
  ceiling: "stretch-matte",
  walls: "paint-white",
};

/** Штучный материал: у него есть раскладка. */
export function isPiecewise(kind: MaterialKind): boolean {
  return kind === "tile" || kind === "laminate";
}

function newId(prefix: string): string {
  return `${prefix}-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
}

/** Новая отделка поверхности помещения. */
export function newFinish(room: string, surface: Surface): Finish {
  return {
    id: newId("finish"),
    room,
    surface,
    material: defaultMaterial[surface],
    bottom: 0,
    layout: { pattern: "straight", joint: 2, shift: 0.5, origin: [0, 0] },
  };
}

/** Новый материал проекта: плитка 300 × 300. */
export function newMaterial(n: number): Material {
  return {
    id: newId("material"),
    name: `Материал ${n}`,
    kind: "tile",
    width: 300,
    length: 300,
    thickness: 8,
    pack: 10,
    color: "#d8d2c4",
  };
}

/** Материалы для выбора: сначала проекта, затем библиотеки. */
export function materialOptions(
  library: readonly Material[],
  own: readonly Material[],
): Material[] {
  return [...own, ...library.filter((m) => !own.some((o) => o.id === m.id))];
}

/** Замена отделки в списке (порядок сохраняется: он задаёт перекрытие на стенах). */
export function replaceFinish(list: readonly Finish[], next: Finish): Finish[] {
  return list.map((f) => (f.id === next.id ? next : f));
}

/** «Плитка 120 шт. (15 уп.)». */
export function formatQuantity(q: Quantity): string {
  const whole = q.unit === "шт." || q.unit === "рул.";
  const amount = whole
    ? String(Math.round(q.amount))
    : formatDecimal(Math.round(q.amount * 10) / 10);
  return q.packs > 0
    ? `${q.name} ${amount} ${q.unit} (${q.packs} уп.)`
    : `${q.name} ${amount} ${q.unit}`;
}

export function setFinishes(finishes: Finish[]) {
  sendToEngine({ type: "command", command: { type: "setFinishes", finishes } });
}

export function setMaterials(materials: Material[]) {
  sendToEngine({ type: "command", command: { type: "setMaterials", materials } });
}
