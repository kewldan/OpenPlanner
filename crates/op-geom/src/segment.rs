//! Отрезки и прямые.

use glam::DVec2;

/// Параметр проекции точки `p` на отрезок `a`–`b` (0 — в `a`, 1 — в `b`), без ограничения.
#[must_use]
pub fn project_param(a: DVec2, b: DVec2, p: DVec2) -> f64 {
    let d = b - a;
    let len2 = d.length_squared();
    if len2 < 1e-12 {
        return 0.0;
    }
    (p - a).dot(d) / len2
}

/// Ближайшая к `p` точка отрезка `a`–`b`.
#[must_use]
pub fn closest_point(a: DVec2, b: DVec2, p: DVec2) -> DVec2 {
    a.lerp(b, project_param(a, b, p).clamp(0.0, 1.0))
}

/// Расстояние от точки до отрезка.
#[must_use]
pub fn distance(a: DVec2, b: DVec2, p: DVec2) -> f64 {
    closest_point(a, b, p).distance(p)
}

/// Пересечение прямых `p + t·d` и `q + s·e`. `None`, если прямые почти параллельны.
#[must_use]
pub fn line_intersection(p: DVec2, d: DVec2, q: DVec2, e: DVec2) -> Option<DVec2> {
    let denom = d.perp_dot(e);
    if denom.abs() < 1e-9 * d.length() * e.length() {
        return None;
    }
    let t = (q - p).perp_dot(e) / denom;
    Some(p + d * t)
}

/// Пересечение отрезков `a`–`b` и `c`–`d` (включая концы).
#[must_use]
pub fn segment_intersection(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> Option<DVec2> {
    let (r, s) = (b - a, d - c);
    let denom = r.perp_dot(s);
    if denom.abs() < 1e-12 {
        return None;
    }
    let t = (c - a).perp_dot(s) / denom;
    let u = (c - a).perp_dot(r) / denom;
    let eps = 1e-9;
    ((-eps..=1.0 + eps).contains(&t) && (-eps..=1.0 + eps).contains(&u)).then(|| a + r * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_and_distance() {
        let (a, b) = (DVec2::ZERO, DVec2::new(100.0, 0.0));
        assert!((project_param(a, b, DVec2::new(25.0, 40.0)) - 0.25).abs() < 1e-12);
        assert!((distance(a, b, DVec2::new(25.0, 40.0)) - 40.0).abs() < 1e-12);
        assert!((distance(a, b, DVec2::new(-30.0, 40.0)) - 50.0).abs() < 1e-12);
    }

    #[test]
    fn lines_cross() {
        let p = line_intersection(DVec2::ZERO, DVec2::X, DVec2::new(5.0, -5.0), DVec2::Y).unwrap();
        assert!(p.distance(DVec2::new(5.0, 0.0)) < 1e-12);
        assert!(line_intersection(DVec2::ZERO, DVec2::X, DVec2::Y, DVec2::X).is_none());
    }

    #[test]
    fn segments_cross_only_within_bounds() {
        let hit = segment_intersection(
            DVec2::ZERO,
            DVec2::new(10.0, 10.0),
            DVec2::new(0.0, 10.0),
            DVec2::new(10.0, 0.0),
        );
        assert!(hit.unwrap().distance(DVec2::new(5.0, 5.0)) < 1e-12);
        assert!(
            segment_intersection(
                DVec2::ZERO,
                DVec2::new(1.0, 1.0),
                DVec2::new(0.0, 10.0),
                DVec2::new(10.0, 0.0)
            )
            .is_none()
        );
    }
}
