//! Протокол UI ↔ воркер. TS-типы генерируются через tsify.

use op_model::{
    Command, DeviceKind, Document, Justification, Material, MaterialKind, PipeSystem, PlumbingKind,
    PriceKind, WallMaterial,
};
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
    /// Мебель: позиция каталога ставится щелчком, у стены — спинкой к ней.
    Furniture,
    /// Электроточка или светильник: на стене — на её грани, иначе — где щёлкнули.
    Device,
    /// Сантехническая точка: вывод — на грани стены, стояк и коллектор — где щёлкнули.
    Plumbing,
    /// Труба: вершины щелчками, двойной щелчок или Enter — конец.
    Pipe,
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
    /// Мебель: щелчок ставит предмет.
    FurniturePlace,
    /// Электроточка: щелчок ставит точку.
    DevicePlace,
    /// Сантехника: щелчок ставит точку.
    PlumbingPlace,
    /// Труба: первая вершина.
    PipeStart,
    /// Труба: следующая вершина, двойной щелчок или Enter — конец.
    PipeNext,
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

/// Позиция каталога мебели для UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    /// Идентификатор.
    pub id: String,
    /// Название.
    pub name: String,
    /// Раздел.
    pub category: String,
    /// Ширина, мм.
    pub width: f64,
    /// Глубина, мм.
    pub depth: f64,
    /// Высота, мм.
    pub height: f64,
    /// Низ над полом, мм.
    pub elevation: f64,
    /// Слова для поиска.
    pub keywords: String,
    /// Обозначение в квадрате 100 × 100: путь SVG.
    pub icon: String,
}

/// Позиция каталога электроточек для UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct DeviceEntry {
    /// Вид.
    pub kind: DeviceKind,
    /// Название.
    pub name: String,
    /// Раздел: «Розетки», «Выключатели», «Светильники»…
    pub group: String,
    /// Светильник или выключатель — показывается в режиме «Освещение».
    pub lighting: bool,
    /// Климат — показывается в режиме «Климат».
    pub climate: bool,
    /// Ставится на стену (иначе — на потолок или пол).
    pub on_wall: bool,
    /// Высота установки по умолчанию, мм.
    pub height: f64,
    /// Мощность по умолчанию, Вт.
    pub power: f64,
    /// Световой поток по умолчанию, лм.
    pub flux: f64,
    /// Условное обозначение в квадрате 100 × 100: путь SVG.
    pub icon: String,
}

/// Позиция каталога сантехники для UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct PlumbingEntry {
    /// Идентификатор позиции.
    pub id: String,
    /// Вид.
    pub kind: PlumbingKind,
    /// Система.
    pub system: PipeSystem,
    /// Название.
    pub name: String,
    /// Раздел.
    pub group: String,
    /// Отметка по умолчанию, мм.
    pub height: f64,
    /// Диаметр по умолчанию, мм.
    pub diameter: f64,
    /// Ставится на стену.
    pub on_wall: bool,
    /// Обозначение в квадрате 100 × 100: путь SVG.
    pub icon: String,
}

/// Расчёт трубы.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct PipeReport {
    /// Труба.
    pub id: String,
    /// Система.
    pub system: PipeSystem,
    /// Диаметр, мм.
    pub diameter: f64,
    /// Длина по оси с подъёмами, м.
    pub length: f64,
    /// Наименьший уклон горизонтальных участков (канализация).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[tsify(optional)]
    pub slope: Option<f64>,
    /// Что обслуживает: назначение вывода.
    pub label: String,
    /// Нарушения, понятные пользователю.
    pub warnings: Vec<String>,
}

/// Строка спецификации: труба, фитинг, арматура.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct BillRow {
    /// Наименование.
    pub name: String,
    /// Единица.
    pub unit: String,
    /// Количество.
    pub amount: f64,
    /// Примечание: хлысты, упаковки.
    pub note: String,
}

/// Расчёт сантехники текущего вида.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct PlumbingReport {
    /// Трубы.
    pub pipes: Vec<PipeReport>,
    /// Трубы и фитинги.
    pub bill: Vec<BillRow>,
    /// Выводов без трубы.
    pub unconnected: u32,
    /// Общие предупреждения: доступ к ревизии, нет стояка.
    pub warnings: Vec<String>,
}

