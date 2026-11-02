//! Сцена документа и оверлей инструментов.

use glam::DVec2;
use op_model::{Document, OpeningKind, Side};
use op_render::{
    scene::{Align, Color, ImageQuad, Scene, TextStyle},
    text::Font,
};

use crate::{
    derived::Derived,
    geometry::{self, NOTE_SIZE, ROOM_AREA_SIZE, ROOM_NAME_SIZE},
};

/// Цвета плана.
pub mod palette {
    use op_render::scene::Color;

    /// Стены.
    #[must_use]
    pub fn wall() -> Color {
        Color::hex(0x3B3B37, 1.0)
    }
    /// Контуры стен и символы.
    #[must_use]
    pub fn ink() -> Color {
        Color::hex(0x23231F, 1.0)
    }
    /// Вторичные линии (дуга двери, выносные линии).
    #[must_use]
    pub fn muted() -> Color {
        Color::hex(0x6B6B64, 1.0)
    }
    /// Заливка помещения.
    #[must_use]
    pub fn room() -> Color {
        Color::hex(0xF1EEE4, 1.0)
    }
    /// Акцент: выделение, привязки, превью.
    #[must_use]
    pub fn accent() -> Color {
        Color::hex(0x0A7C4E, 1.0)
    }
    /// Ошибка и демонтаж.
    #[must_use]
    pub fn danger() -> Color {
        Color::hex(0xC2410C, 1.0)
    }
    /// Новые стены по материалу.
    #[must_use]
    pub fn material(material: Option<op_model::WallMaterial>) -> Color {
        match material {
            Some(op_model::WallMaterial::AeratedConcrete) => Color::hex(0x7E95AD, 1.0),
            Some(op_model::WallMaterial::Gypsum) => Color::hex(0xC9AE5D, 1.0),
            Some(op_model::WallMaterial::Brick) => Color::hex(0xB0583E, 1.0),
            None => Color::hex(0x0A7C4E, 1.0),
        }
    }
}

/// Демонтаж в виде варианта: обмер, его производные и демонтируемые элементы.
#[derive(Debug, Clone, Copy)]
pub struct Demolition<'a> {
    /// Обмер.
    pub survey: &'a Document,
    /// Производные обмера.
    pub derived: &'a Derived,
    /// Демонтируемые элементы.
    pub ids: &'a std::collections::HashSet<String>,
}

/// Демонтируемые стены — красным пунктиром со светлой заливкой, заложенные
/// проёмы — пунктирным символом.
pub fn demolition(scene: &mut Scene, d: &Demolition<'_>) {
    let red = palette::danger();
    for w in d.survey.walls.iter().filter(|w| d.ids.contains(&w.id)) {
        if let Some(outline) = d.derived.wall_outlines.get(&w.id)
            && !outline.is_empty()
        {
            scene.fill(&[outline], red.with_alpha(0.13));
            for i in 0..outline.len() {
                scene.dashed(outline[i], outline[(i + 1) % outline.len()], 1.5, 5.0, red);
            }
        }
    }
    for o in d
        .survey
        .openings
        .iter()
        .filter(|o| d.ids.contains(&o.id) && !d.ids.contains(&o.wall))
    {
        if let Some(g) = geometry::opening(d.survey, d.derived, o) {
            for i in 0..g.rect.len() {
                scene.dashed(g.rect[i], g.rect[(i + 1) % g.rect.len()], 1.5, 4.0, red);
            }
        }
    }
}

/// Стиль текста в мм плана.
#[must_use]
pub fn world_text(size: f64, color: Color) -> TextStyle {
    TextStyle {
        size,
        screen: false,
        align: Align::Center,
        angle: 0.0,
        color,
        halo: 2.0,
    }
}

/// Стиль текста в пикселях экрана (подсказки инструментов).
#[must_use]
pub fn screen_text(size: f64, color: Color) -> TextStyle {
    TextStyle {
        size,
        screen: true,
        align: Align::Center,
        angle: 0.0,
        color,
        halo: 3.0,
    }
}

/// Дуга окружности как ломаная.
#[must_use]
pub fn arc(center: DVec2, radius: f64, from: f64, to: f64) -> Vec<DVec2> {
    let steps = 24;
    (0..=steps)
        .map(|i| {
            let t = from + (to - from) * f64::from(i) / f64::from(steps);
            center + DVec2::new(t.cos(), t.sin()) * radius
        })
        .collect()
}

