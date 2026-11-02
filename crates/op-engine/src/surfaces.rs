//! Поверхности помещения: грани контура (стены по развёртке) с проёмами и мебелью.
//!
//! Грани нумеруются по часовой стрелке от верхней левой вершины и получают
//! буквы А, Б, В… — одинаково на развёртках и в отделке стен.

use std::collections::HashSet;

use glam::DVec2;
use op_geom::left_normal;
use op_model::{Document, OpeningKind};

use crate::derived::{Derived, opening_span};

/// Грани короче этого (мм) не разворачиваются: торцы колонн, стыки.
pub(crate) const MIN_FACE: f64 = 100.0;
/// Буквы стен (без похожих на цифры и друг на друга, как для осей по ГОСТ 21.101).
pub(crate) const LETTERS: [&str; 22] = [
    "А", "Б", "В", "Г", "Д", "Е", "Ж", "И", "К", "Л", "М", "Н", "П", "Р", "С", "Т", "У", "Ф", "Ц",
    "Ш", "Э", "Ю",
];

/// Проём на грани: от и до вдоль грани, низ и верх от пола, мм.
#[derive(Debug, Clone)]
pub(crate) struct FaceOpening {
    pub kind: OpeningKind,
    pub from: f64,
    pub to: f64,
    pub bottom: f64,
    pub top: f64,
    /// Петли двери — у левого края проёма (если смотреть из помещения).
    pub hinge_left: bool,
}

/// Предмет мебели спинкой к грани: от и до вдоль грани, низ и верх, мм.
#[derive(Debug, Clone)]
pub(crate) struct FaceItem {
    pub from: f64,
    pub to: f64,
    pub bottom: f64,
    pub top: f64,
}

/// Грань помещения.
#[derive(Debug, Clone)]
pub(crate) struct Face {
    pub letter: String,
    pub start: DVec2,
    pub end: DVec2,
    pub length: f64,
    pub openings: Vec<FaceOpening>,
    pub furniture: Vec<FaceItem>,
}

