import type {
  Pipe,
  PipeSystem,
  PipeVertex,
  PlumbingEntry,
  PlumbingPoint,
} from "@openplanner/engine-client";
import { sendToEngine } from "../engine";

/** Обозначения систем по смыслу ГОСТ 21.601. */
export const systemCodes: Record<PipeSystem, string> = { cold: "В1", hot: "Т3", drain: "К1" };

/** Позиции каталога по разделам в порядке каталога. */
export function plumbingGroups(items: readonly PlumbingEntry[]): [string, PlumbingEntry[]][] {
  const groups = new Map<string, PlumbingEntry[]>();
  for (const item of items) groups.set(item.group, [...(groups.get(item.group) ?? []), item]);
  return [...groups.entries()];
}

/** Трубы инструмента «Труба»: система и диаметр. */
export const pipeTools: readonly { system: PipeSystem; diameter: number }[] = [
  { system: "cold", diameter: 20 },
  { system: "hot", diameter: 20 },
  { system: "drain", diameter: 50 },
  { system: "drain", diameter: 110 },
];

/** Отметки вершин канализационной трубы при новом уклоне: от первой вершины вниз. */
export function withSlope(points: readonly PipeVertex[], slope: number): PipeVertex[] {
  let z = points[0]?.z ?? 0;
  return points.map((p, i) => {
    const prev = points[i - 1];
    if (prev) z -= Math.hypot(p.x - prev.x, p.y - prev.y) * slope;
    return { ...p, z: Math.round(z * 10) / 10 };
  });
}

/** Наименьший уклон горизонтальных участков трубы. */
export function pipeSlope(points: readonly PipeVertex[]): number | null {
  let min: number | null = null;
  for (let i = 1; i < points.length; i++) {
    const a = points[i - 1];
    const b = points[i];
    if (!a || !b) continue;
    const length = Math.hypot(b.x - a.x, b.y - a.y);
    if (length <= 1) continue;
    const slope = (a.z - b.z) / length;
    min = min === null ? slope : Math.min(min, slope);
  }
  return min;
}

export function putPoint(point: PlumbingPoint) {
  sendToEngine({
    type: "command",
    command: { type: "put", element: { type: "plumbingPoint", ...point } },
  });
}

export function putPipe(pipe: Pipe) {
  sendToEngine({ type: "command", command: { type: "put", element: { type: "pipe", ...pipe } } });
}
