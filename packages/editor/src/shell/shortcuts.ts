import { useEffect } from "react";
import { actions, isEnabled } from "../actions";
import { isMacPlatform, matches } from "../actions/shortcut";
import { useEditor } from "../store/editor";

/** Глобальные горячие клавиши из реестра команд. */
export function useShortcuts() {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.repeat && event.code !== "KeyZ") return;
      const target = event.target as HTMLElement | null;
      const inInput = Boolean(target?.closest("input, textarea, select, [contenteditable='true']"));
      // Пока открыто модальное окно, работают только сочетания, открывающие окна.
      const dialogOpen = useEditor.getState().dialog !== null;

      for (const action of actions) {
        if (!action.keys?.some((key) => matches(event, key, isMacPlatform))) continue;
        if (inInput && !action.inInputs) return;
        if (dialogOpen && action.group !== "help") return;
        event.preventDefault();
        // Файловые команды из поля ввода: сначала зафиксировать правку (поля коммитят по blur).
        if (inInput && action.group === "file" && target instanceof HTMLElement) target.blur();
        if (isEnabled(action)) action.run();
        return;
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}
