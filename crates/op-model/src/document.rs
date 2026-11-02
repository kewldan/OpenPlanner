use std::{collections::BTreeMap, sync::Arc};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Сведения о проекте, которые попадают в основную надпись чертежей.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct ProjectMeta {
    /// Название проекта, например «Комендантский пр.».
    pub title: String,
    /// Адрес объекта.
    pub address: String,
    /// Обозначение (шифр) комплекта чертежей.
    #[serde(default)]
    pub designation: String,
    /// Организация или исполнитель.
    #[serde(default)]
    pub organization: String,
    /// Стадия проектирования («Р» — рабочая документация).
    #[serde(default)]
    pub stage: String,
    /// Разработал.
    #[serde(default)]
    pub developer: String,
    /// Проверил.
    #[serde(default)]
    pub checker: String,
}

/// Формат листа.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub enum PaperFormat {
    /// A3 альбомный, 420 × 297 мм.
    #[default]
    A3Landscape,
    /// A3 книжный, 297 × 420 мм.
    A3Portrait,
    /// A4 альбомный, 297 × 210 мм.
    A4Landscape,
    /// A4 книжный, 210 × 297 мм.
    A4Portrait,
}

impl PaperFormat {
    /// Все форматы.
    pub const ALL: [Self; 4] = [
        Self::A3Landscape,
        Self::A3Portrait,
        Self::A4Landscape,
        Self::A4Portrait,
    ];

    /// Ширина и высота, мм.
    #[must_use]
    pub fn size(self) -> (f64, f64) {
        match self {
            Self::A3Landscape => (420.0, 297.0),
            Self::A3Portrait => (297.0, 420.0),
            Self::A4Landscape => (297.0, 210.0),
            Self::A4Portrait => (210.0, 297.0),
        }
    }
}

/// Настройки комплекта чертежей.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct DrawingSettings {
    /// Формат листов.
    #[serde(default)]
    pub paper: PaperFormat,
    /// Знаменатель масштаба (50 — 1:50); без значения — подбирается по размеру плана.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub scale: Option<u32>,
}

/// Привязка квартиры к местности — для положения солнца.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct Site {
    /// Направление на север на плане: градусы по часовой стрелке от верха листа.
    pub north: f64,
    /// Широта, градусы (северная — положительная).
    pub latitude: f64,
    /// Долгота, градусы (восточная — положительная).
    pub longitude: f64,
    /// Часовой пояс: смещение от UTC, часы.
    pub utc_offset: f64,
}

impl Default for Site {
    /// Москва, север — вверх листа.
    fn default() -> Self {
        Self {
            north: 0.0,
            latitude: 55.75,
            longitude: 37.62,
            utc_offset: 3.0,
        }
    }
}

/// Сохранённый ракурс 3D-вида. Точки — в координатах плана (мм, Y вниз) и высота от пола.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct SavedView {
    /// Идентификатор.
    pub id: String,
    /// Название.
    pub name: String,
    /// Прогулка (иначе — орбита).
    #[serde(default)]
    pub walk: bool,
    /// Глаз: x, y, высота.
    pub eye: [f64; 3],
    /// Точка взгляда: x, y, высота.
    pub target: [f64; 3],
}

/// Системный слой: в него попадают новые элементы своего типа.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub enum LayerKind {
    /// Стены.
    Walls,
    /// Окна, двери, арки.
    Openings,
    /// Помещения.
    Rooms,
    /// Размеры.
    Dimensions,
    /// Выноски.
    Leaders,
    /// Подложки.
    Underlays,
    /// Скан: вид сверху импортированного скана.
    Scan,
    /// Мебель.
    Furniture,
    /// Электрика и освещение.
    Electrical,
    /// Сантехника: выводы, стояки, трубы.
    Plumbing,
}

impl LayerKind {
    /// Все системные слои в порядке по умолчанию (первый — снизу).
    pub const ALL: [Self; 10] = [
        Self::Scan,
        Self::Underlays,
        Self::Rooms,
        Self::Furniture,
        Self::Walls,
        Self::Openings,
        Self::Plumbing,
        Self::Electrical,
        Self::Dimensions,
        Self::Leaders,
    ];

