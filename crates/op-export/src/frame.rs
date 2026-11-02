//! Рамка листа и основная надпись в габаритах формы 3 (ГОСТ Р 21.101): 185 × 55 мм.

use glam::DVec2;

use crate::{
    sheet::{Align, Item, Text},
    table::{THICK, THIN, cell_text, fit_height},
};

/// Графы основной надписи.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TitleBlock {
    /// Обозначение документа (шифр).
    pub designation: String,
    /// Наименование объекта: адрес.
    pub object: String,
    /// Наименование здания (сооружения): название проекта, вариант.
    pub building: String,
    /// Наименование изображения: название листа.
    pub sheet_title: String,
    /// Стадия.
    pub stage: String,
    /// Номер листа.
    pub sheet: u32,
    /// Всего листов.
    pub sheets: u32,
    /// Организация.
    pub organization: String,
    /// Разработал.
    pub developer: String,
    /// Проверил.
    pub checker: String,
    /// Дата (ММ.ГГ).
    pub date: String,
}

/// Ширина основной надписи, мм.
pub const TITLE_WIDTH: f64 = 185.0;
/// Высота основной надписи, мм.
pub const TITLE_HEIGHT: f64 = 55.0;
/// Поле слева (подшивка), мм.
pub const MARGIN_LEFT: f64 = 20.0;
/// Поля сверху, справа и снизу, мм.
pub const MARGIN: f64 = 5.0;

/// Внутренний прямоугольник рамки листа размера `size`.
#[must_use]
pub fn inner(size: DVec2) -> (DVec2, DVec2) {
    (
        DVec2::new(MARGIN_LEFT, MARGIN),
        DVec2::new(size.x - MARGIN, size.y - MARGIN),
    )
}

/// Рамка и основная надпись. Возвращает левый верхний угол основной надписи.
pub fn draw(size: DVec2, block: &TitleBlock, layer: usize, out: &mut Vec<Item>) -> DVec2 {
    let (lo, hi) = inner(size);
    out.push(Item::rect(layer, lo, hi, THICK));
    let o = DVec2::new(hi.x - TITLE_WIDTH, hi.y - TITLE_HEIGHT);
    let p = |x: f64, y: f64| o + DVec2::new(x, y);
    let line = |out: &mut Vec<Item>, a: (f64, f64), b: (f64, f64), w: f64| {
        out.push(Item::line(
            layer,
            p(a.0, a.1),
            p(b.0, b.1),
            w,
            crate::Rgb::BLACK,
        ));
    };
    out.push(Item::rect(layer, o, hi, THICK));

    // Левая часть: таблица изменений (строки 1–5), заголовки (6), подписи (7–11).
    let columns = [0.0, 10.0, 20.0, 30.0, 40.0, 55.0, 65.0];
    for x in &columns[1..6] {
        line(out, (*x, 0.0), (*x, 30.0), THICK);
    }
    for x in [20.0, 40.0, 55.0] {
        line(out, (x, 30.0), (x, 55.0), THICK);
    }
    line(out, (65.0, 0.0), (65.0, 55.0), THICK);
    for row in 1..11 {
        let y = f64::from(row) * 5.0;
        let w = if row == 5 || row == 6 { THICK } else { THIN };
        line(out, (0.0, y), (65.0, y), w);
    }
    let headers = ["Изм.", "Кол.уч", "Лист", "№док.", "Подп.", "Дата"];
    for (i, h) in headers.iter().enumerate() {
        cell_text(
            layer,
            h,
            Align::Center,
            p(columns[i], 25.0),
            columns[i + 1] - columns[i],
            5.0,
            out,
        );
    }
    let roles = [
        ("Разраб.", block.developer.as_str()),
        ("Пров.", block.checker.as_str()),
        ("", ""),
        ("", ""),
        ("Н. контр.", ""),
    ];
    for (i, (role, name)) in roles.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)] // пять строк
        let y = 30.0 + i as f64 * 5.0;
        cell_text(layer, role, Align::Left, p(0.0, y), 20.0, 5.0, out);
        cell_text(layer, name, Align::Left, p(20.0, y), 20.0, 5.0, out);
        if i == 0 && !block.date.is_empty() {
            cell_text(
                layer,
                &block.date,
                Align::Center,
                p(55.0, y),
                10.0,
                5.0,
                out,
            );
        }
    }

    // Правая часть: обозначение, объект, здание, изображение, стадия/лист/листов, организация.
    line(out, (65.0, 15.0), (185.0, 15.0), THICK);
    line(out, (65.0, 30.0), (185.0, 30.0), THICK);
    line(out, (65.0, 40.0), (135.0, 40.0), THICK);
    line(out, (135.0, 30.0), (135.0, 55.0), THICK);
    line(out, (135.0, 35.0), (185.0, 35.0), THICK);
    line(out, (135.0, 40.0), (185.0, 40.0), THICK);
    line(out, (150.0, 30.0), (150.0, 40.0), THICK);
    line(out, (165.0, 30.0), (165.0, 40.0), THICK);

    big(
        layer,
        &block.designation,
        p(65.0, 0.0),
        120.0,
        15.0,
        5.0,
        out,
    );
    big(layer, &block.object, p(65.0, 15.0), 120.0, 15.0, 3.5, out);
    big(layer, &block.building, p(65.0, 30.0), 70.0, 10.0, 3.5, out);
    big(
        layer,
        &block.sheet_title,
        p(65.0, 40.0),
        70.0,
        15.0,
        3.5,
        out,
    );
    for (x, w, h, v) in [
        (135.0, 15.0, "Стадия", block.stage.clone()),
        (150.0, 15.0, "Лист", block.sheet.to_string()),
        (165.0, 20.0, "Листов", block.sheets.to_string()),
    ] {
        cell_text(layer, h, Align::Center, p(x, 30.0), w, 5.0, out);
        cell_text(layer, &v, Align::Center, p(x, 35.0), w, 5.0, out);
    }
    big(
        layer,
        &block.organization,
        p(135.0, 40.0),
        50.0,
        15.0,
        3.5,
        out,
    );
    o
}

