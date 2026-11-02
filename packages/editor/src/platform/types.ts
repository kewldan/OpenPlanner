/** Файл проекта, открытый пользователем. */
export interface OpenedFile {
  /** Путь на диске; `null`, если платформа путей не знает (браузер). */
  path: string | null;
  /** Имя без расширения — для заголовка окна. */
  name: string;
  bytes: Uint8Array;
}

/** Изображение, выбранное пользователем (подложка). */
export interface PickedImage {
  name: string;
  bytes: Uint8Array;
}

/** Куда сохранился проект. */
export interface SavedFile {
  path: string | null;
  name: string;
}

export interface ConfirmOptions {
  title: string;
  okLabel: string;
  cancelLabel: string;
}

/** Журнал автосохранения конкретного проекта. */
export interface JournalStore {
  /** Где лежит журнал проекта (`null` — несохранённый проект). */
  locate(projectPath: string | null, name: string): Promise<string>;
  read(key: string): Promise<string | null>;
  reset(key: string, header: string): Promise<void>;
  append(key: string, text: string): Promise<void>;
  remove(key: string): Promise<void>;
}

/**
 * Возможности платформы: десктоп (Tauri) или браузер.
 * Ядро и UI одинаковы, различаются только файлы и диалоги.
 */
export interface Platform {
  kind: "desktop" | "browser";
  /** Выбрать и прочитать проект. `null` — пользователь передумал. */
  openProject(): Promise<OpenedFile | null>;
  /** Выбрать изображение PNG или JPEG. `null` — пользователь передумал. */
  openImage(): Promise<PickedImage | null>;
  /** Спросить, куда сохранить проект. `null` — пользователь отменил выбор. */
  chooseSaveTarget(suggestedName: string): Promise<SavedFile | null>;
  /** Записать проект туда, куда выбрал пользователь. */
  writeProject(bytes: Uint8Array, target: SavedFile): Promise<void>;
  journal: JournalStore;
  confirm(message: string, options: ConfirmOptions): Promise<boolean>;
  alert(message: string, title: string): Promise<void>;
  setTitle(title: string): void;
}

export const PROJECT_EXTENSION = "oplan";

/** Расширения изображений для подложки. */
export const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg"] as const;

/** Имя файла без папок. */
export function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** Имя файла без папок и расширения `.oplan`. */
export function baseName(path: string): string {
  const file = path.split(/[\\/]/).pop() ?? path;
  return file.endsWith(`.${PROJECT_EXTENSION}`)
    ? file.slice(0, -PROJECT_EXTENSION.length - 1)
    : file;
}

/** Безопасное имя файла из названия проекта. */
export function fileNameFor(title: string): string {
  const cleaned = title
    // biome-ignore lint/suspicious/noControlCharactersInRegex: управляющие символы недопустимы в именах файлов
    .replace(/[\\/:*?"<>|\u0000-\u001f]/g, " ")
    .replace(/\s+/g, " ")
    .slice(0, 120)
    // Windows не допускает точки и пробелы в конце имени.
    .replace(/[.\s]+$/, "")
    .trim();
  return `${cleaned || "Проект"}.${PROJECT_EXTENSION}`;
}
