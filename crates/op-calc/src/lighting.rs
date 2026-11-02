//! Освещённость помещений методом коэффициента использования и нормы для жилья.
//!
//! E = Φ·η / (A·Kз): Φ — суммарный поток светильников, η — коэффициент
//! использования по индексу помещения i = A / (h·(a + b)), Kз — коэффициент запаса.
//! Коэффициент использования — аппроксимация для светлых потолка и стен.

/// Коэффициент запаса для жилых помещений со светодиодными светильниками.
pub const MAINTENANCE: f64 = 1.25;
/// Высота рабочей поверхности, мм.
pub const WORK_PLANE: f64 = 800.0;

/// Индекс помещения по площади (м²), периметру (м) и высоте подвеса над рабочей поверхностью (м).
#[must_use]
pub fn room_index(area: f64, perimeter: f64, height: f64) -> f64 {
    if perimeter <= 0.0 || height <= 0.0 {
        return 0.0;
    }
    area / (height * perimeter / 2.0)
}

/// Коэффициент использования светового потока по индексу помещения.
#[must_use]
pub fn utilization(index: f64) -> f64 {
    (0.25 + 0.18 * index.max(0.0)).min(0.7)
}

/// Средняя освещённость, лк.
#[must_use]
pub fn illuminance(flux_lm: f64, area_m2: f64, perimeter_m: f64, ceiling_mm: f64) -> f64 {
    if area_m2 <= 0.0 {
        return 0.0;
    }
    let h = ((ceiling_mm - WORK_PLANE) / 1000.0).max(0.5);
    let eta = utilization(room_index(area_m2, perimeter_m, h));
    flux_lm * eta / (area_m2 * MAINTENANCE)
}

/// Нормируемая освещённость жилого помещения по названию, лк.
#[must_use]
pub fn norm(room_name: &str) -> f64 {
    let n = room_name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if has(&["кабинет", "рабоч", "мастерск"]) {
        300.0
    } else if has(&["детск"]) {
        200.0
    } else if has(&[
        "ванн",
        "санузел",
        "туалет",
        "с/у",
        "уборн",
        "душ",
        "коридор",
        "прихож",
        "холл",
        "тамбур",
        "гардероб",
        "кладов",
    ]) {
        50.0
    } else {
        150.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn illuminance_of_a_living_room() {
        // 4 × 4 м, потолок 2,7 м, 3000 лм: i = 16 / (1,9·8) ≈ 1,05, η ≈ 0,44, E ≈ 66 лк.
        let e = illuminance(3000.0, 16.0, 16.0, 2700.0);
        assert!((e - 66.0).abs() < 2.0, "{e}");
        assert!(illuminance(3000.0, 0.0, 0.0, 2700.0).abs() < 1e-12);
        // Больше света — больше освещённость.
        assert!(illuminance(9000.0, 16.0, 16.0, 2700.0) > 150.0);
    }

    #[test]
    fn norms_by_room_name() {
        assert!((norm("Кухня-гостиная") - 150.0).abs() < 1e-9);
        assert!((norm("Ванная") - 50.0).abs() < 1e-9);
        assert!((norm("Кабинет") - 300.0).abs() < 1e-9);
        assert!((norm("Детская") - 200.0).abs() < 1e-9);
        assert!((norm("Прихожая") - 50.0).abs() < 1e-9);
        assert!((norm("Помещение 1") - 150.0).abs() < 1e-9);
    }
}
