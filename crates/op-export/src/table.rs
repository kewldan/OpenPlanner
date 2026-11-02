//! Таблицы листа: экспликация, ведомости, легенда. Те же таблицы уходят в XLSX.

use glam::DVec2;

use crate::{
    font::SheetFont,
    sheet::{Align, Item, Rgb, Shape, Text},
};

/// Ячейка.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    /// Текст.
    Text(String),
    /// Число с заданным числом знаков после запятой.
    Number {
        /// Значение.
        value: f64,
        /// Знаков после запятой.
        decimals: u8,
    },
    /// Образец условного обозначения (легенда): примитивы в квадрате 1 × 1.
    Swatch(Vec<Shape>),
}

impl Cell {
    /// Текстовая ячейка.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    /// Число.
    #[must_use]
    pub fn number(value: f64, decimals: u8) -> Self {
        Self::Number { value, decimals }
    }

    /// Текст ячейки на листе: числа с запятой и пробелами между разрядами.
    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::Text(t) => t.clone(),
            Self::Number { value, decimals } => format_number(*value, *decimals),
            Self::Swatch(_) => String::new(),
        }
    }
}

/// Число с запятой и неразрывными пробелами между разрядами: 12 345,67.
#[must_use]
pub fn format_number(value: f64, decimals: u8) -> String {
    let text = format!("{:.*}", usize::from(decimals), value.abs());
    let (int, frac) = text.split_once('.').unwrap_or((text.as_str(), ""));
    let mut grouped = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i).is_multiple_of(3) {
            grouped.push('\u{a0}');
        }
        grouped.push(c);
    }
    let sign = if value < 0.0 && text.chars().any(|c| c.is_ascii_digit() && c != '0') {
        "−"
    } else {
        ""
    };
    if frac.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped},{frac}")
    }
}

/// Столбец.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    /// Заголовок.
    pub header: String,
    /// Ширина на листе, мм.
    pub width: f64,
    /// Выравнивание содержимого.
    pub align: Align,
}

impl Column {
    /// Столбец.
    #[must_use]
    pub fn new(header: impl Into<String>, width: f64, align: Align) -> Self {
        Self {
            header: header.into(),
            width,
            align,
        }
    }
}

/// Таблица с заголовком и необязательной итоговой строкой.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    /// Название таблицы над ней.
    pub title: String,
    /// Столбцы.
    pub columns: Vec<Column>,
    /// Строки.
    pub rows: Vec<Vec<Cell>>,
    /// Итоговая строка.
    pub footer: Option<Vec<Cell>>,
}

/// Высота строки названия, мм.
const TITLE_BAND: f64 = 8.0;
/// Высота строки заголовков, мм.
const HEADER_ROW: f64 = 8.0;
/// Высота строки, мм.
const ROW: f64 = 6.0;
/// Высота текста в таблице, мм.
const TEXT: f64 = 2.5;
/// Поле ячейки, мм.
const PAD: f64 = 1.5;

/// Толщина основной линии, мм.
pub const THICK: f64 = 0.5;
/// Толщина тонкой линии, мм.
pub const THIN: f64 = 0.18;

impl Table {
    /// Ширина таблицы, мм.
    #[must_use]
    pub fn width(&self) -> f64 {
        self.columns.iter().map(|c| c.width).sum()
    }

    /// Высота таблицы с названием, мм.
    #[must_use]
    pub fn height(&self) -> f64 {
        let rows = self.rows.len() + usize::from(self.footer.is_some());
        #[allow(clippy::cast_precision_loss)] // строк — единицы и десятки
        let body = rows as f64 * ROW;
        TITLE_BAND + HEADER_ROW + body
    }

    /// Рисует таблицу; `top_left` — левый верхний угол строки названия.
    pub fn draw(&self, layer: usize, top_left: DVec2, out: &mut Vec<Item>) {
        let width = self.width();
        let x0 = top_left.x;
        out.push(Item::text(
            layer,
            Text::centered(
                top_left + DVec2::new(width / 2.0, TITLE_BAND / 2.0),
                &self.title,
                3.5,
                Align::Center,
            ),
        ));
        let mut y = top_left.y + TITLE_BAND;
        let top = y;
        // Заголовки.
        let mut x = x0;
        for c in &self.columns {
            cell_text(
                layer,
                &c.header,
                Align::Center,
                DVec2::new(x, y),
                c.width,
                HEADER_ROW,
                out,
            );
            x += c.width;
        }
        y += HEADER_ROW;
        out.push(Item::line(
            layer,
            DVec2::new(x0, y),
            DVec2::new(x0 + width, y),
            THICK,
            Rgb::BLACK,
        ));
        let rows = self.rows.iter().chain(self.footer.as_ref());
        for (i, row) in rows.enumerate() {
            let footer = self.footer.is_some() && i == self.rows.len();
            if footer || i > 0 {
                out.push(Item::line(
                    layer,
                    DVec2::new(x0, y),
                    DVec2::new(x0 + width, y),
                    if footer { THICK } else { THIN },
                    Rgb::BLACK,
                ));
            }
            let mut x = x0;
            for (c, cell) in self.columns.iter().zip(row) {
                if let Cell::Swatch(shapes) = cell {
                    swatch(layer, shapes, DVec2::new(x, y), c.width, ROW, out);
                } else {
                    cell_text(
                        layer,
                        &cell.display(),
                        c.align,
                        DVec2::new(x, y),
                        c.width,
                        ROW,
                        out,
                    );
                }
                x += c.width;
            }
            y += ROW;
        }
        // Вертикальные линии и рамка.
        let mut x = x0;
        for c in &self.columns[..self.columns.len().saturating_sub(1)] {
            x += c.width;
            out.push(Item::line(
                layer,
                DVec2::new(x, top),
                DVec2::new(x, y),
                THICK,
                Rgb::BLACK,
            ));
        }
        out.push(Item::rect(
            layer,
            DVec2::new(x0, top),
            DVec2::new(x0 + width, y),
            THICK,
        ));
    }
}

