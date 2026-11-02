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
}

impl LayerKind {
    /// Все системные слои в порядке по умолчанию (первый — снизу).
    pub const ALL: [Self; 7] = [
        Self::Scan,
        Self::Underlays,
        Self::Rooms,
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
    /// Начало.
    pub start: Anchor,
    /// Конец.
    pub end: Anchor,
    /// Смещение размерной линии от линии между точками, мм (влево от направления начало → конец).
    pub offset: f64,
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
                address: String::new(),
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

    /// Подложка по идентификатору.
    #[must_use]
    pub fn underlay(&self, id: &str) -> Option<&Underlay> {
        self.underlays.iter().find(|u| u.id == id)
    }
}
