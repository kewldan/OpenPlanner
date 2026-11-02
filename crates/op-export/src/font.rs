//! Шрифт листов: метрики, глифы для PDF и контуры для SVG.
//!
//! Используется тот же шрифт, что и на холсте, — Onest (SIL OFL 1.1). Лицензия OFL
//! разрешает встраивание в документы.

use std::sync::OnceLock;

use ab_glyph::{Font as _, FontRef, GlyphId, OutlineCurve};
use glam::DVec2;

/// Файл шрифта (TrueType).
pub const FONT_BYTES: &[u8] = include_bytes!("../../op-render/assets/Onest-Medium.ttf");

/// Имя шрифта в PDF и SVG.
pub const FONT_NAME: &str = "Onest-Medium";

/// Шрифт с метриками в долях em.
#[derive(Debug)]
pub struct SheetFont {
    font: FontRef<'static>,
    units_per_em: f64,
    /// Высота прописных, em.
    pub cap_height: f64,
    /// Подъём, em.
    pub ascent: f64,
    /// Спуск (отрицательный), em.
    pub descent: f64,
    /// Габарит всех глифов в единицах шрифта: xmin, ymin, xmax, ymax.
    pub bbox: [i16; 4],
}

/// Глиф строки.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedGlyph {
    /// Номер глифа в шрифте.
    pub id: u16,
    /// Символ.
    pub ch: char,
    /// Сдвиг от начала строки, em.
    pub x: f64,
    /// Ширина, em.
    pub advance: f64,
}

impl SheetFont {
    /// Общий экземпляр встроенного шрифта.
    #[must_use]
    pub fn shared() -> Option<&'static Self> {
        static FONT: OnceLock<Option<SheetFont>> = OnceLock::new();
        FONT.get_or_init(Self::load).as_ref()
    }

    fn load() -> Option<Self> {
        let font = FontRef::try_from_slice(FONT_BYTES).ok()?;
        let units_per_em = f64::from(font.units_per_em()?);
        // Высота прописной «Н»: габарит контура (знак оси Y у ab_glyph не важен).
        let cap = font
            .outline(font.glyph_id('Н'))
            .map_or(0.7 * units_per_em, |o| {
                f64::from(o.bounds.min.y.abs().max(o.bounds.max.y.abs()))
            });
        Some(Self {
            cap_height: cap / units_per_em,
            ascent: f64::from(font.ascent_unscaled()) / units_per_em,
            descent: f64::from(font.descent_unscaled()) / units_per_em,
            bbox: head_bbox(FONT_BYTES).unwrap_or([-200, -300, 1200, 1000]),
            units_per_em,
            font,
        })
    }

    /// Единиц шрифта в em.
    #[must_use]
    pub fn units_per_em(&self) -> f64 {
        self.units_per_em
    }

    /// Размер em для высоты прописных `height`.
    #[must_use]
    pub fn em(&self, height: f64) -> f64 {
        height / self.cap_height
    }

    /// Раскладка строки (без кернинга — так ширина совпадает во всех форматах).
    #[must_use]
    pub fn layout(&self, text: &str) -> Vec<PlacedGlyph> {
        let mut x = 0.0;
        text.chars()
            .map(|ch| {
                let id = self.font.glyph_id(ch);
                let advance = f64::from(self.font.h_advance_unscaled(id)) / self.units_per_em;
                let glyph = PlacedGlyph {
                    id: id.0,
                    ch,
                    x,
                    advance,
                };
                x += advance;
                glyph
            })
            .collect()
    }

    /// Ширина строки, em.
    #[must_use]
    pub fn width(&self, text: &str) -> f64 {
        self.layout(text).iter().map(|g| g.advance).sum()
    }

    /// Ширина строки высотой прописных `height`, мм.
    #[must_use]
    pub fn text_width(&self, text: &str, height: f64) -> f64 {
        self.width(text) * self.em(height)
    }

    /// Ширина глифа в единицах шрифта.
    #[must_use]
    pub fn advance_units(&self, id: u16) -> f64 {
        f64::from(self.font.h_advance_unscaled(GlyphId(id)))
    }

    /// Контур глифа в em с осью Y вниз, как на листе; начало — точка на базовой линии.
    #[must_use]
    pub fn outline(&self, id: u16) -> Vec<Segment> {
        let Some(outline) = self.font.outline(GlyphId(id)) else {
            return Vec::new();
        };
        let k = 1.0 / self.units_per_em;
        // Контуры шрифта — с осью Y вверх; на листе ось Y вниз.
        let p = |p: ab_glyph::Point| DVec2::new(f64::from(p.x) * k, -f64::from(p.y) * k);
        outline
            .curves
            .iter()
            .map(|c| match c {
                OutlineCurve::Line(a, b) => Segment::Line(p(*a), p(*b)),
                OutlineCurve::Quad(a, c, b) => Segment::Quad(p(*a), p(*c), p(*b)),
                OutlineCurve::Cubic(a, c1, c2, b) => Segment::Cubic(p(*a), p(*c1), p(*c2), p(*b)),
            })
            .collect()
    }
}

/// Отрезок контура глифа.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Segment {
    /// Прямая.
    Line(DVec2, DVec2),
    /// Квадратичная кривая: начало, контрольная, конец.
    Quad(DVec2, DVec2, DVec2),
    /// Кубическая кривая.
    Cubic(DVec2, DVec2, DVec2, DVec2),
}

/// Габарит глифов из таблицы `head`.
fn head_bbox(bytes: &[u8]) -> Option<[i16; 4]> {
    let u16_at = |at: usize| Some(u16::from_be_bytes([*bytes.get(at)?, *bytes.get(at + 1)?]));
    let u32_at = |at: usize| {
        Some(u32::from_be_bytes([
            *bytes.get(at)?,
            *bytes.get(at + 1)?,
            *bytes.get(at + 2)?,
            *bytes.get(at + 3)?,
        ]))
    };
    let tables = usize::from(u16_at(4)?);
    for i in 0..tables {
        let record = 12 + i * 16;
        if bytes.get(record..record + 4)? == b"head" {
            let offset = usize::try_from(u32_at(record + 8)?).ok()?;
            let i16_at =
                |at: usize| u16_at(offset + at).map(|v| i16::from_be_bytes(v.to_be_bytes()));
            return Some([i16_at(36)?, i16_at(38)?, i16_at(40)?, i16_at(42)?]);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_has_cyrillic_metrics() {
        let font = SheetFont::shared();
        assert!(font.is_some());
        let Some(font) = font else { return };
        assert!(
            font.cap_height > 0.5 && font.cap_height < 0.9,
            "{}",
            font.cap_height
        );
        assert!(font.bbox[2] > font.bbox[0] && font.bbox[3] > font.bbox[1]);
        let glyphs = font.layout("Кухня");
        assert_eq!(glyphs.len(), 5);
        assert!(glyphs.iter().all(|g| g.id != 0 && g.advance > 0.0));
        // Высота прописных 5 мм: строка из пяти букв шире 10 мм.
        assert!(font.text_width("Кухня", 5.0) > 10.0);
        // Контур «К» в em: ось Y вниз, от базовой линии вверх до высоты прописных.
        let outline = font.outline(glyphs[0].id);
        assert!(!outline.is_empty());
        let top = outline
            .iter()
            .map(|s| match s {
                Segment::Line(a, b) | Segment::Quad(a, _, b) | Segment::Cubic(a, _, _, b) => {
                    a.y.min(b.y)
                }
            })
            .fold(0.0, f64::min);
        assert!(
            (top + font.cap_height).abs() < 0.02,
            "{top} {}",
            font.cap_height
        );
    }
}
