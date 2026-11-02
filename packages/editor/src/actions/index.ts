import type { Tool } from "@openplanner/engine-client";
import { callEngine, sendToEngine } from "../engine";
import { t } from "../i18n";
import { platform } from "../platform";
import { project } from "../project";
import { useEditor } from "../store/editor";
import { applyTheme } from "../theme";
import type { Shortcut } from "./shortcut";

export type ActionGroup = "file" | "edit" | "tools" | "view" | "help";

export const groupTitles: Record<ActionGroup, string> = {
  file: "Файл",
  edit: "Правка",
  tools: "Инструменты",
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

const hasSelection = () => useEditor.getState().selection.length > 0;

/** Добавляет изображение-подложку, выбранное пользователем. */
export async function addUnderlay(): Promise<void> {
  try {
    const picked = await platform.openImage();
    if (!picked) return;
    await callEngine({ type: "addUnderlay", bytes: picked.bytes, name: picked.name });
  } catch (error) {
    useEditor.setState({ lastError: error instanceof Error ? error.message : String(error) });
  }
}

function tool(id: Tool, code: string, keywords: string): Action {
  return {
    id: `tool.${id}`,
    title: t(`tool.${id}`),
    group: "tools",
    keywords,
    keys: [{ code }],
    run: () => sendToEngine({ type: "setTool", tool: id }),
  };
}

/** Инструменты в порядке панели инструментов. */
export const toolActions: readonly Action[] = [
  tool("select", "KeyV", "выделить курсор стрелка"),
  tool("wall", "KeyW", "стены рисовать обмер"),
  tool("door", "KeyD", "проём дверной"),
  tool("window", "KeyO", "проём оконный"),
  tool("arch", "KeyA", "проём арочный"),
  tool("room", "KeyR", "комната площадь"),
  tool("dimension", "KeyM", "размерная линия измерить"),
  tool("leader", "KeyL", "текст подпись примечание"),
  tool("calibrate", "KeyK", "масштаб подложки"),
];

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
    id: "edit.delete",
    title: "Удалить выделенное",
    group: "edit",
    keywords: "стереть убрать",
    keys: [{ code: "Delete" }, { code: "Backspace" }],
    enabled: hasSelection,
    run: () => sendToEngine({ type: "deleteSelection" }),
  },
  {
    id: "edit.selectAll",
    title: "Выделить всё",
    group: "edit",
    keys: [{ code: "KeyA", mod: true }],
    run: () => sendToEngine({ type: "selectAll" }),
  },
  {
    id: "edit.cancel",
    title: "Прервать действие инструмента",
    group: "edit",
    keywords: "отмена снять выделение",
    keys: [{ code: "Escape" }],
    run: () => sendToEngine({ type: "cancel" }),
  },
  {
    id: "edit.finish",
    title: "Закончить цепочку стен",
    group: "edit",
    keys: [{ code: "Enter" }],
    run: () => sendToEngine({ type: "finish" }),
  },
  {
    id: "edit.findRooms",
    title: "Найти помещения",
    group: "edit",
    keywords: "комнаты площади контуры автоматически",
    run: () => sendToEngine({ type: "findRooms" }),
  },
  ...toolActions,
  {
    id: "file.addUnderlay",
    title: "Добавить подложку…",
    group: "file",
    keywords: "изображение картинка план скан png jpg",
    enabled: notBusy,
    run: () => void addUnderlay(),
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
    id: "view.scale50",
    title: "Масштаб 1:50",
    group: "view",
    keywords: "зум 100",
    keys: [{ code: "Digit0", mod: true }],
    inInputs: true,
    run: () => sendToEngine({ type: "zoomToScale", denominator: 50 }),
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