    /// Идентификатор системного слоя.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Walls => "layer-walls",
            Self::Openings => "layer-openings",
            Self::Rooms => "layer-rooms",
            Self::Dimensions => "layer-dimensions",
            Self::Leaders => "layer-leaders",
            Self::Underlays => "layer-underlays",
            Self::Scan => "layer-scan",
            Self::Furniture => "layer-furniture",
            Self::Electrical => "layer-electrical",
            Self::Plumbing => "layer-plumbing",
        }
    }

    /// Название системного слоя по умолчанию.
    #[must_use]
    pub fn default_name(self) -> &'static str {
        match self {
            Self::Walls => "Стены",
            Self::Openings => "Проёмы",
            Self::Rooms => "Помещения",
            Self::Dimensions => "Размеры",
            Self::Leaders => "Выноски",
            Self::Underlays => "Подложка",
            Self::Scan => "Скан",
            Self::Furniture => "Мебель",
            Self::Electrical => "Электрика",
            Self::Plumbing => "Сантехника",
        }
    }
}

/// Слой: группа элементов, которую можно скрыть или заблокировать.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    /// Идентификатор.
    pub id: String,
    /// Название.
    pub name: String,
    /// Элементы слоя видны.
    pub visible: bool,
    /// Элементы слоя нельзя выделить и изменить на холсте.
    pub locked: bool,
    /// Тип системного слоя; у пользовательского слоя — нет.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub system: Option<LayerKind>,
}

impl Layer {
    /// Системный слой с названием по умолчанию.
    #[must_use]
    pub fn system(kind: LayerKind) -> Self {
        Self {
            id: kind.id().into(),
            name: kind.default_name().into(),
            visible: true,
            locked: false,
            system: Some(kind),
        }
    }
}

/// Узел — точка, в которой начинаются и заканчиваются стены.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct Node {
    /// Идентификатор.
    pub id: String,
    /// Вариант, в котором элемент новый; без значения — элемент обмера (общий для всех вариантов).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub variant: Option<String>,
    /// X, мм.
    pub x: f64,
    /// Y, мм (вниз).
    pub y: f64,
}

/// Линия привязки стены: где лежат узлы относительно толщины.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub enum Justification {
    /// По оси: толщина поровну в обе стороны.
    #[default]
    Center,
    /// Стена слева от линии узлов (по направлению рисования).
    Left,
    /// Стена справа от линии узлов.
    Right,
}

/// Стена между двумя узлами.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct Wall {
    /// Идентификатор.
    pub id: String,
    /// Слой.
    pub layer: String,
    /// Вариант, в котором элемент новый; без значения — элемент обмера (общий для всех вариантов).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub variant: Option<String>,
    /// Начальный узел.
    pub a: String,
    /// Конечный узел.
    pub b: String,
    /// Толщина, мм.
    pub thickness: f64,
    /// Линия привязки.
    pub justification: Justification,
    /// Высота, мм; без значения — до потолка.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub height: Option<f64>,
    /// Материал (для новых перегородок).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub material: Option<WallMaterial>,
}

/// Материал стены.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub enum WallMaterial {
    /// Газобетон.
    AeratedConcrete,
    /// Гипсокартон на каркасе.
    Gypsum,
    /// Кирпич.
    Brick,
}

impl WallMaterial {
    /// Все материалы.
    pub const ALL: [Self; 3] = [Self::AeratedConcrete, Self::Gypsum, Self::Brick];

    /// Толщина перегородки по умолчанию, мм.
    #[must_use]
    pub fn default_thickness(self) -> f64 {
        match self {
            Self::AeratedConcrete | Self::Gypsum => 100.0,
            Self::Brick => 120.0,
        }
    }
}

/// Вариант перепланировки: новые элементы (с полем `variant`) и демонтаж
/// элементов обмера. Обмер общий: его правка попадает во все варианты.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct Variant {
    /// Идентификатор.
    pub id: String,
    /// Название, например «Вариант А».
    pub name: String,
    /// Элементы обмера, которые в этом варианте демонтируются.
    #[serde(default)]
    pub demolished: Vec<String>,
}

impl Wall {
    /// Толщина слева и справа от линии узлов, мм.
    #[must_use]
    pub fn sides(&self) -> (f64, f64) {
        match self.justification {
            Justification::Center => (self.thickness / 2.0, self.thickness / 2.0),
            Justification::Left => (self.thickness, 0.0),
            Justification::Right => (0.0, self.thickness),
        }
    }
}

