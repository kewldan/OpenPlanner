import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask, message, open, save } from "@tauri-apps/plugin-dialog";
import { baseName, fileName, IMAGE_EXTENSIONS, type Platform, PROJECT_EXTENSION } from "./types";

const filters = [{ name: "Проект OpenPlanner", extensions: [PROJECT_EXTENSION] }];

function withExtension(path: string): string {
  return path.toLowerCase().endsWith(`.${PROJECT_EXTENSION}`)
    ? path
    : `${path}.${PROJECT_EXTENSION}`;
}

/** Десктоп: нативные диалоги и файловые команды Tauri (apps/desktop/src-tauri/src/files.rs). */
export const desktopPlatform: Platform = {
  kind: "desktop",

  async openProject() {
    const path = await open({
      title: "Открыть проект",
      multiple: false,
      directory: false,
      filters,
    });
    if (typeof path !== "string") return null;
    const buffer = await invoke<ArrayBuffer>("read_project", { path });
    return { path, name: baseName(path), bytes: new Uint8Array(buffer) };
  },

  async openImage() {
    const path = await open({
      title: "Добавить подложку",
      multiple: false,
      directory: false,
      filters: [{ name: "Изображение", extensions: [...IMAGE_EXTENSIONS] }],
    });
    if (typeof path !== "string") return null;
    const buffer = await invoke<ArrayBuffer>("read_image", { path });
    return { name: fileName(path), bytes: new Uint8Array(buffer) };
  },

  async chooseSaveTarget(suggestedName) {
    const chosen = await save({ title: "Сохранить проект", defaultPath: suggestedName, filters });
    if (!chosen) return null;
    const path = withExtension(chosen);
    return { path, name: baseName(path) };
  },

  async writeProject(bytes, target) {
    if (!target.path) throw new Error("не выбран файл для сохранения");
    await invoke("write_project", bytes, {
      headers: { "x-path": encodeURIComponent(target.path) },
    });
  },

  journal: {
    locate: (projectPath) => invoke<string>("journal_path", { project: projectPath }),
    read: (path) => invoke<string | null>("journal_read", { path }),
    reset: (path, header) => invoke("journal_reset", { path, header }),
    append: (path, text) => invoke("journal_append", { path, text }),
    remove: (path) => invoke("journal_remove", { path }),
  },

  confirm: (text, options) =>
    ask(text, {
      title: options.title,
      kind: "warning",
      okLabel: options.okLabel,
      cancelLabel: options.cancelLabel,
    }),

  async alert(text, title) {
    await message(text, { title, kind: "error" });
  },

  setTitle(title) {
    void getCurrentWindow().setTitle(title);
  },
};
