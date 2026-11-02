//! Параметрическая комната Apple RoomPlan (`CapturedRoom`, JSON).
//!
//! Поверхность: `dimensions` (ширина, высота, толщина, м), `transform`
//! (матрица 4×4 по столбцам: ось X поверхности — направление вдоль стены,
//! четвёртый столбец — центр), `category`, `confidence`. Разбор терпим к
//! вариантам записи: матрица — 16 чисел подряд или 4 столбца, перечисления —
//! строкой или объектом с одним ключом.

use glam::{DMat4, DVec3};
use serde_json::Value;

use crate::ScanError;

/// Вид поверхности.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomKind {
    /// Стена.
    Wall,
    /// Дверь.
    Door,
    /// Окно.
    Window,
    /// Проём без заполнения.
    Opening,
}

/// Поверхность комнаты (метры, система координат с осью +Y вверх).
#[derive(Debug, Clone, PartialEq)]
pub struct RoomObject {
    /// Вид.
    pub kind: RoomKind,
    /// Центр, м.
    pub center: DVec3,
    /// Направление ширины (единичное).
    pub along: DVec3,
    /// Ширина, м.
    pub width: f64,
    /// Высота, м.
    pub height: f64,
    /// Толщина, м (0 — неизвестна).
    pub depth: f64,
    /// Уверенность 0–1.
    pub confidence: f64,
}

/// Комната: стены и проёмы.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RoomPlan {
    /// Поверхности.
    pub objects: Vec<RoomObject>,
}

fn tag(v: &Value) -> Option<&str> {
    match v {
        Value::String(s) => Some(s.as_str()),
        Value::Object(map) => map.keys().next().map(String::as_str),
        _ => None,
    }
}

fn floats(v: &Value) -> Vec<f64> {
    match v {
        Value::Array(items) => items.iter().flat_map(floats).collect(),
        Value::Number(n) => n.as_f64().into_iter().collect(),
        _ => Vec::new(),
    }
}

fn object(v: &Value, kind: RoomKind) -> Option<RoomObject> {
    let dims = floats(&v["dimensions"]);
    let m = floats(&v["transform"]);
    if dims.len() < 2 || m.len() != 16 || dims.iter().chain(&m).any(|x| !x.is_finite()) {
        return None;
    }
    let matrix = DMat4::from_cols_slice(&m);
    let along = matrix.x_axis.truncate().normalize_or_zero();
    if along == DVec3::ZERO {
        return None;
    }
    let confidence = match tag(&v["confidence"]) {
        Some("high") => 0.9,
        Some("low") => 0.35,
        _ => 0.65,
    };
    let kind = match (kind, tag(&v["category"])) {
        (RoomKind::Wall, Some("door")) => RoomKind::Door,
        (RoomKind::Wall, Some("window")) => RoomKind::Window,
        (RoomKind::Wall, Some("opening")) => RoomKind::Opening,
        (RoomKind::Wall, Some("floor")) => return None,
        (k, _) => k,
    };
    Some(RoomObject {
        kind,
        center: matrix.w_axis.truncate(),
        along,
        width: dims[0],
        height: dims[1],
        depth: dims.get(2).copied().unwrap_or(0.0),
        confidence,
    })
}

/// Читает `CapturedRoom` в JSON.
pub(super) fn read(bytes: &[u8]) -> Result<RoomPlan, ScanError> {
    let json: Value = serde_json::from_slice(bytes).map_err(|_| ScanError::Corrupt("JSON"))?;
    let mut room = RoomPlan::default();
    for (key, kind) in [
        ("walls", RoomKind::Wall),
        ("doors", RoomKind::Door),
        ("windows", RoomKind::Window),
        ("openings", RoomKind::Opening),
    ] {
        for item in json[key].as_array().into_iter().flatten() {
            if let Some(o) = object(item, kind) {
                room.objects.push(o);
            }
        }
    }
    if !room.objects.iter().any(|o| o.kind == RoomKind::Wall) {
        return Err(ScanError::Corrupt("JSON: нет стен RoomPlan"));
    }
    Ok(room)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_walls_and_doors() {
        let json = br#"{
          "walls": [{"dimensions": [4.0, 2.7, 0.0], "confidence": {"high": {}},
                     "transform": [1,0,0,0, 0,1,0,0, 0,0,1,0, 2,1.35,0,1], "category": {"wall": {}}}],
          "doors": [{"dimensions": [0.8, 2.0, 0.0], "confidence": "medium",
                     "transform": [[1,0,0,0],[0,1,0,0],[0,0,1,0],[1,1,0,1]], "category": {"door": {"isOpen": true}}}]
        }"#;
        let room = read(json).unwrap();
        assert_eq!(room.objects.len(), 2);
        assert_eq!(room.objects[0].center, DVec3::new(2.0, 1.35, 0.0));
        assert!((room.objects[0].confidence - 0.9).abs() < 1e-9);
        assert_eq!(room.objects[1].kind, RoomKind::Door);
    }
}
