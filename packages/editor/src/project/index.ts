import { useStore } from "zustand";
import { callEngine, onEngineEvents, onEngineReady, sendToEngine } from "../engine";
import { t } from "../i18n";
import { platform } from "../platform";
import { useEditor } from "../store/editor";
import { AUTOSAVE_INTERVAL_MS, ProjectController, type ProjectState } from "./controller";

export type { ProjectState };

/** Единственный контроллер проекта в приложении. */
export const project = new ProjectController(platform, {
  send: sendToEngine,
  call: callEngine,
  isDirty: () => useEditor.getState().dirty,
});

/** Подписка React-компонентов на состояние проекта. */
export function useProject<T>(selector: (state: ProjectState) => T): T {
  return useStore(project.store, selector);
}

let installed = false;

/** Связывает контроллер с движком, окном и таймером автосохранения. Вызывается один раз. */
export function installProject(): void {
  if (installed) return;
  installed = true;

  onEngineReady(() => void project.start());
  onEngineEvents((events) => project.onEngineEvents(events));
  setInterval(() => void project.autosave(), AUTOSAVE_INTERVAL_MS);

  // При сворачивании или закрытии окна журнал дописывается сразу.
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "hidden") void project.flushJournal();
  });

  const updateTitle = () => {
    const name = project.store.getState().name ?? t("project.untitled");
    const dirty = useEditor.getState().dirty;
    platform.setTitle(`${dirty ? "• " : ""}${name} — ${t("app.name")}`);
  };
  project.store.subscribe(updateTitle);
  useEditor.subscribe((state, previous) => {
    if (state.dirty !== previous.dirty) updateTitle();
  });
  updateTitle();
}