/// Тип проёма.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub enum OpeningKind {
    /// Дверь.
    Door,
    /// Окно.
    Window,
    /// Арка (проём без заполнения).
    Arch,
}

impl OpeningKind {
    /// Размеры по умолчанию: ширина, высота, высота низа от пола, мм.
    #[must_use]
    pub fn defaults(self) -> (f64, f64, f64) {
        match self {
            Self::Door => (800.0, 2100.0, 0.0),
            Self::Window => (1200.0, 1400.0, 850.0),
            Self::Arch => (900.0, 2100.0, 0.0),
        }
    }
}

/// Сторона стены относительно направления от узла `a` к узлу `b`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub enum Side {
    /// Слева.
    #[default]
    Left,
    /// Справа.
    Right,
}

/// Окно, дверь или арка в стене.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct Opening {
    /// Идентификатор.
    pub id: String,
    /// Слой.
    pub layer: String,
    /// Вариант, в котором элемент новый; без значения — элемент обмера (общий для всех вариантов).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub variant: Option<String>,
    /// Стена.
    pub wall: String,
    /// Тип.
    pub kind: OpeningKind,
    /// Расстояние от узла `a` стены до середины проёма вдоль оси, мм.
    pub offset: f64,
    /// Ширина, мм.
    pub width: f64,
    /// Высота, мм.
    pub height: f64,
    /// Высота низа проёма от пола, мм.
    pub sill: f64,
    /// Петли двери у конца `b` стены (иначе — у конца `a`).
    #[serde(default)]
    pub hinge_at_end: bool,
    /// Сторона стены, в которую открывается дверь.
    #[serde(default)]
    pub swing: Side,
}

/// Помещение. Контур и площадь вычисляются: это замкнутая область стен,
/// в которой лежит точка подписи.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct Room {
    /// Идентификатор.
    pub id: String,
    /// Слой.
    pub layer: String,
    /// Вариант, в котором элемент новый; без значения — элемент обмера (общий для всех вариантов).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub variant: Option<String>,
    /// Название.
    pub name: String,
    /// X точки подписи, мм.
    pub x: f64,
    /// Y точки подписи, мм.
    pub y: f64,
    /// Высота потолка, мм; без значения — высота по умолчанию из проекта.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub ceiling_height: Option<f64>,
}

/// Конец размера: узел стены или свободная точка.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Anchor {
    /// Узел: размер следует за ним.
    Node {
        /// Узел.
        node: String,
    },
    /// Точка плана.
    Point {
        /// X, мм.
        x: f64,
        /// Y, мм.
        y: f64,
    },
}

/// Линейный размер между двумя точками.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct Dimension {
    /// Идентификатор.
    pub id: String,
    /// Слой.
    pub layer: String,
    /// Вариант, в котором элемент новый; без значения — элемент обмера (общий для всех вариантов).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub variant: Option<String>,
    /// Начало.
    pub start: Anchor,
    /// Конец.
    pub end: Anchor,
    /// Смещение размерной линии от линии между точками, мм (влево от направления начало → конец).
    pub offset: f64,
}

/// Предмет мебели или оборудования: позиция каталога или своя модель GLB.
///
/// Локальные оси: ширина — вдоль X, глубина — вдоль Y; спинка (сторона к стене)
/// — на −Y, лицевая сторона — на +Y. Поворот — по часовой стрелке на плане.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct Furniture {
    /// Идентификатор.
    pub id: String,
    /// Слой.
    pub layer: String,
    /// Вариант, в котором предмет новый (без значения — обмер).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub variant: Option<String>,
    /// Позиция каталога (`bed-double`) или `custom` для своей модели.
    pub item: String,
    /// Название.
    pub name: String,
    /// Центр, мм.
    pub x: f64,
    /// Центр, мм.
    pub y: f64,
    /// Поворот по часовой стрелке, градусы.
    #[serde(default)]
    pub rotation: f64,
    /// Отражён по ширине.
    #[serde(default)]
    pub mirrored: bool,
    /// Ширина, мм.
    pub width: f64,
    /// Глубина, мм.
    pub depth: f64,
    /// Высота, мм.
    pub height: f64,
    /// Низ над полом, мм (навесные шкафы, полки).
    #[serde(default)]
    pub elevation: f64,
    /// Своя модель: путь файла GLB внутри `.oplan`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub model: Option<String>,
}

