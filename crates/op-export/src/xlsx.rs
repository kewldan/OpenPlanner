//! Запись таблиц в XLSX (Office Open XML): лист книги — несколько таблиц подряд.

use std::fmt::Write as _;

use crate::{
    ExportError,
    svg::escape,
    table::{Cell, Table},
};

/// Лист книги.
#[derive(Debug, Clone, PartialEq)]
pub struct Worksheet {
    /// Название (до 31 символа).
    pub name: String,
    /// Таблицы сверху вниз.
    pub tables: Vec<Table>,
}

/// Стили ячеек (индексы в `cellXfs`).
const BOLD: u32 = 1;
/// Число с `d` знаками: `NUMBER + d`, жирное — `BOLD_NUMBER + d`.
const NUMBER: u32 = 2;
const BOLD_NUMBER: u32 = 6;

/// Записывает книгу XLSX.
///
/// # Errors
///
/// Ошибка архива.
pub fn write(sheets: &[Worksheet]) -> Result<Vec<u8>, ExportError> {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let overrides: String = (1..=sheets.len())
        .map(|i| {
            format!(
                r#"<Override PartName="/xl/worksheets/sheet{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#
            )
        })
        .collect();
    files.push((
        "[Content_Types].xml".into(),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>{overrides}</Types>"#
        )
        .into_bytes(),
    ));
    files.push((
        "_rels/.rels".into(),
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.to_vec(),
    ));
    let mut names: Vec<String> = Vec::new();
    for sheet in sheets {
        let mut name = sheet_name(&sheet.name);
        let base = name.clone();
        let mut n = 2;
        while names.contains(&name) {
            name = format!("{} {n}", base.chars().take(28).collect::<String>());
            n += 1;
        }
        names.push(name);
    }
    let entries: String = names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            format!(
                r#"<sheet name="{}" sheetId="{}" r:id="rId{}"/>"#,
                escape(name),
                i + 1,
                i + 1
            )
        })
        .collect();
    files.push((
        "xl/workbook.xml".into(),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>{entries}</sheets></workbook>"#
        )
        .into_bytes(),
    ));
    let mut rels: String = (1..=sheets.len())
        .map(|i| {
            format!(
                r#"<Relationship Id="rId{i}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{i}.xml"/>"#
            )
        })
        .collect();
    let _ = write!(
        rels,
        r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#,
        sheets.len() + 1
    );
    files.push((
        "xl/_rels/workbook.xml.rels".into(),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rels}</Relationships>"#
        )
        .into_bytes(),
    ));
    files.push(("xl/styles.xml".into(), styles().into_bytes()));
    for (i, sheet) in sheets.iter().enumerate() {
        files.push((
            format!("xl/worksheets/sheet{}.xml", i + 1),
            worksheet(sheet).into_bytes(),
        ));
    }
    crate::zip(&files)
}

/// Название листа без запрещённых символов, не длиннее 31.
fn sheet_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| !matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\'))
        .take(31)
        .collect();
    if cleaned.trim().is_empty() {
        "Лист".to_owned()
    } else {
        cleaned
    }
}

