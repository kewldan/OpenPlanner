//! Смета: количество по объёму из модели, норме и запасу; стоимость.

/// Количество позиции: объём × норма × (1 + запас). Штучное — целым вверх,
/// остальное — до сотых.
#[must_use]
pub fn quantity(measured: f64, rate: f64, reserve: f64, whole: bool) -> f64 {
    let q = measured.max(0.0) * rate.max(0.0) * (1.0 + reserve.max(0.0));
    if whole {
        (q - 1e-9).ceil().max(0.0)
    } else {
        (q * 100.0).round() / 100.0
    }
}

/// Штучная единица: количество округляется вверх до целого.
#[must_use]
pub fn is_whole(unit: &str) -> bool {
    matches!(
        unit.trim().trim_end_matches('.').to_lowercase().as_str(),
        "шт" | "рул" | "уп" | "компл" | "меш" | "банка" | "банок"
    )
}

/// Стоимость до копеек.
#[must_use]
pub fn cost(quantity: f64, price: f64) -> f64 {
    (quantity * price * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantity_with_rate_and_reserve() {
        assert!((quantity(10.0, 2.0, 0.1, false) - 22.0).abs() < 1e-9);
        assert!((quantity(3.333, 1.0, 0.0, false) - 3.33).abs() < 1e-9);
        assert!((quantity(4.2, 1.0, 0.0, true) - 5.0).abs() < 1e-9);
        assert!((quantity(4.0, 1.0, 0.0, true) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn whole_units_and_cost() {
        assert!(is_whole("шт.") && is_whole("рул.") && !is_whole("м²"));
        assert!((cost(2.5, 199.99) - 499.98).abs() < 1e-9);
    }
}
