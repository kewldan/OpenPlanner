//! Протокол UI ↔ воркер. TS-типы генерируются через tsify.

use op_model::{Command, Document};
use serde::{Deserialize, Serialize};
use tsify::Tsify;

/// Сообщение от UI движку.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ToEngine {
    /// Размер холста изменился.
    Resize {
        /// Ширина, CSS-пиксели.
        width: f64,
        /// Высота, CSS-пиксели.
        height: f64,
        /// Отношение физических пикселей к CSS-пикселям.
        device_pixel_ratio: f64,
    },
    /// Сдвиг плана на экранный вектор, CSS-пиксели.
    Pan {
        /// Сдвиг по X.
        dx: f64,
        /// Сдвиг по Y.
        dy: f64,
    },
    /// Масштабирование относительно точки холста.
    Zoom {
        /// X точки, CSS-пиксели.
        x: f64,
        /// Y точки, CSS-пиксели.
        y: f64,
        /// Множитель масштаба (> 1 — приблизить).
        factor: f64,
    },
    /// Показать весь план (пока — вернуть камеру по умолчанию).
    ZoomToFit,
    /// Выполнить команду над документом.
    Command {
        /// Команда.
        command: Command,
    },
    /// Отменить последнее действие.
    Undo,
    /// Повторить отменённое действие.
    Redo,
    /// Начать новый пустой проект.
    NewDocument,
    /// Текущее состояние записано в файл.
    MarkSaved,
}

/// Событие движка для UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum FromEngine {
    /// Документ изменился. На стадии 0 передаётся целиком — он маленький.
    DocumentChanged {
        /// Текущий документ.
        document: Document,
    },
    /// Изменилось состояние истории.
    HistoryChanged {
        /// Можно отменить.
        can_undo: bool,
        /// Можно повторить.
        can_redo: bool,
        /// Есть несохранённые изменения.
        dirty: bool,
    },
    /// Строка для журнала автосохранения (с переводом строки в конце).
    ///
    /// UI дописывает её в журнал проекта как есть.
    Journal {
        /// Строка JSON Lines.
        line: String,
    },
    /// Изменился вид холста.
    ViewChanged {
        /// Масштаб чертежа: 1:N при текущем зуме, где 1 мм экрана ≈ N мм плана.
        scale_denominator: f64,
        /// Зум в процентах относительно 1:50.
        zoom_percent: f64,
    },
    /// Ошибка выполнения сообщения; текст понятен пользователю.
    Error {
        /// Сообщение об ошибке.
        message: String,
    },
}

/// Пачка событий в ответ на одно сообщение.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, Tsify)]
pub struct EngineEvents {
    /// События в порядке возникновения.
    pub events: Vec<FromEngine>,
}
