//! Раскладка штучного материала (плитка, доска) на поверхности.
//!
//! Поверхность — контур с вырезами (колонны на полу, проёмы на стене). Сетка
//! элементов строится в своей системе координат: ряды вдоль X, шов между
//! элементами, старт — точка, где начинается целый элемент. Каждая ячейка сетки
//! обрезается по поверхности (Сазерленд — Ходжмен по прямоугольнику ячейки):
//! целые элементы, подрезанные и узкие подрезки (уже четверти элемента).

use glam::DVec2;

use crate::polygon;

/// Узкая подрезка: кусок уже этой доли элемента.
pub const NARROW: f64 = 0.25;
/// Больше ячеек не перебирается: мозаика на большой площади считается по площади.
const MAX_CELLS: f64 = 40_000.0;
/// Кандидатов старта по каждой оси при подборе.
const CANDIDATES: u32 = 12;

/// Рисунок раскладки.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pattern {
    /// Шов в шов.
    Straight,
    /// Вразбежку: ряд сдвинут на долю длины относительно предыдущего.
    Offset(f64),
    /// По диагонали (45°).
    Diagonal,
}

/// Элемент и раскладка.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spec {
    /// Длина элемента вдоль ряда, мм.
    pub length: f64,
    /// Ширина элемента поперёк ряда, мм.
    pub width: f64,
    /// Шов, мм.
    pub joint: f64,
    /// Рисунок.
    pub pattern: Pattern,
    /// Старт: угол целого элемента в координатах поверхности, мм.
    pub origin: DVec2,
}

/// Поверхность: внешний контур и вырезы.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Surface {
    /// Контур.
    pub outer: Vec<DVec2>,
    /// Вырезы.
    pub holes: Vec<Vec<DVec2>>,
}

impl Surface {
    /// Прямоугольник от `lo` до `hi`.
    #[must_use]
    pub fn rect(lo: DVec2, hi: DVec2) -> Self {
        Self {
            outer: rect(lo, hi),
            holes: Vec::new(),
        }
    }

    /// Площадь без вырезов, мм².
    #[must_use]
    pub fn area(&self) -> f64 {
        polygon::area(&self.outer) - self.holes.iter().map(|h| polygon::area(h)).sum::<f64>()
    }
}

/// Кусок элемента на поверхности.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    /// Контур куска в координатах поверхности (без вычета вырезов).
    pub outline: Vec<DVec2>,
    /// Целый элемент.
    pub whole: bool,
    /// Узкая подрезка.
    pub narrow: bool,
}

/// Итог раскладки.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Layout {
    /// Целых элементов.
    pub whole: usize,
    /// Подрезанных элементов (каждый — из отдельного элемента).
    pub cut: usize,
    /// Узких подрезок среди подрезанных.
    pub narrow: usize,
    /// Площадь поверхности, мм².
    pub area: f64,
    /// Куски (если просили), иначе пусто.
    pub pieces: Vec<Piece>,
    /// Сетка слишком мелкая: элементы посчитаны по площади.
    pub approximate: bool,
}

impl Layout {
    /// Элементов нужно (без запаса).
    #[must_use]
    pub fn tiles(&self) -> usize {
        self.whole + self.cut
    }

    fn add(&mut self, other: Self) {
        self.whole += other.whole;
        self.cut += other.cut;
        self.narrow += other.narrow;
        self.area += other.area;
        self.pieces.extend(other.pieces);
        self.approximate |= other.approximate;
    }
}

fn rect(lo: DVec2, hi: DVec2) -> Vec<DVec2> {
    vec![lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)]
}

/// Обрезка контура прямоугольником `[lo, hi]` (Сазерленд — Ходжмен).
#[must_use]
pub fn clip(points: &[DVec2], lo: DVec2, hi: DVec2) -> Vec<DVec2> {
    let mut out: Vec<DVec2> = points.to_vec();
    // Четыре полуплоскости: x ≥ lo.x, x ≤ hi.x, y ≥ lo.y, y ≤ hi.y.
    let planes: [(usize, f64, bool); 4] = [
        (0, lo.x, true),
        (0, hi.x, false),
        (1, lo.y, true),
        (1, hi.y, false),
    ];
    for (axis, at, keep_above) in planes {
        if out.is_empty() {
            break;
        }
        let inside = |p: DVec2| {
            let v = if axis == 0 { p.x } else { p.y };
            if keep_above { v >= at } else { v <= at }
        };
        let cross = |a: DVec2, b: DVec2| {
            let (va, vb) = if axis == 0 { (a.x, b.x) } else { (a.y, b.y) };
            let t = (at - va) / (vb - va);
            a + (b - a) * t
        };
        let input = std::mem::take(&mut out);
        let n = input.len();
        for i in 0..n {
            let (a, b) = (input[(i + n - 1) % n], input[i]);
            match (inside(a), inside(b)) {
                (true, true) => out.push(b),
                (true, false) => out.push(cross(a, b)),
                (false, true) => {
                    out.push(cross(a, b));
                    out.push(b);
                }
                (false, false) => {}
            }
        }
    }
    out
}

