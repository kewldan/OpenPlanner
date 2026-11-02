import type { CatalogEntry } from "@openplanner/engine-client";
import { callEngine, sendToEngine } from "../engine";
import { platform } from "../platform";
import { useEditor } from "../store/editor";

/** Тип данных перетаскивания позиции каталога на план. */
export const DRAG_TYPE = "application/x-openplanner-furniture";

/** Позиции, подходящие под запрос: все слова — в названии, разделе или ключевых словах. */
export function searchCatalog(items: readonly CatalogEntry[], query: string): CatalogEntry[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return [...items];
  return items.filter((item) => {
    const haystack = `${item.name} ${item.category} ${item.keywords}`.toLowerCase();
    return words.every((word) => haystack.includes(word));
  });
}

/** Позиции по разделам в порядке каталога. */
export function byCategory(items: readonly CatalogEntry[]): [string, CatalogEntry[]][] {
  const groups = new Map<string, CatalogEntry[]>();
  for (const item of items) {
    const list = groups.get(item.category) ?? [];
    list.push(item);
    groups.set(item.category, list);
  }
  return [...groups.entries()];
}

/** Инструмент «Мебель» с позицией каталога. */
export function pickItem(id: string) {
  sendToEngine({ type: "furnitureTool", item: id });
}

/** Добавляет свою модель GLB в проект. */
export async function importModel(): Promise<void> {
  try {
    const picked = await platform.openModel();
    if (!picked) return;
    await callEngine({ type: "importFurniture", bytes: picked.bytes, name: picked.name });
  } catch (error) {
    useEditor.setState({ lastError: error instanceof Error ? error.message : String(error) });
  }
}
