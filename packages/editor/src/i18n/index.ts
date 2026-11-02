import { type MessageKey, ru } from "./ru";

export type { MessageKey };

const catalog: Record<MessageKey, string> = ru;

/** Строка интерфейса по ключу с подстановкой параметров вида {name}. */
export function t(key: MessageKey, params?: Record<string, string | number>): string {
  const template = catalog[key];
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (match, name: string) => {
    const value = params[name];
    return value === undefined ? match : String(value);
  });
}

const numberFormat = new Intl.NumberFormat("ru-RU", { maximumFractionDigits: 0 });

/** Число по правилам локали: неразрывные пробелы в разрядах, десятичная запятая. */
export function formatNumber(value: number): string {
  return numberFormat.format(value);
}
