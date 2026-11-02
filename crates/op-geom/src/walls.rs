//! Контуры стен со стыками в узлах.
//!
//! Стены — рёбра графа узлов. У стены своя толщина по каждую сторону от
//! линии узлов (левая и правая сторона — по направлению от узла `a` к узлу `b`).
//! В узле, где сходятся несколько стен, соседние по углу стороны продлеваются
//! до пересечения (усовое соединение). Слишком острый угол срезается.
//! Т-стыки не требуют разбиения стены: конец примыкающей стены лежит на оси
//! основной, и объединение контуров даёт чистый стык.

use glam::DVec2;

use crate::{left_normal, segment::line_intersection};

/// Стена для расчёта контура.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallInput {
    /// Индекс начального узла.
    pub a: usize,
    /// Индекс конечного узла.
    pub b: usize,
    /// Толщина слева от линии `a → b`, мм.
    pub left: f64,
    /// Толщина справа от линии `a → b`, мм.
    pub right: f64,
}

/// Во сколько раз усовый угол может выступать дальше толщины стены, прежде чем его срежут.
const MITER_LIMIT: f64 = 4.0;

/// Углы конца стены: слева и справа, если смотреть из узла вдоль стены.
#[derive(Debug, Clone, Copy)]
struct EndCorners {
    left: DVec2,
    right: DVec2,
}

/// Конец стены, примыкающий к узлу.
#[derive(Debug, Clone, Copy)]
struct Incident {
    wall: usize,
    /// Конец `a` (`false`) или `b` (`true`).
    at_b: bool,
    dir: DVec2,
    angle: f64,
    left: f64,
    right: f64,
}

/// Контуры стен (по одному на стену, в порядке `walls`).
///
/// Стена нулевой длины или со ссылкой на несуществующий узел даёт пустой контур.
/// Контуры ориентированы одинаково (положительная [`signed_area`](crate::signed_area)).
#[must_use]
pub fn wall_outlines(nodes: &[DVec2], walls: &[WallInput]) -> Vec<Vec<DVec2>> {
    let valid = |w: &WallInput| {
        w.a < nodes.len() && w.b < nodes.len() && nodes[w.a].distance(nodes[w.b]) > 1e-6
    };

    let mut incident: Vec<Vec<Incident>> = vec![Vec::new(); nodes.len()];
    for (i, w) in walls.iter().enumerate() {
        if !valid(w) {
            continue;
        }
        let dir = (nodes[w.b] - nodes[w.a]).normalize();
        incident[w.a].push(Incident {
            wall: i,
            at_b: false,
            dir,
            angle: dir.y.atan2(dir.x),
            left: w.left,
            right: w.right,
        });
        // Из узла `b` стена смотрит в обратную сторону: стороны меняются местами.
        incident[w.b].push(Incident {
            wall: i,
            at_b: true,
            dir: -dir,
            angle: (-dir.y).atan2(-dir.x),
            left: w.right,
            right: w.left,
        });
    }

    let mut ends: Vec<[Option<EndCorners>; 2]> = vec![[None, None]; walls.len()];
    let mut degree = vec![0usize; nodes.len()];
    for (node, list) in incident.iter_mut().enumerate() {
        degree[node] = list.len();
        list.sort_by(|x, y| x.angle.total_cmp(&y.angle));
        let p = nodes[node];
        let n = list.len();
        let mut corners: Vec<EndCorners> = list
            .iter()
            .map(|e| EndCorners {
                left: p + left_normal(e.dir) * e.left,
                right: p - left_normal(e.dir) * e.right,
            })
            .collect();
        if n >= 2 {
            for i in 0..n {
                let j = (i + 1) % n;
                let (ei, ej) = (list[i], list[j]);
                // При обходе по возрастанию угла (Y вниз) между стеной i и
                // следующей j лежит правая сторона i и левая сторона j.
                let pi = p - left_normal(ei.dir) * ei.right;
                let pj = p + left_normal(ej.dir) * ej.left;
                let limit = MITER_LIMIT * ei.right.max(ej.left).max(1.0);
                if let Some(x) = line_intersection(pi, ei.dir, pj, ej.dir)
                    && x.distance(p) <= limit
                {
                    corners[i].right = x;
                    corners[j].left = x;
                }
            }
        }
        for (e, c) in list.iter().zip(corners) {
            ends[e.wall][usize::from(e.at_b)] = Some(c);
        }
    }

    walls
        .iter()
        .zip(ends)
        .map(|(w, ends)| {
            let [Some(at_a), Some(at_b)] = ends else {
                return Vec::new();
            };
            let mut outline = Vec::with_capacity(6);
            // Левая сторона a → b, затем торец у b, правая сторона обратно, торец у a.
            outline.push(at_a.left);
            outline.push(at_b.right);
            if degree[w.b] >= 2 {
                outline.push(nodes[w.b]);
            }
            outline.push(at_b.left);
            outline.push(at_a.right);
            if degree[w.a] >= 2 {
                outline.push(nodes[w.a]);
            }
            if crate::signed_area(&outline) < 0.0 {
                outline.reverse();
            }
            outline
        })
        .collect()
}

