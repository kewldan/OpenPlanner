//! Запись плана листа в DXF (`AutoCAD` R12, ASCII) по слоям.
//!
//! В DXF уходит содержимое видовых экранов — план в натуральную величину (мм), а не
//! лист с рамкой: так файл удобно продолжать в CAD. Высота текста пересчитывается
//! из масштаба листа. Ось Y в DXF направлена вверх, поэтому координаты отражаются.
//! Строки — в кодировке Windows-1251 (`$DWGCODEPAGE ANSI_1251`), символы вне неё
//! записываются как `\U+XXXX`.

use std::fmt::Write as _;

use glam::DVec2;

use crate::{
    ExportError,
    sheet::{Align, Rgb, Shape, Sheet},
};

/// Записывает план листа в DXF.
#[must_use]
pub fn write(sheet: &Sheet) -> Vec<u8> {
    let denominator = sheet.viewports.first().map_or(1.0, |v| v.denominator);
    let names: Vec<String> = sheet.layers.iter().map(|l| layer_name(l)).collect();
    let mut s = String::new();
    let mut pair = |code: u16, value: &str| {
        let _ = write!(s, "{code:>3}\r\n{value}\r\n");
    };

    pair(0, "SECTION");
    pair(2, "HEADER");
    pair(9, "$ACADVER");
    pair(1, "AC1009");
    pair(9, "$DWGCODEPAGE");
    pair(3, "ANSI_1251");
    pair(9, "$INSUNITS");
    pair(70, "4");
    pair(0, "ENDSEC");

    pair(0, "SECTION");
    pair(2, "TABLES");
    pair(0, "TABLE");
    pair(2, "LTYPE");
    pair(70, "2");
    pair(0, "LTYPE");
    pair(2, "CONTINUOUS");
    pair(70, "0");
    pair(3, "Solid line");
    pair(72, "65");
    pair(73, "0");
    pair(40, "0.0");
    let (on, off) = (1.5 * denominator, 1.0 * denominator);
    pair(0, "LTYPE");
    pair(2, "DASHED");
    pair(70, "0");
    pair(3, "__ __ __");
    pair(72, "65");
    pair(73, "2");
    pair(40, &num(on + off));
    pair(49, &num(on));
    pair(49, &num(-off));
    pair(0, "ENDTAB");
    pair(0, "TABLE");
    pair(2, "LAYER");
    pair(70, &(names.len() + 1).to_string());
    for name in std::iter::once("0").chain(names.iter().map(String::as_str)) {
        pair(0, "LAYER");
        pair(2, &encode(name));
        pair(70, "0");
        pair(62, "7");
        pair(6, "CONTINUOUS");
    }
    pair(0, "ENDTAB");
    pair(0, "TABLE");
    pair(2, "STYLE");
    pair(70, "1");
    pair(0, "STYLE");
    pair(2, "STANDARD");
    pair(70, "0");
    pair(40, "0.0");
    pair(41, "1.0");
    pair(50, "0.0");
    pair(71, "0");
    pair(42, "2.5");
    pair(3, "txt");
    pair(4, "");
    pair(0, "ENDTAB");
    pair(0, "ENDSEC");

    pair(0, "SECTION");
    pair(2, "ENTITIES");
    let flip = |p: DVec2| DVec2::new(p.x, -p.y);
    for viewport in &sheet.viewports {
        for item in &viewport.items {
            let layer = encode(names.get(item.layer).map_or("0", String::as_str));
            match &item.shape {
                // Заливки в DXF не переносятся: контуры стен и штриховка — отдельными линиями.
                Shape::Fill { .. } => {}
                Shape::Stroke {
                    points,
                    closed,
                    color,
                    dash,
                    ..
                } => {
                    if points.len() < 2 {
                        continue;
                    }
                    let common = |pair: &mut dyn FnMut(u16, &str)| {
                        pair(8, &layer);
                        if dash.is_some() {
                            pair(6, "DASHED");
                        }
                        pair(62, &aci(*color).to_string());
                    };
                    if points.len() == 2 && !closed {
                        let (a, b) = (flip(points[0]), flip(points[1]));
                        pair(0, "LINE");
                        common(&mut pair);
                        pair(10, &num(a.x));
                        pair(20, &num(a.y));
                        pair(30, "0.0");
                        pair(11, &num(b.x));
                        pair(21, &num(b.y));
                        pair(31, "0.0");
                    } else {
                        pair(0, "POLYLINE");
                        common(&mut pair);
                        pair(66, "1");
                        pair(10, "0.0");
                        pair(20, "0.0");
                        pair(30, "0.0");
                        pair(70, if *closed { "1" } else { "0" });
                        for p in points {
                            let p = flip(*p);
                            pair(0, "VERTEX");
                            pair(8, &layer);
                            pair(10, &num(p.x));
                            pair(20, &num(p.y));
                            pair(30, "0.0");
                        }
                        pair(0, "SEQEND");
                        pair(8, &layer);
                    }
                }
                Shape::Text(t) => {
                    if t.text.trim().is_empty() {
                        continue;
                    }
                    let at = flip(t.at);
                    pair(0, "TEXT");
                    pair(8, &layer);
                    pair(62, &aci(t.color).to_string());
                    pair(10, &num(at.x));
                    pair(20, &num(at.y));
                    pair(30, "0.0");
                    pair(40, &num(t.height * viewport.denominator));
                    pair(1, &encode(&t.text));
                    let angle = -t.angle.to_degrees();
                    if angle.abs() > 1e-9 {
                        pair(50, &num(angle));
                    }
                    let justify = match t.align {
                        Align::Left => 0,
                        Align::Center => 1,
                        Align::Right => 2,
                    };
                    if justify != 0 {
                        pair(72, &justify.to_string());
                        pair(11, &num(at.x));
                        pair(21, &num(at.y));
                        pair(31, "0.0");
                    }
                }
            }
        }
    }
    pair(0, "ENDSEC");
    pair(0, "EOF");
    to_cp1251(&s)
}

