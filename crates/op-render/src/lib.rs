//! Рендерер OpenPlanner на wgpu.
//!
//! Один рендерер рисует 2D-план, облако точек скана и 3D-вид. На вебе и в
//! Tauri он работает в WASM внутри Web Worker и рисует в `OffscreenCanvas`
//! (бэкенд WebGPU, при его отсутствии — WebGL2).
//!
//! Сцена ([`scene::Scene`]) строится на CPU: заливки, линии постоянной
//! экранной толщины, SDF-текст ([`text`]) и изображения-подложки. 3D-сцена
//! ([`three::Scene3D`]) — сетки с нормалями и цветом, солнце с тенями.

mod camera;
mod gpu;
mod gpu3d;
#[cfg(not(target_arch = "wasm32"))]
mod offscreen;
mod renderer;
pub mod scene;
pub mod text;
pub mod three;

pub use camera::Camera2D;
#[cfg(not(target_arch = "wasm32"))]
pub use offscreen::{Image, SceneImage, render_3d_image, render_plan_image};
pub use renderer::{RenderError, Renderer, SceneSlot};