/// Вид электроточки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub enum DeviceKind {
    /// Розетка.
    Socket,
    /// Розетка двойная.
    SocketDouble,
    /// Розетка влагозащищённая (IP44).
    SocketWaterproof,
    /// Силовая розетка 380 В.
    SocketPower,
    /// Компьютерная розетка (RJ-45).
    SocketData,
    /// Телевизионная розетка.
    SocketTv,
    /// Вывод кабеля под прибор (плита, бойлер, кондиционер).
    CableOutlet,
    /// Напольный лючок.
    FloorOutlet,
    /// Потолочный вывод под светильник.
    CeilingOutlet,
    /// Выключатель одноклавишный.
    Switch1,
    /// Выключатель двухклавишный.
    Switch2,
    /// Выключатель трёхклавишный.
    Switch3,
    /// Проходной выключатель.
    PassSwitch,
    /// Перекрёстный выключатель.
    CrossSwitch,
    /// Потолочный светильник.
    CeilingLight,
    /// Подвесной светильник.
    Pendant,
    /// Точечный светильник.
    Spot,
    /// Настенный светильник (бра).
    WallLight,
    /// Светодиодная лента.
    LedStrip,
    /// Распределительная коробка.
    JunctionBox,
    /// Электрощит.
    Panel,
}

impl DeviceKind {
    /// Светильник.
    #[must_use]
    pub fn is_light(self) -> bool {
        matches!(
            self,
            Self::CeilingLight | Self::Pendant | Self::Spot | Self::WallLight | Self::LedStrip
        )
    }

    /// Число клавиш выключателя (0 — не выключатель).
    #[must_use]
    pub fn keys(self) -> usize {
        match self {
            Self::Switch1 | Self::PassSwitch | Self::CrossSwitch => 1,
            Self::Switch2 => 2,
            Self::Switch3 => 3,
            _ => 0,
        }
    }
}

/// Электроточка: розетка, вывод, выключатель, светильник, коробка или щит.
///
/// Как и мебель, ставится по локальным осям: на стене — спинкой (−Y) к грани.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// Идентификатор.
    pub id: String,
    /// Слой.
    pub layer: String,
    /// Вариант, в котором точка новая (без значения — обмер).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub variant: Option<String>,
    /// Вид.
    pub kind: DeviceKind,
    /// Центр, мм.
    pub x: f64,
    /// Центр, мм.
    pub y: f64,
    /// Поворот по часовой стрелке, градусы.
    #[serde(default)]
    pub rotation: f64,
    /// Высота установки H от чистого пола, мм.
    pub height: f64,
    /// Назначение: «плита», «телевизор».
    #[serde(default)]
    pub label: String,
    /// Мощность нагрузки, Вт.
    #[serde(default)]
    pub power: f64,
    /// Световой поток светильника, лм.
    #[serde(default)]
    pub flux: f64,
    /// Группа щита.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub circuit: Option<String>,
    /// Выключатель: светильники, которые включает каждая клавиша.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<Vec<String>>,
}

/// Назначение группы щита.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub enum CircuitKind {
    /// Освещение.
    Lighting,
    /// Розетки.
    Sockets,
    /// Силовой потребитель (плита, бойлер, кондиционер).
    Power,
}

/// Группа электрощита.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct Circuit {
    /// Идентификатор.
    pub id: String,
    /// Обозначение: «Гр.1».
    pub name: String,
    /// Назначение: «Розетки кухни».
    #[serde(default)]
    pub purpose: String,
    /// Вид нагрузки.
    pub kind: CircuitKind,
    /// Число фаз: 1 или 3.
    pub phases: u8,
    /// Номинальный ток автомата, А.
    pub breaker: f64,
    /// Дифференциальная защита 30 мА (дифавтомат).
    #[serde(default)]
    pub rcd: bool,
    /// Сечение жилы, мм².
    pub section: f64,
    /// Коэффициент спроса Кс.
    pub demand: f64,
}

/// Сценарий освещения: светильники, включённые вместе.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct LightScene {
    /// Идентификатор.
    pub id: String,
    /// Название: «Вечер», «Кино».
    pub name: String,
    /// Светильники.
    pub lights: Vec<String>,
}

