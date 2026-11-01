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
  "error.title": "Ошибка",

  "project.untitled": "Без имени",
  "project.unsaved": "Не сохранено",
  "project.saved": "Сохранено {time}",
  "project.notSavedYet": "Ещё не сохранялся",

  "recovery.title": "Восстановление",
  "recovery.untitled": "После прошлого сеанса остались несохранённые изменения. Восстановить их?",
  "recovery.file":
    "В проекте «{name}» есть изменения, которые не успели сохраниться. Восстановить их?",
  "recovery.restore": "Восстановить",
  "recovery.discard": "Не восстанавливать",

  "confirm.discardTitle": "Несохранённые изменения",
  "confirm.discard": "Проект ещё не сохранён в файл. Если продолжить, изменения пропадут.",
  "confirm.discardOk": "Продолжить без сохранения",
  "confirm.cancel": "Отмена",
} as const;

export type MessageKey = keyof typeof ru;
