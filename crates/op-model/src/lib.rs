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
    Anchor, AssetData, Circuit, CircuitKind, Device, DeviceGroup, DeviceKind, Dimension, Document,
    DrawingSettings, Finish, Furniture, Justification, Layer, LayerKind, Leader, LightScene,
    Material, MaterialKind, Measure, Node, Opening, OpeningKind, PaperFormat, Pipe, PipeSystem,
    PipeVertex, PlumbingKind, PlumbingPoint, PriceItem, PriceKind, PricePoint, ProjectMeta,
    QuantityOverride, Room, SavedView, Side, Site, Surface, TileLayout, TilePattern, Underlay,
    Variant, Wall, WallMaterial,
};
pub use element::Element;
pub use history::{History, HistoryOp};
pub use schema::model_schema;
