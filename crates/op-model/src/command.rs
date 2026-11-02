use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{AssetData, Document, DrawingSettings, Element, ProjectMeta, element};

/// Ошибка применения команды. Документ при ошибке не меняется.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CommandError {
    /// Значение вне допустимого диапазона.
    #[error("недопустимое значение «{field}»: {reason}")]
    InvalidValue {
        /// Имя поля.
        field: &'static str,
        /// Причина на человеческом языке.
        reason: &'static str,
    },
    /// Элемент, на который ссылается команда, не найден.
    #[error("элемент «{id}» не найден")]
    Missing {
        /// Идентификатор.
        id: String,
    },
    /// Идентификатор уже занят элементом другого типа.
    #[error("идентификатор «{id}» уже занят")]
    Conflict {
        /// Идентификатор.
        id: String,
    },
    /// На элемент ссылаются другие элементы.
    #[error("элемент «{id}» используется другими элементами")]
    Referenced {
        /// Идентификатор.
        id: String,
    },
}

/// Атомарное обратимое изменение документа.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Command {
    /// Переименовать проект.
    SetTitle {
        /// Новое название.
        title: String,
    },
    /// Изменить адрес объекта.
    SetAddress {
        /// Новый адрес.
        address: String,
    },
    /// Изменить высоту потолка по умолчанию.
    SetDefaultCeilingHeight {
        /// Новая высота, мм.
        height: f64,
    },
    /// Заменить сведения о проекте целиком (графы основной надписи).
    SetMeta {
        /// Новые сведения.
        meta: ProjectMeta,
    },
    /// Изменить настройки чертежей.
    SetDrawing {
        /// Новые настройки.
        drawing: DrawingSettings,
    },
    /// Добавить элемент или заменить элемент с тем же идентификатором.
    Put {
        /// Элемент.
        element: Element,
        /// Позиция в списке для нового элемента; без значения — в конец.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "wasm", tsify(optional))]
        index: Option<usize>,
    },
    /// Удалить элемент. На него не должны ссылаться другие элементы.
    Remove {
        /// Идентификатор.
        id: String,
    },
    /// Добавить двоичный файл в проект.
    PutAsset {
        /// Путь внутри `.oplan`.
        path: String,
        /// Содержимое (в JSON — base64).
        #[serde(with = "base64_bytes")]
        #[schemars(with = "String")]
        #[cfg_attr(feature = "wasm", tsify(type = "string"))]
        data: AssetData,
    },
    /// Удалить двоичный файл. На него не должны ссылаться подложки.
    RemoveAsset {
        /// Путь внутри `.oplan`.
        path: String,
    },
    /// Несколько команд как одно действие: применяются все или ни одной.
    Batch {
        /// Команды в порядке применения.
        commands: Vec<Command>,
    },
}

