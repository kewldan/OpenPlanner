//! Попадание курсора в элементы и ручки выделения.

use glam::DVec2;
use op_geom::{contains, segment};
use op_model::Document;
use op_render::text::Font;

use crate::{
    derived::Derived,
    geometry::{self, NOTE_SIZE, ROOM_AREA_SIZE, ROOM_NAME_SIZE},
};

/// Что перетаскивается за ручку.
#[derive(Debug, Clone, PartialEq)]
pub enum HandleKind {
    /// Узел стены.
    Node(String),
    /// Проём вдоль стены.
    Opening(String),
    /// Размерная линия (смещение).
    DimensionOffset(String),
    /// Конец размера: `true` — конец, `false` — начало.
    DimensionEnd(String, bool),
    /// Точка выноски: `true` — полка с текстом, `false` — стрелка.
    LeaderPoint(String, bool),
    /// Подпись помещения.
    RoomLabel(String),
}

impl HandleKind {
    /// Элемент, который двигает ручка.
    #[must_use]
    pub fn target(&self) -> &str {
        match self {
            Self::Node(id)
            | Self::Opening(id)
            | Self::DimensionOffset(id)
            | Self::DimensionEnd(id, _)
            | Self::LeaderPoint(id, _)
            | Self::RoomLabel(id) => id,
        }
    }
}

/// Ручка на холсте.
#[derive(Debug, Clone, PartialEq)]
pub struct Handle {
    /// Положение, мм.
    pub pos: DVec2,
    /// Что двигает.
    pub kind: HandleKind,
}

fn inside_box(p: DVec2, (lo, hi): (DVec2, DVec2), pad: f64) -> bool {
    p.x >= lo.x - pad && p.x <= hi.x + pad && p.y >= lo.y - pad && p.y <= hi.y + pad
}

/// Верхний элемент под точкой на видимых незаблокированных слоях.
#[must_use]
pub fn hit(
    doc: &Document,
    derived: &Derived,
    font: Option<&Font>,
    p: DVec2,
    radius: f64,
) -> Option<String> {
    for l in doc
        .leaders
        .iter()
        .rev()
        .filter(|l| derived.editable(&l.layer))
    {
        let target = DVec2::new(l.x, l.y);
        let text = DVec2::new(l.text_x, l.text_y);
        let shelf_end = text + DVec2::X * geometry::leader_shelf(font, l);
        let tb = geometry::text_box(
            font,
            &l.text,
            geometry::leader_text_point(l),
            NOTE_SIZE,
            false,
        );
        if segment::distance(target, text, p) <= radius
            || segment::distance(text, shelf_end, p) <= radius
            || inside_box(p, tb, radius)
        {
            return Some(l.id.clone());
        }
    }
    for d in doc
        .dimensions
        .iter()
        .rev()
        .filter(|d| derived.editable(&d.layer))
    {
        let Some(g) = geometry::dimension(derived, d) else {
            continue;
        };
        let (_, up) = geometry::readable(g.dir);
        let mid = g.qa.midpoint(g.qb) + up * NOTE_SIZE * 0.8;
        let tb = geometry::text_box(
            font,
            &geometry::format_length(g.length),
            mid,
            NOTE_SIZE,
            true,
        );
        if segment::distance(g.qa, g.qb, p) <= radius || inside_box(p, tb, radius) {
            return Some(d.id.clone());
        }
    }
    for o in doc
        .openings
        .iter()
        .rev()
        .filter(|o| derived.editable(&o.layer))
    {
        if let Some(g) = geometry::opening(doc, derived, o)
            && (contains(&g.rect, p) || segment::distance(g.start, g.end, p) <= radius)
        {
            return Some(o.id.clone());
        }
    }
    for r in doc
        .rooms
        .iter()
        .rev()
        .filter(|r| derived.editable(&r.layer))
    {
        let (name, _) = geometry::room_label_points(r);
        let tb = geometry::text_box(font, &r.name, name, ROOM_NAME_SIZE, true);
        let ab = (
            DVec2::new(tb.0.x, name.y),
            DVec2::new(tb.1.x, name.y + ROOM_NAME_SIZE + ROOM_AREA_SIZE),
        );
        if inside_box(p, tb, radius) || inside_box(p, ab, radius) {
            return Some(r.id.clone());
        }
    }
    for d in doc
        .devices
        .iter()
        .rev()
        .filter(|d| derived.editable(&d.layer))
    {
        if crate::electric::contains(d, p, radius) {
            return Some(d.id.clone());
        }
    }
    for q in doc
        .plumbing
        .iter()
        .rev()
        .filter(|q| derived.editable(&q.layer))
    {
        if crate::plumbing::contains(q, p, radius) {
            return Some(q.id.clone());
        }
    }
    for q in doc
        .pipes
        .iter()
        .rev()
        .filter(|q| derived.editable(&q.layer))
    {
        if crate::plumbing::pipe_contains(q, p, radius) {
            return Some(q.id.clone());
        }
    }
    for f in doc
        .furniture
        .iter()
        .rev()
        .filter(|f| derived.editable(&f.layer))
    {
        if crate::furniture::contains(f, p, radius) {
            return Some(f.id.clone());
        }
    }
    for w in doc
        .walls
        .iter()
        .rev()
        .filter(|w| derived.editable(&w.layer))
    {
        let inside = derived
            .wall_outlines
            .get(&w.id)
            .is_some_and(|o| contains(o, p));
        let near = derived
            .wall_axis(w)
            .is_some_and(|(a, b)| segment::distance(a, b, p) <= radius);
        if inside || near {
            return Some(w.id.clone());
        }
    }
    for r in doc
        .rooms
        .iter()
        .rev()
        .filter(|r| derived.editable(&r.layer))
    {
        if let Some(&i) = derived.room_regions.get(&r.id)
            && derived.regions[i].contains(p)
        {
            return Some(r.id.clone());
        }
    }
    for u in doc
        .underlays
        .iter()
        .rev()
        .filter(|u| derived.editable(&u.layer))
    {
        if inside_box(p, geometry::underlay_rect(u), 0.0) {
            return Some(u.id.clone());
        }
    }
    None
}

