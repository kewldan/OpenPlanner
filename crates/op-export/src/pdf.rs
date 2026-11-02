//! Запись листов в PDF: вектор, слои (optional content), встроенный шрифт.
//!
//! Шрифт встраивается целиком как `CIDFontType2` с кодировкой `Identity-H`; таблица
//! `ToUnicode` позволяет искать и копировать текст.

use std::{collections::BTreeMap, fmt::Write as _, io::Write as _};

use flate2::{Compression, write::ZlibEncoder};
use glam::DVec2;

use crate::{
    ExportError,
    font::{FONT_BYTES, FONT_NAME, SheetFont},
    sheet::{Align, Shape, Sheet, Text},
};

/// Пунктов в миллиметре.
const PT_PER_MM: f64 = 72.0 / 25.4;

/// Сведения о документе.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PdfInfo {
    /// Заголовок.
    pub title: String,
    /// Автор.
    pub author: String,
}

/// Записывает листы в один PDF (лист — страница).
///
/// # Errors
///
/// Шрифт не читается или ошибка сжатия.
pub fn write(sheets: &[Sheet], info: &PdfInfo) -> Result<Vec<u8>, ExportError> {
    let font = SheetFont::shared().ok_or(ExportError::Font)?;

    // Слои документа: объединение слоёв всех листов в порядке появления.
    let mut layers: Vec<String> = Vec::new();
    for sheet in sheets {
        for l in &sheet.layers {
            if !layers.contains(l) {
                layers.push(l.clone());
            }
        }
    }

    // Номера объектов.
    const CATALOG: usize = 1;
    const PAGES: usize = 2;
    const FONT: usize = 3;
    const CID_FONT: usize = 4;
    const DESCRIPTOR: usize = 5;
    const FONT_FILE: usize = 6;
    const TO_UNICODE: usize = 7;
    const INFO: usize = 8;
    let ocg = |i: usize| 9 + i;
    let page = |i: usize| 9 + layers.len() + 2 * i;
    let content = |i: usize| page(i) + 1;
    let count = 9 + layers.len() + 2 * sheets.len();

    let mut pdf = Writer::new(count);
    let mut used: BTreeMap<u16, char> = BTreeMap::new();

    // Страницы.
    for (i, sheet) in sheets.iter().enumerate() {
        let stream = page_content(sheet, &layers, font, &mut used);
        let size = sheet.size * PT_PER_MM;
        let properties: String = sheet
            .layers
            .iter()
            .filter_map(|l| layers.iter().position(|x| x == l))
            .map(|k| format!("/L{k} {} 0 R ", ocg(k)))
            .collect();
        pdf.object(
            page(i),
            &format!(
                "<< /Type /Page /Parent {PAGES} 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 {FONT} 0 R >> /Properties << {properties}>> >> /Contents {} 0 R >>",
                num(size.x),
                num(size.y),
                content(i)
            ),
        );
        pdf.stream(content(i), "", stream.as_bytes())?;
    }

    // Слои.
    for (k, name) in layers.iter().enumerate() {
        pdf.object(
            ocg(k),
            &format!("<< /Type /OCG /Name {} >>", text_string(name)),
        );
    }
    let ocg_refs: String = (0..layers.len())
        .map(|k| format!("{} 0 R ", ocg(k)))
        .collect();
    pdf.object(
        CATALOG,
        &format!(
            "<< /Type /Catalog /Pages {PAGES} 0 R /PageMode /UseOC /OCProperties << /OCGs [{ocg_refs}] /D << /Order [{ocg_refs}] /ON [{ocg_refs}] /BaseState /ON >> >> >>"
        ),
    );
    let kids: String = (0..sheets.len())
        .map(|i| format!("{} 0 R ", page(i)))
        .collect();
    pdf.object(
        PAGES,
        &format!("<< /Type /Pages /Kids [{kids}] /Count {} >>", sheets.len()),
    );

    // Шрифт.
    let k = 1000.0 / font.units_per_em();
    pdf.object(
        FONT,
        &format!(
            "<< /Type /Font /Subtype /Type0 /BaseFont /{FONT_NAME} /Encoding /Identity-H /DescendantFonts [{CID_FONT} 0 R] /ToUnicode {TO_UNICODE} 0 R >>"
        ),
    );
    let widths: String = used
        .keys()
        .map(|id| format!("{id} [{}] ", num(font.advance_units(*id) * k)))
        .collect();
    pdf.object(
        CID_FONT,
        &format!(
            "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{FONT_NAME} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {DESCRIPTOR} 0 R /DW 500 /W [{widths}] /CIDToGIDMap /Identity >>"
        ),
    );
    let b = font.bbox.map(|v| num(f64::from(v) * k));
    pdf.object(
        DESCRIPTOR,
        &format!(
            "<< /Type /FontDescriptor /FontName /{FONT_NAME} /Flags 32 /FontBBox [{} {} {} {}] /ItalicAngle 0 /Ascent {} /Descent {} /CapHeight {} /StemV 80 /FontFile2 {FONT_FILE} 0 R >>",
            b[0],
            b[1],
            b[2],
            b[3],
            num(font.ascent * 1000.0),
            num(font.descent * 1000.0),
            num(font.cap_height * 1000.0),
        ),
    );
    pdf.stream(
        FONT_FILE,
        &format!("/Length1 {}", FONT_BYTES.len()),
        FONT_BYTES,
    )?;
    pdf.stream(TO_UNICODE, "", to_unicode(&used).as_bytes())?;
    pdf.object(
        INFO,
        &format!(
            "<< /Title {} /Author {} /Creator (OpenPlanner) /Producer (OpenPlanner) >>",
            text_string(&info.title),
            text_string(&info.author)
        ),
    );
    Ok(pdf.finish(CATALOG, INFO))
}

