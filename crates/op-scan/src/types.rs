//! Результат распознавания и параметры импорта (типы протокола с UI).

use op_model::{Justification, OpeningKind};
use serde::{Deserialize, Serialize};

/// Точка плана, мм (Y вниз).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Point {
    /// X, мм.
    pub x: f64,
    /// Y, мм.
    pub y: f64,
}

impl Point {
    /// Точка из вектора.
    #[must_use]
    pub fn from_vec(v: glam::DVec2) -> Self {
        Self { x: v.x, y: v.y }
    }

    /// Вектор из точки.
    #[must_use]
    pub fn vec(self) -> glam::DVec2 {
        glam::DVec2::new(self.x, self.y)
    }
}

/// Формат исходного файла.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub enum ScanFormat {
    /// glTF 2.0 Binary (сетка).
    Glb,
    /// Wavefront OBJ (сетка).
    Obj,
    /// PLY (облако точек или сетка).
    Ply,
    /// Параметрическая комната Apple RoomPlan (JSON).
    RoomPlan,
}

/// Откуда взялась стена.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub enum WallSource {
    /// Отсканированы обе поверхности: толщина измерена.
    Paired,
    /// Отсканирована одна поверхность: толщина задаётся параметром.
    Single,
    /// Параметрическая стена из данных RoomPlan.
    RoomPlan,
}

/// Распознанная стена. Линия `a`–`b` — линия узлов; для стены с одной
/// поверхностью она совпадает с отсканированной поверхностью.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct ScanWall {
    /// Номер.
    pub id: u32,
    /// Начало, мм.
    pub a: Point,
    /// Конец, мм.
    pub b: Point,
    /// Толщина, мм (для `Single` — по умолчанию из параметров).
    pub thickness: f64,
    /// Где тело стены относительно линии `a`–`b`.
    pub justification: Justification,
    /// Откуда стена.
    pub source: WallSource,
    /// Уверенность 0–1.
    pub confidence: f64,
}

/// Распознанный проём.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct ScanOpening {
    /// Номер.
    pub id: u32,
    /// Стена.
    pub wall: u32,
    /// Тип.
    pub kind: OpeningKind,
    /// Середина проёма на плане, мм.
    pub center: Point,
    /// Ширина, мм.
    pub width: f64,
    /// Высота, мм.
    pub height: f64,
    /// Низ от пола, мм.
    pub sill: f64,
    /// Уверенность 0–1.
    pub confidence: f64,
}

/// Сомнительное место для проверки пользователем.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum IssueKind {
    /// «Лишняя стена?» — согласие удаляет стену.
    ExtraWall {
        /// Стена.
        wall: u32,
    },
    /// «Пропущена стена» — согласие добавляет стену между точками.
    MissingWall {
        /// Начало.
        a: Point,
        /// Конец.
        b: Point,
    },
    /// «Дверь или арка?» — согласие делает проём дверью.
    DoorOrArch {
        /// Проём.
        opening: u32,
    },
}

/// Вопрос проверки.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct ScanIssue {
    /// Номер.
    pub id: u32,
    /// Что предлагается.
    pub kind: IssueKind,
    /// Уверенность в предложении 0–1; от 0,5 предложение принято по умолчанию.
    pub confidence: f64,
    /// Где показать на плане.
    pub at: Point,
}

/// Вид сверху для подложки: PNG и его положение на плане.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct ScanRaster {
    /// PNG.
    #[serde(with = "serde_bytes")]
    #[cfg_attr(feature = "wasm", tsify(type = "Uint8Array"))]
    pub png: Vec<u8>,
    /// X левого верхнего угла, мм.
    pub x: f64,
    /// Y левого верхнего угла, мм.
    pub y: f64,
    /// Ширина, пиксели.
    pub width_px: u32,
    /// Высота, пиксели.
    pub height_px: u32,
    /// Миллиметров плана в пикселе.
    pub mm_per_px: f64,
}

/// Почему распознать план не удалось.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub enum ScanFailure {
    /// Не найден пол.
    NoFloor,
    /// Найдено слишком мало стен.
    TooFewWalls,
}

/// Сведения об исходном файле.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct ScanSource {
    /// Формат.
    pub format: ScanFormat,
    /// Треугольников в сетке.
    pub triangles: u32,
    /// Точек в облаке.
    pub points: u32,
    /// Использованы параметрические данные комнаты.
    pub room_data: bool,
}

/// Результат распознавания.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    /// Исходный файл.
    pub source: ScanSource,
    /// Высота потолка над полом, мм.
    pub ceiling_height: f64,
    /// Потолок найден в скане (иначе — значение по умолчанию).
    pub ceiling_detected: bool,
    /// Стены.
    pub walls: Vec<ScanWall>,
    /// Проёмы.
    pub openings: Vec<ScanOpening>,
    /// Вопросы проверки.
    pub issues: Vec<ScanIssue>,
    /// Вид сверху.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub raster: Option<ScanRaster>,
    /// Причина неудачи распознавания (вид сверху при этом может быть).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "wasm", tsify(optional))]
    pub failure: Option<ScanFailure>,
}

/// Ответ на вопрос проверки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct ScanAnswer {
    /// Вопрос.
    pub issue: u32,
    /// Принять предложение.
    pub accept: bool,
}

/// Параметры импорта.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct ScanParams {
    /// Высота потолка, мм.
    pub ceiling_height: f64,
    /// Толщина стен, у которых отсканирована одна поверхность, мм.
    pub single_thickness: f64,
    /// Толщина добавленных («пропущенных») стен, мм.
    pub added_thickness: f64,
    /// Создать помещения во всех замкнутых контурах.
    pub create_rooms: bool,
    /// Положить вид сверху подложкой на слой «Скан».
    pub underlay: bool,
    /// Сохранить исходный файл скана в проекте.
    pub keep_source: bool,
}

impl Default for ScanParams {
    fn default() -> Self {
        Self {
            ceiling_height: 2700.0,
            single_thickness: 200.0,
            added_thickness: 120.0,
            create_rooms: true,
            underlay: true,
            keep_source: true,
        }
    }
}

/// Стена готового плана: концы — номера узлов.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanWall {
    /// Начальный узел.
    pub a: usize,
    /// Конечный узел.
    pub b: usize,
    /// Толщина, мм.
    pub thickness: f64,
    /// Привязка.
    pub justification: Justification,
}

/// Проём готового плана.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanOpening {
    /// Стена (номер в `walls`).
    pub wall: usize,
    /// Тип.
    pub kind: OpeningKind,
    /// Расстояние от узла `a` стены до середины, мм.
    pub offset: f64,
    /// Ширина, мм.
    pub width: f64,
    /// Высота, мм.
    pub height: f64,
    /// Низ от пола, мм.
    pub sill: f64,
}

/// План после ответов на вопросы: узлы, стены, проёмы.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScanPlan {
    /// Узлы, мм.
    pub nodes: Vec<glam::DVec2>,
    /// Стены.
    pub walls: Vec<PlanWall>,
    /// Проёмы.
    pub openings: Vec<PlanOpening>,
}
