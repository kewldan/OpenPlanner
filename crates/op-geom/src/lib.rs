//! Геометрия OpenPlanner.
//!
//! Координаты плана — миллиметры, ось X вправо, ось Y вниз (как на экране и листе).
//!
//! - [`polygon`] — площади, периметры, принадлежность точки, внутренняя точка контура;
//! - [`segment`] — проекции и расстояния до отрезков, пересечения прямых;
//! - [`walls`] — контуры стен со стыками в узлах;
//! - [`shapes`] — объединение стен, вырезы проёмов, контуры помещений;
//! - [`tiling`] — раскладка плитки и досок с подрезками и подбором старта.

pub mod polygon;
pub mod segment;
pub mod shapes;
pub mod tiling;
pub mod walls;

pub use glam::DVec2;
pub use polygon::{area, centroid, contains, hull, interior_point, perimeter, signed_area};

/// Перевод мм² в м².
#[must_use]
pub fn mm2_to_m2(value: f64) -> f64 {
    value / 1_000_000.0
}

/// Левая нормаль направления (единичного) в экранных координатах (Y вниз):
/// для направления вправо — вверх по экрану.
#[must_use]
pub fn left_normal(dir: DVec2) -> DVec2 {
    DVec2::new(dir.y, -dir.x)
}
