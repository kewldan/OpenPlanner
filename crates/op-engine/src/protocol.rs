//! Протокол UI ↔ воркер. TS-типы генерируются через tsify.

use op_model::{Command, Document, Justification, WallMaterial};
use op_scan::{ScanAnswer, ScanParams, ScanResult};
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
    /// Демонтаж: щелчок по элементу обмера отмечает или снимает демонтаж.
    Demolish,
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
    /// Демонтаж: щелчок по стене, проёму или помещению обмера.
    DemolishPick,
    /// Перепланировка недоступна в этом виде (сравнение, обмер).
    Unavailable,
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
    /// Материал новых перегородок в варианте.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[tsify(optional)]
    pub material: Option<WallMaterial>,
}

impl Default for ToolOptions {
    fn default() -> Self {
        Self {
            wall_thickness: 120.0,
            justification: Justification::Center,
            material: None,
        }
    }
}

/// Что показывает холст.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum View {
    /// Обмер «как есть» — общий для всех вариантов.
    #[default]
    Survey,
    /// Вариант перепланировки.
    Variant {
        /// Вариант.
        id: String,
    },
    /// Два вида рядом (синхронный просмотр); `None` — обмер.
    Compare {
        /// Слева.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[tsify(optional)]
        left: Option<String>,
        /// Справа.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[tsify(optional)]
        right: Option<String>,
    },
}

/// Режим камеры 3D-вида.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub enum CameraMode {
    /// Орбита вокруг точки: обзор квартиры сверху и сбоку.
    #[default]
    Orbit,
    /// Прогулка на высоте глаз (WASD).
    Walk,
}

/// Дата и время для солнца.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct SunTime {
    /// День года, 1–365.
    pub day: u32,
    /// Местное время, часы (13,5 — 13:30).
    pub hour: f64,
}

/// Настройки 3D-вида (состояние окна, в документ не входят).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct Options3d {
    /// Холст показывает 3D.
    pub enabled: bool,
    /// Режим камеры.
    pub mode: CameraMode,
    /// Потолки (снаружи и сверху их не видно — рисуется только нижняя сторона).
    pub ceiling: bool,
    /// Белая модель вместо цветов материалов.
    pub white: bool,
    /// Срез стен по высоте, мм; без значения — стены целиком.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[tsify(optional)]
    pub cut: Option<f64>,
    /// Солнце с тенями; без значения — рассеянный свет.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[tsify(optional)]
    pub sun: Option<SunTime>,
}

impl Default for Options3d {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: CameraMode::Orbit,
            ceiling: true,
            white: false,
            cut: None,
            sun: None,
        }
    }
}

/// Действие с камерой 3D-вида.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CameraAction {
    /// Поворот: орбита вокруг точки или взгляд по сторонам; CSS-пиксели перетаскивания.
    Rotate {
        /// По горизонтали.
        dx: f64,
        /// По вертикали.
        dy: f64,
    },
    /// Сдвиг точки обзора в плоскости экрана; CSS-пиксели.
    Pan {
        /// По горизонтали.
        dx: f64,
        /// По вертикали.
        dy: f64,
    },
    /// Приближение (> 1) или отдаление; в прогулке — шаг вперёд или назад.
    Zoom {
        /// Множитель.
        factor: f64,
    },
    /// Перемещение по горизонтали относительно взгляда и по высоте, мм.
    Move {
        /// Вперёд.
        forward: f64,
        /// Вправо.
        right: f64,
        /// Вверх.
        up: f64,
    },
}

/// Камера 3D-вида для UI: точки в координатах плана (мм, Y вниз) и высота.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct CameraInfo {
    /// Режим.
    pub mode: CameraMode,
    /// Глаз: x, y, высота.
    pub eye: [f64; 3],
    /// Точка взгляда: x, y, высота.
    pub target: [f64; 3],
    /// Направление взгляда: градусы по часовой стрелке от верха листа.
    pub heading: f64,
}

/// Солнце для UI.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct SunInfo {
    /// Азимут от севера по часовой стрелке, градусы.
    pub azimuth: f64,
    /// Высота над горизонтом, градусы.
    pub altitude: f64,
}

/// Длина и площадь стен одного материала.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct MaterialAmount {
    /// Материал (без значения — не указан).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[tsify(optional)]
    pub material: Option<WallMaterial>,
    /// Длина по оси, м.
    pub length: f64,
    /// Площадь (длина × высота), м².
    pub area: f64,
}

/// Площадь помещения в виде.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct RoomArea {
    /// Помещение.
    pub id: String,
    /// Название.
    pub name: String,
    /// Площадь, м² (0 — контур не замкнут).
    pub area: f64,
}

