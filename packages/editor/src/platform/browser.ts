import {
  baseName,
  IMAGE_EXTENSIONS,
  type Platform,
  PROJECT_EXTENSION,
  SCAN_EXTENSIONS,
} from "./types";

const JOURNAL_PREFIX = "openplanner.journal:";

function storage(): Storage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

function pickFile(accept: string): Promise<File | null> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = accept;
    input.addEventListener("change", () => resolve(input.files?.[0] ?? null), { once: true });
    input.addEventListener("cancel", () => resolve(null), { once: true });
    input.click();
  });
}

function download(bytes: Uint8Array, name: string) {
  const url = URL.createObjectURL(new Blob([bytes as BlobPart], { type: "application/zip" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  // Chromium учитывает имя из `download` только у ссылки, вставленной в документ.
  link.style.display = "none";
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

/**
 * Браузер (веб-версия и e2e-тесты): открытие через выбор файла, сохранение —
 * скачиванием, журнал — в localStorage.
 */
export const browserPlatform: Platform = {
  kind: "browser",

  async openProject() {
    const file = await pickFile(`.${PROJECT_EXTENSION}`);
    if (!file) return null;
    return {
      path: null,
      name: baseName(file.name),
      bytes: new Uint8Array(await file.arrayBuffer()),
    };
  },

  async openImage() {
    const file = await pickFile(IMAGE_EXTENSIONS.map((ext) => `.${ext}`).join(","));
    if (!file) return null;
    return { name: file.name, bytes: new Uint8Array(await file.arrayBuffer()) };
  },

  async openScan() {
    const file = await pickFile(SCAN_EXTENSIONS.map((ext) => `.${ext}`).join(","));
    if (!file) return null;
    return { name: file.name, bytes: new Uint8Array(await file.arrayBuffer()) };
  },

  async chooseSaveTarget(suggestedName) {
    return { path: null, name: baseName(suggestedName) };
  },

  async writeProject(bytes, target) {
    download(bytes, `${target.name}.${PROJECT_EXTENSION}`);
  },

  journal: {
    locate: async (_path, name) => `${JOURNAL_PREFIX}${name}`,
    read: async (key) => storage()?.getItem(key) ?? null,
    reset: async (key, header) => storage()?.setItem(key, header),
    append: async (key, text) => {
      const store = storage();
      store?.setItem(key, (store.getItem(key) ?? "") + text);
    },
    remove: async (key) => storage()?.removeItem(key),
  },

  confirm: async (text) => window.confirm(text),

  alert: async (text) => window.alert(text),

  setTitle(title) {
    document.title = title;
  },
};
