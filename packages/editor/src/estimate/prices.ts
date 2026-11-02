import type { Measure, PriceItem, QuantityOverride, TableFormat } from "@openplanner/engine-client";
import { callEngine, sendToEngine } from "../engine";
import { platform } from "../platform";

/** Правило объёма одной строкой — как в CSV прайса (`finish:floor:tile`, `devices:outlet`). */
export function measureKey(m: Measure): string {
  const opt = (base: string, v: string | undefined) => (v ? `${base}:${v}` : base);
  switch (m.type) {
    case "partitions":
      return opt("partitions", m.material);
    case "devices":
      return opt("devices", m.group);
    case "outlets":
      return opt("outlets", m.system);
    case "pipes":
      return opt("pipes", m.system);
    case "finish":
      return `finish:${m.surface ?? "*"}:${m.kind ?? "*"}`;
    case "consumption":
      return m.material ? `consumption:${m.name}:${m.material}` : `consumption:${m.name}`;
    default:
      return m.type;
  }
}

/** Правила объёма для выбора в прайсе: название и правило. */
export const measurePresets: readonly { label: string; measure: Measure }[] = [
  { label: "Вручную", measure: { type: "manual" } },
  { label: "Площадь пола", measure: { type: "floorArea" } },
  { label: "Площадь потолка", measure: { type: "ceilingArea" } },
  { label: "Площадь стен", measure: { type: "wallArea" } },
  { label: "Периметр помещений", measure: { type: "perimeter" } },
  { label: "Демонтаж стен, м²", measure: { type: "demolition" } },
  { label: "Новые перегородки, м²", measure: { type: "partitions" } },
  {
    label: "Перегородки из газобетона",
    measure: { type: "partitions", material: "aeratedConcrete" },
  },
  { label: "Перегородки из ГКЛ", measure: { type: "partitions", material: "gypsum" } },
  { label: "Перегородки из кирпича", measure: { type: "partitions", material: "brick" } },
  { label: "Новые проёмы", measure: { type: "newOpenings" } },
  { label: "Электроточки", measure: { type: "devices" } },
  { label: "Розетки и выводы", measure: { type: "devices", group: "outlet" } },
  { label: "Выключатели", measure: { type: "devices", group: "switch" } },
  { label: "Светильники", measure: { type: "devices", group: "light" } },
  { label: "Щит", measure: { type: "devices", group: "panel" } },
  { label: "Кабель, м", measure: { type: "cable" } },
  { label: "Выводы сантехники", measure: { type: "outlets" } },
  { label: "Трубы, м", measure: { type: "pipes" } },
  { label: "Трубы ХВС, м", measure: { type: "pipes", system: "cold" } },
  { label: "Трубы ГВС, м", measure: { type: "pipes", system: "hot" } },
  { label: "Трубы канализации, м", measure: { type: "pipes", system: "drain" } },
  { label: "Сантехприборы", measure: { type: "fixtures" } },
  { label: "Мебель", measure: { type: "furniture" } },
  { label: "Отделка, м²", measure: { type: "finish" } },
  { label: "Плитка на полу, м²", measure: { type: "finish", surface: "floor", kind: "tile" } },
  { label: "Плитка на стенах, м²", measure: { type: "finish", surface: "walls", kind: "tile" } },
  { label: "Плитка, м²", measure: { type: "finish", kind: "tile" } },
  { label: "Ламинат и доска, м²", measure: { type: "finish", surface: "floor", kind: "laminate" } },
  { label: "Окраска стен, м²", measure: { type: "finish", surface: "walls", kind: "paint" } },
  { label: "Обои, м²", measure: { type: "finish", surface: "walls", kind: "wallpaper" } },
  { label: "Окраска потолка, м²", measure: { type: "finish", surface: "ceiling", kind: "paint" } },
  { label: "Натяжной потолок, м²", measure: { type: "finish", surface: "ceiling", kind: "sheet" } },
  { label: "Расход: плитка, шт.", measure: { type: "consumption", name: "Плитка" } },
  { label: "Расход: клей, кг", measure: { type: "consumption", name: "Клей" } },
  { label: "Расход: затирка, кг", measure: { type: "consumption", name: "Затирка" } },
  { label: "Расход: краска, л", measure: { type: "consumption", name: "Краска" } },
  { label: "Расход: обои, рул.", measure: { type: "consumption", name: "Обои" } },
  { label: "Расход: покрытие, м²", measure: { type: "consumption", name: "Покрытие" } },
];

/** Сегодняшняя дата для истории цен: «02.11.2026». */
export function today(now = new Date()): string {
  const d = String(now.getDate()).padStart(2, "0");
  const m = String(now.getMonth() + 1).padStart(2, "0");
  return `${d}.${m}.${now.getFullYear()}`;
}

/** Позиция с новой ценой: прежняя уходит в историю. */
export function withPrice(item: PriceItem, price: number, date: string): PriceItem {
  if (price === item.price) return item;
  return { ...item, price, history: [...(item.history ?? []), { date, price: item.price }] };
}

/** Новая позиция прайса с ручным объёмом. */
export function newPriceItem(n: number): PriceItem {
  return {
    id: `price-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`,
    code: "",
    section: "Прочее",
    name: `Позиция ${n}`,
    kind: "work",
    unit: "шт.",
    price: 0,
    measure: { type: "manual" },
    rate: 1,
    reserve: 0,
  };
}

/** Ручной объём строки: `null` — вернуть объём по модели. */
export function withOverride(
  list: readonly QuantityOverride[],
  item: string,
  room: string | undefined,
  quantity: number | null,
): QuantityOverride[] {
  const rest = list.filter((o) => !(o.item === item && o.room === room));
  if (quantity === null) return rest;
  return [...rest, { item, ...(room === undefined ? {} : { room }), quantity }];
}

/** Деньги: «12 345,50 ₽». */
export function formatMoney(value: number): string {
  return `${new Intl.NumberFormat("ru-RU", { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(value)} ₽`;
}

export function setPrices(prices: PriceItem[]) {
  sendToEngine({ type: "command", command: { type: "setPrices", prices } });
}

export function setOverrides(overrides: QuantityOverride[]) {
  sendToEngine({ type: "command", command: { type: "setOverrides", overrides } });
}

/** Сохраняет смету или прайс файлом; возвращает имя файла или `null` (отмена). */
export async function saveTable(
  what: "estimate" | "prices",
  format: TableFormat,
  project: string,
): Promise<string | null> {
  const bytes = await callEngine(
    what === "estimate" ? { type: "exportEstimate", format } : { type: "exportPrices", format },
  );
  const name = `${project} — ${what === "estimate" ? "смета" : "прайс"}.${format}`;
  return (await platform.saveExport(bytes, name)) ? name : null;
}

/** Импортирует прайс из выбранного файла CSV или XLSX. `false` — пользователь передумал. */
export async function importPrices(): Promise<boolean> {
  const picked = await platform.openTable();
  if (!picked) return false;
  await callEngine({ type: "importPrices", bytes: picked.bytes, date: today() });
  return true;
}