/// Система трубопровода (обозначения по смыслу ГОСТ 21.601: В1, Т3, К1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub enum PipeSystem {
    /// Холодное водоснабжение.
    Cold,
    /// Горячее водоснабжение.
    Hot,
    /// Бытовая канализация.
    Drain,
}

impl PipeSystem {
    /// Все системы.
    pub const ALL: [Self; 3] = [Self::Cold, Self::Hot, Self::Drain];

    /// Водоснабжение (не канализация).
    #[must_use]
    pub fn is_water(self) -> bool {
        self != Self::Drain
    }
}

/// Вид сантехнической точки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub enum PlumbingKind {
    /// Вывод под прибор.
    Outlet,
    /// Стояк.
    Riser,
    /// Коллектор водоснабжения.
    Manifold,
}

/// Сантехническая точка: вывод под прибор, стояк или коллектор.
///
/// Как электроточка, ставится по локальным осям: на стене — спинкой (−Y) к грани.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct PlumbingPoint {
    /// Идентификатор.
    pub id: String,
    /// Слой.
    pub layer: String,
    /// Вариант, в котором точка новая (без значения — обмер).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub variant: Option<String>,
    /// Вид.
    pub kind: PlumbingKind,
    /// Система.
    pub system: PipeSystem,
    /// Центр, мм.
    pub x: f64,
    /// Центр, мм.
    pub y: f64,
    /// Поворот по часовой стрелке, градусы.
    #[serde(default)]
    pub rotation: f64,
    /// Отметка оси от чистого пола, мм: у вывода — ось трубы, у стояка — ось врезки.
    pub height: f64,
    /// Условный диаметр, мм.
    pub diameter: f64,
    /// Назначение: «раковина», «стиральная машина».
    #[serde(default)]
    pub label: String,
    /// Прибор (предмет мебели), который обслуживает вывод.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub fixture: Option<String>,
}

/// Вершина трубы: точка плана и отметка оси от пола.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct PipeVertex {
    /// X, мм.
    pub x: f64,
    /// Y, мм.
    pub y: f64,
    /// Отметка оси от чистого пола, мм.
    pub z: f64,
}

/// Труба: ломаная с отметками вершин.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct Pipe {
    /// Идентификатор.
    pub id: String,
    /// Слой.
    pub layer: String,
    /// Вариант, в котором труба новая (без значения — обмер).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub variant: Option<String>,
    /// Система.
    pub system: PipeSystem,
    /// Условный диаметр, мм.
    pub diameter: f64,
    /// Вершины от прибора к стояку или коллектору.
    pub points: Vec<PipeVertex>,
    /// Проложена автоматически (пересчитывается при новой прокладке).
    #[serde(default)]
    pub auto: bool,
}

/// Вид отделочного материала: от него зависят раскладка и расход.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub enum MaterialKind {
    /// Плитка, керамогранит, мозаика: раскладка, клей, затирка.
    Tile,
    /// Ламинат, паркетная или инженерная доска: упаковки по площади.
    Laminate,
    /// Краска: литры по площади и числу слоёв.
    Paint,
    /// Обои: рулоны по полосам.
    Wallpaper,
    /// Натяжной потолок, ковролин и т. п.: площадь.
    Sheet,
}

/// Отделочный материал проекта (встроенные — в библиотеке движка).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct Material {
    /// Идентификатор.
    pub id: String,
    /// Название.
    pub name: String,
    /// Вид.
    pub kind: MaterialKind,
    /// Ширина элемента, мм (плитка, доска; у обоев — ширина рулона).
    #[serde(default)]
    pub width: f64,
    /// Длина элемента, мм (плитка, доска; у обоев — длина рулона).
    #[serde(default)]
    pub length: f64,
    /// Толщина, мм.
    #[serde(default)]
    pub thickness: f64,
    /// В упаковке: плитка — штук, ламинат — м², краска — литров, обои — рулонов.
    #[serde(default)]
    pub pack: f64,
    /// Цвет для плана и 3D: «#rrggbb».
    #[serde(default)]
    pub color: String,
}

/// Поверхность помещения.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub enum Surface {
    /// Пол.
    Floor,
    /// Потолок.
    Ceiling,
    /// Стены.
    Walls,
}

/// Рисунок раскладки.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub enum TilePattern {
    /// Шов в шов.
    #[default]
    Straight,
    /// Вразбежку: каждый ряд сдвинут на долю длины.
    Offset,
    /// По диагонали (45°).
    Diagonal,
}

