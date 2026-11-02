import { invoke, isTauri } from "@tauri-apps/api/core";
import { sendToEngine } from "./engine";
import { project } from "./project";
import { useEditor } from "./store/editor";

/**
 * Самопроверка десктопа для CI. Включается переменными окружения приложения:
 *
 * - `OPENPLANNER_SELFTEST=<файл>` — после запуска движка (или ошибки) в файл
 *   пишется результат с графическим бэкендом, и приложение завершается;
 * - `OPENPLANNER_SELFTEST_SCENARIO` — сценарий перед отчётом. Настоящие
 *   системные диалоги проходит внешний драйвер (`scripts/desktop-dialogs.sh`),
 *   ориентируясь на строки `STEP …` в файле отчёта:
 *   - `dialogs` — «Сохранить» и «Открыть» через системные диалоги;
 *   - `crash` — открыть файл, внести правку и дождаться записи журнала
 *     (после этого драйвер аварийно завершает процесс);
 *   - `recover` — открыть тот же файл, согласиться на восстановление в
 *     системном вопросе и сверить восстановленную правку.
 */
export async function installSelftest(): Promise<void> {
  if (!isTauri() || !(await invoke<boolean>("selftest_enabled"))) return;
  const scenario = await invoke<string | null>("selftest_scenario");
  let reported = false;
  const report = (ok: boolean, details: string) => {
    if (reported) return;
    reported = true;
    void invoke("selftest_report", { ok, details: `${details} | ${navigator.userAgent}` });
  };

  const onReady = async (backend: string, webgpu: string) => {
    const summary = `backend=${backend} webgpu=${webgpu}`;
    const run = scenarios[scenario ?? ""];
    if (scenario && !run) {
      report(false, `${summary} неизвестный сценарий ${scenario}`);
      return;
    }
    if (run) {
      try {
        await run();
      } catch (error) {
        report(
          false,
          `${summary} сценарий: ${error instanceof Error ? error.message : String(error)}`,
        );
        return;
      }
    }
    report(true, scenario ? `${summary} scenario=${scenario}` : summary);
  };

  const check = () => {
    const { engine } = useEditor.getState();
    if (engine.state === "ready") void onReady(engine.backend, engine.webgpu);
    if (engine.state === "failed") report(false, engine.message);
    return engine.state !== "starting";
  };
  if (!check()) {
    const unsubscribe = useEditor.subscribe(() => {
      if (check()) unsubscribe();
    });
  }
  setTimeout(() => report(false, "самопроверка не завершилась за 180 с"), 180_000);
}

const step = (name: string) => invoke("selftest_step", { step: name });

async function waitFor(what: string, condition: () => boolean, timeout = 30_000) {
  const start = Date.now();
  while (!condition()) {
    if (Date.now() - start > timeout) throw new Error(`не дождались: ${what}`);
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}

const title = () => useEditor.getState().document?.meta.title;

const CRASH_TITLE = "Recovered";

/** Открыть сохранённый файл, внести правку и дождаться её записи в журнал. */
async function crashScenario() {
  await waitFor("запуск проекта", () => !project.store.getState().busy && title() !== undefined);
  await step("dialog:open");
  await project.openProject();
  await waitFor("открытый файл", () => title() === "Selftest 2");
  sendToEngine({ type: "command", command: { type: "setTitle", title: CRASH_TITLE } });
  await waitFor("правка", () => title() === CRASH_TITLE);
  await project.flushJournal();
  await step("journaled");
  // Дальше драйвер завершает процесс через kill -9; отчёт не пишется.
  await new Promise(() => {});
}

/** После сбоя: открыть файл и восстановить правку из журнала. */
async function recoverScenario() {
  await waitFor("запуск проекта", () => !project.store.getState().busy && title() !== undefined);
  await step("dialog:open");
  await project.openProject();
  await waitFor("восстановленная правка", () => title() === CRASH_TITLE, 60_000);
  if (!useEditor.getState().dirty) throw new Error("восстановленное должно быть несохранённым");
  await step("recovered");
}

const scenarios: Record<string, (() => Promise<void>) | undefined> = {
  dialogs: dialogsScenario,
  crash: crashScenario,
  recover: recoverScenario,
};

/** Сохранение и открытие через настоящие системные диалоги. */
async function dialogsScenario() {
  await waitFor("запуск проекта", () => !project.store.getState().busy && title() !== undefined);
  sendToEngine({ type: "command", command: { type: "setTitle", title: "Selftest" } });
  await waitFor("правка названия", () => title() === "Selftest");

  await step("dialog:save");
  if (!(await project.save())) throw new Error("сохранение отменено или не удалось");
  const path = project.store.getState().path;
  if (!path) throw new Error("после сохранения нет пути файла");
  await step(`saved:${path}`);

  // Повторное сохранение — в тот же файл, без диалога.
  sendToEngine({ type: "command", command: { type: "setTitle", title: "Selftest 2" } });
  await waitFor("вторая правка", () => title() === "Selftest 2");
  if (!(await project.save())) throw new Error("повторное сохранение не удалось");

  await project.newProject();
  await waitFor("новый проект", () => title() !== "Selftest 2");

  await step("dialog:open");
  await project.openProject();
  await waitFor("открытый файл", () => title() === "Selftest 2");
  if (project.store.getState().path !== path) throw new Error("открыт не тот файл");
  await step("opened");
}
