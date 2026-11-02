//! Таблицы в CSV: запись для Excel с русской локалью (разделитель «;», UTF-8
//! с BOM) и разбор с определением разделителя («;», «,» или табуляция).

use crate::table::{Cell, Table};

fn quote(field: &str, separator: char) -> String {
    if field.contains([separator, '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_owned()
    }
}

/// CSV из строк: разделитель «;», UTF-8 с BOM, строки через CRLF.
#[must_use]
pub fn write(rows: &[Vec<String>]) -> String {
    let mut out = String::from('\u{feff}');
    for row in rows {
        let line: Vec<String> = row.iter().map(|f| quote(f, ';')).collect();
        out.push_str(&line.join(";"));
        out.push_str("\r\n");
    }
    out
}

/// Значение ячейки для CSV: числа — с запятой без разрядных пробелов.
fn plain(cell: &Cell) -> String {
    match cell {
        Cell::Number { value, decimals } => {
            format!("{:.*}", usize::from(*decimals), value).replace('.', ",")
        }
        other => other.display(),
    }
}

/// Строки CSV из таблиц: заголовок таблицы, шапка, строки, итог, пустая строка.
#[must_use]
pub fn from_tables(tables: &[Table]) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    for (i, t) in tables.iter().enumerate() {
        if i > 0 {
            out.push(Vec::new());
        }
        out.push(vec![t.title.clone()]);
        out.push(t.columns.iter().map(|c| c.header.clone()).collect());
        out.extend(t.rows.iter().map(|r| r.iter().map(plain).collect()));
        if let Some(footer) = &t.footer {
            out.push(footer.iter().map(plain).collect());
        }
    }
    out
}

/// Разбирает CSV: разделитель — тот из «;», «,», табуляции, которого больше в первой строке.
#[must_use]
pub fn parse(text: &str) -> Vec<Vec<String>> {
    let text = text.trim_start_matches('\u{feff}');
    let first = text.lines().next().unwrap_or("");
    let separator = [';', '\t', ',']
        .into_iter()
        .max_by_key(|c| first.matches(*c).count())
        .unwrap_or(';');
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == separator && !quoted => row.push(std::mem::take(&mut field)),
            '\r' if !quoted => {}
            '\n' if !quoted => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            c => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_with_quotes_and_separators() {
        let rows = vec![
            vec!["Код".to_owned(), "Наименование".to_owned()],
            vec!["Д-1".to_owned(), "Демонтаж; «стен» \"ГКЛ\"".to_owned()],
        ];
        assert_eq!(parse(&write(&rows)), rows);
    }

    #[test]
    fn detects_comma_separator() {
        assert_eq!(
            parse("a,b\n1,\"2,5\"\n"),
            vec![vec!["a", "b"], vec!["1", "2,5"]]
        );
    }

    #[test]
    fn tables_become_rows() {
        let t = Table {
            title: "Смета".into(),
            columns: vec![crate::Column::new("Сумма", 20.0, crate::Align::Right)],
            rows: vec![vec![Cell::number(12_345.5, 2)]],
            footer: None,
        };
        assert_eq!(
            from_tables(&[t]),
            vec![vec!["Смета"], vec!["Сумма"], vec!["12345,50"]]
        );
    }
}