/// Высота текста, при которой строка помещается в ширину `width`.
#[must_use]
pub fn fit_height(text: &str, height: f64, width: f64) -> f64 {
    let Some(font) = SheetFont::shared() else {
        return height;
    };
    let w = font.text_width(text, height);
    if w <= width || w <= 0.0 {
        height
    } else {
        (height * width / w).max(1.2)
    }
}

/// Текст в ячейке: по вертикали по центру, по горизонтали — с полем.
pub fn cell_text(
    layer: usize,
    text: &str,
    align: Align,
    lo: DVec2,
    width: f64,
    height: f64,
    out: &mut Vec<Item>,
) {
    if text.is_empty() {
        return;
    }
    let size = fit_height(text, TEXT, width - 2.0 * PAD);
    let x = match align {
        Align::Left => lo.x + PAD,
        Align::Center => lo.x + width / 2.0,
        Align::Right => lo.x + width - PAD,
    };
    out.push(Item::text(
        layer,
        Text::centered(DVec2::new(x, lo.y + height / 2.0), text, size, align),
    ));
}

/// Образец легенды в ячейке: квадрат 1 × 1 растягивается в прямоугольник с полями.
fn swatch(layer: usize, shapes: &[Shape], lo: DVec2, width: f64, height: f64, out: &mut Vec<Item>) {
    let inner_lo = lo + DVec2::new(PAD * 2.0, 1.2);
    let size = DVec2::new(width - PAD * 4.0, height - 2.4);
    let map = |p: DVec2| inner_lo + p * size;
    for shape in shapes {
        let shape = match shape {
            Shape::Fill { rings, color } => Shape::Fill {
                rings: rings
                    .iter()
                    .map(|r| r.iter().map(|p| map(*p)).collect())
                    .collect(),
                color: *color,
            },
            Shape::Stroke {
                points,
                closed,
                width,
                color,
                dash,
            } => Shape::Stroke {
                points: points.iter().map(|p| map(*p)).collect(),
                closed: *closed,
                width: *width,
                color: *color,
                dash: *dash,
            },
            Shape::Text(t) => Shape::Text(Text {
                at: map(t.at),
                ..t.clone()
            }),
        };
        out.push(Item { layer, shape });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_use_comma_and_groups() {
        assert_eq!(format_number(12345.678, 2), "12\u{a0}345,68");
        assert_eq!(format_number(5.0, 0), "5");
        assert_eq!(format_number(-3.5, 1), "−3,5");
        assert_eq!(format_number(-0.001, 2), "0,00");
    }

    #[test]
    fn table_draws_rows_and_fits_text() {
        let table = Table {
            title: "Экспликация помещений".into(),
            columns: vec![
                Column::new("№", 10.0, Align::Center),
                Column::new("Наименование", 40.0, Align::Left),
                Column::new("Площадь, м²", 25.0, Align::Right),
            ],
            rows: vec![
                vec![
                    Cell::text("1"),
                    Cell::text("Кухня-гостиная с очень длинным названием"),
                    Cell::number(20.5, 2),
                ],
                vec![
                    Cell::text("2"),
                    Cell::text("Спальня"),
                    Cell::number(12.0, 2),
                ],
            ],
            footer: Some(vec![
                Cell::text(""),
                Cell::text("Итого"),
                Cell::number(32.5, 2),
            ]),
        };
        assert!((table.width() - 75.0).abs() < 1e-9);
        assert!((table.height() - (8.0 + 8.0 + 18.0)).abs() < 1e-9);
        let mut out = Vec::new();
        table.draw(0, DVec2::ZERO, &mut out);
        let texts: Vec<&Text> = out
            .iter()
            .filter_map(|i| match &i.shape {
                Shape::Text(t) => Some(t),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t.text == "32,50"));
        let long = texts
            .iter()
            .find(|t| t.text.starts_with("Кухня"))
            .map(|t| t.height);
        assert!(long.is_some_and(|h| h < 2.5), "длинное название ужато");
    }
}
