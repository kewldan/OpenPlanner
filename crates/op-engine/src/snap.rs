//! Привязки курсора: концы и середины стен, перпендикуляр, точка на стене,
//! выравнивание по узлам и углам, сетка.

use glam::DVec2;
use op_geom::segment;
use op_model::Document;

use crate::{derived::Derived, protocol::SnapKind};

/// Радиус привязки, CSS-пиксели.
pub const SNAP_RADIUS_PX: f64 = 10.0;

/// Результат привязки.
#[derive(Debug, Clone, PartialEq)]
pub struct Snap {
    /// Точка после привязки, мм.
    pub point: DVec2,
    /// Вид привязки.
    pub kind: Option<SnapKind>,
    /// Узел, если точка совпала с узлом.
    pub node: Option<String>,
    /// Стена, на оси которой лежит точка.
    pub wall: Option<String>,
    /// Направляющие для отрисовки.
    pub guides: Vec<(DVec2, DVec2)>,
}

impl Snap {
    fn free(point: DVec2) -> Self {
        Self {
            point,
            kind: None,
            node: None,
            wall: None,
            guides: Vec::new(),
        }
    }
}

/// Параметры привязки.
#[derive(Debug, Clone, Copy)]
pub struct SnapInput<'a> {
    /// Курсор, мм.
    pub cursor: DVec2,
    /// Масштаб: CSS-пикселей на мм.
    pub px_per_mm: f64,
    /// Предыдущая точка цепочки.
    pub anchor: Option<DVec2>,
    /// Shift: ограничить направление углами 0/45/90°.
    pub ortho: bool,
    /// Alt: без привязок.
    pub disabled: bool,
    /// Узлы, к которым не привязываться (перетаскиваемые).
    pub exclude: &'a [String],
}

/// Шаг сетки привязки для масштаба: не мельче 8 CSS-пикселей.
#[must_use]
pub fn grid_step(px_per_mm: f64) -> f64 {
    [10.0, 50.0, 100.0, 500.0, 1000.0, 5000.0]
        .into_iter()
        .find(|s| s * px_per_mm >= 8.0)
        .unwrap_or(10_000.0)
}

fn round_to(v: f64, step: f64) -> f64 {
    (v / step).round() * step
}

/// Ограничивает направление от `anchor` углами, кратными 45°.
fn constrain(anchor: DVec2, p: DVec2, step: f64) -> DVec2 {
    let d = p - anchor;
    let len = d.length();
    if len < 1e-9 {
        return p;
    }
    let angle = d.y.atan2(d.x);
    let k = std::f64::consts::FRAC_PI_4;
    let snapped = (angle / k).round() * k;
    anchor + DVec2::new(snapped.cos(), snapped.sin()) * round_to(len, step).max(step)
}

