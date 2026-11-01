//! JSON Schema документа — `schema/model.schema.json`.
//!
//! Схема генерируется из Rust-типов. Тест сверяет её с файлом в репозитории;
//! чтобы обновить файл, запустите `UPDATE_SCHEMA=1 cargo test -p op-model`.

use crate::Document;

/// JSON Schema файла `model.json` внутри `.oplan`.
#[must_use]
pub fn model_schema() -> serde_json::Value {
    let mut schema = schemars::schema_for!(Document).to_value();
    if let Some(object) = schema.as_object_mut() {
        object.insert(
            "$id".into(),
            "https://github.com/kewldan/OpenPlanner/schema/model.schema.json".into(),
        );
        object.insert(
            "title".into(),
            "Модель проекта OpenPlanner (model.json)".into(),
        );
    }
    schema
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn committed_schema_is_up_to_date() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schema/model.schema.json");
        let generated = serde_json::to_string_pretty(&model_schema()).unwrap() + "\n";
        if std::env::var_os("UPDATE_SCHEMA").is_some() {
            std::fs::write(&path, &generated).unwrap();
            return;
        }
        let committed = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            committed == generated,
            "schema/model.schema.json устарела: запустите UPDATE_SCHEMA=1 cargo test -p op-model"
        );
    }
}
