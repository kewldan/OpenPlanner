//! Штриховка контуров параллельными линиями.

use glam::DVec2;

/// Отрезки штриховки контуров (правило чёт-нечет): линии под углом `angle` (радианы)
/// через `spacing`. Сетка линий привязана к началу координат, поэтому штриховка
/// соседних контуров совпадает.
#[must_use]
pub fn hatch(rings: &[Vec<DVec2>], angle: f64, spacing: f64) -> Vec<(DVec2, DVec2)> {
    if spacing <= 0.0 || !spacing.is_finite() {
        return Vec::new();
    }
    // Поворачиваем так, чтобы линии штриховки стали горизонтальными.
    let (sin, cos) = angle.sin_cos();
    let to_local = |p: DVec2| DVec2::new(p.x * cos + p.y * sin, -p.x * sin + p.y * cos);
    let to_world = |p: DVec2| DVec2::new(p.x * cos - p.y * sin, p.x * sin + p.y * cos);
    let local: Vec<Vec<DVec2>> = rings
        .iter()
        .map(|r| r.iter().map(|p| to_local(*p)).collect())
        .collect();
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in local.iter().flatten() {
        lo = lo.min(p.y);
        hi = hi.max(p.y);
    }
    if !lo.is_finite() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut k = (lo / spacing).ceil();
    while k * spacing <= hi {
        let y = k * spacing;
        let mut xs: Vec<f64> = Vec::new();
        for ring in &local {
            for (i, a) in ring.iter().enumerate() {
                let b = ring[(i + 1) % ring.len()];
                // Полуоткрытый интервал: вершина на линии считается один раз.
                if (a.y <= y) != (b.y <= y) {
                    xs.push(a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x));
                }
            }
        }
        xs.sort_by(f64::total_cmp);
        for [x0, x1] in xs.as_chunks::<2>().0 {
            if x1 - x0 > 1e-9 {
                out.push((to_world(DVec2::new(*x0, y)), to_world(DVec2::new(*x1, y))));
            }
        }
        k += 1.0;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(lo: f64, hi: f64) -> Vec<DVec2> {
        vec![
            DVec2::new(lo, lo),
            DVec2::new(hi, lo),
            DVec2::new(hi, hi),
            DVec2::new(lo, hi),
        ]
    }

    #[test]
    fn horizontal_lines_fill_square() {
        let lines = hatch(&[square(0.5, 10.5)], 0.0, 1.0);
        assert_eq!(lines.len(), 10);
        for (a, b) in &lines {
            assert!((a.x - 0.5).abs() < 1e-9 && (b.x - 10.5).abs() < 1e-9);
        }
    }

    #[test]
    fn holes_are_skipped() {
        let lines = hatch(&[square(0.5, 10.5), square(3.5, 6.5)], 0.0, 1.0);
        // Строки 4–6 разбиты дыркой на два отрезка.
        assert_eq!(lines.len(), 13);
        let total: f64 = lines.iter().map(|(a, b)| a.distance(*b)).sum();
        assert!((total - (100.0 - 9.0)).abs() < 1e-6, "{total}");
    }

    #[test]
    fn diagonal_hatch_stays_inside() {
        let ring = square(0.0, 10.0);
        let lines = hatch(
            std::slice::from_ref(&ring),
            std::f64::consts::FRAC_PI_4,
            1.0,
        );
        assert!(!lines.is_empty());
        for (a, b) in lines {
            for p in [a, b] {
                assert!(p.x > -1e-6 && p.x < 10.0 + 1e-6 && p.y > -1e-6 && p.y < 10.0 + 1e-6);
            }
        }
    }
}