/// Грань помещения (стена по развёртке).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct FaceInfo {
    /// Буква стены.
    pub letter: String,
    /// Длина, мм.
    pub length: f64,
}

/// Поверхности помещения.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct RoomSurfaces {
    /// Помещение.
    pub id: String,
    /// Название.
    pub name: String,
    /// Пол, м².
    pub floor: f64,
    /// Потолок, м².
    pub ceiling: f64,
    /// Периметр, м.
    pub perimeter: f64,
    /// Высота, м.
    pub height: f64,
    /// Стены без вычета проёмов, м².
    pub walls_gross: f64,
    /// Стены за вычетом проёмов, м².
    pub walls_net: f64,
    /// Грани.
    pub faces: Vec<FaceInfo>,
}

/// Количество материала.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct Quantity {
    /// Что: «Плитка», «Клей», «Затирка».
    pub name: String,
    /// Количество.
    pub amount: f64,
    /// Единица.
    pub unit: String,
    /// Упаковок (0 — не считается).
    pub packs: u32,
}

/// Расчёт отделки поверхности.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct FinishInfo {
    /// Отделка.
    pub id: String,
    /// Площадь, м².
    pub area: f64,
    /// Целых элементов.
    pub whole: u32,
    /// Подрезанных.
    pub cut: u32,
    /// Узких подрезок.
    pub narrow: u32,
    /// Расход.
    pub quantities: Vec<Quantity>,
    /// Предупреждения.
    pub warnings: Vec<String>,
}

/// Итог по материалу.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct MaterialTotal {
    /// Материал.
    pub material: String,
    /// Название.
    pub name: String,
    /// Вид.
    pub kind: MaterialKind,
    /// Площадь, м².
    pub area: f64,
    /// Расход.
    pub quantities: Vec<Quantity>,
}

/// Расчёт отделки текущего вида.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct FinishReport {
    /// Поверхности помещений.
    pub rooms: Vec<RoomSurfaces>,
    /// Отделка.
    pub finishes: Vec<FinishInfo>,
    /// Итоги по материалам.
    pub totals: Vec<MaterialTotal>,
}

/// Слой чертежа DXF.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct DxfLayer {
    /// Название.
    pub name: String,
    /// Линий на слое.
    pub paths: u32,
}

/// Сведения о чертеже DXF перед импортом.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct DxfInfo {
    /// Слои с линиями.
    pub layers: Vec<DxfLayer>,
    /// Ширина, мм.
    pub width: f64,
    /// Высота, мм.
    pub height: f64,
}

/// Как импортировать DXF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub enum DxfMode {
    /// Подложкой: линии растром в масштабе чертежа.
    Underlay,
    /// Стенами: отрезки линий — оси стен.
    Walls,
}

/// Параметры импорта DXF.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct DxfImport {
    /// Подложкой или стенами.
    pub mode: DxfMode,
    /// Слои (пусто — все).
    pub layers: Vec<String>,
    /// Толщина стен, мм.
    pub thickness: f64,
}

/// Зона тёплого пола с расчётом.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct HeatingInfo {
    /// Зона.
    pub id: String,
    /// Помещение.
    pub room: String,
    /// Название помещения.
    pub room_name: String,
    /// Площадь помещения, м².
    pub area: f64,
    /// Площадь обогрева (без мебели на полу и отступа от стен), м².
    pub heated: f64,
    /// Мощность, Вт.
    pub power: f64,
    /// Терморегулятор.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[tsify(optional)]
    pub thermostat: Option<String>,
}

/// Климат помещения: воздухообмен и охлаждение.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct RoomClimate {
    /// Помещение.
    pub id: String,
    /// Название.
    pub name: String,
    /// Площадь, м².
    pub area: f64,
    /// Нужный воздухообмен, м³/ч.
    pub air_required: f64,
    /// Приток и вытяжка установленными устройствами, м³/ч.
    pub air_provided: f64,
    /// Рекомендуемая мощность охлаждения, кВт.
    pub cooling: f64,
    /// Внутренних блоков кондиционеров.
    pub ac_units: u32,
}

