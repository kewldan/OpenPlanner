//! Протокол UI ↔ воркер. TS-типы генерируются через tsify.

use op_model::{Command, Document, Justification};
use serde::{Deserialize, Serialize};
use tsify::Tsify;

/// Инструмент холста.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub enum Tool {
    /// Выбор и перетаскивание.
    #[default]
    Select,
    /// Стены цепочкой.
    Wall,
    /// Дверь.
    Door,
    /// Окно.
    Window,
    /// Арка.
    Arch,
    /// Помещение по замкнутому контуру.
    Room,
    /// Размер.
    Dimension,
    /// Выноска.
    Leader,
    /// Калибровка масштаба подложки.
    Calibrate,
}

/// Шаг инструмента — по нему UI показывает подсказку.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub enum ToolStep {
    /// Выбор: ничего не происходит.
    Idle,
    /// Стена: первая точка.
    WallStart,
    /// Стена: следующая точка или длина с клавиатуры.
    WallNext,
    /// Проём: щелчок по стене.
    OpeningPlace,
    /// Помещение: щелчок внутри замкнутого контура.
    RoomPlace,
    /// Размер: первая точка.
    DimensionStart,
    /// Размер: вторая точка.
    DimensionEnd,
    /// Размер: положение размерной линии.
    DimensionOffset,
    /// Выноска: точка, на которую указывает.
    LeaderTarget,
    /// Выноска: место текста.
    LeaderText,
    /// Калибровка: первая точка на подложке.
    CalibrateFirst,
    /// Калибровка: вторая точка.
    CalibrateSecond,
    /// Калибровка: настоящее расстояние с клавиатуры.
    CalibrateDistance,
}

/// Фаза ввода указателя (левая кнопка).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub enum PointerPhase {
    /// Нажатие.
    Down,
    /// Движение (с нажатой кнопкой или без).
    Move,
    /// Отпускание.
    Up,
    /// Двойной щелчок.
    DoubleClick,
    /// Указатель ушёл с холста.
    Leave,
}

/// Вид привязки курсора.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub enum SnapKind {
    /// Конец стены (узел).
    Endpoint,
    /// Середина стены.
    Midpoint,
    /// Перпендикуляр к стене.
    Perpendicular,
    /// Точка на оси стены.
    OnWall,
    /// Выравнивание по другому узлу или углу.
    Axis,
    /// Узел сетки.
    Grid,
}

/// Параметры новых стен.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct ToolOptions {
    /// Толщина новых стен, мм.
    pub wall_thickness: f64,
    /// Линия привязки новых стен.
    pub justification: Justification,
}

impl Default for ToolOptions {
    fn default() -> Self {
        Self {
            wall_thickness: 120.0,
            justification: Justification::Center,
        }
    }
}

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
    /// Показать весь план.
    ZoomToFit,
    /// Масштаб 1:50.
    ZoomToScale {
        /// Знаменатель масштаба.
        denominator: f64,
    },
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
    /// Выбрать инструмент.
    SetTool {
        /// Инструмент.
        tool: Tool,
    },
    /// Параметры новых стен.
    SetToolOptions {
        /// Параметры.
        options: ToolOptions,
    },
    /// Ввод указателя на холсте.
    Pointer {
        /// Фаза.
        phase: PointerPhase,
        /// X, CSS-пиксели от левого края холста.
        x: f64,
        /// Y, CSS-пиксели от верхнего края.
        y: f64,
        /// Shift: ортогональность при рисовании, добавление к выделению.
        shift: bool,
        /// Alt: без привязок.
        alt: bool,
    },
    /// Esc: прервать шаг инструмента; без шага — вернуться к выбору.
    Cancel,
    /// Enter: закончить цепочку.
    Finish,
    /// Число с клавиатуры: длина стены (и угол) или расстояние калибровки, мм.
    NumericInput {
        /// Длина, мм.
        value: f64,
        /// Угол, градусы против часовой стрелки от оси X.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[tsify(optional)]
        angle: Option<f64>,
    },
    /// Выделить элементы.
    Select {
        /// Идентификаторы.
        ids: Vec<String>,
    },
    /// Выделить всё на видимых и незаблокированных слоях.
    SelectAll,
    /// Удалить выделенное.
    DeleteSelection,
    /// Изменить длину стены, сдвинув её конечный узел.
    SetWallLength {
        /// Стена.
        id: String,
        /// Новая длина, мм.
        length: f64,
    },
    /// Создать помещения во всех замкнутых контурах без помещения.
    FindRooms,
}

/// Помещение с вычисленными величинами.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct RoomInfo {
    /// Помещение.
    pub id: String,
    /// Контур замкнут (точка подписи внутри области стен).
    pub closed: bool,
    /// Площадь пола, м².
    pub area: f64,
    /// Периметр по стенам, м.
    pub perimeter: f64,
}

/// Длина элемента (стены по оси, размера).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct LengthInfo {
    /// Элемент.
    pub id: String,
    /// Длина, мм.
    pub length: f64,
}

/// Событие движка для UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum FromEngine {
    /// Документ изменился. Передаётся целиком (без двоичных файлов).
    DocumentChanged {
        /// Текущий документ.
        document: Document,
    },
    /// Вычисленные величины: площади помещений, длины стен и размеров.
    DerivedChanged {
        /// Помещения.
        rooms: Vec<RoomInfo>,
        /// Стены.
        walls: Vec<LengthInfo>,
        /// Размеры.
        dimensions: Vec<LengthInfo>,
        /// Количество замкнутых контуров без помещения.
        free_regions: u32,
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
    /// Изменилось выделение.
    SelectionChanged {
        /// Выделенные элементы.
        ids: Vec<String>,
    },
    /// Изменился инструмент или его шаг.
    ToolChanged {
        /// Инструмент.
        tool: Tool,
        /// Шаг.
        step: ToolStep,
        /// Параметры новых стен.
        options: ToolOptions,
    },
    /// Положение курсора на плане (после привязки).
    Cursor {
        /// X, мм.
        x: f64,
        /// Y, мм.
        y: f64,
        /// Сработавшая привязка.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[tsify(optional)]
        snap: Option<SnapKind>,
        /// Длина рисуемого отрезка, мм.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[tsify(optional)]
        length: Option<f64>,
        /// Угол рисуемого отрезка, градусы против часовой от оси X.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[tsify(optional)]
        angle: Option<f64>,
    },
    /// Новый элемент ждёт ввода текста (выноска): UI ставит фокус в поле.
    EditText {
        /// Элемент.
        id: String,
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
