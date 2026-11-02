//! Запись листа в SVG: миллиметры бумаги, слои — группы (видны как слои в Inkscape).

use std::fmt::Write as _;

use base64::Engine as _;
use glam::DVec2;

use crate::{
    font::{FONT_BYTES, Segment, SheetFont},
    pdf::text_origin,
    sheet::{Align, Rgb, Shape, Sheet, Text},
};

/// Как записывать текст.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SvgOptions {
    /// Текст контурами глифов: выглядит одинаково везде, но не выделяется
    /// (для растеризации в PNG).
    pub text_as_paths: bool,
    /// Встроить шрифт в файл (`@font-face`), чтобы текст выглядел как в PDF.
    pub embed_font: bool,
}

/// Записывает лист в SVG.
#[must_use]
pub fn write(sheet: &Sheet, options: SvgOptions) -> String {
    let font = SheetFont::shared();
    let mut s = String::new();
    let _ = write!(
        s,
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" width="{w}mm" height="{h}mm" viewBox="0 0 {w} {h}">"#,
        w = num(sheet.size.x),
        h = num(sheet.size.y)
    );
    let _ = write!(s, "<title>{}</title>", escape(&sheet.title));
    if options.embed_font && !options.text_as_paths {
        let data = base64::engine::general_purpose::STANDARD.encode(FONT_BYTES);
        let _ = write!(
            s,
            "<defs><style>@font-face{{font-family:'Onest';src:url(data:font/ttf;base64,{data}) format('truetype');}}</style></defs>"
        );
    }
    let _ = write!(
        s,
        r##"<rect width="{}" height="{}" fill="#ffffff"/>"##,
        num(sheet.size.x),
        num(sheet.size.y)
    );
    for (g, group) in sheet.groups().iter().enumerate() {
        if let Some((lo, hi)) = group.clip {
            let _ = write!(
                s,
                r#"<clipPath id="clip{g}"><rect x="{}" y="{}" width="{}" height="{}"/></clipPath><g clip-path="url(#clip{g})">"#,
                num(lo.x),
                num(lo.y),
                num(hi.x - lo.x),
                num(hi.y - lo.y)
            );
        }
        let mut current: Option<usize> = None;
        for item in &group.items {
            if current != Some(item.layer) {
                if current.is_some() {
                    s.push_str("</g>");
                }
                let name = sheet.layers.get(item.layer).map_or("", String::as_str);
                let _ = write!(
                    s,
                    r#"<g inkscape:groupmode="layer" inkscape:label="{0}" data-layer="{0}">"#,
                    escape(name)
                );
                current = Some(item.layer);
            }
            match &item.shape {
                Shape::Fill { rings, color } => {
                    let mut d = String::new();
                    for ring in rings {
                        path(&mut d, ring, true);
                    }
                    let _ = write!(
                        s,
                        r#"<path d="{d}" fill="{}" fill-rule="evenodd"/>"#,
                        hex(*color)
                    );
                }
                Shape::Stroke {
                    points,
                    closed,
                    width,
                    color,
                    dash,
                } => {
                    let mut d = String::new();
                    path(&mut d, points, *closed);
                    let dash = dash.map_or(String::new(), |(on, off)| {
                        format!(r#" stroke-dasharray="{} {}""#, num(on), num(off))
                    });
                    let _ = write!(
                        s,
                        r#"<path d="{d}" fill="none" stroke="{}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"{dash}/>"#,
                        hex(*color),
                        num(*width)
                    );
                }
                Shape::Text(t) => {
                    if let Some(font) = font {
                        if options.text_as_paths {
                            text_paths(&mut s, t, font);
                        } else {
                            text(&mut s, t, font);
                        }
                    }
                }
            }
        }
        if current.is_some() {
            s.push_str("</g>");
        }
        if group.clip.is_some() {
            s.push_str("</g>");
        }
    }
    s.push_str("</svg>");
    s
}

fn text(s: &mut String, t: &Text, font: &SheetFont) {
    if t.text.is_empty() {
        return;
    }
    let anchor = match t.align {
        Align::Left => "start",
        Align::Center => "middle",
        Align::Right => "end",
    };
    let rotate = if t.angle.abs() > 1e-9 {
        format!(
            r#" transform="rotate({} {} {})""#,
            num(t.angle.to_degrees()),
            num(t.at.x),
            num(t.at.y)
        )
    } else {
        String::new()
    };
    let _ = write!(
        s,
        r#"<text x="{}" y="{}" font-family="Onest, Arial, sans-serif" font-weight="500" font-size="{}" text-anchor="{anchor}" fill="{}" xml:space="preserve"{rotate}>{}</text>"#,
        num(t.at.x),
        num(t.at.y),
        num(font.em(t.height)),
        hex(t.color),
        escape(&t.text)
    );
}

fn text_paths(s: &mut String, t: &Text, font: &SheetFont) {
    if t.text.trim().is_empty() {
        return;
    }
    let (origin, dir, em) = text_origin(t, font);
    let mut d = String::new();
    for glyph in font.layout(&t.text) {
        let shift = DVec2::new(glyph.x, 0.0);
        let mut last: Option<DVec2> = None;
        for segment in font.outline(glyph.id) {
            let (start, end) = match segment {
                Segment::Line(a, b) | Segment::Quad(a, _, b) | Segment::Cubic(a, _, _, b) => {
                    (a + shift, b + shift)
                }
            };
            if last.is_none_or(|l| l.distance(start) > 1e-6) {
                if last.is_some() {
                    d.push('Z');
                }
                let _ = write!(d, "M{} {}", num4(start.x), num4(start.y));
            }
            match segment {
                Segment::Line(..) => {
                    let _ = write!(d, "L{} {}", num4(end.x), num4(end.y));
                }
                Segment::Quad(_, c, _) => {
                    let c = c + shift;
                    let _ = write!(
                        d,
                        "Q{} {} {} {}",
                        num4(c.x),
                        num4(c.y),
                        num4(end.x),
                        num4(end.y)
                    );
                }
                Segment::Cubic(_, c1, c2, _) => {
                    let (c1, c2) = (c1 + shift, c2 + shift);
                    let _ = write!(
                        d,
                        "C{} {} {} {} {} {}",
                        num4(c1.x),
                        num4(c1.y),
                        num4(c2.x),
                        num4(c2.y),
                        num4(end.x),
                        num4(end.y)
                    );
                }
            }
            last = Some(end);
        }
        if last.is_some() {
            d.push('Z');
        }
    }
    // Контуры в em с осью Y вниз: поворот и масштаб — одной матрицей.
    let _ = write!(
        s,
        r#"<path d="{d}" fill="{}" transform="matrix({} {} {} {} {} {})"><title>{}</title></path>"#,
        hex(t.color),
        num4(em * dir.x),
        num4(em * dir.y),
        num4(-em * dir.y),
        num4(em * dir.x),
        num(origin.x),
        num(origin.y),
        escape(&t.text)
    );
}

fn path(d: &mut String, points: &[DVec2], closed: bool) {
    for (i, p) in points.iter().enumerate() {
        let op = if i == 0 { 'M' } else { 'L' };
        let _ = write!(d, "{op}{} {}", num(p.x), num(p.y));
    }
    if closed {
        d.push('Z');
    }
}

fn hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)
}

