import type { Tool } from "@openplanner/engine-client";
import { pickDxf } from "../dxf/store";
import { callEngine, sendToEngine } from "../engine";
import { t } from "../i18n";
import { platform } from "../platform";
import { project } from "../project";
import { openScanWizard } from "../scan/store";
import { useEditor } from "../store/editor";
import { applyTheme } from "../theme";
import { exitCompare } from "../variants/actions";
import { set3d, toggle3d } from "../view3d/actions";
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

/** План можно править: не сравнение и не 3D. */
const editable = () => {
  const { view, three } = useEditor.getState();
  return view.type !== "compare" && !three.options.enabled;
};

const in3d = () => useEditor.getState().three.options.enabled;

/** Есть ли что поворачивать: инструмент «Мебель» или выделенная мебель. */
const furnitureActive = () => {
  const { tool, selection, document } = useEditor.getState();
  if (!editable()) return false;
  if (tool === "furniture") return true;
  return (document?.furniture ?? []).some((f) => selection.includes(f.id));
};

function tool(id: Tool, code: string, keywords: string): Action {
  return {
    id: `tool.${id}`,
    title: t(`tool.${id}`),
    group: "tools",
    keywords,
    keys: [{ code }],
    enabled:
      id === "demolish"
        ? () => useEditor.getState().view.type === "variant" && editable()
        : editable,
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
  tool("demolish", "KeyX", "перепланировка снос убрать стену"),
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
    run: () => {
      if (useEditor.getState().view.type === "compare") exitCompare();
      else sendToEngine({ type: "cancel" });
    },
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
    id: "file.importScan",
    title: "Импортировать скан…",
    group: "file",
    keywords: "lidar polycam roomplan glb облако точек обмер распознать",
    keys: [{ code: "KeyI", mod: true }],
    inInputs: true,
    enabled: notBusy,
    run: openScanWizard,
  },
  {
    id: "file.drawings",
    title: "Чертежи и экспорт…",
    group: "file",
    keywords: "листы pdf dxf svg png xlsx печать штамп пакет бригада экспорт",
    keys: [{ code: "KeyP", mod: true }],
    inInputs: true,
    enabled: notBusy,
    run: () => useEditor.getState().setDialog("drawings"),
  },
  {
    id: "file.estimate",
    title: "Смета…",
    group: "file",
    keywords: "смета стоимость прайс цены закупки материалы работы xlsx csv",
    keys: [{ code: "KeyE", mod: true, shift: true }],
    inInputs: true,
    enabled: notBusy,
    run: () => useEditor.getState().setDialog("estimate"),
  },
  {
    id: "file.addUnderlay",
    title: "Добавить подложку…",
    group: "file",
    keywords: "изображение картинка план скан png jpg",
    enabled: notBusy,
    run: () => void addUnderlay(),
  },
  {
    id: "view.render",
    title: "Фотореалистичный рендер…",
    group: "view",
    keywords: "рендер визуализация фото кадр картинка png трассировка",
    enabled: notBusy,
    run: () => useEditor.getState().setDialog("render"),
  },
  {
    id: "file.importDxf",
    title: "Импорт DXF…",
    group: "file",
    keywords: "dxf чертёж cad автокад подложка стены импорт план",
    enabled: notBusy,
    run: () => void pickDxf(),
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
    id: "furniture.rotate",
    title: "Повернуть мебель по часовой",
    group: "edit",
    keywords: "мебель поворот 90",
    keys: [{ code: "BracketRight" }],
    enabled: furnitureActive,
    run: () => sendToEngine({ type: "transformFurniture", rotate: 90, mirror: false }),
  },
  {
    id: "furniture.rotateBack",
    title: "Повернуть мебель против часовой",
    group: "edit",
    keywords: "мебель поворот 90",
    keys: [{ code: "BracketLeft" }],
    enabled: furnitureActive,
    run: () => sendToEngine({ type: "transformFurniture", rotate: -90, mirror: false }),
  },
  {
    id: "furniture.mirror",
    title: "Отразить мебель",
    group: "edit",
    keywords: "мебель зеркально",
    keys: [{ code: "KeyF" }],
    enabled: furnitureActive,
    run: () => sendToEngine({ type: "transformFurniture", rotate: 0, mirror: true }),
  },
  {
    id: "furniture.catalog",
    title: "Каталог мебели",
    group: "tools",
    keywords: "мебель расстановка кровать диван кухня",
    run: () => {
      useEditor.getState().setMode("furniture");
      useEditor.getState().setSidebarTab("catalog");
    },
  },
  {
    id: "view.toggle3d",
    title: "Переключить 2D / 3D",
    group: "view",
    keywords: "трёхмерный объём модель вид",
    keys: [{ code: "Digit3", alt: true }],
    enabled: () => useEditor.getState().view.type !== "compare",
    run: () => toggle3d(),
  },
  {
    id: "view.3dOrbit",
    title: "3D: орбита",
    group: "view",
    keywords: "вращение обзор сверху кукольный дом",
    enabled: in3d,
    run: () => set3d({ mode: "orbit" }),
  },
  {
    id: "view.3dWalk",
    title: "3D: прогулка",
    group: "view",
    keywords: "ходьба wasd первое лицо",
    enabled: in3d,
    run: () => set3d({ mode: "walk" }),
  },
  {
    id: "view.3dReset",
    title: "3D: показать всю квартиру",
    group: "view",
    keywords: "сброс камера вписать",
    enabled: in3d,
    run: () => sendToEngine({ type: "resetCamera3d" }),
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
