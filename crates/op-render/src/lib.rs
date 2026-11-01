//! Рендерер OpenPlanner на wgpu.
//!
//! Один рендерер рисует 2D-план, облако точек скана и 3D-вид. На вебе и в
//! Tauri он работает в WASM внутри Web Worker и рисует в `OffscreenCanvas`
//! (бэкенд WebGPU, при его отсутствии — WebGL2).
//!
//! Стадия 0: камера плана и фоновая сетка.

mod camera;
mod grid;
mod renderer;

pub use camera::Camera2D;
pub use renderer::{RenderError, Renderer};
