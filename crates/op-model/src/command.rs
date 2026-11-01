use serde::{Deserialize, Serialize};

use crate::Document;

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
}

/// Атомарное обратимое изменение документа.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
        }
    }
}
