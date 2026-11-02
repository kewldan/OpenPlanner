import { invoke, isTauri } from "@tauri-apps/api/core";
import { useEditor } from "./store/editor";

/**
 * Самопроверка десктопа для CI: если приложение запущено с
 * `OPENPLANNER_SELFTEST=<файл>`, после запуска движка (или ошибки) результат
 * с графическим бэкендом записывается в файл, и приложение завершается.
 */
export async function installSelftest(): Promise<void> {
  if (!isTauri() || !(await invoke<boolean>("selftest_enabled"))) return;
  const report = (ok: boolean, details: string) =>
    void invoke("selftest_report", { ok, details: `${details} | ${navigator.userAgent}` });

  const check = () => {
    const { engine } = useEditor.getState();
    if (engine.state === "ready") report(true, `backend=${engine.backend} webgpu=${engine.webgpu}`);
    if (engine.state === "failed") report(false, engine.message);
    return engine.state !== "starting";
  };
  if (check()) return;
  const unsubscribe = useEditor.subscribe(() => {
    if (check()) unsubscribe();
  });
  setTimeout(() => report(false, "движок не запустился за 60 с"), 60_000);
}