/// Ручки выделенных элементов.
#[must_use]
pub fn handles(doc: &Document, derived: &Derived, selection: &[String]) -> Vec<Handle> {
    let mut out = Vec::new();
    let selected = |id: &str| selection.iter().any(|s| s == id);
    let mut seen_nodes = Vec::new();
    for w in doc
        .walls
        .iter()
        .filter(|w| selected(&w.id) && derived.editable(&w.layer))
    {
        for node in [&w.a, &w.b] {
            if seen_nodes.contains(node) {
                continue;
            }
            seen_nodes.push(node.clone());
            if let Some(&pos) = derived.nodes.get(node) {
                out.push(Handle {
                    pos,
                    kind: HandleKind::Node(node.clone()),
                });
            }
        }
    }
    for o in doc
        .openings
        .iter()
        .filter(|o| selected(&o.id) && derived.editable(&o.layer))
    {
        if let Some(g) = geometry::opening(doc, derived, o) {
            out.push(Handle {
                pos: g.start.midpoint(g.end),
                kind: HandleKind::Opening(o.id.clone()),
            });
        }
    }
    for d in doc
        .dimensions
        .iter()
        .filter(|d| selected(&d.id) && derived.editable(&d.layer))
    {
        if let Some(g) = geometry::dimension(derived, d) {
            out.push(Handle {
                pos: g.qa,
                kind: HandleKind::DimensionEnd(d.id.clone(), false),
            });
            out.push(Handle {
                pos: g.qb,
                kind: HandleKind::DimensionEnd(d.id.clone(), true),
            });
            out.push(Handle {
                pos: g.qa.midpoint(g.qb),
                kind: HandleKind::DimensionOffset(d.id.clone()),
            });
        }
    }
    for l in doc
        .leaders
        .iter()
        .filter(|l| selected(&l.id) && derived.editable(&l.layer))
    {
        out.push(Handle {
            pos: DVec2::new(l.x, l.y),
            kind: HandleKind::LeaderPoint(l.id.clone(), false),
        });
        out.push(Handle {
            pos: DVec2::new(l.text_x, l.text_y),
            kind: HandleKind::LeaderPoint(l.id.clone(), true),
        });
    }
    for r in doc
        .rooms
        .iter()
        .filter(|r| selected(&r.id) && derived.editable(&r.layer))
    {
        out.push(Handle {
            pos: DVec2::new(r.x, r.y),
            kind: HandleKind::RoomLabel(r.id.clone()),
        });
    }
    out
}

/// Элементы, целиком лежащие в прямоугольнике.
#[must_use]
pub fn in_rect(doc: &Document, derived: &Derived, lo: DVec2, hi: DVec2) -> Vec<String> {
    let inside = |p: DVec2| p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y;
    let mut ids = Vec::new();
    for w in doc.walls.iter().filter(|w| derived.editable(&w.layer)) {
        if derived
            .wall_axis(w)
            .is_some_and(|(a, b)| inside(a) && inside(b))
        {
            ids.push(w.id.clone());
        }
    }
    for o in doc.openings.iter().filter(|o| derived.editable(&o.layer)) {
        if geometry::opening(doc, derived, o).is_some_and(|g| inside(g.start) && inside(g.end)) {
            ids.push(o.id.clone());
        }
    }
    for r in doc.rooms.iter().filter(|r| derived.editable(&r.layer)) {
        if inside(DVec2::new(r.x, r.y)) {
            ids.push(r.id.clone());
        }
    }
    for d in doc.dimensions.iter().filter(|d| derived.editable(&d.layer)) {
        if geometry::dimension(derived, d).is_some_and(|g| inside(g.qa) && inside(g.qb)) {
            ids.push(d.id.clone());
        }
    }
    for l in doc.leaders.iter().filter(|l| derived.editable(&l.layer)) {
        if inside(DVec2::new(l.x, l.y)) && inside(DVec2::new(l.text_x, l.text_y)) {
            ids.push(l.id.clone());
        }
    }
    for f in doc.furniture.iter().filter(|f| derived.editable(&f.layer)) {
        if crate::furniture::corners(f).into_iter().all(inside) {
            ids.push(f.id.clone());
        }
    }
    for d in doc.devices.iter().filter(|d| derived.editable(&d.layer)) {
        if inside(DVec2::new(d.x, d.y)) {
            ids.push(d.id.clone());
        }
    }
    for q in doc.plumbing.iter().filter(|q| derived.editable(&q.layer)) {
        if inside(DVec2::new(q.x, q.y)) {
            ids.push(q.id.clone());
        }
    }
    for q in doc.pipes.iter().filter(|q| derived.editable(&q.layer)) {
        if q.points.iter().all(|v| inside(crate::plumbing::xy(v))) {
            ids.push(q.id.clone());
        }
    }
    for u in doc.underlays.iter().filter(|u| derived.editable(&u.layer)) {
        let (a, b) = geometry::underlay_rect(u);
        if inside(a) && inside(b) {
            ids.push(u.id.clone());
        }
    }
    ids
}
