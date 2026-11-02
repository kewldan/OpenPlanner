//! Контейнер проекта OpenPlanner — файл `.oplan`.
//!
//! `.oplan` — ZIP-архив:
//!
//! ```text
//! manifest.json   версия формата, приложение
//! model.json      документ проекта
//! assets/         двоичные файлы проекта (изображения подложек)
//! scan/           исходные файлы сканов
//! previews/ renders/ prices/   — появятся на следующих стадиях
//! ```
//!
//! Крейт не зависит от модели приложения: документ передаётся как любой
//! `serde`-тип. Лицензия — MIT.
//!
//! Рядом с файлом проекта может лежать журнал автосохранения — см. [`journal`].

pub mod journal;
mod migrate;

use std::io::{Read, Seek, Write};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

/// Текущая версия формата.
pub const FORMAT_VERSION: u32 = 8;

/// Имя файла манифеста внутри контейнера.
pub const MANIFEST_PATH: &str = "manifest.json";
/// Имя файла модели внутри контейнера.
pub const MODEL_PATH: &str = "model.json";
/// Папка двоичных файлов внутри контейнера.
pub const ASSETS_DIR: &str = "assets/";
/// Папка исходных файлов сканов.
pub const SCAN_DIR: &str = "scan/";

/// Двоичный файл контейнера: путь внутри архива и содержимое.
pub type Asset = (String, Vec<u8>);

/// Ошибки чтения и записи `.oplan`.
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    /// Ошибка ввода-вывода.
    #[error("ошибка ввода-вывода: {0}")]
    Io(#[from] std::io::Error),
    /// Повреждённый ZIP.
    #[error("файл повреждён или это не проект OpenPlanner: {0}")]
    Zip(#[from] zip::result::ZipError),
    /// Некорректный JSON.
    #[error("некорректные данные проекта: {0}")]
    Json(#[from] serde_json::Error),
    /// Не удалось перевести модель из старой версии формата.
    #[error("не удалось обновить проект из формата {from}: {reason}")]
    Migration {
        /// Версия формата в файле.
        from: u32,
        /// Причина.
        reason: String,
    },
    /// Файл создан более новой версией приложения.
    #[error(
        "проект создан более новой версией OpenPlanner (формат {found}, поддерживается до {supported})"
    )]
    UnsupportedVersion {
        /// Версия формата в файле.
        found: u32,
        /// Максимальная поддерживаемая версия.
        supported: u32,
    },
}

/// Манифест контейнера.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// Версия формата.
    pub format_version: u32,
    /// Приложение и версия, записавшие файл, например `OpenPlanner 0.1.0`.
    pub generator: String,
}

impl Manifest {
    /// Манифест текущей версии формата.
    #[must_use]
    pub fn current(generator: impl Into<String>) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            generator: generator.into(),
        }
    }
}

/// Записывает контейнер с манифестом, моделью и двоичными файлами.
///
/// Файлы с путями вне `assets/` пропускаются.
///
/// # Errors
///
/// Возвращает [`FormatError`] при ошибке сериализации или записи.
pub fn write<'a, W, M, A>(
    writer: W,
    manifest: &Manifest,
    model: &M,
    assets: A,
) -> Result<W, FormatError>
where
    W: Write + Seek,
    M: Serialize,
    A: IntoIterator<Item = (&'a str, &'a [u8])>,
{
    let mut zip = ZipWriter::new(writer);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip.start_file(MANIFEST_PATH, options)?;
    serde_json::to_writer_pretty(&mut zip, manifest)?;
    zip.start_file(MODEL_PATH, options)?;
    serde_json::to_writer_pretty(&mut zip, model)?;

    // Изображения уже сжаты — храним без повторного сжатия.
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (path, bytes) in assets {
        if !is_asset_path(path) {
            continue;
        }
        zip.start_file(path, stored)?;
        zip.write_all(bytes)?;
    }

    Ok(zip.finish()?)
}

/// Путь допустим для двоичного файла: внутри `assets/` или `scan/`, без выхода наверх.
#[must_use]
pub fn is_asset_path(path: &str) -> bool {
    [ASSETS_DIR, SCAN_DIR]
        .iter()
        .any(|dir| path.starts_with(dir) && path.len() > dir.len())
        && !path.contains("..")
}

/// Читает контейнер и возвращает манифест, модель и двоичные файлы.
///
/// # Errors
///
/// Возвращает [`FormatError`], если файл повреждён, не является проектом
/// или создан более новой версией формата.
pub fn read<R, M>(reader: R) -> Result<(Manifest, M, Vec<Asset>), FormatError>
where
    R: Read + Seek,
    M: DeserializeOwned,
{
    let mut zip = ZipArchive::new(reader)?;
    let manifest: Manifest = read_json(&mut zip, MANIFEST_PATH)?;
    if manifest.format_version > FORMAT_VERSION {
        return Err(FormatError::UnsupportedVersion {
            found: manifest.format_version,
            supported: FORMAT_VERSION,
        });
    }
    let raw: serde_json::Value = read_json(&mut zip, MODEL_PATH)?;
    let model = serde_json::from_value(migrate::migrate(raw, manifest.format_version)?)?;

    let paths: Vec<String> = zip
        .file_names()
        .filter_map(Result::ok)
        .filter(|name| is_asset_path(name))
        .map(std::borrow::Cow::into_owned)
        .collect();
    let mut assets = Vec::with_capacity(paths.len());
    for path in paths {
        let mut file = zip.by_name(&path)?;
        let mut bytes = Vec::with_capacity(usize::try_from(file.size()).unwrap_or(0));
        file.read_to_end(&mut bytes)?;
        assets.push((path, bytes));
    }
    Ok((manifest, model, assets))
}

fn read_json<R: Read + Seek, T: DeserializeOwned>(
    zip: &mut ZipArchive<R>,
    path: &str,
) -> Result<T, FormatError> {
    let mut file = zip.by_name(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Model {
        title: String,
    }

    #[test]
    fn roundtrip() {
        let model = Model {
            title: "Комендантский пр.".into(),
        };
        let manifest = Manifest::current("OpenPlanner test");
        let image: &[u8] = &[1, 2, 3];
        let bytes = write(
            Cursor::new(Vec::new()),
            &manifest,
            &model,
            [
                ("assets/a.png", image),
                ("scan/room.glb", image),
                ("../evil", image),
                ("scan/../x", image),
            ],
        )
        .unwrap()
        .into_inner();

        let (read_manifest, read_model, assets): (Manifest, Model, _) =
            read(Cursor::new(bytes)).unwrap();
        assert_eq!(read_manifest, manifest);
        assert_eq!(read_model, model);
        assert_eq!(
            assets,
            vec![
                ("assets/a.png".to_owned(), vec![1, 2, 3]),
                ("scan/room.glb".to_owned(), vec![1, 2, 3])
            ]
        );
    }

    #[test]
    fn rejects_newer_format() {
        let manifest = Manifest {
            format_version: FORMAT_VERSION + 1,
            generator: "future".into(),
        };
        let bytes = write(
            Cursor::new(Vec::new()),
            &manifest,
            &Model {
                title: String::new(),
            },
            [],
        )
        .unwrap()
        .into_inner();
        let result: Result<(Manifest, Model, _), _> = read(Cursor::new(bytes));
        assert!(matches!(
            result,
            Err(FormatError::UnsupportedVersion { .. })
        ));
    }

    #[test]
    fn rejects_garbage() {
        let result: Result<(Manifest, Model, _), _> = read(Cursor::new(b"not a zip".to_vec()));
        assert!(matches!(result, Err(FormatError::Zip(_))));
    }
}
