import type { Document, Level } from "@openplanner/engine-client";
import { sendToEngine } from "../engine";

/** Первый этаж — неявный: элементы без поля `level`. */
export const FIRST_LEVEL = "Этаж 1";

/** Этажи по порядку отметок: первый (`id: null`), затем добавленные. */
export function levelList(doc: Document): { id: string | null; name: string; elevation: number }[] {
  const extra = [...(doc.levels ?? [])].sort((a, b) => a.elevation - b.elevation);
  return [
    { id: null, name: FIRST_LEVEL, elevation: 0 },
    ...extra.map((l) => ({ id: l.id, name: l.name, elevation: l.elevation })),
  ];
}

export function setLevel(level: string | null) {
  sendToEngine(level === null ? { type: "setLevel" } : { type: "setLevel", level });
}

export function setLevels(levels: Level[]) {
  sendToEngine({ type: "command", command: { type: "setLevels", levels } });
}