impl Command {
    /// Применяет команду и возвращает обратную к ней.
    ///
    /// # Errors
    ///
    /// Возвращает [`CommandError`], если команда недопустима. В этом случае
    /// документ остаётся без изменений.
    pub fn apply(self, doc: &mut Document) -> Result<Self, CommandError> {
        match self {
            Self::SetTitle { title } => {
                let previous = std::mem::replace(&mut doc.meta.title, title);
                Ok(Self::SetTitle { title: previous })
            }
            Self::SetAddress { address } => {
                let previous = std::mem::replace(&mut doc.meta.address, address);
                Ok(Self::SetAddress { address: previous })
            }
            Self::SetDefaultCeilingHeight { height } => {
                if !height.is_finite() || height <= 0.0 {
                    return Err(CommandError::InvalidValue {
                        field: "height",
                        reason: "высота потолка должна быть положительным числом",
                    });
                }
                let previous = std::mem::replace(&mut doc.default_ceiling_height, height);
                Ok(Self::SetDefaultCeilingHeight { height: previous })
            }
            Self::SetMeta { meta } => {
                let previous = std::mem::replace(&mut doc.meta, meta);
                Ok(Self::SetMeta { meta: previous })
            }
            Self::SetDrawing { drawing } => {
                if drawing.scale.is_some_and(|s| s == 0 || s > 10_000) {
                    return Err(CommandError::InvalidValue {
                        field: "scale",
                        reason: "масштаб должен быть от 1:1 до 1:10000",
                    });
                }
                let previous = std::mem::replace(&mut doc.drawing, drawing);
                Ok(Self::SetDrawing { drawing: previous })
            }
            Self::Put { element, index } => {
                let id = element.id().to_owned();
                Ok(match element::put(doc, element, index)? {
                    Some(previous) => Self::Put {
                        element: previous,
                        index: None,
                    },
                    None => Self::Remove { id },
                })
            }
            Self::Remove { id } => {
                let snapshot =
                    element::get(doc, &id).ok_or(CommandError::Missing { id: id.clone() })?;
                element::ensure_unreferenced(doc, &snapshot)?;
                let (element, index) =
                    element::remove(doc, &id).ok_or(CommandError::Missing { id })?;
                Ok(Self::Put {
                    element,
                    index: Some(index),
                })
            }
            Self::PutAsset { path, data } => {
                if path.is_empty() || path.starts_with('/') || path.contains("..") {
                    return Err(CommandError::InvalidValue {
                        field: "path",
                        reason: "путь файла должен быть относительным",
                    });
                }
                Ok(match doc.assets.insert(path.clone(), data) {
                    Some(previous) => Self::PutAsset {
                        path,
                        data: previous,
                    },
                    None => Self::RemoveAsset { path },
                })
            }
            Self::RemoveAsset { path } => {
                if doc.underlays.iter().any(|u| u.asset == path) {
                    return Err(CommandError::Referenced { id: path });
                }
                let data = doc
                    .assets
                    .remove(&path)
                    .ok_or(CommandError::Missing { id: path.clone() })?;
                Ok(Self::PutAsset { path, data })
            }
            Self::Batch { commands } => {
                let mut inverses = Vec::with_capacity(commands.len());
                for command in commands {
                    match command.apply(doc) {
                        Ok(inverse) => inverses.push(inverse),
                        Err(err) => {
                            // Откат уже применённых команд: обратные применяются всегда.
                            for inverse in inverses.into_iter().rev() {
                                let _ = inverse.apply(doc);
                            }
                            return Err(err);
                        }
                    }
                }
                inverses.reverse();
                Ok(Self::Batch { commands: inverses })
            }
        }
    }

    /// Пачка команд; одна команда — как есть.
    #[must_use]
    pub fn batch(mut commands: Vec<Self>) -> Self {
        if commands.len() == 1
            && let Some(single) = commands.pop()
        {
            return single;
        }
        Self::Batch { commands }
    }

    /// Двоичный файл для команды [`Command::PutAsset`].
    #[must_use]
    pub fn asset(path: impl Into<String>, data: Vec<u8>) -> Self {
        Self::PutAsset {
            path: path.into(),
            data: Arc::new(data),
        }
    }
}

mod base64_bytes {
    use std::sync::Arc;

    use base64::{Engine, engine::general_purpose::STANDARD};
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    #[allow(clippy::ptr_arg)] // сигнатура задаётся serde(with)
    pub(super) fn serialize<S: Serializer>(data: &Arc<Vec<u8>>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&STANDARD.encode(data.as_slice()))
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Arc<Vec<u8>>, D::Error> {
        let text = String::deserialize(d)?;
        STANDARD
            .decode(text)
            .map(Arc::new)
            .map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Anchor, Dimension, Justification, LayerKind, Node, Wall};

    fn node(id: &str, x: f64, y: f64) -> Command {
        Command::Put {
            element: Element::Node(Node {
                variant: None,
                id: id.into(),
                x,
                y,
            }),
            index: None,
        }
    }

    fn wall(id: &str, a: &str, b: &str) -> Command {
        Command::Put {
            element: Element::Wall(Wall {
                material: None,
                variant: None,
                id: id.into(),
                layer: LayerKind::Walls.id().into(),
                a: a.into(),
                b: b.into(),
                thickness: 100.0,
                justification: Justification::Center,
                height: None,
            }),
            index: None,
        }
    }