/// Раскладка штучного материала.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct TileLayout {
    /// Рисунок.
    #[serde(default)]
    pub pattern: TilePattern,
    /// Ширина шва, мм.
    pub joint: f64,
    /// Сдвиг рядов вразбежку: доля длины элемента (0,5 — в полплитки).
    pub shift: f64,
    /// Старт: смещение сетки от угла поверхности, мм (пол — от левого верхнего угла
    /// помещения, стена — от левого нижнего угла грани).
    pub origin: [f64; 2],
}

impl Default for TileLayout {
    fn default() -> Self {
        Self {
            pattern: TilePattern::Straight,
            joint: 2.0,
            shift: 0.5,
            origin: [0.0, 0.0],
        }
    }
}

/// Отделка поверхности помещения.
///
/// Отделки стен одного помещения перекрываются по порядку списка: следующая
/// закрывает предыдущую в своей полосе высот.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct Finish {
    /// Идентификатор.
    pub id: String,
    /// Помещение.
    pub room: String,
    /// Поверхность.
    pub surface: Surface,
    /// Только одна стена: номер грани по развёртке (0 — «А»); без значения — все стены.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub face: Option<u32>,
    /// Материал: встроенный из библиотеки или материал проекта.
    pub material: String,
    /// Низ полосы на стене от пола, мм.
    #[serde(default)]
    pub bottom: f64,
    /// Верх полосы на стене, мм; без значения — до потолка.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub top: Option<f64>,
    /// Раскладка (для плитки и досок).
    pub layout: TileLayout,
}

/// Вид позиции прайса.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub enum PriceKind {
    /// Работа.
    Work,
    /// Материал (попадает в список закупок).
    Material,
}

/// Группа электроточек для правила объёма.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub enum DeviceGroup {
    /// Розетки и выводы.
    Outlet,
    /// Выключатели.
    Switch,
    /// Светильники.
    Light,
    /// Щит.
    Panel,
}

/// Откуда берётся объём позиции сметы.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Measure {
    /// Объём вводится вручную.
    Manual,
    /// Площадь пола помещений, м².
    FloorArea,
    /// Площадь потолка, м².
    CeilingArea,
    /// Площадь стен за вычетом проёмов, м².
    WallArea,
    /// Периметр помещений, м.
    Perimeter,
    /// Демонтируемые стены, м².
    Demolition,
    /// Новые перегородки, м² (без материала — все).
    Partitions {
        /// Материал.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "wasm", tsify(optional))]
        material: Option<WallMaterial>,
    },
    /// Новые проёмы, шт.
    NewOpenings,
    /// Электроточки, шт. (без группы — все).
    Devices {
        /// Группа.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "wasm", tsify(optional))]
        group: Option<DeviceGroup>,
    },
    /// Кабель по трассам групп, м.
    Cable,
    /// Выводы сантехники, шт. (без системы — все).
    Outlets {
        /// Система.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "wasm", tsify(optional))]
        system: Option<PipeSystem>,
    },
    /// Трубы, м (без системы — все).
    Pipes {
        /// Система.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "wasm", tsify(optional))]
        system: Option<PipeSystem>,
    },
    /// Сантехнические приборы, шт.
    Fixtures,
    /// Предметы мебели, шт.
    Furniture,
    /// Площадь отделки, м² (по поверхности и виду материала; без значения — любые).
    Finish {
        /// Поверхность.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "wasm", tsify(optional))]
        surface: Option<Surface>,
        /// Вид материала.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "wasm", tsify(optional))]
        kind: Option<MaterialKind>,
    },
    /// Расход из расчёта отделки: «Плитка», «Клей», «Затирка», «Краска»…
    Consumption {
        /// Название количества.
        name: String,
        /// Только этот материал.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "wasm", tsify(optional))]
        material: Option<String>,
    },
}

/// Цена на дату.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct PricePoint {
    /// Дата: «ДД.ММ.ГГГГ».
    pub date: String,
    /// Цена за единицу, ₽.
    pub price: f64,
}