/// Привязывает курсор.
#[must_use]
pub fn snap(doc: &Document, derived: &Derived, input: SnapInput<'_>) -> Snap {
    let p = input.cursor;
    if input.disabled {
        return Snap::free(p);
    }
    let radius = SNAP_RADIUS_PX / input.px_per_mm;
    let step = grid_step(input.px_per_mm);
    let excluded = |id: &str| input.exclude.iter().any(|e| e == id);
    let walls: Vec<_> = doc
        .walls
        .iter()
        .filter(|w| derived.visible.contains(&w.layer))
        .filter(|w| !excluded(&w.a) && !excluded(&w.b))
        .filter_map(|w| derived.wall_axis(w).map(|(a, b)| (w, a, b)))
        .collect();

    if input.ortho
        && let Some(anchor) = input.anchor
    {
        let mut s = Snap::free(constrain(anchor, p, step));
        s.kind = Some(SnapKind::Axis);
        s.guides.push((anchor, s.point));
        return s;
    }

    // 1. Узлы стен (и размеров).
    let node = derived
        .nodes
        .iter()
        .filter(|(id, _)| !excluded(id))
        .filter(|(id, _)| {
            walls.iter().any(|(w, ..)| &&w.a == id || &&w.b == id)
                || doc.walls.iter().all(|w| &&w.a != id && &&w.b != id)
        })
        .map(|(id, q)| (id, *q, q.distance(p)))
        .filter(|(.., d)| *d <= radius)
        .min_by(|x, y| x.2.total_cmp(&y.2));
    if let Some((id, q, _)) = node {
        return Snap {
            point: q,
            kind: Some(SnapKind::Endpoint),
            node: Some(id.clone()),
            wall: None,
            guides: Vec::new(),
        };
    }

    // 2. Середины стен.
    if let Some((w, mid)) = walls
        .iter()
        .map(|(w, a, b)| (w, a.midpoint(*b)))
        .filter(|(_, m)| m.distance(p) <= radius)
        .min_by(|x, y| x.1.distance(p).total_cmp(&y.1.distance(p)))
    {
        return Snap {
            point: mid,
            kind: Some(SnapKind::Midpoint),
            node: None,
            wall: Some(w.id.clone()),
            guides: Vec::new(),
        };
    }

    // 3. Перпендикуляр из предыдущей точки на стену.
    if let Some(anchor) = input.anchor {
        let foot = walls
            .iter()
            .filter_map(|(w, a, b)| {
                let t = segment::project_param(*a, *b, anchor);
                (0.0..=1.0)
                    .contains(&t)
                    .then(|| (w, a.lerp(*b, t)))
                    .filter(|(_, f)| f.distance(p) <= radius && f.distance(anchor) > 1.0)
            })
            .min_by(|x, y| x.1.distance(p).total_cmp(&y.1.distance(p)));
        if let Some((w, f)) = foot {
            return Snap {
                point: f,
                kind: Some(SnapKind::Perpendicular),
                node: None,
                wall: Some(w.id.clone()),
                guides: vec![(anchor, f)],
            };
        }
    }

    // 4. Выравнивание по узлам (по X и Y) и по углам 0/90° от предыдущей точки.
    let mut point = p;
    let mut guides = Vec::new();
    let mut aligned = false;
    let mut refs: Vec<DVec2> = derived
        .nodes
        .iter()
        .filter(|(id, _)| !excluded(id))
        .map(|(_, q)| *q)
        .collect();
    if let Some(anchor) = input.anchor {
        refs.push(anchor);
    }
    let best_x = refs
        .iter()
        .filter(|q| (q.x - p.x).abs() <= radius)
        .min_by(|a, b| (a.x - p.x).abs().total_cmp(&(b.x - p.x).abs()));
    let best_y = refs
        .iter()
        .filter(|q| (q.y - p.y).abs() <= radius)
        .min_by(|a, b| (a.y - p.y).abs().total_cmp(&(b.y - p.y).abs()));
    if let Some(q) = best_x {
        point.x = q.x;
        aligned = true;
    }
    if let Some(q) = best_y {
        point.y = q.y;
        aligned = true;
    }
    if let Some(q) = best_x {
        guides.push((*q, point));
    }
    if let Some(q) = best_y {
        guides.push((*q, point));
    }

    // 5. Точка на оси стены (для Т-стыков) — с учётом выравнивания вдоль стены.
    let on_wall = walls
        .iter()
        .map(|(w, a, b)| (w, segment::closest_point(*a, *b, p)))
        .filter(|(w, q)| q.distance(p) <= radius.max(w.thickness / 2.0))
        .min_by(|x, y| x.1.distance(p).total_cmp(&y.1.distance(p)));
    if let Some((w, q)) = on_wall {
        let (a, b) = derived.wall_axis(w).unwrap_or((q, q));
        // Если выравнивание дало точку на оси — оставляем его, иначе — проекция, округлённая вдоль стены.
        let snapped = if aligned && segment::distance(a, b, point) < 1e-6 {
            point
        } else {
            let len = a.distance(b);
            let t = segment::project_param(a, b, p).clamp(0.0, 1.0);
            let along = round_to(t * len, step.clamp(1.0, 10.0));
            a.lerp(b, (along / len.max(1e-9)).clamp(0.0, 1.0))
        };
        return Snap {
            point: snapped,
            kind: Some(SnapKind::OnWall),
            node: None,
            wall: Some(w.id.clone()),
            guides,
        };
    }

    if aligned {
        // Оставшуюся координату — по сетке.
        if best_x.is_none() {
            point.x = round_to(point.x, step);
        }
        if best_y.is_none() {
            point.y = round_to(point.y, step);
        }
        return Snap {
            point,
            kind: Some(SnapKind::Axis),
            node: None,
            wall: None,
            guides,
        };
    }

    // 6. Сетка.
    Snap {
        point: DVec2::new(round_to(p.x, step), round_to(p.y, step)),
        kind: Some(SnapKind::Grid),
        node: None,
        wall: None,
        guides: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use op_model::{Command, Element, Justification, LayerKind, Node, Wall};

    use super::*;

    fn doc_with_wall() -> Document {
        let mut doc = Document::new("т");
        let put = |e: Element| Command::Put {
            element: e,
            index: None,
        };
        Command::Batch {
            commands: vec![
                put(Element::Node(Node {
                    id: "a".into(),
                    x: 0.0,
                    y: 0.0,
                })),
                put(Element::Node(Node {
                    id: "b".into(),
                    x: 3000.0,
                    y: 0.0,
                })),
                put(Element::Wall(Wall {
                    id: "w".into(),
                    layer: LayerKind::Walls.id().into(),
                    a: "a".into(),
                    b: "b".into(),
                    thickness: 100.0,
                    justification: Justification::Center,
                    height: None,
                })),
            ],
        }
        .apply(&mut doc)
        .unwrap();
        doc
    }

    fn input(cursor: DVec2) -> SnapInput<'static> {
        SnapInput {
            cursor,
            px_per_mm: 0.1,
            anchor: None,
            ortho: false,
            disabled: false,
            exclude: &[],
        }
    }

    #[test]
    fn snaps_to_node_then_midpoint_then_wall() {
        let doc = doc_with_wall();
        let d = Derived::new(&doc);
        let s = snap(&doc, &d, input(DVec2::new(40.0, 30.0)));
        assert_eq!(s.kind, Some(SnapKind::Endpoint));
        assert_eq!(s.node.as_deref(), Some("a"));
        let s = snap(&doc, &d, input(DVec2::new(1530.0, 20.0)));
        assert_eq!(s.kind, Some(SnapKind::Midpoint));
        let s = snap(&doc, &d, input(DVec2::new(800.0, 40.0)));
        assert_eq!(s.kind, Some(SnapKind::OnWall));
        assert!(s.point.y.abs() < 1e-9);
    }

    #[test]
    fn grid_when_nothing_near() {
        let doc = doc_with_wall();
        let d = Derived::new(&doc);
        let s = snap(&doc, &d, input(DVec2::new(1234.0, 5678.0)));
        assert_eq!(s.kind, Some(SnapKind::Grid));
        assert_eq!(s.point, DVec2::new(1200.0, 5700.0));
    }

    #[test]
    fn ortho_constrains_direction() {
        let doc = Document::new("т");
        let d = Derived::new(&doc);
        let mut i = input(DVec2::new(1000.0, 90.0));
        i.anchor = Some(DVec2::ZERO);
        i.ortho = true;
        let s = snap(&doc, &d, i);
        assert!(s.point.y.abs() < 1e-9);
        assert!((s.point.x - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn alt_disables() {
        let doc = doc_with_wall();
        let d = Derived::new(&doc);
        let mut i = input(DVec2::new(40.0, 30.0));
        i.disabled = true;
        assert_eq!(snap(&doc, &d, i).kind, None);
    }
}