/// Сводка вида: площади и изменения относительно обмера.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct ViewSummary {
    /// Вариант (`None` — обмер).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[tsify(optional)]
    pub variant: Option<String>,
    /// Название.
    pub name: String,
    /// Помещения.
    pub rooms: Vec<RoomArea>,
    /// Общая площадь помещений, м².
    pub total_area: f64,
    /// Демонтируемые стены: длина по оси, м.
    pub demolished_length: f64,
    /// Демонтируемые стены: площадь, м².
    pub demolished_area: f64,
    /// Новые стены по материалам.
    pub new_walls: Vec<MaterialAmount>,
    /// Новые проёмы.
    pub openings_added: u32,
    /// Заложенные проёмы.
    pub openings_closed: u32,
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
    /// Показать обмер, вариант или сравнение.
    SetView {
        /// Вид.
        view: View,
    },
    /// Новый вариант (копия другого или пустой) — становится текущим.
    CreateVariant {
        /// Название.
        name: String,
        /// Скопировать вариант.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[tsify(optional)]
        copy_of: Option<String>,
    },
    /// Удалить вариант вместе с его элементами.
    DeleteVariant {
        /// Вариант.
        id: String,
    },
    /// Настройки 3D-вида: включение, режим камеры, потолки, белая модель, срез, солнце.
    Set3d {
        /// Настройки.
        options: Options3d,
    },
    /// Действие с камерой 3D-вида.
    Camera3d {
        /// Действие.
        action: CameraAction,
    },
    /// Показать всю квартиру в 3D (орбита).
    ResetCamera3d,
    /// Сохранить текущий ракурс в проекте.
    SaveView3d {
        /// Название.
        name: String,
    },
    /// Перейти к сохранённому ракурсу.
    ApplyView3d {
        /// Ракурс.
        id: String,
    },
    /// Отметить или снять демонтаж элементов обмера в текущем варианте.
    SetDemolished {
        /// Элементы.
        ids: Vec<String>,
        /// Демонтировать.
        demolished: bool,
    },
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
        document: Box<Document>,
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
    /// Изменился вид холста (обмер, вариант, сравнение).
    ViewModeChanged {
        /// Вид.
        view: View,
    },
    /// Сводка обмера и всех вариантов: площади и изменения.
    VariantsSummary {
        /// Обмер первым, затем варианты по порядку.
        views: Vec<ViewSummary>,
    },
    /// Изменились настройки или камера 3D-вида.
    View3dChanged {
        /// Настройки.
        options: Options3d,
        /// Камера.
        camera: CameraInfo,
        /// Солнце (если включено), в том числе за горизонтом.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[tsify(optional)]
        sun: Option<SunInfo>,
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

/// Формат экспорта чертежей (PNG растеризует UI из SVG листов).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub enum DrawingFormat {
    /// Векторный PDF со слоями: все листы в одном файле.
    Pdf,
    /// SVG по листу; несколько листов — ZIP.
    Svg,
    /// DXF плана по слоям, по листу; несколько листов — ZIP.
    Dxf,
    /// Ведомости и экспликации в XLSX.
    Xlsx,
    /// «Пакет для бригады»: PDF чертежей и XLSX ведомостей в ZIP.
    Bundle,
}

/// Запрос комплекта чертежей.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct DrawingRequest {
    /// Вариант перепланировки; без значения — только обмер.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[tsify(optional)]
    pub variant: Option<String>,
    /// Дата для основной надписи (ММ.ГГ).
    pub date: String,
}

/// Лист комплекта для предпросмотра.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct SheetPreview {
    /// Название листа.
    pub title: String,
    /// Ширина листа, мм.
    pub width: f64,
    /// Высота листа, мм.
    pub height: f64,
    /// Лист в SVG, текст — контурами (одинаково выглядит везде, годится для PNG).
    pub svg: String,
}

/// Комплект чертежей для окна «Чертежи».
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct DrawingSetInfo {
    /// Листы по порядку.
    pub sheets: Vec<SheetPreview>,
    /// Знаменатель масштаба планов.
    pub scale: u32,
    /// Масштаб подобран автоматически.
    pub auto_scale: bool,
    /// Предупреждения (план не помещается, план пуст).
    pub warnings: Vec<String>,
    /// Основа имён файлов экспорта.
    pub name: String,
    /// Имена файлов экспорта по форматам.
    pub file_names: ExportNames,
}

/// Имена файлов экспорта (несколько листов SVG или DXF — ZIP).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct ExportNames {
    /// PDF.
    pub pdf: String,
    /// SVG или ZIP с ними.
    pub svg: String,
    /// DXF или ZIP с ними.
    pub dxf: String,
    /// XLSX.
    pub xlsx: String,
    /// «Пакет для бригады».
    pub bundle: String,
}

/// Что импортировать из скана.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub enum ScanImportMode {
    /// Стены, проёмы, помещения и подложка.
    Plan,
    /// Только вид сверху подложкой — обвести вручную.
    UnderlayOnly,
}

/// Импорт распознанного скана в проект.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct ScanImport {
    /// Результат распознавания.
    pub result: ScanResult,
    /// Ответы на вопросы проверки.
    pub answers: Vec<ScanAnswer>,
    /// Параметры.
    pub params: ScanParams,
    /// Что импортировать.
    pub mode: ScanImportMode,
    /// Имя исходного файла.
    pub name: String,
}