/// Крупный текст графы: одна строка по центру, при необходимости — две.
fn big(
    layer: usize,
    text: &str,
    lo: DVec2,
    width: f64,
    height: f64,
    size: f64,
    out: &mut Vec<Item>,
) {
    if text.trim().is_empty() {
        return;
    }
    let avail = width - 4.0;
    let center = lo + DVec2::new(width / 2.0, height / 2.0);
    if fit_height(text, size, avail) >= size * 0.8 {
        let h = fit_height(text, size, avail);
        out.push(Item::text(
            layer,
            Text::centered(center, text, h, Align::Center),
        ));
        return;
    }
    // Две строки: перенос по ближайшему к середине пробелу.
    let chars: Vec<char> = text.chars().collect();
    let mid = chars.len() / 2;
    let split = (0..chars.len())
        .filter(|&i| chars[i] == ' ')
        .min_by_key(|&i| i.abs_diff(mid));
    let (first, second) = match split {
        Some(i) => (
            chars[..i].iter().collect::<String>(),
            chars[i + 1..].iter().collect::<String>(),
        ),
        None => (text.to_owned(), String::new()),
    };
    let h = fit_height(&first, size, avail)
        .min(fit_height(&second, size, avail))
        .min(height / 3.2);
    let gap = h * 0.85;
    out.push(Item::text(
        layer,
        Text::centered(center - DVec2::new(0.0, gap), first, h, Align::Center),
    ));
    out.push(Item::text(
        layer,
        Text::centered(center + DVec2::new(0.0, gap), second, h, Align::Center),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Shape;

    #[test]
    fn title_block_sits_in_the_corner() {
        let block = TitleBlock {
            designation: "2026-01-АР".into(),
            object: "Санкт-Петербург, Комендантский проспект, дом 1, квартира 25".into(),
            building: "Квартира · Вариант А".into(),
            sheet_title: "План демонтажа".into(),
            stage: "Р".into(),
            sheet: 2,
            sheets: 3,
            organization: "".into(),
            developer: "Иванов".into(),
            checker: "".into(),
            date: "11.26".into(),
        };
        let mut out = Vec::new();
        let size = DVec2::new(420.0, 297.0);
        let o = draw(size, &block, 0, &mut out);
        assert!((o - DVec2::new(230.0, 237.0)).length() < 1e-9);
        let texts: Vec<String> = out
            .iter()
            .filter_map(|i| match &i.shape {
                Shape::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect();
        for expected in [
            "План демонтажа",
            "2",
            "3",
            "Иванов",
            "Р",
            "Разраб.",
            "11.26",
        ] {
            assert!(texts.iter().any(|t| t == expected), "{expected}: {texts:?}");
        }
        // Длинный адрес переносится на две строки.
        assert!(
            texts
                .iter()
                .any(|t| t.starts_with("Санкт-Петербург") && t.len() < 60)
        );
    }
}