/// Поток содержимого страницы.
fn page_content(
    sheet: &Sheet,
    layers: &[String],
    font: &SheetFont,
    used: &mut BTreeMap<u16, char>,
) -> String {
    let mut s = String::new();
    // Миллиметры, ось Y вниз от левого верхнего угла.
    let _ = writeln!(
        s,
        "q {} 0 0 {} 0 {} cm 1 J 1 j",
        num(PT_PER_MM),
        num(-PT_PER_MM),
        num(sheet.size.y * PT_PER_MM)
    );
    for group in sheet.groups() {
        if let Some((lo, hi)) = group.clip {
            let _ = writeln!(
                s,
                "q {} {} {} {} re W n",
                num(lo.x),
                num(lo.y),
                num(hi.x - lo.x),
                num(hi.y - lo.y)
            );
        }
        let mut current: Option<usize> = None;
        for item in &group.items {
            let layer = sheet
                .layers
                .get(item.layer)
                .and_then(|name| layers.iter().position(|l| l == name));
            if layer != current {
                if current.is_some() {
                    s.push_str("EMC\n");
                }
                if let Some(k) = layer {
                    let _ = writeln!(s, "/OC /L{k} BDC");
                }
                current = layer;
            }
            match &item.shape {
                Shape::Fill { rings, color } => {
                    let [r, g, b] = color.unit();
                    let _ = write!(s, "{} {} {} rg ", num(r), num(g), num(b));
                    for ring in rings {
                        path(&mut s, ring, true);
                    }
                    s.push_str("f*\n");
                }
                Shape::Stroke {
                    points,
                    closed,
                    width,
                    color,
                    dash,
                } => {
                    let [r, g, b] = color.unit();
                    let _ = write!(s, "{} {} {} RG {} w ", num(r), num(g), num(b), num(*width));
                    match dash {
                        Some((on, off)) => {
                            let _ = write!(s, "[{} {}] 0 d ", num(*on), num(*off));
                        }
                        None => s.push_str("[] 0 d "),
                    }
                    path(&mut s, points, *closed);
                    s.push_str("S\n");
                }
                Shape::Text(t) => text(&mut s, t, font, used),
            }
        }
        if current.is_some() {
            s.push_str("EMC\n");
        }
        if group.clip.is_some() {
            s.push_str("Q\n");
        }
    }
    s.push_str("Q\n");
    s
}