/// Имя слоя без символов, запрещённых в DXF.
fn layer_name(name: &str) -> String {
    let cleaned: String =
        name.chars()
            .map(|c| match c {
                '<' | '>' | '/' | '\\' | '"' | ':' | ';' | '?' | '*' | '|' | '=' | '`' | ','
                | ' ' => '_',
                c => c,
            })
            .collect();
    if cleaned.is_empty() {
        "0".to_owned()
    } else {
        cleaned
    }
}

/// Символы вне Windows-1251 — как `\U+XXXX`.
fn encode(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_ascii() || cp1251(c).is_some() {
            out.push(c);
        } else if (c as u32) <= 0xFFFF {
            let _ = write!(out, "\\U+{:04X}", c as u32);
        }
    }
    out
}

/// Байт символа в Windows-1251 (только не-ASCII).
fn cp1251(c: char) -> Option<u8> {
    let code = c as u32;
    if (0x0410..=0x044F).contains(&code) {
        return u8::try_from(code - 0x0410 + 0xC0).ok();
    }
    Some(match c {
        'Ё' => 0xA8,
        'ё' => 0xB8,
        '№' => 0xB9,
        '«' => 0xAB,
        '»' => 0xBB,
        '—' => 0x97,
        '–' => 0x96,
        '°' => 0xB0,
        '±' => 0xB1,
        '·' => 0xB7,
        '\u{a0}' => 0xA0,
        '…' => 0x85,
        'Є' => 0xAA,
        'є' => 0xBA,
        'І' => 0xB2,
        'і' => 0xB3,
        'Ї' => 0xAF,
        'ї' => 0xBF,
        'Ў' => 0xA1,
        'ў' => 0xA2,
        _ => return None,
    })
}

fn to_cp1251(text: &str) -> Vec<u8> {
    text.chars()
        .filter_map(|c| {
            if c.is_ascii() {
                u8::try_from(c as u32).ok()
            } else {
                cp1251(c)
            }
        })
        .collect()
}

