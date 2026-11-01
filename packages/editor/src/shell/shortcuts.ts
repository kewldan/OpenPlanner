import { useEffect } from "react";
import { sendToEngine } from "../engine";

const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

/** Глобальные горячие клавиши редактора. Не срабатывают при вводе в поля. */
export function useShortcuts() {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.closest("input, textarea, [contenteditable='true']")) return;

      const mod = isMac ? event.metaKey : event.ctrlKey;
      const key = event.key.toLowerCase();
      if (mod && key === "z") {
        event.preventDefault();
        sendToEngine({ type: event.shiftKey ? "redo" : "undo" });
      } else if (mod && key === "y" && !isMac) {
        event.preventDefault();
        sendToEngine({ type: "redo" });
      } else if (event.shiftKey && key === "1") {
        event.preventDefault();
        sendToEngine({ type: "zoomToFit" });
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}
