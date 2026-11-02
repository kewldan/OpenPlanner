//! Миграции `model.json` между версиями формата.
//!
//! Каждая миграция переводит модель из версии `N` в `N + 1`. При изменении
//! формата: увеличить [`FORMAT_VERSION`](crate::FORMAT_VERSION) и добавить
//! функцию в конец [`MIGRATIONS`].

use serde_json::Value;

use crate::{FORMAT_VERSION, FormatError};

/// Миграция модели на одну версию вперёд.
pub(crate) type Migration = fn(Value) -> Result<Value, String>;

/// `MIGRATIONS[i]` переводит формат `i + 1` в `i + 2`.
const MIGRATIONS: &[Migration] = &[];

const _: () = assert!(
    MIGRATIONS.len() + 1 == FORMAT_VERSION as usize,
    "на каждую версию формата после первой нужна миграция",
);

/// Переводит модель из версии `from` в текущую.
pub(crate) fn migrate(model: Value, from: u32) -> Result<Value, FormatError> {
    migrate_with(model, from, MIGRATIONS)
}

fn migrate_with(
    mut model: Value,
    from: u32,
    migrations: &[Migration],
) -> Result<Value, FormatError> {
    if from == 0 {
        return Err(FormatError::Migration {
            from,
            reason: "версия формата 0 не существует".into(),
        });
    }
    let start = usize::try_from(from - 1).unwrap_or(usize::MAX);
    for migration in migrations.iter().skip(start) {
        model = migration(model).map_err(|reason| FormatError::Migration { from, reason })?;
    }
    Ok(model)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn v1_to_v2(mut model: Value) -> Result<Value, String> {
        let object = model.as_object_mut().ok_or("ожидался объект")?;
        let title = object.remove("name").unwrap_or(Value::Null);
        object.insert("meta".into(), json!({ "title": title }));
        Ok(model)
    }

    fn v2_to_v3(mut model: Value) -> Result<Value, String> {
        model["defaultCeilingHeight"] = json!(2700.0);
        Ok(model)
    }

    const CHAIN: &[Migration] = &[v1_to_v2, v2_to_v3];

    #[test]
    fn applies_chain_from_old_version() {
        let migrated = migrate_with(json!({ "name": "Кв." }), 1, CHAIN).unwrap();
        assert_eq!(
            migrated,
            json!({ "meta": { "title": "Кв." }, "defaultCeilingHeight": 2700.0 })
        );
    }

    #[test]
    fn skips_migrations_already_applied() {
        let model = json!({ "meta": { "title": "Кв." } });
        let migrated = migrate_with(model, 2, CHAIN).unwrap();
        assert_eq!(migrated["defaultCeilingHeight"], json!(2700.0));
    }

    #[test]
    fn current_version_is_untouched() {
        let model = json!({ "anything": 1 });
        assert_eq!(migrate(model.clone(), FORMAT_VERSION).unwrap(), model);
    }

    #[test]
    fn reports_failed_migration() {
        let err = migrate_with(json!([]), 1, CHAIN).unwrap_err();
        assert!(matches!(err, FormatError::Migration { from: 1, .. }));
    }
}
