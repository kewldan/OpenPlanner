//! WASM-движок OpenPlanner.
//!
//! Работает в Web Worker: владеет документом ([`op_model::History`]) и
//! рендерером ([`op_render::Renderer`]), принимает сообщения UI
//! ([`protocol::ToEngine`]) и отвечает событиями ([`protocol::FromEngine`]).

pub mod protocol;

mod state;

#[cfg(target_arch = "wasm32")]
mod wasm;

pub use state::EngineState;