/// Сцена документа: подложки, помещения, стены, проёмы, размеры, выноски.
#[must_use]
pub fn document(
    doc: &Document,
    derived: &Derived,
    font: Option<&Font>,
    images: &dyn Fn(&str) -> bool,
) -> Scene {
    let mut scene = Scene::new();
    let shown = |layer: &str| derived.visible.contains(layer);

    for u in doc.underlays.iter().filter(|u| shown(&u.layer)) {
        if !images(&u.asset) {
            continue;
        }
        let (lo, hi) = geometry::underlay_rect(u);
        #[allow(clippy::cast_possible_truncation)] // непрозрачность в [0, 1]
        scene.image(ImageQuad {
            key: u.asset.clone(),
            origin: lo,
            size: hi - lo,
            opacity: u.opacity as f32,
        });
    }

    let rooms: Vec<_> = doc.rooms.iter().filter(|r| shown(&r.layer)).collect();
    for r in &rooms {
        if let Some(&i) = derived.room_regions.get(&r.id) {
            let region = &derived.regions[i];
            let mut contours: Vec<&[DVec2]> = vec![&region.contour];
            contours.extend(region.islands.iter().map(Vec::as_slice));
            scene.fill(&contours, palette::room());
        }
    }

    let groups = std::iter::once((palette::wall(), &derived.wall_shapes)).chain(
        derived
            .new_wall_shapes
            .iter()
            .map(|(m, shapes)| (palette::material(*m), shapes)),
    );
    for (color, shapes) in groups {
        for shape in shapes {
            let mut contours: Vec<&[DVec2]> = vec![&shape.outer];
            contours.extend(shape.holes.iter().map(Vec::as_slice));
            scene.fill(&contours, color);
            scene.polyline(&shape.outer, true, 1.0, palette::ink());
            for hole in &shape.holes {
                scene.polyline(hole, true, 1.0, palette::ink());
            }
        }
    }

    for o in doc.openings.iter().filter(|o| shown(&o.layer)) {
        let Some(g) = geometry::opening(doc, derived, o) else {
            continue;
        };
        opening_symbol(
            &mut scene,
            &g,
            o.kind,
            o.hinge_at_end,
            o.swing,
            palette::ink(),
            1.0,
        );
    }

    if let Some(font) = font {
        for r in &rooms {
            let (name_at, area_at) = geometry::room_label_points(r);
            scene.text(
                font,
                name_at,
                &r.name,
                world_text(ROOM_NAME_SIZE, palette::ink()),
            );
            let area = derived
                .room_regions
                .get(&r.id)
                .map(|&i| &derived.regions[i]);
            let (text, color) = match area {
                Some(region) => (
                    geometry::format_area(op_geom::mm2_to_m2(region.area)),
                    palette::muted(),
                ),
                None => ("контур не замкнут".to_owned(), palette::danger()),
            };
            scene.text(font, area_at, &text, world_text(ROOM_AREA_SIZE, color));
        }
    }

    for d in doc.dimensions.iter().filter(|d| shown(&d.layer)) {
        if let Some(g) = geometry::dimension(derived, d) {
            dimension_symbol(&mut scene, font, &g, palette::ink(), 1.0);
        }
    }

    for l in doc.leaders.iter().filter(|l| shown(&l.layer)) {
        let target = DVec2::new(l.x, l.y);
        let text = DVec2::new(l.text_x, l.text_y);
        scene.line(target, text, 1.0, palette::ink());
        let shelf = geometry::leader_shelf(font, l);
        scene.line(text, text + DVec2::X * shelf, 1.0, palette::ink());
        scene.dot(target, 5.0, palette::ink());
        if let Some(font) = font {
            let mut style = world_text(NOTE_SIZE, palette::ink());
            style.align = Align::Left;
            scene.text(font, geometry::leader_text_point(l), &l.text, style);
        }
    }
    scene
}

/// Куда рисуется символ: сцена холста или лист чертежа.
pub trait Pen {
    /// Отрезок; `weight` — толщина относительно основной линии символа.
    fn line(&mut self, a: DVec2, b: DVec2, weight: f64);
    /// Штриховой отрезок.
    fn dashed(&mut self, a: DVec2, b: DVec2, weight: f64);
    /// Вспомогательная ломаная (дуга открывания двери).
    fn faint(&mut self, points: &[DVec2], weight: f64);
}

/// Перо сцены холста: толщины в пикселях экрана.
struct ScenePen<'a> {
    scene: &'a mut Scene,
    color: Color,
    width: f32,
}

