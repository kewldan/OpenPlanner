// Русская локаль. Ключи сгруппированы по экранам; параметры — {name}.
export const ru = {
  "app.name": "OpenPlanner",
  "project.untitledVariant": "Вариант А · как есть",

  "mode.label": "Режим редактора",
  "mode.measure": "Обмер",
  "mode.replan": "Перепланировка",
  "mode.furniture": "Мебель",
  "mode.electric": "Электрика",
  "mode.light": "Освещение",
  "mode.plumbing": "Сантехника",
  "mode.finish": "Отделка",

  "view.label": "Вид",
  "view.2d": "2D",
  "view.3d": "3D",

  "action.drawings": "Чертежи",
  "action.estimate": "Смета",
  "action.export": "Экспорт",
  "action.undo": "Отменить",
  "action.redo": "Повторить",
  "action.zoomToFit": "Показать весь план",

  "sidebar.objects": "Объекты",
  "sidebar.layers": "Слои",
  "sidebar.empty": "Пока пусто. Инструменты обмера появятся на стадии 1.",

  "inspector.project": "Проект",
  "inspector.title": "Название",
  "inspector.address": "Адрес",
  "inspector.ceiling": "Потолок",

  "unit.mm": "мм",

  "status.scale": "1:{denominator}",
  "status.zoom": "{percent} %",
  "status.backend": "GPU: {backend}",
  "status.starting": "Запуск движка…",
  "status.hint": "Прокрутка — сдвиг, ⌘ + прокрутка или щипок — масштаб, средняя кнопка — сдвиг",

  "error.engine": "Движок не запустился: {message}",
} as const;

export type MessageKey = keyof typeof ru;
