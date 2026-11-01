//! Документ проекта OpenPlanner.
//!
//! Ядро — источник истины: все изменения документа проходят через [`Command`],
//! каждая команда обратима, а [`History`] хранит стек отмены и повтора.
//! Единицы длины в модели — миллиметры (`f64`).

mod command;
mod document;
mod history;
mod schema;

pub use command::{Command, CommandError};
pub use document::{Document, ProjectMeta};
pub use history::{History, HistoryOp};
pub use schema::model_schema;
