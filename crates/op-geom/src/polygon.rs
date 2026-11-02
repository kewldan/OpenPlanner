//! Замкнутые контуры: площадь, периметр, принадлежность точки.
//!
//! Контур задаётся без повторения первой точки в конце.

use glam::DVec2;

/// Ориентированная площадь замкнутого контура (формула Гаусса), мм².
///
/// Положительна для обхода по часовой стрелке в экранных координатах (Y вниз).
#[must_use]
pub fn signed_area(points: &[DVec2]) -> f64 {
    let n = points.len();
    if n < 3 {
        return 0.0;
    }
    let twice: f64 = (0..n)
        .map(|i| points[i].perp_dot(points[(i + 1) % n]))
        .sum();
    twice / 2.0
}

/// Площадь замкнутого контура без учёта направления обхода, мм².
#[must_use]
pub fn area(points: &[DVec2]) -> f64 {
    signed_area(points).abs()
}

/// Периметр замкнутого контура, мм.
#[must_use]
pub fn perimeter(points: &[DVec2]) -> f64 {
    let n = points.len();
    if n < 2 {
        return 0.0;
    }
    (0..n)
        .map(|i| points[i].distance(points[(i + 1) % n]))
        .sum()
}

/// Лежит ли точка внутри контура (правило чёт-нечет).
#[must_use]
pub fn contains(points: &[DVec2], p: DVec2) -> bool {
    let n = points.len();
    let mut inside = false;
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let (a, b) = (points[i], points[j]);
        if (a.y > p.y) != (b.y > p.y) {
            let x = (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x;
            if p.x < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// Центр масс контура. Для вырожденного контура — среднее точек.
#[must_use]
pub fn centroid(points: &[DVec2]) -> DVec2 {
    let n = points.len();
    if n == 0 {
        return DVec2::ZERO;
    }
    let a = signed_area(points);
    if a.abs() < 1e-9 {
        #[allow(clippy::cast_precision_loss)] // точек в контуре заведомо меньше 2^52
        return points.iter().copied().sum::<DVec2>() / n as f64;
    }
    let mut c = DVec2::ZERO;
    for i in 0..n {
        let (p, q) = (points[i], points[(i + 1) % n]);
        c += (p + q) * p.perp_dot(q);
    }
    c / (6.0 * a)
}

/// Точка заведомо внутри области: внешний контур `outer` минус `holes`.
///
/// Берётся центр масс, если он внутри области, иначе — середина самого
/// широкого отрезка горизонтали через центр масс (и соседних горизонталей).
#[must_use]
pub fn interior_point(outer: &[DVec2], holes: &[Vec<DVec2>]) -> Option<DVec2> {
    let inside = |p: DVec2| contains(outer, p) && !holes.iter().any(|h| contains(h, p));
    let c = centroid(outer);
    if inside(c) {
        return Some(c);
    }
    let (min_y, max_y) = outer.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
        (lo.min(p.y), hi.max(p.y))
    });
    if min_y >= max_y {
        return None;
    }
    let mut best: Option<(f64, DVec2)> = None;
    for k in 1..16 {
        let y = min_y + (max_y - min_y) * f64::from(k) / 16.0;
        let mut xs = Vec::new();
        for contour in std::iter::once(outer).chain(holes.iter().map(Vec::as_slice)) {
            let n = contour.len();
            for i in 0..n {
                let (a, b) = (contour[i], contour[(i + 1) % n]);
                if (a.y > y) != (b.y > y) {
                    xs.push((b.x - a.x) * (y - a.y) / (b.y - a.y) + a.x);
                }
            }
        }
        xs.sort_by(f64::total_cmp);
        for [x0, x1] in xs.as_chunks::<2>().0 {
            let width = x1 - x0;
            let mid = DVec2::new(f64::midpoint(*x0, *x1), y);
            if best.is_none_or(|(w, _)| width > w) && inside(mid) {
                best = Some((width, mid));
            }
        }
    }
    best.map(|(_, p)| p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: f64, h: f64) -> Vec<DVec2> {
        vec![
            DVec2::ZERO,
            DVec2::new(w, 0.0),
            DVec2::new(w, h),
            DVec2::new(0.0, h),
        ]
    }

    #[test]
    fn rectangle_area_and_perimeter() {
        let room = rect(4000.0, 5000.0);
        assert!((crate::mm2_to_m2(area(&room)) - 20.0).abs() < 1e-9);
        assert!((perimeter(&room) - 18_000.0).abs() < 1e-9);
    }

    #[test]
    fn orientation_changes_sign_only() {
        let mut room = rect(3000.0, 2000.0);
        let cw = signed_area(&room);
        room.reverse();
        assert!((cw + signed_area(&room)).abs() < 1e-9);
    }

    #[test]
    fn degenerate_contours() {
        assert!(area(&[]).abs() < f64::EPSILON);
        assert!(area(&[DVec2::ZERO, DVec2::X]).abs() < f64::EPSILON);
    }

    #[test]
    fn point_in_polygon() {
        let room = rect(100.0, 100.0);
        assert!(contains(&room, DVec2::new(50.0, 50.0)));
        assert!(!contains(&room, DVec2::new(150.0, 50.0)));
    }

    #[test]
    fn centroid_of_rectangle() {
        let c = centroid(&rect(400.0, 200.0));
        assert!(c.distance(DVec2::new(200.0, 100.0)) < 1e-9);
    }

    #[test]
    fn interior_point_of_l_shape() {
        // Г-образная комната: центр масс попадает в вырез.
        let l = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(1000.0, 0.0),
            DVec2::new(1000.0, 100.0),
            DVec2::new(100.0, 100.0),
            DVec2::new(100.0, 1000.0),
            DVec2::new(0.0, 1000.0),
        ];
        assert!(!contains(&l, centroid(&l)));
        let p = interior_point(&l, &[]).unwrap();
        assert!(contains(&l, p));
    }
}