/// Ближайший цвет палитры `AutoCAD` (ACI); чёрный — 7 (чёрный на белом, белый на чёрном).
fn aci(color: Rgb) -> u8 {
    const PALETTE: [(u8, Rgb); 10] = [
        (1, Rgb(255, 0, 0)),
        (2, Rgb(255, 255, 0)),
        (3, Rgb(0, 255, 0)),
        (4, Rgb(0, 255, 255)),
        (5, Rgb(0, 0, 255)),
        (6, Rgb(255, 0, 255)),
        (7, Rgb(0, 0, 0)),
        (8, Rgb(128, 128, 128)),
        (9, Rgb(192, 192, 192)),
        (30, Rgb(255, 127, 0)),
    ];
    let distance = |a: Rgb, b: Rgb| {
        let d = |x: u8, y: u8| (i32::from(x) - i32::from(y)).pow(2);
        d(a.0, b.0) + d(a.1, b.1) + d(a.2, b.2)
    };
    PALETTE
        .iter()
        .min_by_key(|(_, c)| distance(*c, color))
        .map_or(7, |(i, _)| *i)
}

fn num(v: f64) -> String {
    let v = if v.is_finite() { v } else { 0.0 };
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0');
    if s.ends_with('.') {
        format!("{s}0")
    } else {
        s.to_owned()
    }
}

// ---------------------------------------------------------------- чтение

/// Линия чертежа DXF: слой и вершины в миллиметрах плана (Y вниз).
#[derive(Debug, Clone, PartialEq)]
pub struct DxfPath {
    /// Слой.
    pub layer: String,
    /// Вершины.
    pub points: Vec<DVec2>,
    /// Замкнута.
    pub closed: bool,
}

/// Прочитанный чертёж: линии по слоям в миллиметрах.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DxfDrawing {
    /// Линии.
    pub paths: Vec<DxfPath>,
    /// Слои в порядке появления.
    pub layers: Vec<String>,
}

/// Символ байта Windows-1251 (обратно к [`cp1251`]).
fn from_cp1251(b: u8) -> char {
    if b.is_ascii() {
        return char::from(b);
    }
    if b >= 0xC0 {
        return char::from_u32(0x0410 + u32::from(b - 0xC0)).unwrap_or('?');
    }
    (0x80..=0xFFu32)
        .filter_map(char::from_u32)
        .chain([
            'Ё', 'ё', '№', '«', '»', '—', '–', '…', 'Є', 'є', 'І', 'і', 'Ї', 'ї', 'Ў', 'ў',
        ])
        .find(|c| cp1251(*c) == Some(b))
        .unwrap_or('?')
}

/// Текст файла: UTF-8 (DXF 2007+) или Windows-1251 (старые версии).
fn decode(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => bytes.iter().map(|b| from_cp1251(*b)).collect(),
    }
}

/// Множитель единиц `$INSUNITS` в миллиметры (0 — без единиц: считаем миллиметрами).
fn units_to_mm(code: i64) -> f64 {
    match code {
        1 => 25.4,
        2 => 304.8,
        5 => 10.0,
        6 => 1000.0,
        14 => 100.0,
        _ => 1.0,
    }
}

/// Дуга окружности ломаной: углы — градусы против часовой стрелки (DXF, Y вверх).
fn arc_points(c: DVec2, r: f64, from: f64, to: f64) -> Vec<DVec2> {
    let mut sweep = to - from;
    if sweep <= 0.0 {
        sweep += 360.0;
    }
    // Шаг не больше 10°.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // сегментов ≤ 36
    let n = (sweep / 10.0).ceil().max(1.0) as u32;
    (0..=n)
        .map(|i| {
            let a = (from + sweep * f64::from(i) / f64::from(n)).to_radians();
            c + DVec2::new(a.cos(), a.sin()) * r
        })
        .collect()
}

