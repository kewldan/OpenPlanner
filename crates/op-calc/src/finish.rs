//! Расход отделочных материалов: плитка, клей, затирка, краска, обои, доска.
//!
//! Нормы — типовые для бытового ремонта: клей по размеру плитки (зубчатый шпатель
//! растёт с размером), затирка по формуле объёма шва, краска — 0,12 л/м² на слой.

/// Запас плитки на бой и подбор рисунка (подрезки уже посчитаны поштучно).
pub const TILE_RESERVE: f64 = 0.05;
/// Запас ламината и доски на подрезку по площади.
pub const BOARD_RESERVE: f64 = 0.07;
/// Расход краски на один слой, л/м².
pub const PAINT_RATE: f64 = 0.12;
/// Слоёв краски по умолчанию.
pub const PAINT_COATS: f64 = 2.0;
/// Плотность затирки, кг/дм³.
pub const GROUT_DENSITY: f64 = 1.6;
/// Мешок плиточного клея, кг.
pub const ADHESIVE_BAG: f64 = 25.0;
/// Упаковка затирки, кг.
pub const GROUT_PACK: f64 = 2.0;

/// Плиток с запасом на бой.
#[must_use]
pub fn tiles_with_reserve(pieces: usize) -> u32 {
    #[allow(clippy::cast_precision_loss)] // плиток — тысячи, не больше
    let n = (pieces as f64 * (1.0 + TILE_RESERVE)).ceil();
    whole(n)
}

/// Целое число упаковок, вмещающих `amount` при `per_pack` в упаковке.
#[must_use]
pub fn packs(amount: f64, per_pack: f64) -> u32 {
    if per_pack <= 0.0 || amount <= 0.0 {
        return 0;
    }
    whole((amount / per_pack - 1e-9).ceil())
}

fn whole(v: f64) -> u32 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // ограничено диапазоном u32
    let n = v.clamp(0.0, f64::from(u32::MAX)) as u32;
    n
}

/// Расход плиточного клея, кг/м², по большей стороне плитки (мм).
#[must_use]
pub fn adhesive_rate(width: f64, length: f64) -> f64 {
    match width.max(length) {
        s if s <= 100.0 => 2.0,
        s if s <= 300.0 => 4.0,
        s if s <= 600.0 => 6.0,
        _ => 8.0,
    }
}

/// Расход затирки, кг/м²: (A + B) / (A·B) · толщина · шов · плотность (размеры — мм).
#[must_use]
pub fn grout_rate(width: f64, length: f64, thickness: f64, joint: f64) -> f64 {
    if width <= 0.0 || length <= 0.0 {
        return 0.0;
    }
    (width + length) / (width * length) * thickness * joint * GROUT_DENSITY
}

/// Краска, л: площадь (м²) × слои × норма.
#[must_use]
pub fn paint_liters(area: f64, coats: f64) -> f64 {
    area.max(0.0) * coats * PAINT_RATE
}

/// Рулонов обоев: полос по периметру (мм) при высоте стен (мм); из рулона —
/// целое число полос, запас 100 мм на полосу для подгонки.
#[must_use]
pub fn wallpaper_rolls(perimeter: f64, height: f64, roll_width: f64, roll_length: f64) -> u32 {
    if perimeter <= 0.0 || roll_width <= 0.0 || roll_length <= 0.0 {
        return 0;
    }
    let strips = (perimeter / roll_width - 1e-9).ceil();
    let per_roll = (roll_length / (height + 100.0)).floor();
    if per_roll < 1.0 {
        return 0;
    }
    whole((strips / per_roll).ceil())
}

/// Площадь доски с запасом, м².
#[must_use]
pub fn boards_area(area: f64) -> f64 {
    area.max(0.0) * (1.0 + BOARD_RESERVE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grout_for_a_common_tile() {
        // 300 × 300 × 8, шов 3 мм: ≈ 0,26 кг/м².
        let r = grout_rate(300.0, 300.0, 8.0, 3.0);
        assert!((r - 0.256).abs() < 1e-9, "{r}");
    }

    #[test]
    fn adhesive_grows_with_tile_size() {
        assert!(adhesive_rate(100.0, 100.0) < adhesive_rate(300.0, 600.0));
        assert!((adhesive_rate(600.0, 1200.0) - 8.0).abs() < 1e-9);
    }

    #[test]
    fn packs_round_up() {
        assert_eq!(packs(10.0, 4.0), 3);
        assert_eq!(packs(8.0, 4.0), 2);
        assert_eq!(packs(0.0, 4.0), 0);
        assert_eq!(tiles_with_reserve(100), 105);
    }

    #[test]
    fn wallpaper_by_strips() {
        // Периметр 14 м, высота 2,7 м, рулон 1,06 × 10,05 м: 14 полос, 3 полосы из рулона.
        assert_eq!(wallpaper_rolls(14_000.0, 2_700.0, 1_060.0, 10_050.0), 5);
    }

    #[test]
    fn paint_two_coats() {
        assert!((paint_liters(30.0, 2.0) - 7.2).abs() < 1e-9);
    }
}
