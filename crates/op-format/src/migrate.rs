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
const MIGRATIONS: &[Migration] = &[
    v1_to_v2, v2_to_v3, v3_to_v4, v4_to_v5, v5_to_v6, v6_to_v7, v7_to_v8, v8_to_v9, v9_to_v10,
];

/// Формат 10 (стадия 10): прайс проекта и объёмы сметы, заданные вручную.
fn v9_to_v10(mut model: Value) -> Result<Value, String> {
    let object = model.as_object_mut().ok_or("ожидался объект модели")?;
    for key in ["prices", "overrides"] {
        object
            .entry(key)
            .or_insert_with(|| Value::Array(Vec::new()));
    }
    Ok(model)
}

/// Формат 9 (стадии 8 и 9): сантехника (точки, трубы, слой «Сантехника»), материалы и отделка.
fn v8_to_v9(mut model: Value) -> Result<Value, String> {
    let object = model.as_object_mut().ok_or("ожидался объект модели")?;
    for key in ["plumbing", "pipes", "materials", "finishes"] {
        object
            .entry(key)
            .or_insert_with(|| Value::Array(Vec::new()));
    }
    let Some(layers) = object.get_mut("layers").and_then(Value::as_array_mut) else {
        return Ok(model);
    };
    if !layers.iter().any(|l| l["id"] == "layer-plumbing") {
        let at = layers
            .iter()
            .position(|l| l["id"] == "layer-openings")
            .map_or(layers.len(), |i| i + 1);
        layers.insert(
            at,
            serde_json::json!({ "id": "layer-plumbing", "name": "Сантехника", "visible": true, "locked": false, "system": "plumbing" }),
        );
    }
    Ok(model)
}

/// Формат 8 (стадия 7): электроточки, группы щита, сценарии освещения и слой «Электрика».
fn v7_to_v8(mut model: Value) -> Result<Value, String> {
    let object = model.as_object_mut().ok_or("ожидался объект модели")?;
    for key in ["devices", "circuits", "scenes"] {
        object
            .entry(key)
            .or_insert_with(|| Value::Array(Vec::new()));
    }
    let Some(layers) = object.get_mut("layers").and_then(Value::as_array_mut) else {
        return Ok(model);
    };
    if !layers.iter().any(|l| l["id"] == "layer-electrical") {
        let at = layers
            .iter()
            .position(|l| l["id"] == "layer-openings")
            .map_or(layers.len(), |i| i + 1);
        layers.insert(
            at,
            serde_json::json!({ "id": "layer-electrical", "name": "Электрика", "visible": true, "locked": false, "system": "electrical" }),
        );
    }
    Ok(model)
}

/// Формат 7 (стадия 6): мебель и системный слой «Мебель» над помещениями.
fn v6_to_v7(mut model: Value) -> Result<Value, String> {
    let object = model.as_object_mut().ok_or("ожидался объект модели")?;
    object
        .entry("furniture")
        .or_insert_with(|| Value::Array(Vec::new()));
    // Списка слоёв нет только в обрезанных моделях (тесты миграций) — тогда слои
    // возьмутся из документа по умолчанию.
    let Some(layers) = object.get_mut("layers").and_then(Value::as_array_mut) else {
        return Ok(model);
    };
    if !layers.iter().any(|l| l["id"] == "layer-furniture") {
        let at = layers
            .iter()
            .position(|l| l["id"] == "layer-rooms")
            .map_or(layers.len(), |i| i + 1);
        layers.insert(
            at,
            serde_json::json!({ "id": "layer-furniture", "name": "Мебель", "visible": true, "locked": false, "system": "furniture" }),
        );
    }
    Ok(model)
}

/// Формат 6 (стадия 5): привязка к местности для солнца и сохранённые ракурсы 3D.
fn v5_to_v6(mut model: Value) -> Result<Value, String> {
    let object = model.as_object_mut().ok_or("ожидался объект модели")?;
    object.entry("site").or_insert_with(|| {
        serde_json::json!({ "north": 0.0, "latitude": 55.75, "longitude": 37.62, "utcOffset": 3.0 })
    });
    object
        .entry("views")
        .or_insert_with(|| Value::Array(Vec::new()));
    Ok(model)
}

/// Формат 5 (стадия 4): графы основной надписи и настройки чертежей.
fn v4_to_v5(mut model: Value) -> Result<Value, String> {
    let object = model.as_object_mut().ok_or("ожидался объект модели")?;
    object
        .entry("drawing")
        .or_insert_with(|| serde_json::json!({ "paper": "a3Landscape" }));
    let meta = object
        .get_mut("meta")
        .and_then(Value::as_object_mut)
        .ok_or("ожидались сведения о проекте")?;
    for key in ["designation", "organization", "developer", "checker"] {
        meta.entry(key)
            .or_insert_with(|| Value::String(String::new()));
    }
    meta.entry("stage")
        .or_insert_with(|| Value::String("Р".into()));
    Ok(model)
}

/// Формат 4 (стадия 3): варианты перепланировки. Элементы без поля
/// `variant` остаются элементами обмера.
fn v3_to_v4(mut model: Value) -> Result<Value, String> {
    let object = model.as_object_mut().ok_or("ожидался объект модели")?;
    object
        .entry("variants")
        .or_insert_with(|| Value::Array(Vec::new()));
    Ok(model)
}

