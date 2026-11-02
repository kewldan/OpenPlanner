//! Геометрия элементов для отрисовки и попадания курсора.

use glam::DVec2;
use op_geom::{left_normal, walls::opening_rect};
use op_model::{Dimension, Document, Leader, Opening, Room, Underlay};
use op_render::text::Font;

use crate::derived::{Derived, opening_span};

/// Размер текста на плане, мм (2,5 мм на листе 1:50 ≈ 125 мм).
pub const ROOM_NAME_SIZE: f64 = 170.0;
/// Размер площади помещения, мм.
pub const ROOM_AREA_SIZE: f64 = 140.0;
/// Размер текста размеров и выносок, мм.
pub const NOTE_SIZE: f64 = 130.0;

/// Проём, разложенный на точки.
#[derive(Debug, Clone)]
pub struct OpeningGeometry {
    /// Направление стены (единичное).
    pub dir: DVec2,
    /// Левая нормаль стены.
    pub normal: DVec2,
    /// Начало и конец проёма на оси.
    pub start: DVec2,
    /// Конец.
    pub end: DVec2,
    /// Толщина стены слева и справа от оси.
    pub left: f64,
    /// Справа.
    pub right: f64,
    /// Контур выреза.
    pub rect: Vec<DVec2>,
}

/// Геометрия проёма.
#[must_use]
pub fn opening(doc: &Document, derived: &Derived, o: &Opening) -> Option<OpeningGeometry> {
    let wall = doc.wall(&o.wall)?;
    let (a, b) = derived.wall_axis(wall)?;
    let len = a.distance(b);
    if len < 1e-6 {
        return None;
    }
    let dir = (b - a) / len;
    let (from, to) = opening_span(o, len);
    let (left, right) = wall.sides();
    Some(OpeningGeometry {
        dir,
        normal: left_normal(dir),
        start: a + dir * from,
        end: a + dir * to,
        left,
        right,
        rect: opening_rect(a, b, left, right, from, to),
    })
}

/// Размер, разложенный на точки.
#[derive(Debug, Clone, Copy)]
pub struct DimensionGeometry {
    /// Точки, которые измеряются.
    pub a: DVec2,
    /// Вторая точка.
    pub b: DVec2,
    /// Концы размерной линии.
    pub qa: DVec2,
    /// Второй конец.
    pub qb: DVec2,
    /// Направление размерной линии.
    pub dir: DVec2,
    /// Нормаль (сторона смещения).
    pub normal: DVec2,
    /// Длина, мм.
    pub length: f64,
}

/// Геометрия размера.
#[must_use]
pub fn dimension(derived: &Derived, d: &Dimension) -> Option<DimensionGeometry> {
    let (a, b) = (derived.anchor(&d.start)?, derived.anchor(&d.end)?);
    let length = a.distance(b);
    let dir = if length > 1e-9 {
        (b - a) / length
    } else {
        DVec2::X
    };
    let normal = left_normal(dir);
    Some(DimensionGeometry {
        a,
        b,
        qa: a + normal * d.offset,
        qb: b + normal * d.offset,
        dir,
        normal,
        length,
    })
}

/// Угол, при котором текст вдоль направления читается слева направо, и направление «вверх» для текста.
#[must_use]
pub fn readable(dir: DVec2) -> (f64, DVec2) {
    let d = if dir.x < -1e-9 || (dir.x.abs() <= 1e-9 && dir.y > 0.0) {
        -dir
    } else {
        dir
    };
    (d.y.atan2(d.x), left_normal(d))
}

/// Подпись размера: число миллиметров.
#[must_use]
pub fn format_length(mm: f64) -> String {
    format!("{}", mm.round())
}

/// Площадь в м² с двумя знаками и запятой.
#[must_use]
pub fn format_area(m2: f64) -> String {
    format!("{m2:.2} м²").replace('.', ",")
}

/// Прямоугольник (мин, макс) строки текста размера `size` мм в точке `at`.
#[must_use]
pub fn text_box(
    font: Option<&Font>,
    text: &str,
    at: DVec2,
    size: f64,
    centered: bool,
) -> (DVec2, DVec2) {
    #[allow(clippy::cast_precision_loss)] // длина подписи — единицы символов
    let fallback = 0.6 * text.chars().count() as f64;
    let width = font.map_or(fallback, |f| f64::from(f.layout(text).width)) * size;
    let half_h = size * 0.5;
    if centered {
        (
            at - DVec2::new(width / 2.0, half_h),
            at + DVec2::new(width / 2.0, half_h),
        )
    } else {
        (at - DVec2::new(0.0, half_h), at + DVec2::new(width, half_h))
    }
}

/// Точки подписи помещения: название и площадь.
#[must_use]
pub fn room_label_points(room: &Room) -> (DVec2, DVec2) {
    let p = DVec2::new(room.x, room.y);
    (
        p - DVec2::new(0.0, ROOM_NAME_SIZE * 0.6),
        p + DVec2::new(0.0, ROOM_AREA_SIZE * 0.75),
    )
}

/// Начало текста выноски (над полкой).
#[must_use]
pub fn leader_text_point(l: &Leader) -> DVec2 {
    DVec2::new(l.text_x + NOTE_SIZE * 0.3, l.text_y - NOTE_SIZE * 0.65)
}

/// Длина полки выноски по длине текста.
#[must_use]
pub fn leader_shelf(font: Option<&Font>, l: &Leader) -> f64 {
    let (lo, hi) = text_box(font, &l.text, leader_text_point(l), NOTE_SIZE, false);
    (hi.x - lo.x) + NOTE_SIZE * 0.6
}

/// Углы подложки: левый верхний и правый нижний.
#[must_use]
pub fn underlay_rect(u: &Underlay) -> (DVec2, DVec2) {
    let origin = DVec2::new(u.x, u.y);
    let size = DVec2::new(f64::from(u.width_px), f64::from(u.height_px)) * u.mm_per_px;
    (origin, origin + size)
}
