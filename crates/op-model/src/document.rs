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
}

impl LayerKind {
    /// Все системные слои в порядке по умолчанию (первый — снизу).
    pub const ALL: [Self; 8] = [
        Self::Scan,
        Self::Underlays,
        Self::Rooms,
        Self::Furniture,
        Self::Walls,
        Self::Openings,
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