/// Читает текстовый DXF: отрезки, полилинии (LWPOLYLINE и POLYLINE), окружности
/// и дуги из раздела ENTITIES. Блоки (INSERT) и текст пропускаются. Координаты
/// переводятся в миллиметры по `$INSUNITS`, ось Y — вниз, как на плане.
///
/// # Errors
///
/// Двоичный DXF или файл без раздела ENTITIES.
pub fn read(bytes: &[u8]) -> Result<DxfDrawing, ExportError> {
    if bytes.starts_with(b"AutoCAD Binary DXF") {
        return Err(ExportError::Format(
            "двоичный DXF не поддерживается — сохраните чертёж как текстовый DXF".into(),
        ));
    }
    let text = decode(bytes);
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let pairs: Vec<(i32, &str)> = lines
        .chunks(2)
        .filter_map(|c| match c {
            [code, value] => Some((code.parse().ok()?, *value)),
            _ => None,
        })
        .collect();
    let mut scale = 1.0;
    if let Some(i) = pairs.iter().position(|p| *p == (9, "$INSUNITS"))
        && let Some((70, v)) = pairs.get(i + 1)
    {
        scale = units_to_mm(v.parse().unwrap_or(0));
    }
    let start = pairs
        .windows(2)
        .position(|w| w[0] == (0, "SECTION") && w[1] == (2, "ENTITIES"))
        .ok_or_else(|| {
            ExportError::Format("в файле нет раздела ENTITIES — это не чертёж DXF".into())
        })?;
    let mut out = DxfDrawing::default();
    let point = |x: f64, y: f64| DVec2::new(x * scale, -y * scale);
    let num = |v: &str| v.parse::<f64>().unwrap_or(0.0);
    let mut i = start + 2;
    // Открытая POLYLINE (R12): вершины идут отдельными сущностями VERTEX до SEQEND.
    let mut open: Option<DxfPath> = None;
    while i < pairs.len() {
        let (code, kind) = pairs[i];
        if code != 0 {
            i += 1;
            continue;
        }
        if kind == "ENDSEC" {
            break;
        }
        // Поля сущности — до следующей пары с кодом 0.
        let end = pairs[i + 1..]
            .iter()
            .position(|p| p.0 == 0)
            .map_or(pairs.len(), |n| i + 1 + n);
        let fields = &pairs[i + 1..end];
        let get = |c: i32| fields.iter().find(|p| p.0 == c).map(|p| p.1);
        let layer = get(8).unwrap_or("0").to_owned();
        let mut path = |points: Vec<DVec2>, closed: bool, layer: String| {
            if points.len() >= 2 {
                if !out.layers.contains(&layer) {
                    out.layers.push(layer.clone());
                }
                out.paths.push(DxfPath {
                    layer,
                    points,
                    closed,
                });
            }
        };
        match kind {
            "LINE" => path(
                vec![
                    point(num(get(10).unwrap_or("0")), num(get(20).unwrap_or("0"))),
                    point(num(get(11).unwrap_or("0")), num(get(21).unwrap_or("0"))),
                ],
                false,
                layer,
            ),
            "LWPOLYLINE" => {
                let xs: Vec<f64> = fields
                    .iter()
                    .filter(|p| p.0 == 10)
                    .map(|p| num(p.1))
                    .collect();
                let ys: Vec<f64> = fields
                    .iter()
                    .filter(|p| p.0 == 20)
                    .map(|p| num(p.1))
                    .collect();
                let flags = get(70).map_or(0, |v| v.parse::<i64>().unwrap_or(0));
                path(
                    xs.iter().zip(&ys).map(|(x, y)| point(*x, *y)).collect(),
                    flags & 1 == 1,
                    layer,
                );
            }
            "POLYLINE" => {
                let flags = get(70).map_or(0, |v| v.parse::<i64>().unwrap_or(0));
                open = Some(DxfPath {
                    layer,
                    points: Vec::new(),
                    closed: flags & 1 == 1,
                });
            }
            "VERTEX" => {
                if let Some(p) = open.as_mut() {
                    p.points.push(point(
                        num(get(10).unwrap_or("0")),
                        num(get(20).unwrap_or("0")),
                    ));
                }
            }
            "SEQEND" => {
                if let Some(p) = open.take() {
                    path(p.points, p.closed, p.layer);
                }
            }
            "CIRCLE" | "ARC" => {
                let c = DVec2::new(num(get(10).unwrap_or("0")), num(get(20).unwrap_or("0")));
                let r = num(get(40).unwrap_or("0"));
                let (from, to, closed) = if kind == "ARC" {
                    (
                        num(get(50).unwrap_or("0")),
                        num(get(51).unwrap_or("360")),
                        false,
                    )
                } else {
                    (0.0, 360.0, true)
                };
                let mut pts: Vec<DVec2> = arc_points(c, r, from, to)
                    .into_iter()
                    .map(|p| point(p.x, p.y))
                    .collect();
                if closed {
                    pts.pop();
                }
                path(pts, closed, layer);
            }
            _ => {}
        }
        i = end;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sheet::{Item, Text, Viewport};

    #[test]
    fn dxf_has_layers_entities_and_cyrillic() {
        let mut sheet = Sheet {
            title: "План".into(),
            size: DVec2::new(420.0, 297.0),
            layers: vec![],
            paper: vec![],
            viewports: vec![],
        };
        let walls = sheet.layer("Стены");
        let notes = sheet.layer("Новые перегородки");
        sheet
            .paper
            .push(Item::rect(walls, DVec2::ZERO, DVec2::ONE, 0.5));
        sheet.viewports.push(Viewport {
            items: vec![
                Item::polyline(
                    walls,
                    vec![
                        DVec2::ZERO,
                        DVec2::new(3000.0, 0.0),
                        DVec2::new(3000.0, 2000.0),
                    ],
                    true,
                    0.5,
                    Rgb::BLACK,
                ),
                Item::line(
                    notes,
                    DVec2::ZERO,
                    DVec2::new(0.0, 1000.0),
                    0.3,
                    Rgb(200, 30, 30),
                ),
                Item::text(
                    notes,
                    Text::centered(
                        DVec2::new(100.0, 200.0),
                        "Кухня 12,5 м²",
                        2.5,
                        Align::Center,
                    ),
                ),
            ],
            center: DVec2::ZERO,
            origin: DVec2::ZERO,
            denominator: 50.0,
            clip: (DVec2::ZERO, DVec2::ONE),
        });
        let bytes = write(&sheet);
        let text: String = bytes.iter().map(|b| char::from(*b)).collect();
        assert!(text.contains("AC1009"));
        assert!(text.contains("ANSI_1251"));
        assert!(text.contains("POLYLINE") && text.contains("SEQEND"));
        assert!(text.contains("LINE"));
        // Слой «Новые_перегородки» в cp1251, «м²» — «м\U+00B2».
        let layer = to_cp1251("Новые_перегородки");
        assert!(bytes.windows(layer.len()).any(|w| w == layer.as_slice()));
        assert!(text.contains("\\U+00B2"));
        // Высота текста: 2,5 мм на листе 1:50 — 125 мм в модели; ось Y отражена.
        assert!(text.contains("\r\n 40\r\n125.0\r\n"));
        assert!(text.contains(" 20\r\n-201.25"), "{text}");
        assert_eq!(aci(Rgb(200, 30, 30)), 1);
        assert!(
            !text.contains("ENTITIES\r\n  0\r\nPOLYLINE\r\n  8\r\n0\r\n"),
            "рамка листа не попадает"
        );
    }

    #[test]
    fn reads_back_own_drawing_and_other_entities() {
        let dxf = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n6\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nСтены\n10\n0\n20\n0\n11\n4\n21\n0\n0\nLWPOLYLINE\n8\nA\n90\n3\n70\n1\n10\n0\n20\n0\n10\n1\n20\n0\n10\n1\n20\n1\n0\nCIRCLE\n8\nA\n10\n0\n20\n0\n40\n0.5\n0\nINSERT\n8\nA\n0\nENDSEC\n0\nEOF\n";
        let d = read(dxf.as_bytes()).unwrap();
        assert_eq!(d.layers, vec!["Стены".to_owned(), "A".to_owned()]);
        assert_eq!(d.paths.len(), 3);
        // Метры → мм, Y вниз.
        assert_eq!(d.paths[0].points[1], DVec2::new(4000.0, 0.0));
        assert_eq!(d.paths[1].points[2], DVec2::new(1000.0, -1000.0));
        assert!(d.paths[1].closed && d.paths[2].closed);
        assert!(read(b"AutoCAD Binary DXF\r\n").is_err());
        assert!(read(b"0\nEOF\n").is_err());
    }

    #[test]
    fn cp1251_decodes() {
        assert_eq!(decode(&[0xD1, 0xF2, 0xE5, 0xED, 0xFB]), "Стены");
        assert_eq!(from_cp1251(0xA8), 'Ё');
    }
}