/// Контур по часовой стрелке на экране (ось Y вниз) от верхней левой вершины,
/// без вершин посреди прямой. Так грань от начала к концу видна изнутри слева направо.
pub(crate) fn normalize(contour: &[DVec2]) -> Vec<DVec2> {
    let mut points: Vec<DVec2> = contour.to_vec();
    let twice_area: f64 = (0..points.len())
        .map(|i| points[i].perp_dot(points[(i + 1) % points.len()]))
        .sum();
    if twice_area < 0.0 {
        points.reverse();
    }
    // Вершины на прямой.
    let mut i = 0;
    while points.len() > 3 && i < points.len() {
        let n = points.len();
        let (prev, p, next) = (points[(i + n - 1) % n], points[i], points[(i + 1) % n]);
        let (u, v) = (p - prev, next - p);
        if u.perp_dot(v).abs() <= 1e-6 * u.length() * v.length() && u.dot(v) > 0.0 {
            points.remove(i);
        } else {
            i += 1;
        }
    }
    let start = (0..points.len())
        .min_by(|&a, &b| {
            let (pa, pb) = (points[a], points[b]);
            (pa.y.round(), pa.x)
                .partial_cmp(&(pb.y.round(), pb.x))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(0);
    points.rotate_left(start);
    points
}

/// Грани контура и проёмы в стенах, на гранях которых они лежат.
pub(crate) fn faces(doc: &Document, derived: &Derived, contour: &[DVec2]) -> Vec<Face> {
    let mut out = Vec::new();
    for i in 0..contour.len() {
        let (p, q) = (contour[i], contour[(i + 1) % contour.len()]);
        let length = p.distance(q);
        if length < MIN_FACE {
            continue;
        }
        let dir = (q - p) / length;
        let mut seen = HashSet::new();
        let mut openings = Vec::new();
        for w in doc
            .walls
            .iter()
            .filter(|w| derived.visible.contains(&w.layer))
        {
            let Some((a, b)) = derived.wall_axis(w) else {
                continue;
            };
            let wall_len = a.distance(b);
            if wall_len < 1e-6 {
                continue;
            }
            let wdir = (b - a) / wall_len;
            if wdir.perp_dot(dir).abs() > 1e-4 {
                continue;
            }
            let n = left_normal(wdir);
            let (left, right) = w.sides();
            let on_face = [a + n * left, a - n * right]
                .iter()
                .any(|o| (p - *o).perp_dot(wdir).abs() < 1.0);
            if !on_face {
                continue;
            }
            for o in doc
                .openings
                .iter()
                .filter(|o| o.wall == w.id && derived.visible.contains(&o.layer))
            {
                if !seen.insert(o.id.as_str()) {
                    continue;
                }
                let (from, to) = opening_span(o, wall_len);
                let along = |t: f64| (a + wdir * t - p).dot(dir);
                let (t0, t1) = (along(from), along(to));
                let (lo, hi) = (t0.min(t1).max(0.0), t0.max(t1).min(length));
                if hi - lo < 1.0 {
                    continue;
                }
                let hinge = along(if o.hinge_at_end { to } else { from });
                let bottom = if o.kind == OpeningKind::Window {
                    o.sill
                } else {
                    0.0
                };
                openings.push(FaceOpening {
                    kind: o.kind,
                    from: lo,
                    to: hi,
                    bottom,
                    top: bottom + o.height,
                    hinge_left: (hinge - lo).abs() <= (hinge - hi).abs(),
                });
            }
        }
        openings.sort_by(|a, b| a.from.total_cmp(&b.from));
        // Мебель спинкой к грани (со стороны помещения).
        let mut furniture = Vec::new();
        for f in doc
            .furniture
            .iter()
            .filter(|f| derived.visible.contains(&f.layer))
        {
            let back = [
                crate::furniture::to_plan(f, DVec2::new(-f.width / 2.0, -f.depth / 2.0)),
                crate::furniture::to_plan(f, DVec2::new(f.width / 2.0, -f.depth / 2.0)),
            ];
            let against = back.iter().all(|b| dir.perp_dot(*b - p).abs() < 60.0);
            let inside = dir.perp_dot(DVec2::new(f.x, f.y) - p) > 0.0;
            if !against || !inside {
                continue;
            }
            let (t0, t1) = ((back[0] - p).dot(dir), (back[1] - p).dot(dir));
            let (lo, hi) = (t0.min(t1).max(0.0), t0.max(t1).min(length));
            if hi - lo > 1.0 {
                furniture.push(FaceItem {
                    from: lo,
                    to: hi,
                    bottom: f.elevation,
                    top: f.elevation + f.height,
                });
            }
        }
        let letter = LETTERS.get(out.len()).map_or_else(
            || {
                format!(
                    "{}{}",
                    LETTERS[out.len() % LETTERS.len()],
                    out.len() / LETTERS.len()
                )
            },
            |l| (*l).to_owned(),
        );
        out.push(Face {
            letter,
            start: p,
            end: q,
            length,
            openings,
            furniture,
        });
    }
    out
}

/// Контур и грани помещения в виде (если контур замкнут).
#[must_use]
pub(crate) fn room_faces(
    doc: &Document,
    derived: &Derived,
    room: &str,
) -> Option<(Vec<DVec2>, Vec<Face>)> {
    let region = &derived.regions[*derived.room_regions.get(room)?];
    let contour = normalize(&region.contour);
    let faces = faces(doc, derived, &contour);
    Some((contour, faces))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contour_goes_clockwise_from_the_top_left() {
        // Против часовой стрелки на экране, с лишней вершиной посреди стороны.
        let ccw = [
            DVec2::new(0.0, 0.0),
            DVec2::new(0.0, 3000.0),
            DVec2::new(4000.0, 3000.0),
            DVec2::new(4000.0, 0.0),
            DVec2::new(2000.0, 0.0),
        ];
        let c = normalize(&ccw);
        assert_eq!(
            c,
            [
                DVec2::new(0.0, 0.0),
                DVec2::new(4000.0, 0.0),
                DVec2::new(4000.0, 3000.0),
                DVec2::new(0.0, 3000.0)
            ]
        );
    }

    #[test]
    fn letters_skip_lookalikes() {
        assert!(!LETTERS.contains(&"З") && !LETTERS.contains(&"О") && !LETTERS.contains(&"Ч"));
    }
}
