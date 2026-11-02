import type { Document, View } from "@openplanner/engine-client";

/** Статус элемента в варианте перепланировки. */
export type Status = "existing" | "demolish" | "new";

/** Вариант текущего вида. */
export function viewVariant(view: View): string | null {
  return view.type === "variant" ? view.id : null;
}

/** Виден ли элемент в виде: в обмере — только обмер, в варианте — обмер и его элементы. */
export function visibleIn(view: View, elementVariant: string | undefined): boolean {
  const variant = viewVariant(view);
  return elementVariant === undefined || elementVariant === variant;
}

/** Демонтированные в варианте элементы (с проёмами демонтированных стен). */
export function demolishedIds(doc: Document, variant: string | null): Set<string> {
  const ids = new Set((doc.variants ?? []).find((v) => v.id === variant)?.demolished ?? []);
  for (const o of doc.openings) {
    if (o.variant === undefined && ids.has(o.wall)) ids.add(o.id);
  }
  return ids;
}

/** Статус элемента в виде варианта; в обмере статусов нет. */
export function statusOf(
  view: View,
  demolished: Set<string>,
  id: string,
  elementVariant: string | undefined,
): Status | null {
  if (viewVariant(view) === null) return null;
  if (elementVariant !== undefined) return "new";
  return demolished.has(id) ? "demolish" : "existing";
}