fn path(s: &mut String, points: &[DVec2], closed: bool) {
    for (i, p) in points.iter().enumerate() {
        let op = if i == 0 { "m" } else { "l" };
        let _ = write!(s, "{} {} {op} ", num(p.x), num(p.y));
    }
    if closed {
        s.push_str("h ");
    }
}

/// Начало строки с учётом выравнивания, направление и размер em.
pub(crate) fn text_origin(t: &Text, font: &SheetFont) -> (DVec2, DVec2, f64) {
    let em = font.em(t.height);
    let dir = DVec2::new(t.angle.cos(), t.angle.sin());
    let width = font.width(&t.text) * em;
    let shift = match t.align {
        Align::Left => 0.0,
        Align::Center => -width / 2.0,
        Align::Right => -width,
    };
    (t.at + dir * shift, dir, em)
}

fn text(s: &mut String, t: &Text, font: &SheetFont, used: &mut BTreeMap<u16, char>) {
    if t.text.is_empty() {
        return;
    }
    let (origin, dir, em) = text_origin(t, font);
    let [r, g, b] = t.color.unit();
    let mut hex = String::new();
    for glyph in font.layout(&t.text) {
        used.entry(glyph.id).or_insert(glyph.ch);
        let _ = write!(hex, "{:04X}", glyph.id);
    }
    // Ось Y страницы направлена вниз, глифы — вверх: «верх» строки — (sin, −cos).
    let _ = writeln!(
        s,
        "BT {} {} {} rg /F1 1 Tf {} {} {} {} {} {} Tm <{hex}> Tj ET",
        num(r),
        num(g),
        num(b),
        num(em * dir.x),
        num(em * dir.y),
        num(em * dir.y),
        num(-em * dir.x),
        num(origin.x),
        num(origin.y),
    );
}

/// Таблица `ToUnicode`: номер глифа → символ.
fn to_unicode(used: &BTreeMap<u16, char>) -> String {
    let mut s = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let entries: Vec<(&u16, &char)> = used.iter().collect();
    for chunk in entries.chunks(100) {
        let _ = writeln!(s, "{} beginbfchar", chunk.len());
        for (id, ch) in chunk {
            let mut units = [0u16; 2];
            let hex: String = ch
                .encode_utf16(&mut units)
                .iter()
                .map(|u| format!("{u:04X}"))
                .collect();
            let _ = writeln!(s, "<{id:04X}> <{hex}>");
        }
        s.push_str("endbfchar\n");
    }
    s.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    s
}

/// Строка PDF в UTF-16BE с меткой порядка байтов.
fn text_string(text: &str) -> String {
    let mut s = String::from("<FEFF");
    for unit in text.encode_utf16() {
        let _ = write!(s, "{unit:04X}");
    }
    s.push('>');
    s
}

/// Число без лишних нулей (до трёх знаков после точки).
fn num(v: f64) -> String {
    let v = if v.is_finite() { v } else { 0.0 };
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}

/// Последовательная запись объектов с таблицей смещений.
struct Writer {
    buf: Vec<u8>,
    offsets: Vec<usize>,
}

impl Writer {
    fn new(count: usize) -> Self {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n");
        Self {
            buf,
            offsets: vec![0; count],
        }
    }

    fn object(&mut self, id: usize, body: &str) {
        self.offsets[id] = self.buf.len();
        self.buf
            .extend_from_slice(format!("{id} 0 obj\n{body}\nendobj\n").as_bytes());
    }

