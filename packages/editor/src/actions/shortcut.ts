/**
 * Горячая клавиша. Клавиша задаётся физическим кодом (`KeyboardEvent.code`),
 * поэтому ⌘Z работает и в русской раскладке, где `event.key` — «я».
 */
export interface Shortcut {
  /** Код клавиши: `KeyZ`, `Digit1`, `Slash`, `Equal`… */
  code: string;
  /** ⌘ на macOS, Ctrl на Windows и Linux. */
  mod?: boolean;
  shift?: boolean;
  alt?: boolean;
}

export interface KeyLike {
  code: string;
  metaKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

/** Совпадает ли нажатие с сочетанием. Лишние модификаторы — несовпадение. */
export function matches(event: KeyLike, shortcut: Shortcut, isMac: boolean): boolean {
  const mod = isMac ? event.metaKey : event.ctrlKey;
  const otherMod = isMac ? event.ctrlKey : event.metaKey;
  return (
    event.code === shortcut.code &&
    mod === Boolean(shortcut.mod) &&
    !otherMod &&
    event.shiftKey === Boolean(shortcut.shift) &&
    event.altKey === Boolean(shortcut.alt)
  );
}

const codeLabels: Record<string, string> = {
  Slash: "/",
  Equal: "=",
  Minus: "−",
  Comma: ",",
  Period: ".",
  Backspace: "⌫",
  Delete: "Del",
  Escape: "Esc",
  Enter: "↵",
  Space: "Пробел",
};

function keyLabel(code: string): string {
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  return codeLabels[code] ?? code;
}

/** Подпись сочетания: «⇧⌘S» на macOS, «Ctrl+Shift+S» на других системах. */
export function format(shortcut: Shortcut, isMac: boolean): string {
  const key = keyLabel(shortcut.code);
  if (isMac) {
    return `${shortcut.alt ? "⌥" : ""}${shortcut.shift ? "⇧" : ""}${shortcut.mod ? "⌘" : ""}${key}`;
  }
  return [shortcut.mod && "Ctrl", shortcut.alt && "Alt", shortcut.shift && "Shift", key]
    .filter(Boolean)
    .join("+");
}

export const isMacPlatform =
  typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);
