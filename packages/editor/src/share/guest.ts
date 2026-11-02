/** Ключ гостевой ссылки в адресе: `…/#guest=<проект>`. */
export const GUEST_KEY = "guest";
/** Адрес веб-версии по умолчанию для ссылок из десктопа. */
export const DEFAULT_WEB_URL = "https://kewldan.github.io/OpenPlanner/";
/** Длиннее этого ссылку обрезают мессенджеры — лучше отправить файл. */
export const LONG_LINK = 32_000;

const STORAGE = "openplanner.webUrl";

/** Байты → base64url без дополнения. */
export function toBase64Url(bytes: Uint8Array): string {
  let binary = "";
  for (let i = 0; i < bytes.length; i += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "");
}

/** base64url → байты; `null` — не base64. */
export function fromBase64Url(text: string): Uint8Array | null {
  try {
    const b64 = text.replaceAll("-", "+").replaceAll("_", "/");
    const binary = atob(b64 + "=".repeat((4 - (b64.length % 4)) % 4));
    return Uint8Array.from(binary, (c) => c.charCodeAt(0));
  } catch {
    return null;
  }
}

/** Гостевая ссылка: адрес веб-версии и проект (`.oplan` без файлов) в якоре. */
export function guestLink(base: string, project: Uint8Array): string {
  const url = base.split("#")[0] ?? base;
  return `${url}#${GUEST_KEY}=${toBase64Url(project)}`;
}

/** Проект из якоря адреса; `null` — это не гостевая ссылка. */
export function guestProject(hash: string): Uint8Array | null {
  const prefix = `#${GUEST_KEY}=`;
  return hash.startsWith(prefix) ? fromBase64Url(hash.slice(prefix.length)) : null;
}

/** Адрес веб-версии: сохранённый пользователем, в браузере — текущий, иначе по умолчанию. */
export function webUrl(kind: "desktop" | "browser"): string {
  try {
    const saved = window.localStorage.getItem(STORAGE);
    if (saved) return saved;
  } catch {
    // Хранилище недоступно — берём по умолчанию.
  }
  return kind === "browser" ? `${location.origin}${location.pathname}` : DEFAULT_WEB_URL;
}

export function saveWebUrl(url: string) {
  try {
    window.localStorage.setItem(STORAGE, url);
  } catch {
    // Не запомнили — в следующий раз будет адрес по умолчанию.
  }
}
