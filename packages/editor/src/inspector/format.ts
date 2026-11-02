import { t } from "../i18n";

const area = new Intl.NumberFormat("ru-RU", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
const decimal = new Intl.NumberFormat("ru-RU", { maximumFractionDigits: 2 });

/** Площадь: «12,50 м²». */
export function formatArea(m2: number): string {
  return `${area.format(m2)} ${t("unit.m2")}`;
}

/** Число с запятой и до двух знаков. */
export function formatDecimal(value: number): string {
  return decimal.format(value);
}

/** Число из поля ввода: запятая или точка, пробелы в разрядах. `null` — не число. */
export function parseNumber(raw: string): number | null {
  const cleaned = raw.replace(/[\s  ]/g, "").replace(",", ".");
  if (cleaned === "") return null;
  const value = Number(cleaned);
  return Number.isFinite(value) ? value : null;
}