/// Климат текущего вида: тёплые полы, воздухообмен, охлаждение.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct ClimateReport {
    /// Зоны тёплого пола.
    pub zones: Vec<HeatingInfo>,
    /// Помещения.
    pub rooms: Vec<RoomClimate>,
    /// Предупреждения.
    pub warnings: Vec<String>,
}

/// Строка сметы: позиция прайса в помещении (или без помещения).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct EstimateLine {
    /// Позиция прайса.
    pub item: String,
    /// Помещение (без значения — общая строка).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[tsify(optional)]
    pub room: Option<String>,
    /// Название помещения или «Общие работы».
    pub room_name: String,
    /// Раздел.
    pub section: String,
    /// Код.
    pub code: String,
    /// Наименование.
    pub name: String,
    /// Работа или материал.
    pub kind: PriceKind,
    /// Единица.
    pub unit: String,
    /// Объём по модели (до нормы и запаса).
    pub measured: f64,
    /// Количество.
    pub quantity: f64,
    /// Цена, ₽.
    pub price: f64,
    /// Сумма, ₽.
    pub cost: f64,
    /// Количество задано вручную.
    pub overridden: bool,
}

/// Итог группы сметы (раздел или помещение).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct EstimateGroup {
    /// Название.
    pub name: String,
    /// Работы, ₽.
    pub works: f64,
    /// Материалы, ₽.
    pub materials: f64,
    /// Всего, ₽.
    pub total: f64,
}

/// Строка списка закупок.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct Purchase {
    /// Позиция прайса.
    pub item: String,
    /// Код.
    pub code: String,
    /// Наименование.
    pub name: String,
    /// Единица.
    pub unit: String,
    /// Количество.
    pub quantity: f64,
    /// Цена, ₽.
    pub price: f64,
    /// Сумма, ₽.
    pub cost: f64,
}

/// Смета текущего вида.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct EstimateReport {
    /// Строки в порядке прайса.
    pub lines: Vec<EstimateLine>,
    /// Итоги по разделам.
    pub sections: Vec<EstimateGroup>,
    /// Итоги по помещениям.
    pub rooms: Vec<EstimateGroup>,
    /// Материалы к закупке.
    pub purchases: Vec<Purchase>,
    /// Работы, ₽.
    pub works: f64,
    /// Материалы, ₽.
    pub materials: f64,
    /// Всего, ₽.
    pub total: f64,
}

/// Формат таблицы для экспорта сметы и прайса.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub enum TableFormat {
    /// Книга Excel.
    Xlsx,
    /// CSV (разделитель «;», UTF-8).
    Csv,
}

/// Расчёт группы щита.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct CircuitReport {
    /// Группа.
    pub id: String,
    /// Обозначение.
    pub name: String,
    /// Назначение.
    pub purpose: String,
    /// Электроточек в группе.
    pub devices: u32,
    /// Установленная мощность, Вт.
    pub installed: f64,
    /// Расчётная мощность, Вт.
    pub demand: f64,
    /// Расчётный ток, А.
    pub current: f64,
    /// Длина кабеля по трассе с запасом, м.
    pub length: f64,
    /// Падение напряжения, %.
    pub drop: f64,
    /// Допустимый ток кабеля, А.
    pub ampacity: f64,
    /// Нарушения, понятные пользователю.
    pub warnings: Vec<String>,
}

/// Аппарат щита.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct PanelItemInfo {
    /// Название.
    pub name: String,
    /// Модулей DIN на аппарат.
    pub modules: u32,
    /// Количество.
    pub count: u32,
}

/// Состав щита.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct PanelReport {
    /// Аппараты.
    pub items: Vec<PanelItemInfo>,
    /// Занято модулей.
    pub modules: u32,
    /// Корпус, модулей.
    pub enclosure: u32,
    /// Ввод трёхфазный.
    pub three_phase: bool,
    /// Вводной автомат, А.
    pub main_breaker: f64,
}

/// Освещённость помещения.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct RoomLighting {
    /// Помещение.
    pub id: String,
    /// Название.
    pub name: String,
    /// Площадь, м².
    pub area: f64,
    /// Световой поток светильников, лм.
    pub flux: f64,
    /// Средняя освещённость, лк.
    pub lux: f64,
    /// Норма, лк.
    pub norm: f64,
}

