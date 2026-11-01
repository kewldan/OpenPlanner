//! Геометрия OpenPlanner.
//!
//! Координаты плана — миллиметры, ось X вправо, ось Y вниз (как на экране и листе).

pub use glam::DVec2;

/// Ориентированная площадь замкнутого контура (формула Гаусса), мм².
///
/// Положительна для обхода по часовой стрелке в экранных координатах (Y вниз).
/// Контур задаётся без повторения первой точки в конце.
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

/// Перевод мм² в м².
#[must_use]
pub fn mm2_to_m2(value: f64) -> f64 {
    value / 1_000_000.0
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
        assert!((mm2_to_m2(area(&room)) - 20.0).abs() < 1e-9);
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
}
