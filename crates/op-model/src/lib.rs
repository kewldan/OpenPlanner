//! Документ проекта OpenPlanner.
//!
//! Ядро — источник истины: все изменения документа проходят через [`Command`],
//! каждая команда обратима, а [`History`] хранит стек отмены и повтора.
//! Единицы длины в модели — миллиметры (`f64`).

mod command;
mod document;
mod element;
mod history;
mod schema;
pub mod view;

pub use command::{Command, CommandError};
pub use document::{
    Anchor, AssetData, Circuit, CircuitKind, Device, DeviceKind, Dimension, Document,
    DrawingSettings, Furniture, Justification, Layer, LayerKind, Leader, LightScene, Node, Opening,
    OpeningKind, PaperFormat, ProjectMeta, Room, SavedView, Side, Site, Underlay, Variant, Wall,
    WallMaterial,
};
pub use element::Element;
pub use history::{History, HistoryOp};
pub use schema::model_schema;
