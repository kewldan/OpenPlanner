//! Журнал автосохранения.
//!
//! Журнал — текстовый файл рядом с проектом (`<имя>.oplan.journal`), по
//! одной JSON-записи на строку (JSON Lines). Первая строка — заголовок с
//! версией формата, дальше — операции над документом. Воспроизведение
//! операций поверх сохранённого проекта восстанавливает несохранённые
//! изменения после сбоя.
//!
//! Последняя строка могла записаться не полностью (сбой посреди записи):
//! такая строка отбрасывается, остальные читаются.

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{FORMAT_VERSION, FormatError};

/// Расширение файла журнала, добавляемое к имени проекта.
pub const EXTENSION: &str = "journal";

/// Заголовок журнала.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Header {
    /// Версия формата проекта, к которому относится журнал.
    pub format_version: u32,
}

/// Строка заголовка для нового журнала (с переводом строки).
///
/// # Errors
///
/// Ошибка сериализации (не должна возникать).
pub fn header_line() -> Result<String, FormatError> {
    Ok(serde_json::to_string(&Header {
        format_version: FORMAT_VERSION,
    })? + "\n")
}

/// Строка журнала для одной операции (с переводом строки).
///
/// # Errors
///
/// Ошибка сериализации операции.
pub fn entry_line<T: Serialize>(entry: &T) -> Result<String, FormatError> {
    Ok(serde_json::to_string(entry)? + "\n")
}

/// Разбирает журнал.
///
/// Возвращает операции; оборванная последняя строка отбрасывается.
///
/// # Errors
///
/// Ошибка, если нет заголовка, журнал от более новой версии формата или
/// повреждена строка в середине файла.
pub fn parse<T: DeserializeOwned>(text: &str) -> Result<Vec<T>, FormatError> {
    let mut lines = text.split_inclusive('\n');
    let header: Header = match lines.next() {
        Some(line) if line.ends_with('\n') => serde_json::from_str(line)?,
        _ => return Ok(Vec::new()),
    };
    if header.format_version > FORMAT_VERSION {
        return Err(FormatError::UnsupportedVersion {
            found: header.format_version,
            supported: FORMAT_VERSION,
        });
    }
    let mut entries = Vec::new();
    for line in lines {
        if !line.ends_with('\n') {
            break; // запись оборвалась на середине
        }
        let line = line.trim();
        if !line.is_empty() {
            entries.push(serde_json::from_str(line)?);
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(tag = "op", rename_all = "camelCase")]
    enum Op {
        Rename { title: String },
        Undo,
    }

    fn journal(ops: &[Op]) -> String {
        let mut text = header_line().unwrap();
        for op in ops {
            text += &entry_line(op).unwrap();
        }
        text
    }

    #[test]
    fn roundtrip() {
        let ops = vec![
            Op::Rename {
                title: "Кв.".into(),
            },
            Op::Undo,
        ];
        let parsed: Vec<Op> = parse(&journal(&ops)).unwrap();
        assert_eq!(parsed, ops);
    }

    #[test]
    fn torn_last_line_is_dropped() {
        let mut text = journal(&[Op::Undo]);
        text += r#"{"op":"rename","tit"#;
        let parsed: Vec<Op> = parse(&text).unwrap();
        assert_eq!(parsed, vec![Op::Undo]);
    }

    #[test]
    fn empty_or_headerless_journal_is_empty() {
        assert!(parse::<Op>("").unwrap().is_empty());
        assert!(parse::<Op>(r#"{"formatVer"#).unwrap().is_empty());
    }

    #[test]
    fn corrupted_middle_line_is_an_error() {
        let text = header_line().unwrap() + "мусор\n" + &entry_line(&Op::Undo).unwrap();
        assert!(parse::<Op>(&text).is_err());
    }

    #[test]
    fn newer_journal_is_rejected() {
        let text = format!("{{\"formatVersion\":{}}}\n", FORMAT_VERSION + 1);
        assert!(matches!(
            parse::<Op>(&text),
            Err(FormatError::UnsupportedVersion { .. })
        ));
    }
}