/// Экранирование для XML.
#[must_use]
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c.is_control() && c != '\n' && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

fn fixed(v: f64, digits: usize) -> String {
    let v = if v.is_finite() { v } else { 0.0 };
    let s = format!("{v:.digits$}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}

fn num(v: f64) -> String {
    fixed(v, 3)
}

fn num4(v: f64) -> String {
    fixed(v, 4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sheet::Item;

    fn sheet() -> Sheet {
        let mut sheet = Sheet {
            title: "План <демонтажа> & монтажа".into(),
            size: DVec2::new(297.0, 210.0),
            layers: vec![],
            paper: vec![],
            viewports: vec![],
        };
        let layer = sheet.layer("Рамка");
        sheet.paper.push(Item::rect(
            layer,
            DVec2::new(20.0, 5.0),
            DVec2::new(292.0, 205.0),
            0.5,
        ));
        sheet.paper.push(Item::text(
            layer,
            Text::centered(DVec2::new(100.0, 100.0), "Кухня", 5.0, Align::Center),
        ));
        sheet
    }

    #[test]
    fn svg_text_and_layers() {
        let svg = write(
            &sheet(),
            SvgOptions {
                text_as_paths: false,
                embed_font: true,
            },
        );
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
        assert!(svg.contains(r#"width="297mm""#));
        assert!(svg.contains("inkscape:label=\"Рамка\""));
        assert!(svg.contains(">Кухня</text>"));
        assert!(svg.contains("@font-face"));
        assert!(svg.contains("План &lt;демонтажа&gt; &amp; монтажа"));
    }

    #[test]
    fn svg_text_as_paths() {
        let svg = write(
            &sheet(),
            SvgOptions {
                text_as_paths: true,
                embed_font: false,
            },
        );
        assert!(!svg.contains("<text"));
        assert!(svg.contains("<title>Кухня</title>"));
        assert!(svg.matches('Q').count() + svg.matches('C').count() > 5);
    }
}
