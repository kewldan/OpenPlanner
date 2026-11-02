//! Экспорт OpenPlanner: листы чертежей и их запись в файлы.
//!
//! Лист ([`Sheet`]) — векторное описание в миллиметрах бумаги: заливки, линии,
//! текст по слоям и видовые экраны с планом в своём масштабе. Из модели листы
//! собирает движок; здесь — рамка с основной надписью, таблицы, штриховки и
//! запись в PDF (векторный, со слоями), SVG, DXF (по слоям), XLSX и ZIP.

pub mod dxf;
pub mod font;
pub mod frame;
pub mod hatch;
pub mod pdf;
pub mod sheet;
pub mod svg;
pub mod table;
pub mod xlsx;

pub use frame::TitleBlock;
pub use sheet::{Align, Item, Rgb, Shape, Sheet, Text, Viewport};
pub use table::{Cell, Column, Table};

/// Ошибка записи файла.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// Встроенный шрифт не читается.
    #[error("встроенный шрифт не читается")]
    Font,
    /// Ошибка ZIP-архива.
    #[error("ошибка архива: {0}")]
    Zip(#[from] zip::result::ZipError),
    /// Ошибка ввода-вывода (в памяти — не должна возникать).
    #[error("ошибка записи: {0}")]
    Io(#[from] std::io::Error),
}

/// ZIP-архив из файлов (имя, содержимое).
///
/// # Errors
///
/// Ошибка архива.
pub fn zip(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, ExportError> {
    use std::io::Write as _;
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    let mut out = ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, bytes) in files {
        out.start_file(name.as_str(), options)?;
        out.write_all(bytes)?;
    }
    Ok(out.finish()?.into_inner())
}

/// Имя файла без символов, недопустимых в Windows, macOS и Linux.
#[must_use]
pub fn safe_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches('.').trim();
    if trimmed.is_empty() {
        "Без имени".to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_safe() {
        assert_eq!(safe_file_name("План: кв. 5/6"), "План_ кв. 5_6");
        assert_eq!(safe_file_name("  ..."), "Без имени");
    }

    #[test]
    fn zip_holds_files() {
        let bytes = zip(&[
            ("a.txt".into(), b"1".to_vec()),
            ("б.txt".into(), b"2".to_vec()),
        ]);
        let bytes = bytes.unwrap_or_default();
        assert!(bytes.starts_with(b"PK"));
        let archive = zip::ZipArchive::new(std::io::Cursor::new(bytes));
        assert_eq!(archive.map(|a| a.len()).unwrap_or(0), 2);
    }
}
