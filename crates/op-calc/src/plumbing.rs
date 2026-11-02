//! Сантехника: уклоны канализации, отметки сливов, трубы и фитинги.
//!
//! Уклоны — по смыслу СП 30.13330 для внутренней канализации: для труб до 50 мм
//! нормальный 0,03 и наименьший 0,025, для 85–110 мм — 0,02 и 0,012, наибольший
//! — 0,15. Трубы считаются хлыстами: водопровод — 4 м, канализация — 2 м.

/// Наибольший уклон канализации.
pub const MAX_SLOPE: f64 = 0.15;
/// Запас труб на обрезки.
pub const PIPE_RESERVE: f64 = 0.1;

/// Нормальный и наименьший уклон канализационной трубы диаметра `d` (мм).
#[must_use]
pub fn slopes(d: f64) -> (f64, f64) {
    if d <= 50.0 {
        (0.03, 0.025)
    } else if d <= 110.0 {
        (0.02, 0.012)
    } else {
        (0.01, 0.007)
    }
}

/// Длина хлыста трубы, м.
#[must_use]
pub fn stick(water: bool) -> f64 {
    if water { 4.0 } else { 2.0 }
}

/// Хлыстов на длину `length` (м) с запасом.
#[must_use]
pub fn sticks(length: f64, water: bool) -> u32 {
    let n = (length.max(0.0) * (1.0 + PIPE_RESERVE) / stick(water) - 1e-9).ceil();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    // хлыстов — единицы и десятки
    let n = n.clamp(0.0, f64::from(u32::MAX)) as u32;
    n
}

/// Нарушение на трубе.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Issue {
    /// Уклон меньше наименьшего (фактический, наименьший).
    FlatSlope(f64, f64),
    /// Уклон больше наибольшего.
    SteepSlope(f64),
    /// Труба идёт вверх по ходу стока.
    Uphill,
    /// Конец трубы ниже врезки в стояк: слив не дотягивается (не хватает, мм).
    BelowInlet(f64),
}

/// Участок трубы: длина по горизонтали и перепад отметок (начало минус конец), мм.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Run {
    /// Длина в плане, мм.
    pub length: f64,
    /// Перепад отметок, мм (положительный — труба опускается).
    pub fall: f64,
}

/// Проверки канализационной трубы диаметра `d`: уклоны горизонтальных участков,
/// подъёмы, отметка конца относительно врезки (если труба приходит в стояк).
#[must_use]
pub fn check_drain(d: f64, runs: &[Run], end: f64, inlet: Option<f64>) -> Vec<Issue> {
    let (_, min) = slopes(d);
    let mut out = Vec::new();
    let horizontal: Vec<&Run> = runs.iter().filter(|r| r.length > 1.0).collect();
    if horizontal.iter().any(|r| r.fall < -0.5) {
        out.push(Issue::Uphill);
    }
    let slopes: Vec<f64> = horizontal.iter().map(|r| r.fall / r.length).collect();
    if let Some(flat) = slopes
        .iter()
        .copied()
        .filter(|s| *s >= 0.0)
        .reduce(f64::min)
        && flat < min - 1e-6
    {
        out.push(Issue::FlatSlope(flat, min));
    }
    if let Some(steep) = slopes.iter().copied().reduce(f64::max)
        && steep > MAX_SLOPE + 1e-6
    {
        out.push(Issue::SteepSlope(steep));
    }
    if let Some(inlet) = inlet
        && end < inlet - 0.5
    {
        out.push(Issue::BelowInlet(inlet - end));
    }
    out
}

/// Фитинг по углу поворота (градусы) трубы: водопровод — отвод 90° или 45°,
/// канализация — поворот на 90° двумя отводами 45° (плавнее для стока).
#[must_use]
pub fn bends(angle: f64, water: bool) -> (u32, u32) {
    if angle < 10.0 {
        (0, 0)
    } else if water {
        if angle > 60.0 { (1, 0) } else { (0, 1) }
    } else if angle > 60.0 {
        (0, 2)
    } else {
        (0, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slopes_by_diameter() {
        assert_eq!(slopes(50.0), (0.03, 0.025));
        assert_eq!(slopes(110.0), (0.02, 0.012));
    }

    #[test]
    fn sticks_with_reserve() {
        assert_eq!(sticks(7.0, true), 2);
        assert_eq!(sticks(7.5, true), 3);
        assert_eq!(sticks(3.0, false), 2);
    }

    #[test]
    fn drain_checks() {
        let ok = [Run {
            length: 1000.0,
            fall: 30.0,
        }];
        assert!(check_drain(50.0, &ok, 470.0, Some(400.0)).is_empty());
        let flat = [Run {
            length: 2000.0,
            fall: 20.0,
        }];
        assert_eq!(
            check_drain(50.0, &flat, 80.0, None),
            vec![Issue::FlatSlope(0.01, 0.025)]
        );
        assert_eq!(
            check_drain(50.0, &ok, 70.0, Some(100.0)),
            vec![Issue::BelowInlet(30.0)]
        );
        let up = [Run {
            length: 1000.0,
            fall: -10.0,
        }];
        assert!(check_drain(110.0, &up, 0.0, None).contains(&Issue::Uphill));
    }

    #[test]
    fn bends_by_angle() {
        assert_eq!(bends(90.0, true), (1, 0));
        assert_eq!(bends(90.0, false), (0, 2));
        assert_eq!(bends(45.0, false), (0, 1));
        assert_eq!(bends(2.0, true), (0, 0));
    }
}