/// Формат 3 (стадия 2): системный слой «Скан» в самом низу.
fn v2_to_v3(mut model: Value) -> Result<Value, String> {
    let layers = model
        .get_mut("layers")
        .and_then(Value::as_array_mut)
        .ok_or("ожидался список слоёв")?;
    if !layers.iter().any(|l| l["id"] == "layer-scan") {
        layers.insert(
            0,
            serde_json::json!({ "id": "layer-scan", "name": "Скан", "visible": true, "locked": false, "system": "scan" }),
        );
    }
    Ok(model)
}

/// Формат 2 (стадия 1): слои и элементы обмера.
fn v1_to_v2(mut model: Value) -> Result<Value, String> {
    let object = model.as_object_mut().ok_or("ожидался объект модели")?;
    let layer = |id: &str, name: &str, kind: &str| serde_json::json!({ "id": id, "name": name, "visible": true, "locked": false, "system": kind });
    object.entry("layers").or_insert_with(|| {
        Value::Array(vec![
            layer("layer-underlays", "Подложка", "underlays"),
            layer("layer-rooms", "Помещения", "rooms"),
            layer("layer-walls", "Стены", "walls"),
            layer("layer-openings", "Проёмы", "openings"),
            layer("layer-dimensions", "Размеры", "dimensions"),
            layer("layer-leaders", "Выноски", "leaders"),
        ])
    });
    for key in [
        "nodes",
        "walls",
        "openings",
        "rooms",
        "dimensions",
        "leaders",
        "underlays",
    ] {
        object
            .entry(key)
            .or_insert_with(|| Value::Array(Vec::new()));
    }
    Ok(model)
}

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
    fn v1_gets_system_layers_and_empty_lists() {
        let model =
            json!({ "meta": { "title": "Кв.", "address": "" }, "defaultCeilingHeight": 2700.0 });
        let migrated = migrate(model, 1).unwrap();
        assert_eq!(migrated["layers"].as_array().map(Vec::len), Some(10));
        assert_eq!(migrated["layers"][0]["id"], json!("layer-scan"));
        assert_eq!(migrated["walls"], json!([]));
        assert_eq!(migrated["variants"], json!([]));
        assert_eq!(migrated["meta"]["title"], json!("Кв."));
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

#[cfg(test)]
mod format5 {
    use super::*;

    #[test]
    #[allow(clippy::unwrap_used)] // тест
    fn v4_gets_title_block_fields_and_drawing_settings() {
        let model = serde_json::json!({ "meta": { "title": "Кв", "address": "" }, "variants": [] });
        let model = migrate(model, 4).unwrap();
        assert_eq!(model["meta"]["stage"], "Р");
        assert_eq!(model["meta"]["designation"], "");
        assert_eq!(model["drawing"]["paper"], "a3Landscape");
    }

    #[test]
    #[allow(clippy::unwrap_used)] // тест
    fn v9_gets_prices_and_overrides() {
        let model = v9_to_v10(serde_json::json!({ "layers": [] })).unwrap();
        assert_eq!(model["prices"], serde_json::json!([]));
        assert_eq!(model["overrides"], serde_json::json!([]));
    }

    #[test]
    #[allow(clippy::unwrap_used)] // тест
    fn v8_gets_plumbing_finishes_and_layer() {
        let model = v8_to_v9(serde_json::json!({
            "layers": [
                { "id": "layer-walls" },
                { "id": "layer-openings" },
                { "id": "layer-electrical" }
            ]
        }))
        .unwrap();
        for key in ["plumbing", "pipes", "materials", "finishes"] {
            assert_eq!(model[key], serde_json::json!([]));
        }
        assert_eq!(model["layers"][2]["id"], "layer-plumbing");
        assert_eq!(model["layers"][3]["id"], "layer-electrical");
    }

    #[test]
    fn v7_gets_devices_circuits_scenes_and_layer() {
        let model = serde_json::json!({
            "layers": [{ "id": "layer-openings" }, { "id": "layer-dimensions" }],
        });
        let model = migrate(model, 7).unwrap();
        assert_eq!(model["devices"], serde_json::json!([]));
        assert_eq!(model["circuits"], serde_json::json!([]));
        // Следующая миграция ставит «Сантехнику» сразу над проёмами.
        assert_eq!(model["layers"][1]["id"], "layer-plumbing");
        assert_eq!(model["layers"][2]["id"], "layer-electrical");
    }

    #[test]
    #[allow(clippy::unwrap_used)] // тест
    fn v6_gets_furniture_layer_above_rooms() {
        let model = serde_json::json!({
            "layers": [{ "id": "layer-rooms" }, { "id": "layer-walls" }],
        });
        let model = migrate(model, 6).unwrap();
        assert_eq!(model["furniture"], serde_json::json!([]));
        assert_eq!(model["layers"][1]["id"], "layer-furniture");
        assert_eq!(model["layers"][2]["id"], "layer-walls");
    }

    #[test]
    #[allow(clippy::unwrap_used)] // тест
    fn v5_gets_site_and_saved_views() {
        let model = serde_json::json!({ "meta": { "title": "Кв", "address": "" }, "variants": [] });
        let model = migrate(model, 5).unwrap();
        assert_eq!(model["site"]["latitude"], 55.75);
        assert_eq!(model["views"], serde_json::json!([]));
    }
}
