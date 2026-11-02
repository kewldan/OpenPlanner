import { sendToEngine } from "../engine";
import { project } from "../project";
import { useEditor } from "../store/editor";
import { applyTheme } from "../theme";
import type { Shortcut } from "./shortcut";

export type ActionGroup = "file" | "edit" | "view" | "help";

export const groupTitles: Record<ActionGroup, string> = {
  file: "Файл",
  edit: "Правка",
  view: "Вид",
  help: "Справка",
};

/** Команда приложения: доступна из палитры ⌘K и по горячим клавишам. */
export interface Action {
  id: string;
  title: string;
  group: ActionGroup;
  /** Дополнительные слова для поиска. */
  keywords?: string;
  keys?: Shortcut[];
  /** Срабатывает ли сочетание, когда фокус в поле ввода. */
  inInputs?: boolean;
  enabled?: () => boolean;
  run: () => void;
}

const zoomAtCenter = (factor: number) => {
  const { width, height } = useEditor.getState().viewport;
  sendToEngine({ type: "zoom", x: width / 2, y: height / 2, factor });
};

const notBusy = () => !project.store.getState().busy;

/** Все команды приложения. Порядок — порядок в палитре. */
export const actions: readonly Action[] = [
  {
    id: "file.new",
    title: "Новый проект",
    group: "file",
    keywords: "создать",
    keys: [{ code: "KeyN", mod: true }],
    inInputs: true,
    enabled: notBusy,
    run: () => void project.newProject(),
  },
  {
    id: "file.open",
    title: "Открыть…",
    group: "file",
    keywords: "файл oplan загрузить",
    keys: [{ code: "KeyO", mod: true }],
    inInputs: true,
    enabled: notBusy,
    run: () => void project.openProject(),
  },
  {
    id: "file.save",
    title: "Сохранить",
    group: "file",
    keys: [{ code: "KeyS", mod: true }],
    inInputs: true,
    enabled: notBusy,
    run: () => void project.save(),
  },
  {
    id: "file.saveAs",
    title: "Сохранить как…",
    group: "file",
    keywords: "копия экспорт",
    keys: [{ code: "KeyS", mod: true, shift: true }],
    inInputs: true,
    enabled: notBusy,
    run: () => void project.saveAs(),
  },
  {
    id: "edit.undo",
    title: "Отменить",
    group: "edit",
    keys: [{ code: "KeyZ", mod: true }],
    enabled: () => useEditor.getState().canUndo,
    run: () => sendToEngine({ type: "undo" }),
  },
  {
    id: "edit.redo",
    title: "Повторить",
    group: "edit",
    keys: [
      { code: "KeyZ", mod: true, shift: true },
      { code: "KeyY", mod: true },
    ],
    enabled: () => useEditor.getState().canRedo,
    run: () => sendToEngine({ type: "redo" }),
  },
  {
    id: "view.zoomIn",
    title: "Приблизить",
    group: "view",
    keywords: "масштаб зум",
    keys: [{ code: "Equal", mod: true }],
    inInputs: true,
    run: () => zoomAtCenter(1.25),
  },
  {
    id: "view.zoomOut",
    title: "Отдалить",
    group: "view",
    keywords: "масштаб зум",
    keys: [{ code: "Minus", mod: true }],
    inInputs: true,
    run: () => zoomAtCenter(0.8),
  },
  {
    id: "view.zoomToFit",
    title: "Показать весь план",
    group: "view",
    keywords: "масштаб вписать",
    keys: [{ code: "Digit1", shift: true }],
    run: () => sendToEngine({ type: "zoomToFit" }),
  },
  {
    id: "view.themeAuto",
    title: "Тема: как в системе",
    group: "view",
    keywords: "оформление",
    run: () => applyTheme("auto"),
  },
  {
    id: "view.themeLight",
    title: "Тема: светлая",
    group: "view",
    keywords: "оформление",
    run: () => applyTheme("light"),
  },
  {
    id: "view.themeDark",
    title: "Тема: тёмная",
    group: "view",
    keywords: "оформление ночь",
    run: () => applyTheme("dark"),
  },
  {
    id: "help.palette",
    title: "Палитра команд",
    group: "help",
    keys: [{ code: "KeyK", mod: true }],
    inInputs: true,
    run: () => useEditor.getState().setDialog("palette"),
  },
  {
    id: "help.shortcuts",
    title: "Горячие клавиши",
    group: "help",
    keywords: "сочетания шпаргалка",
    keys: [{ code: "Slash", mod: true }],
    inInputs: true,
    run: () => useEditor.getState().setDialog("shortcuts"),
  },
];

/** Доступна ли команда сейчас. */
export function isEnabled(action: Action): boolean {
  return action.enabled?.() ?? true;
}

/** Ищет команды по словам запроса (все слова должны найтись). */
export function searchActions(query: string, list: readonly Action[] = actions): Action[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return [...list];
  return list.filter((action) => {
    const haystack =
      `${action.title} ${groupTitles[action.group]} ${action.keywords ?? ""}`.toLowerCase();
    return words.every((word) => haystack.includes(word));
  });
}