/// Позиция прайса: работа или материал, цена и правило объёма из модели.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct PriceItem {
    /// Идентификатор.
    pub id: String,
    /// Код (артикул) — по нему сопоставляется импорт.
    #[serde(default)]
    pub code: String,
    /// Раздел сметы: «Демонтаж», «Электрика»…
    pub section: String,
    /// Наименование.
    pub name: String,
    /// Работа или материал.
    pub kind: PriceKind,
    /// Единица: «м²», «шт.», «кг».
    pub unit: String,
    /// Цена за единицу, ₽.
    pub price: f64,
    /// Откуда объём.
    pub measure: Measure,
    /// Норма: единиц позиции на единицу объёма (расход материала).
    pub rate: f64,
    /// Запас: доля сверх нормы (0,1 — 10 %).
    #[serde(default)]
    pub reserve: f64,
    /// Прежние цены, от старых к новым.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<PricePoint>,
}

/// Объём строки сметы, заданный вручную вместо посчитанного по модели.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct QuantityOverride {
    /// Позиция прайса.
    pub item: String,
    /// Помещение строки (без значения — строка без помещения).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub room: Option<String>,
    /// Объём.
    pub quantity: f64,
}

/// Выноска: стрелка к точке и текст.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct Leader {
    /// Идентификатор.
    pub id: String,
    /// Слой.
    pub layer: String,
    /// Вариант, в котором элемент новый; без значения — элемент обмера (общий для всех вариантов).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub variant: Option<String>,
    /// X точки, на которую указывает выноска, мм.
    pub x: f64,
    /// Y точки, мм.
    pub y: f64,
    /// X начала полки с текстом, мм.
    pub text_x: f64,
    /// Y начала полки, мм.
    pub text_y: f64,
    /// Текст.
    pub text: String,
}

/// Изображение-подложка (сканированный план, фото обмера).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct Underlay {
    /// Идентификатор.
    pub id: String,
    /// Слой.
    pub layer: String,
    /// Путь файла изображения внутри `.oplan`, например `assets/<id>.png`.
    pub asset: String,
    /// Исходное имя файла.
    pub name: String,
    /// X левого верхнего угла, мм.
    pub x: f64,
    /// Y левого верхнего угла, мм.
    pub y: f64,
    /// Ширина изображения, пиксели.
    pub width_px: u32,
    /// Высота изображения, пиксели.
    pub height_px: u32,
    /// Масштаб: миллиметров плана в пикселе изображения.
    pub mm_per_px: f64,
    /// Непрозрачность, 0–1.
    pub opacity: f64,
}

/// Двоичный файл внутри проекта (изображение подложки и т. п.).
pub type AssetData = Arc<Vec<u8>>;

/// Документ проекта.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct Document {
    /// Сведения о проекте.
    pub meta: ProjectMeta,
    /// Высота потолка по умолчанию, мм.
    pub default_ceiling_height: f64,
    /// Слои в порядке отрисовки (первый — снизу).
    pub layers: Vec<Layer>,
    /// Узлы стен.
    pub nodes: Vec<Node>,
    /// Стены.
    pub walls: Vec<Wall>,
    /// Окна, двери, арки.
    pub openings: Vec<Opening>,
    /// Помещения.
    pub rooms: Vec<Room>,
    /// Размеры.
    pub dimensions: Vec<Dimension>,
    /// Выноски.
    pub leaders: Vec<Leader>,
    /// Подложки.
    pub underlays: Vec<Underlay>,
    /// Мебель и оборудование.
    #[serde(default)]
    pub furniture: Vec<Furniture>,
    /// Электроточки и светильники.
    #[serde(default)]
    pub devices: Vec<Device>,
    /// Группы электрощита.
    #[serde(default)]
    pub circuits: Vec<Circuit>,
    /// Сценарии освещения.
    #[serde(default)]
    pub scenes: Vec<LightScene>,
    /// Сантехнические точки: выводы, стояки, коллекторы.
    #[serde(default)]
    pub plumbing: Vec<PlumbingPoint>,
    /// Трубы.
    #[serde(default)]
    pub pipes: Vec<Pipe>,
    /// Отделочные материалы проекта.
    #[serde(default)]
    pub materials: Vec<Material>,
    /// Отделка поверхностей помещений.
    #[serde(default)]
    pub finishes: Vec<Finish>,
    /// Прайс проекта: позиции сметы.
    #[serde(default)]
    pub prices: Vec<PriceItem>,
    /// Объёмы строк сметы, заданные вручную.
    #[serde(default)]
    pub overrides: Vec<QuantityOverride>,
    /// Варианты перепланировки.
    #[serde(default)]
    pub variants: Vec<Variant>,
    /// Настройки чертежей.
    #[serde(default)]
    pub drawing: DrawingSettings,
    /// Привязка к местности.
    #[serde(default)]
    pub site: Site,
    /// Сохранённые ракурсы 3D-вида.
    #[serde(default)]
    pub views: Vec<SavedView>,
    /// Двоичные файлы проекта по путям внутри `.oplan`. В `model.json` не входят.
    #[serde(skip)]
    #[schemars(skip)]
    #[cfg_attr(feature = "wasm", tsify(type = "never"))]
    pub assets: BTreeMap<String, AssetData>,
}