    fn stream(&mut self, id: usize, extra: &str, data: &[u8]) -> Result<(), ExportError> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data)?;
        let packed = encoder.finish()?;
        self.offsets[id] = self.buf.len();
        self.buf.extend_from_slice(
            format!(
                "{id} 0 obj\n<< /Length {} /Filter /FlateDecode {extra} >>\nstream\n",
                packed.len()
            )
            .as_bytes(),
        );
        self.buf.extend_from_slice(&packed);
        self.buf.extend_from_slice(b"\nendstream\nendobj\n");
        Ok(())
    }

    fn finish(mut self, root: usize, info: usize) -> Vec<u8> {
        let xref = self.buf.len();
        let mut table = format!("xref\n0 {}\n0000000000 65535 f \n", self.offsets.len());
        for offset in &self.offsets[1..] {
            let _ = writeln!(table, "{offset:010} 00000 n ");
        }
        let _ = write!(
            table,
            "trailer\n<< /Size {} /Root {root} 0 R /Info {info} 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            self.offsets.len()
        );
        self.buf.extend_from_slice(table.as_bytes());
        self.buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sheet::{Item, Rgb, Viewport};

    pub(crate) fn sample() -> Sheet {
        let mut sheet = Sheet {
            title: "Обмерный план".into(),
            size: DVec2::new(420.0, 297.0),
            layers: vec![],
            paper: vec![],
            viewports: vec![],
        };
        let frame = sheet.layer("Рамка");
        let walls = sheet.layer("Стены");
        sheet.paper.push(Item::rect(
            frame,
            DVec2::new(20.0, 5.0),
            DVec2::new(415.0, 292.0),
            0.5,
        ));
        sheet.paper.push(Item::text(
            frame,
            Text::centered(
                DVec2::new(300.0, 270.0),
                "Обмерный план",
                5.0,
                Align::Center,
            ),
        ));
        sheet.viewports.push(Viewport {
            items: vec![
                Item::fill(
                    walls,
                    vec![vec![
                        DVec2::new(0.0, 0.0),
                        DVec2::new(3000.0, 0.0),
                        DVec2::new(3000.0, 120.0),
                        DVec2::new(0.0, 120.0),
                    ]],
                    Rgb::hex(0x404040),
                ),
                Item::text(
                    walls,
                    Text {
                        at: DVec2::new(1500.0, 1000.0),
                        text: "Кухня 12,5 м²".into(),
                        height: 3.5,
                        align: Align::Center,
                        angle: std::f64::consts::FRAC_PI_2,
                        color: Rgb::BLACK,
                    },
                ),
            ],
            center: DVec2::new(1500.0, 1000.0),
            origin: DVec2::new(150.0, 120.0),
            denominator: 50.0,
            clip: (DVec2::new(20.0, 5.0), DVec2::new(415.0, 237.0)),
        });
        sheet
    }

    #[test]
    fn pdf_has_pages_layers_and_font() {
        let bytes = write(
            &[sample(), sample()],
            &PdfInfo {
                title: "Квартира".into(),
                author: "Иванов".into(),
            },
        );
        assert!(bytes.is_ok());
        let bytes = bytes.unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.starts_with("%PDF-1.7"));
        assert!(text.contains("/Count 2"));
        assert!(text.contains("/Type /OCG"));
        assert!(text.contains("/FontFile2"));
        assert!(text.trim_end().ends_with("%%EOF"));
        // Смещения в таблице указывают на начала объектов.
        let xref = text.rfind("xref\n").unwrap_or(0);
        for (i, line) in text[xref..].lines().skip(3).take(5).enumerate() {
            let offset: usize = line[..10].parse().unwrap_or(0);
            assert!(
                text[offset..].starts_with(&format!("{} 0 obj", i + 1)),
                "{line}"
            );
        }
    }

    #[test]
    fn numbers_are_compact() {
        assert_eq!(num(1.0), "1");
        assert_eq!(num(-0.0001), "0");
        assert_eq!(num(2.834_645), "2.835");
    }
}