    #[test]
    fn batch_and_inverse_restore_document() {
        let mut doc = Document::new("Кв.");
        let before = doc.clone();
        let inverse = Command::Batch {
            commands: vec![
                node("n1", 0.0, 0.0),
                node("n2", 1000.0, 0.0),
                wall("w1", "n1", "n2"),
            ],
        }
        .apply(&mut doc)
        .unwrap();
        assert_eq!(doc.walls.len(), 1);
        inverse.apply(&mut doc).unwrap();
        assert_eq!(doc, before);
    }

    #[test]
    fn failed_batch_rolls_back() {
        let mut doc = Document::new("Кв.");
        let before = doc.clone();
        let err = Command::Batch {
            commands: vec![node("n1", 0.0, 0.0), wall("w1", "n1", "nope")],
        }
        .apply(&mut doc)
        .unwrap_err();
        assert_eq!(err, CommandError::Missing { id: "nope".into() });
        assert_eq!(doc, before);
    }

    #[test]
    fn remove_keeps_position_on_undo() {
        let mut doc = Document::new("Кв.");
        for (i, x) in [0.0, 1.0, 2.0].into_iter().enumerate() {
            node(&format!("n{i}"), x, 0.0).apply(&mut doc).unwrap();
        }
        let before = doc.clone();
        let inverse = Command::Remove { id: "n1".into() }.apply(&mut doc).unwrap();
        inverse.apply(&mut doc).unwrap();
        assert_eq!(doc, before);
    }

    #[test]
    fn referenced_elements_cannot_be_removed() {
        let mut doc = Document::new("Кв.");
        Command::Batch {
            commands: vec![
                node("n1", 0.0, 0.0),
                node("n2", 1000.0, 0.0),
                wall("w1", "n1", "n2"),
            ],
        }
        .apply(&mut doc)
        .unwrap();
        let err = Command::Remove { id: "n1".into() }
            .apply(&mut doc)
            .unwrap_err();
        assert!(matches!(err, CommandError::Referenced { .. }));
        let err = Command::Remove {
            id: LayerKind::Walls.id().into(),
        }
        .apply(&mut doc)
        .unwrap_err();
        assert!(matches!(err, CommandError::InvalidValue { .. }));
    }

    #[test]
    fn put_replaces_and_inverse_restores() {
        let mut doc = Document::new("Кв.");
        node("n1", 0.0, 0.0).apply(&mut doc).unwrap();
        let inverse = node("n1", 5.0, 5.0).apply(&mut doc).unwrap();
        assert!((doc.nodes[0].x - 5.0).abs() < f64::EPSILON);
        inverse.apply(&mut doc).unwrap();
        assert!(doc.nodes[0].x.abs() < f64::EPSILON);
    }

    #[test]
    fn ids_of_other_kind_conflict() {
        let mut doc = Document::new("Кв.");
        node("x", 0.0, 0.0).apply(&mut doc).unwrap();
        let err = Command::Put {
            element: Element::Dimension(Dimension {
                variant: None,
                id: "x".into(),
                layer: LayerKind::Dimensions.id().into(),
                start: Anchor::Point { x: 0.0, y: 0.0 },
                end: Anchor::Node { node: "x".into() },
                offset: 0.0,
            }),
            index: None,
        }
        .apply(&mut doc)
        .unwrap_err();
        assert_eq!(err, CommandError::Conflict { id: "x".into() });
    }

    #[test]
    fn assets_roundtrip_through_json() {
        let command = Command::asset("assets/a.png", vec![1, 2, 3, 250]);
        let json = serde_json::to_string(&command).unwrap();
        assert!(json.contains("\"AQID+g==\""));
        let back: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(back, command);

        let mut doc = Document::new("Кв.");
        let inverse = command.apply(&mut doc).unwrap();
        assert_eq!(doc.assets.len(), 1);
        inverse.apply(&mut doc).unwrap();
        assert!(doc.assets.is_empty());
    }

    #[test]
    fn element_json_shape() {
        let json = serde_json::to_value(node("n1", 1.0, 2.0)).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"type": "put", "element": {"type": "node", "id": "n1", "x": 1.0, "y": 2.0}})
        );
    }
}