/// Прямоугольник проёма в стене `a`–`b` от `from` до `to` мм вдоль оси от `a`.
///
/// Прямоугольник чуть шире стены, чтобы вырез гарантированно прошёл насквозь.
#[must_use]
pub fn opening_rect(a: DVec2, b: DVec2, left: f64, right: f64, from: f64, to: f64) -> Vec<DVec2> {
    let dir = (b - a).normalize_or_zero();
    let n = left_normal(dir);
    let (s, e) = (a + dir * from, a + dir * to);
    let (l, r) = (left + 1.0, right + 1.0);
    let mut rect = vec![s + n * l, e + n * l, e - n * r, s - n * r];
    if crate::signed_area(&rect) < 0.0 {
        rect.reverse();
    }
    rect
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{area, contains};

    fn wall(a: usize, b: usize, t: f64) -> WallInput {
        WallInput {
            a,
            b,
            left: t / 2.0,
            right: t / 2.0,
        }
    }

    #[test]
    fn single_wall_is_rectangle() {
        let nodes = [DVec2::ZERO, DVec2::new(1000.0, 0.0)];
        let outlines = wall_outlines(&nodes, &[wall(0, 1, 200.0)]);
        assert!((area(&outlines[0]) - 200_000.0).abs() < 1e-6);
    }

    #[test]
    fn l_corner_is_mitered() {
        let nodes = [
            DVec2::ZERO,
            DVec2::new(1000.0, 0.0),
            DVec2::new(1000.0, 1000.0),
        ];
        let outlines = wall_outlines(&nodes, &[wall(0, 1, 200.0), wall(1, 2, 200.0)]);
        // Внешний угол (1100, -100) и внутренний (900, 100) принадлежат обеим стенам.
        for outline in &outlines {
            assert!(
                outline
                    .iter()
                    .any(|p| p.distance(DVec2::new(1100.0, -100.0)) < 1e-6)
            );
            assert!(
                outline
                    .iter()
                    .any(|p| p.distance(DVec2::new(900.0, 100.0)) < 1e-6)
            );
        }
    }

    #[test]
    fn one_sided_thickness() {
        let nodes = [DVec2::ZERO, DVec2::new(1000.0, 0.0)];
        let w = WallInput {
            a: 0,
            b: 1,
            left: 0.0,
            right: 100.0,
        };
        let outline = &wall_outlines(&nodes, &[w])[0];
        // Справа от направления вправо (Y вниз) — это положительный Y.
        assert!(contains(outline, DVec2::new(500.0, 50.0)));
        assert!(!contains(outline, DVec2::new(500.0, -50.0)));
    }

    #[test]
    fn straight_continuation_keeps_width() {
        let nodes = [
            DVec2::ZERO,
            DVec2::new(1000.0, 0.0),
            DVec2::new(2000.0, 0.0),
        ];
        let outlines = wall_outlines(&nodes, &[wall(0, 1, 100.0), wall(1, 2, 100.0)]);
        assert!((area(&outlines[0]) - 100_000.0).abs() < 1e-6);
        assert!((area(&outlines[1]) - 100_000.0).abs() < 1e-6);
    }

    #[test]
    fn sharp_angle_is_clipped() {
        let nodes = [DVec2::ZERO, DVec2::new(1000.0, 0.0), DVec2::new(0.0, 10.0)];
        let outlines = wall_outlines(&nodes, &[wall(0, 1, 100.0), wall(1, 2, 100.0)]);
        for outline in &outlines {
            for p in outline {
                assert!(p.distance(nodes[1]) < 1000.0 + MITER_LIMIT * 50.0 + 1.0);
            }
        }
    }

    #[test]
    fn degenerate_wall_is_empty() {
        let nodes = [DVec2::ZERO, DVec2::ZERO];
        assert!(wall_outlines(&nodes, &[wall(0, 1, 100.0)])[0].is_empty());
        assert!(wall_outlines(&nodes, &[wall(0, 5, 100.0)])[0].is_empty());
    }
}