fn bounds(points: &[DVec2]) -> (DVec2, DVec2) {
    points.iter().fold(
        (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)),
        |(lo, hi), p| (lo.min(*p), hi.max(*p)),
    )
}

fn rotate(p: DVec2, about: DVec2, angle: f64) -> DVec2 {
    let (s, c) = angle.sin_cos();
    let d = p - about;
    about + DVec2::new(d.x * c - d.y * s, d.x * s + d.y * c)
}

/// Раскладка на одной поверхности.
#[must_use]
pub fn layout(surface: &Surface, spec: &Spec, pieces: bool) -> Layout {
    let area = surface.area();
    let tile_area = spec.length * spec.width;
    if spec.length <= 0.0 || spec.width <= 0.0 || surface.outer.len() < 3 || area <= 0.0 {
        return Layout {
            area: area.max(0.0),
            ..Layout::default()
        };
    }
    // Диагональ: поверхность поворачивается вокруг старта на −45°, сетка — по осям.
    let angle = if spec.pattern == Pattern::Diagonal {
        -std::f64::consts::FRAC_PI_4
    } else {
        0.0
    };
    let to_grid = |p: &DVec2| rotate(*p, spec.origin, angle) - spec.origin;
    let outer: Vec<DVec2> = surface.outer.iter().map(to_grid).collect();
    let holes: Vec<Vec<DVec2>> = surface
        .holes
        .iter()
        .map(|h| h.iter().map(to_grid).collect())
        .collect();
    let module = DVec2::new(spec.length + spec.joint, spec.width + spec.joint);
    let shift = match spec.pattern {
        Pattern::Offset(s) => s.rem_euclid(1.0) * module.x,
        _ => 0.0,
    };
    let (lo, hi) = bounds(&outer);
    let j0 = (lo.y / module.y).floor();
    let j1 = (hi.y / module.y).ceil();
    let rows = (j1 - j0).max(0.0);
    let cols = ((hi.x - lo.x) / module.x).ceil() + 2.0;
    if rows * cols > MAX_CELLS {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // число элементов ≥ 0
        let whole = (area / tile_area).ceil() as usize;
        return Layout {
            whole,
            area,
            approximate: true,
            ..Layout::default()
        };
    }
    let mut out = Layout {
        area,
        ..Layout::default()
    };
    #[allow(clippy::cast_possible_truncation)] // строк и столбцов — не больше MAX_CELLS
    let (j0, j1) = (j0 as i64, j1 as i64);
    for j in j0..j1 {
        #[allow(clippy::cast_precision_loss)] // номера рядов малы
        let y = j as f64 * module.y;
        #[allow(clippy::cast_precision_loss)]
        let row_shift = (j as f64 * shift).rem_euclid(module.x);
        let i0 = ((lo.x - row_shift) / module.x).floor();
        let i1 = ((hi.x - row_shift) / module.x).ceil();
        #[allow(clippy::cast_possible_truncation)]
        let (i0, i1) = (i0 as i64, i1 as i64);
        for i in i0..i1 {
            #[allow(clippy::cast_precision_loss)]
            let x = i as f64 * module.x + row_shift;
            let (a, b) = (
                DVec2::new(x, y),
                DVec2::new(x + spec.length, y + spec.width),
            );
            let piece = clip(&outer, a, b);
            if piece.len() < 3 {
                continue;
            }
            let covered = polygon::area(&piece)
                - holes
                    .iter()
                    .map(|h| polygon::area(&clip(h, a, b)))
                    .sum::<f64>();
            if covered <= tile_area * 1e-4 {
                continue;
            }
            let whole = covered >= tile_area * (1.0 - 1e-6);
            let (plo, phi) = bounds(&piece);
            let size = phi - plo;
            let narrow = !whole && (size.x < spec.length * NARROW || size.y < spec.width * NARROW);
            if whole {
                out.whole += 1;
            } else {
                out.cut += 1;
                if narrow {
                    out.narrow += 1;
                }
            }
            if pieces {
                out.pieces.push(Piece {
                    outline: piece
                        .iter()
                        .map(|p| rotate(*p + spec.origin, spec.origin, -angle))
                        .collect(),
                    whole,
                    narrow,
                });
            }
        }
    }
    out
}

/// Раскладка на нескольких поверхностях с одним стартом.
#[must_use]
pub fn layout_all(surfaces: &[Surface], spec: &Spec, pieces: bool) -> Layout {
    let mut out = Layout::default();
    for s in surfaces {
        out.add(layout(s, spec, pieces));
    }
    out
}

