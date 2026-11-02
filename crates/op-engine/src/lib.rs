//! Движок OpenPlanner для Web Worker: ядро модели, инструменты холста и рендерер.
//!
//! Нативная часть ([`EngineState`]) не зависит от GPU и тестируется обычным
//! `cargo test`; WASM-обёртка связывает её с рендерером и `OffscreenCanvas`.

pub mod derived;
pub mod draw;
pub mod edit;
pub mod geometry;
pub mod hit;
pub mod ids;
mod overlay;
pub mod protocol;
pub mod scan_import;
pub mod snap;
mod state;
#[cfg(test)]
mod tests;
pub mod tools;
pub mod variants;
#[cfg(target_arch = "wasm32")]
mod wasm;

pub use state::{DecodedImage, EngineState, ImageUpload, UNTITLED, decode_image};
