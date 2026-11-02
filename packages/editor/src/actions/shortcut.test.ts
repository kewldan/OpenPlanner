import { describe, expect, test } from "bun:test";
import { format, type KeyLike, matches } from "./shortcut";

const press = (code: string, mods: Partial<KeyLike> = {}): KeyLike => ({
  code,
  metaKey: false,
  ctrlKey: false,
  shiftKey: false,
  altKey: false,
  ...mods,
});

describe("горячие клавиши", () => {
  test("⌘ на Mac и Ctrl на остальных", () => {
    const undo = { code: "KeyZ", mod: true };
    expect(matches(press("KeyZ", { metaKey: true }), undo, true)).toBe(true);
    expect(matches(press("KeyZ", { ctrlKey: true }), undo, true)).toBe(false);
    expect(matches(press("KeyZ", { ctrlKey: true }), undo, false)).toBe(true);
  });

  test("лишний Shift — другое сочетание", () => {
    expect(
      matches(press("KeyZ", { metaKey: true, shiftKey: true }), { code: "KeyZ", mod: true }, true),
    ).toBe(false);
  });

  test("подписи", () => {
    expect(format({ code: "KeyS", mod: true, shift: true }, true)).toBe("⇧⌘S");
    expect(format({ code: "KeyS", mod: true, shift: true }, false)).toBe("Ctrl+Shift+S");
    expect(format({ code: "Digit1", shift: true }, true)).toBe("⇧1");
  });
});
