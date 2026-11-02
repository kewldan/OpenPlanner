import { useStore } from "zustand";
import { callEngine, onEngineEvents, onEngineReady, sendToEngine } from "../engine";
import { t } from "../i18n";
import { platform } from "../platform";
import { guestProject } from "../share/guest";
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

  // Гостевая ссылка открывает проект из адреса только для просмотра.
  const guest = guestProject(window.location.hash);
  onEngineReady(() => void (guest ? project.openGuest(guest) : project.start()));
  // Файл в облачной папке мог измениться на другом устройстве.
  window.addEventListener("focus", () => void project.checkExternal());
  onEngineEvents((events) => project.onEngineEvents(events));
  setInterval(
    () => void project.checkExternal().then(() => project.autosave()),
    AUTOSAVE_INTERVAL_MS,
  );

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