/// Старт с наименьшим числом узких подрезок, затем — подрезок вообще.
/// Перебирает сдвиги сетки на доли модуля и сдвиги «элемент по центру».
#[must_use]
pub fn best_origin(surfaces: &[Surface], spec: &Spec) -> DVec2 {
    let module = DVec2::new(spec.length + spec.joint, spec.width + spec.joint);
    let base = surfaces
        .iter()
        .flat_map(|s| s.outer.iter().copied())
        .fold(DVec2::splat(f64::INFINITY), DVec2::min);
    if !base.is_finite() || module.x <= 0.0 || module.y <= 0.0 {
        return spec.origin;
    }
    let mut best = (usize::MAX, usize::MAX, spec.origin);
    for i in 0..CANDIDATES {
        for j in 0..CANDIDATES {
            let t = DVec2::new(f64::from(i), f64::from(j)) / f64::from(CANDIDATES);
            let origin = base + t * module;
            let l = layout_all(surfaces, &Spec { origin, ..*spec }, false);
            let score = (l.narrow, l.cut, origin);
            if (score.0, score.1) < (best.0, best.1) {
                best = score;
            }
        }
    }
    best.2
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(origin: DVec2) -> Spec {
        Spec {
            length: 300.0,
            width: 300.0,
            joint: 0.0,
            pattern: Pattern::Straight,
            origin,
        }
    }

    #[test]
    fn exact_fit_has_only_whole_tiles() {
        let s = Surface::rect(DVec2::ZERO, DVec2::new(1200.0, 900.0));
        let l = layout(&s, &spec(DVec2::ZERO), true);
        assert_eq!((l.whole, l.cut, l.narrow), (12, 0, 0));
        assert_eq!(l.pieces.len(), 12);
    }

    #[test]
    fn shifted_start_cuts_edges_and_finds_narrow_strips() {
        let s = Surface::rect(DVec2::ZERO, DVec2::new(1000.0, 600.0));
        // Старт в нуле: 3 целых в ряду и полоса 100 мм — не узкая (больше четверти, 75 мм).
        let l = layout(&s, &spec(DVec2::ZERO), false);
        assert_eq!((l.whole, l.cut, l.narrow), (6, 2, 0));
        // Старт 50 мм: полосы 50 и 50 — обе узкие.
        let l = layout(&s, &spec(DVec2::new(50.0, 0.0)), false);
        assert_eq!((l.whole, l.cut, l.narrow), (6, 4, 4));
    }

    #[test]
    fn holes_are_not_covered() {
        let mut s = Surface::rect(DVec2::ZERO, DVec2::new(900.0, 900.0));
        s.holes
            .push(rect(DVec2::new(300.0, 300.0), DVec2::new(600.0, 600.0)));
        let l = layout(&s, &spec(DVec2::ZERO), false);
        assert_eq!((l.whole, l.cut), (8, 0));
        assert!((l.area - 720_000.0).abs() < 1e-6);
    }

    #[test]
    fn offset_rows_are_shifted() {
        let s = Surface::rect(DVec2::ZERO, DVec2::new(1200.0, 600.0));
        let mut sp = spec(DVec2::ZERO);
        sp.pattern = Pattern::Offset(0.5);
        let l = layout(&s, &sp, false);
        // Первый ряд — 4 целых, второй — 3 целых и две половинки.
        assert_eq!((l.whole, l.cut), (7, 2));
    }

    #[test]
    fn diagonal_covers_the_area_with_cuts() {
        let s = Surface::rect(DVec2::ZERO, DVec2::new(1200.0, 1200.0));
        let mut sp = spec(DVec2::ZERO);
        sp.pattern = Pattern::Diagonal;
        let l = layout(&s, &sp, true);
        let covered: f64 = l.pieces.iter().map(|p| polygon::area(&p.outline)).sum();
        assert!((covered - 1_440_000.0).abs() < 1.0, "{covered}");
        assert!(l.cut > 0);
    }

    #[test]
    fn best_origin_avoids_narrow_strips() {
        let s = vec![Surface::rect(DVec2::ZERO, DVec2::new(1000.0, 600.0))];
        let bad = layout_all(&s, &spec(DVec2::new(50.0, 0.0)), false);
        assert!(bad.narrow > 0);
        let origin = best_origin(&s, &spec(DVec2::new(50.0, 0.0)));
        let good = layout_all(&s, &spec(origin), false);
        assert_eq!(good.narrow, 0);
        assert!(good.cut <= 2);
    }

    #[test]
    fn clip_keeps_the_inside() {
        let tri = [DVec2::ZERO, DVec2::new(200.0, 0.0), DVec2::new(0.0, 200.0)];
        let c = clip(&tri, DVec2::ZERO, DVec2::splat(100.0));
        assert!((polygon::area(&c) - 10_000.0).abs() < 1e-6);
    }
}
