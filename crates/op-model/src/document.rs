use serde::{Deserialize, Serialize};

/// Сведения о проекте, которые попадают в основную надпись чертежей.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct ProjectMeta {
    /// Название проекта, например «Комендантский пр.».
    pub title: String,
    /// Адрес объекта.
    pub address: String,
}

/// Документ проекта.
///
/// На стадии 0 содержит только сведения о проекте. Обмер, варианты, слои
/// и объекты добавляются на следующих стадиях.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "camelCase")]
pub struct Document {
    /// Сведения о проекте.
    pub meta: ProjectMeta,
    /// Высота потолка по умолчанию, мм.
    pub default_ceiling_height: f64,
}

impl Document {
    /// Высота потолка по умолчанию для нового проекта, мм.
    pub const DEFAULT_CEILING_HEIGHT: f64 = 2700.0;

    /// Создаёт пустой документ.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            meta: ProjectMeta {
                title: title.into(),
                address: String::new(),
            },
            default_ceiling_height: Self::DEFAULT_CEILING_HEIGHT,
        }
    }
}