/// Расчёт электрики текущего вида.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct ElectricalReport {
    /// Группы.
    pub circuits: Vec<CircuitReport>,
    /// Щит.
    pub panel: PanelReport,
    /// Освещённость по помещениям.
    pub rooms: Vec<RoomLighting>,
    /// Потребители без группы.
    pub unassigned: u32,
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
    /// Стоимость по смете, ₽.
    pub cost: f64,
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
    /// Инструмент «Мебель» с позицией каталога.
    FurnitureTool {
        /// Позиция каталога.
        item: String,
    },
    /// Инструмент «Электроточка» с видом точки.
    DeviceTool {
        /// Вид.
        kind: DeviceKind,
    },
    /// Распределить электроточки по группам щита (группы создаются заново).
    AutoCircuits,
    /// Инструмент «Сантехника» с позицией каталога.
    PlumbingTool {
        /// Позиция каталога.
        item: String,
    },
    /// Инструмент «Труба».
    PipeTool {
        /// Система.
        system: PipeSystem,
        /// Диаметр, мм.
        diameter: f64,
    },
    /// Поставить выводы под приборы (ванна, раковина, унитаз…) без выводов.
    FixtureOutlets,
    /// Проложить трубы от выводов к стоякам и коллекторам (автоматические трубы — заново).
    RoutePipes,
    /// Подобрать старт раскладки отделки с наименьшим числом узких подрезок.
    OptimizeLayout {
        /// Отделка.
        id: String,
    },
    /// Добавить в прайс проекта позиции типового прайса, которых в нём нет (по коду).
    DefaultPrices,
    /// Перейти на этаж (`None` — первый).
    SetLevel {
        /// Этаж.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[tsify(optional)]
        level: Option<String>,
    },
    /// Добавить этаж над верхним и перейти на него.
    AddLevel {
        /// Название.
        name: String,
    },
    /// Удалить этаж вместе со всем, что на нём стоит.
    DeleteLevel {
        /// Этаж.
        id: String,
    },
    /// Показывать раскладку отделки пола на плане.
    ShowFinishes {
        /// Показывать.
        show: bool,
    },
    /// Повернуть и (или) отразить выделенную мебель или предмет инструмента.
    TransformFurniture {
        /// Поворот по часовой стрелке, градусы.
        rotate: f64,
        /// Отразить по ширине.
        mirror: bool,
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
    /// Каталог мебели (один раз при запуске).
    Catalog {
        /// Позиции.
        items: Vec<CatalogEntry>,
    },
    /// Каталог электроточек и светильников (один раз при запуске).
    DeviceCatalog {
        /// Позиции.
        items: Vec<DeviceEntry>,
    },
    /// Расчёт электрики и освещённости текущего вида.
    Electrical {
        /// Расчёт.
        report: ElectricalReport,
    },
    /// Каталог сантехнических точек (один раз при запуске).
    PlumbingCatalog {
        /// Позиции.
        items: Vec<PlumbingEntry>,
    },
    /// Расчёт сантехники текущего вида.
    Plumbing {
        /// Расчёт.
        report: PlumbingReport,
    },
    /// Библиотека отделочных материалов (один раз при запуске).
    MaterialLibrary {
        /// Материалы.
        items: Vec<Material>,
    },
    /// Поверхности помещений и расчёт отделки текущего вида.
    Finishes {
        /// Расчёт.
        report: FinishReport,
    },
    /// Климат текущего вида: тёплые полы, вентиляция, кондиционирование.
    Climate {
        /// Расчёт.
        report: ClimateReport,
    },
    /// Сменился текущий этаж.
    LevelChanged {
        /// Этаж (`None` — первый).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[tsify(optional)]
        level: Option<String>,
    },
    /// Смета текущего вида.
    Estimate {
        /// Смета.
        report: EstimateReport,
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
    /// Модель здания в IFC 4 (для BIM-программ).
    Ifc,
}

/// Запрос комплекта чертежей.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
pub struct DrawingRequest {
    /// Вариант перепланировки; без значения — только обмер.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[tsify(optional)]
    pub variant: Option<String>,
    /// Этаж; без значения — первый.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[tsify(optional)]
    pub level: Option<String>,
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
    /// IFC 4.
    pub ifc: String,
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