impl Default for Document {
    fn default() -> Self {
        Self::new("")
    }
}

impl Document {
    /// Высота потолка по умолчанию для нового проекта, мм.
    pub const DEFAULT_CEILING_HEIGHT: f64 = 2700.0;

    /// Создаёт пустой документ с системными слоями.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            meta: ProjectMeta {
                title: title.into(),
                stage: "Р".into(),
                ..ProjectMeta::default()
            },
            default_ceiling_height: Self::DEFAULT_CEILING_HEIGHT,
            layers: LayerKind::ALL.into_iter().map(Layer::system).collect(),
            nodes: Vec::new(),
            walls: Vec::new(),
            openings: Vec::new(),
            rooms: Vec::new(),
            dimensions: Vec::new(),
            leaders: Vec::new(),
            underlays: Vec::new(),
            furniture: Vec::new(),
            devices: Vec::new(),
            circuits: Vec::new(),
            scenes: Vec::new(),
            plumbing: Vec::new(),
            pipes: Vec::new(),
            materials: Vec::new(),
            finishes: Vec::new(),
            prices: Vec::new(),
            overrides: Vec::new(),
            variants: Vec::new(),
            drawing: DrawingSettings::default(),
            site: Site::default(),
            views: Vec::new(),
            assets: BTreeMap::new(),
        }
    }

    /// Слой по идентификатору.
    #[must_use]
    pub fn layer(&self, id: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    /// Узел по идентификатору.
    #[must_use]
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Стена по идентификатору.
    #[must_use]
    pub fn wall(&self, id: &str) -> Option<&Wall> {
        self.walls.iter().find(|w| w.id == id)
    }

    /// Проём по идентификатору.
    #[must_use]
    pub fn opening(&self, id: &str) -> Option<&Opening> {
        self.openings.iter().find(|o| o.id == id)
    }

    /// Помещение по идентификатору.
    #[must_use]
    pub fn room(&self, id: &str) -> Option<&Room> {
        self.rooms.iter().find(|r| r.id == id)
    }

    /// Размер по идентификатору.
    #[must_use]
    pub fn dimension(&self, id: &str) -> Option<&Dimension> {
        self.dimensions.iter().find(|d| d.id == id)
    }

    /// Выноска по идентификатору.
    #[must_use]
    pub fn leader(&self, id: &str) -> Option<&Leader> {
        self.leaders.iter().find(|l| l.id == id)
    }

    /// Предмет мебели по идентификатору.
    #[must_use]
    pub fn furniture_item(&self, id: &str) -> Option<&Furniture> {
        self.furniture.iter().find(|f| f.id == id)
    }

    /// Электроточка по идентификатору.
    #[must_use]
    pub fn device(&self, id: &str) -> Option<&Device> {
        self.devices.iter().find(|d| d.id == id)
    }

    /// Сантехническая точка по идентификатору.
    #[must_use]
    pub fn plumbing_point(&self, id: &str) -> Option<&PlumbingPoint> {
        self.plumbing.iter().find(|p| p.id == id)
    }

    /// Труба по идентификатору.
    #[must_use]
    pub fn pipe(&self, id: &str) -> Option<&Pipe> {
        self.pipes.iter().find(|p| p.id == id)
    }

    /// Вариант по идентификатору.
    #[must_use]
    pub fn variant(&self, id: &str) -> Option<&Variant> {
        self.variants.iter().find(|v| v.id == id)
    }

    /// Подложка по идентификатору.
    #[must_use]
    pub fn underlay(&self, id: &str) -> Option<&Underlay> {
        self.underlays.iter().find(|u| u.id == id)
    }
}