impl ScenePen<'_> {
    #[allow(clippy::cast_possible_truncation)] // относительная толщина — единицы
    fn width(&self, weight: f64) -> f32 {
        self.width * weight as f32
    }
}

impl Pen for ScenePen<'_> {
    fn line(&mut self, a: DVec2, b: DVec2, weight: f64) {
        let width = self.width(weight);
        self.scene.line(a, b, width, self.color);
    }

    fn dashed(&mut self, a: DVec2, b: DVec2, weight: f64) {
        let width = self.width(weight);
        self.scene.dashed(a, b, width, 4.0, self.color);
    }

    fn faint(&mut self, points: &[DVec2], weight: f64) {
        let width = self.width(weight);
        self.scene
            .polyline(points, false, width, self.color.with_alpha(0.7));
    }
}

/// Символ проёма на сцене холста.
pub fn opening_symbol(
    scene: &mut Scene,
    g: &geometry::OpeningGeometry,
    kind: OpeningKind,
    hinge_at_end: bool,
    swing: Side,
    color: Color,
    width: f32,
) {
    let mut pen = ScenePen {
        scene,
        color,
        width,
    };
    opening_lines(&mut pen, g, kind, hinge_at_end, swing);
}

/// Символ проёма: откосы, створка двери с дугой, стекло окна.
pub fn opening_lines(
    pen: &mut impl Pen,
    g: &geometry::OpeningGeometry,
    kind: OpeningKind,
    hinge_at_end: bool,
    swing: Side,
) {
    let (n, l, r) = (g.normal, g.left, g.right);
    // Откосы поперёк стены.
    for p in [g.start, g.end] {
        pen.line(p + n * l, p - n * r, 1.0);
    }
    match kind {
        OpeningKind::Window => {
            pen.line(g.start + n * l, g.end + n * l, 1.0);
            pen.line(g.start - n * r, g.end - n * r, 1.0);
            let mid = (l - r) / 2.0;
            let gap = ((l + r) * 0.12).max(10.0);
            pen.line(g.start + n * (mid + gap), g.end + n * (mid + gap), 1.0);
            pen.line(g.start + n * (mid - gap), g.end + n * (mid - gap), 1.0);
        }
        OpeningKind::Arch => {
            pen.dashed(g.start + n * l, g.end + n * l, 1.0);
            pen.dashed(g.start - n * r, g.end - n * r, 1.0);
        }
        OpeningKind::Door => {
            let side = if swing == Side::Left { 1.0 } else { -1.0 };
            let face = if swing == Side::Left { l } else { r };
            let (hinge, free) = if hinge_at_end {
                (g.end, g.start)
            } else {
                (g.start, g.end)
            };
            let hinge = hinge + n * side * face;
            let free = free + n * side * face;
            let leaf = hinge.distance(free);
            let open = hinge + n * side * leaf;
            pen.line(hinge, open, 1.6);
            let a0 = (free - hinge).y.atan2((free - hinge).x);
            let a1 = (open - hinge).y.atan2((open - hinge).x);
            let mut delta = a1 - a0;
            while delta > std::f64::consts::PI {
                delta -= std::f64::consts::TAU;
            }
            while delta < -std::f64::consts::PI {
                delta += std::f64::consts::TAU;
            }
            pen.faint(&arc(hinge, leaf, a0, a0 + delta), 0.8);
        }
    }
}

/// Размер: выносные линии, размерная линия, засечки, число.
pub fn dimension_symbol(
    scene: &mut Scene,
    font: Option<&Font>,
    g: &geometry::DimensionGeometry,
    color: Color,
    width: f32,
) {
    let over = 80.0
        * g.normal
        * if (g.qa - g.a).dot(g.normal) < 0.0 {
            -1.0
        } else {
            1.0
        };
    scene.line(g.a, g.qa + over, width, color.with_alpha(0.75));
    scene.line(g.b, g.qb + over, width, color.with_alpha(0.75));
    scene.line(g.qa - g.dir * 80.0, g.qb + g.dir * 80.0, width, color);
    let tick = (g.dir + g.normal).normalize() * 70.0;
    for q in [g.qa, g.qb] {
        scene.line(q - tick, q + tick, width * 1.8, color);
    }
    if let Some(font) = font {
        let (angle, up) = geometry::readable(g.dir);
        let mut style = world_text(NOTE_SIZE, color);
        style.angle = angle;
        scene.text(
            font,
            g.qa.midpoint(g.qb) + up * NOTE_SIZE * 0.8,
            &geometry::format_length(g.length),
            style,
        );
    }
}