fn styles() -> String {
    let formats = ["0", "0.0", "0.00", "0.000"];
    let num_fmts: String = formats
        .iter()
        .enumerate()
        .map(|(i, f)| format!(r#"<numFmt numFmtId="{}" formatCode="{f}"/>"#, 164 + i))
        .collect();
    let mut xfs = String::from(
        r#"<xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1"/>"#,
    );
    for font in [0, 1] {
        for i in 0..formats.len() {
            let _ = write!(
                xfs,
                r#"<xf numFmtId="{}" fontId="{font}" fillId="0" borderId="0" xfId="0" applyNumberFormat="1" applyFont="1"/>"#,
                164 + i
            );
        }
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><numFmts count="{}">{num_fmts}</numFmts><fonts count="2"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="{}">{xfs}</cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#,
        formats.len(),
        2 + 2 * formats.len()
    )
}

/// Буквы столбца: 0 → A, 26 → AA.
fn column_letter(mut index: usize) -> String {
    let mut out = Vec::new();
    loop {
        out.push(b'A' + u8::try_from(index % 26).unwrap_or(0));
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

fn worksheet(sheet: &Worksheet) -> String {
    let mut rows = String::new();
    let mut r = 0usize;
    let mut widths: Vec<f64> = Vec::new();
    let mut row = |cells: &[(Option<&Cell>, Option<&str>)], bold: bool, rows: &mut String| {
        r += 1;
        let _ = write!(rows, r#"<row r="{r}">"#);
        for (c, (cell, text)) in cells.iter().enumerate() {
            let at = format!("{}{r}", column_letter(c));
            let text: Option<&str> = match (cell, text) {
                (Some(Cell::Text(t)), _) => Some(t.as_str()),
                (None, Some(t)) => Some(t),
                _ => None,
            };
            if let Some(Cell::Number { value, decimals }) = cell
                && value.is_finite()
            {
                let style = if bold { BOLD_NUMBER } else { NUMBER } + u32::from((*decimals).min(3));
                let _ = write!(rows, r#"<c r="{at}" s="{style}"><v>{value}</v></c>"#);
            } else if let Some(t) = text
                && !t.is_empty()
            {
                let style = if bold { BOLD } else { 0 };
                let _ = write!(
                    rows,
                    r#"<c r="{at}" t="inlineStr" s="{style}"><is><t xml:space="preserve">{}</t></is></c>"#,
                    escape(t)
                );
            }
        }
        rows.push_str("</row>");
    };
    for (k, table) in sheet.tables.iter().enumerate() {
        if k > 0 {
            row(&[], false, &mut rows);
        }
        row(&[(None, Some(table.title.as_str()))], true, &mut rows);
        let headers: Vec<(Option<&Cell>, Option<&str>)> = table
            .columns
            .iter()
            .map(|c| (None, Some(c.header.as_str())))
            .collect();
        row(&headers, true, &mut rows);
        for (i, c) in table.columns.iter().enumerate() {
            let w = (c.width / 2.2).clamp(6.0, 60.0);
            if widths.len() <= i {
                widths.push(w);
            } else {
                widths[i] = widths[i].max(w);
            }
        }
        for cells in &table.rows {
            let cells: Vec<_> = cells.iter().map(|c| (Some(c), None)).collect();
            row(&cells, false, &mut rows);
        }
        if let Some(footer) = &table.footer {
            let cells: Vec<_> = footer.iter().map(|c| (Some(c), None)).collect();
            row(&cells, true, &mut rows);
        }
    }
    let cols: String = widths
        .iter()
        .enumerate()
        .map(|(i, w)| {
            format!(
                r#"<col min="{0}" max="{0}" width="{1:.1}" customWidth="1"/>"#,
                i + 1,
                w
            )
        })
        .collect();
    let cols = if cols.is_empty() {
        String::new()
    } else {
        format!("<cols>{cols}</cols>")
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">{cols}<sheetData>{rows}</sheetData></worksheet>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{sheet::Align, table::Column};

    #[test]
    fn workbook_has_sheets_and_cells() {
        let table = Table {
            title: "Экспликация помещений".into(),
            columns: vec![
                Column::new("№", 15.0, Align::Center),
                Column::new("Наименование", 100.0, Align::Left),
                Column::new("Площадь, м²", 40.0, Align::Right),
            ],
            rows: vec![vec![
                Cell::text("1"),
                Cell::text("Кухня & столовая"),
                Cell::number(12.5, 2),
            ]],
            footer: Some(vec![
                Cell::text(""),
                Cell::text("Итого"),
                Cell::number(12.5, 2),
            ]),
        };
        let bytes = write(&[
            Worksheet {
                name: "Обмер".into(),
                tables: vec![table.clone()],
            },
            Worksheet {
                name: "Обмер".into(),
                tables: vec![table.clone(), table],
            },
        ]);
        assert!(bytes.is_ok());
        let mut archive =
            zip::ZipArchive::new(std::io::Cursor::new(bytes.unwrap_or_default())).ok();
        let mut read = |name: &str| {
            use std::io::Read as _;
            let mut s = String::new();
            if let Some(a) = archive.as_mut()
                && let Ok(mut f) = a.by_name(name)
            {
                let _ = f.read_to_string(&mut s);
            }
            s
        };
        let workbook = read("xl/workbook.xml");
        assert!(workbook.contains(r#"name="Обмер""#) && workbook.contains(r#"name="Обмер 2""#));
        let sheet = read("xl/worksheets/sheet2.xml");
        assert!(sheet.contains("Кухня &amp; столовая"));
        assert!(sheet.contains("<v>12.5</v>"));
        assert!(
            sheet.contains(r#"<row r="7">"#),
            "две таблицы с пустой строкой"
        );
        assert!(!read("xl/styles.xml").is_empty());
    }

    #[test]
    fn column_letters() {
        assert_eq!(column_letter(0), "A");
        assert_eq!(column_letter(25), "Z");
        assert_eq!(column_letter(26), "AA");
    }
}
