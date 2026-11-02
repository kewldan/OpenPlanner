//! Климат квартиры: тёплый пол, воздухообмен, охлаждение.
//!
//! Воздухообмен — по смыслу СП 54.13330 для квартир: жилые комнаты — 3 м³/ч
//! на 1 м² площади, кухня с электроплитой — 60 м³/ч, ванная и туалет — по 25,
//! совмещённый санузел — 50. Охлаждение — оценка 0,1 кВт на 1 м² при потолке
//! 2,7 м (бытовые помещения без больших окон на юг).

/// Приток или вытяжка бризера, м³/ч.
pub const BREATHER_AIR: f64 = 120.0;
/// Вытяжной вентилятор, м³/ч.
pub const FAN_AIR: f64 = 100.0;
/// Решётка естественной вытяжки на канале, м³/ч.
pub const GRILLE_AIR: f64 = 30.0;
/// Типовые бытовые сплит-системы по холоду, кВт.
pub const AC_SIZES: [f64; 6] = [2.0, 2.5, 3.5, 5.0, 7.0, 8.0];

/// Площадь обогрева: пол без отступа от стен и без мебели на полу, м².
#[must_use]
pub fn heated_area(area: f64, perimeter: f64, margin_mm: f64, furniture: f64) -> f64 {
    let a = area - perimeter * margin_mm / 1000.0 - furniture;
    (a.max(0.0) * 100.0).round() / 100.0
}

/// Нужный воздухообмен помещения по названию и площади, м³/ч (0 — не нормируется).
#[must_use]
pub fn air_required(name: &str, area: f64) -> f64 {
    let n = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if has(&["кухн"]) {
        60.0
    } else if has(&["совмещ"]) {
        50.0
    } else if has(&["ванн", "санузел", "туалет", "с/у", "уборн", "душ"])
    {
        25.0
    } else if has(&[
        "коридор",
        "прихож",
        "холл",
        "тамбур",
        "кладов",
        "гардероб",
        "балкон",
        "лоджи",
    ]) {
        0.0
    } else {
        (area * 3.0).ceil()
    }
}

/// Рекомендуемая мощность охлаждения, кВт (до десятых).
#[must_use]
pub fn cooling_kw(area: f64, ceiling_mm: f64) -> f64 {
    (area * 0.1 * ceiling_mm / 2700.0 * 10.0).ceil() / 10.0
}

/// Ближайшая типовая сплит-система не меньше нужной мощности.
#[must_use]
pub fn ac_size(kw: f64) -> f64 {
    AC_SIZES
        .iter()
        .copied()
        .find(|s| *s >= kw - 1e-9)
        .unwrap_or(AC_SIZES[AC_SIZES.len() - 1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heating_area_excludes_margin_and_furniture() {
        // 4 × 3 м, отступ 100 мм, кровать 3,2 м².
        assert!((heated_area(12.0, 14.0, 100.0, 3.2) - 7.4).abs() < 1e-9);
        assert!((heated_area(2.0, 6.0, 500.0, 0.0)).abs() < 1e-9);
    }

    #[test]
    fn air_by_room() {
        assert!((air_required("Кухня-гостиная", 25.0) - 60.0).abs() < 1e-9);
        assert!((air_required("Спальня", 14.2) - 43.0).abs() < 1e-9);
        assert!((air_required("Санузел", 4.0) - 25.0).abs() < 1e-9);
        assert!(air_required("Коридор", 8.0).abs() < 1e-9);
    }

    #[test]
    fn cooling_and_unit_size() {
        assert!((cooling_kw(20.0, 2700.0) - 2.0).abs() < 1e-9);
        assert!((cooling_kw(20.0, 3000.0) - 2.3).abs() < 1e-9);
        assert!((ac_size(2.3) - 2.5).abs() < 1e-9);
        assert!((ac_size(9.0) - 8.0).abs() < 1e-9);
    }
}
