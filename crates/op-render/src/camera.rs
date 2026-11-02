use glam::DVec2;

/// Камера 2D-плана.
///
/// Мировые координаты — миллиметры плана (Y вниз), экранные — CSS-пиксели
/// от левого верхнего угла холста.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera2D {
    /// Точка плана в центре холста, мм.
    pub center: DVec2,
    /// Масштаб: CSS-пикселей на миллиметр.
    pub px_per_mm: f64,
}

impl Camera2D {
    /// Минимальный масштаб: 1 px на 200 мм.
    pub const MIN_PX_PER_MM: f64 = 0.005;
    /// Максимальный масштаб: 20 px на 1 мм.
    pub const MAX_PX_PER_MM: f64 = 20.0;

    /// Камера по умолчанию: квартира ~10 м помещается в холст ~1000 px.
    #[must_use]
    pub fn new() -> Self {
        Self {
            center: DVec2::new(4000.0, 3000.0),
            px_per_mm: 0.1,
        }
    }

    /// Переводит экранную точку в координаты плана.
    #[must_use]
    pub fn screen_to_world(&self, screen: DVec2, viewport: DVec2) -> DVec2 {
        self.center + (screen - viewport / 2.0) / self.px_per_mm
    }

    /// Переводит точку плана в экранные координаты.
    #[must_use]
    pub fn world_to_screen(&self, world: DVec2, viewport: DVec2) -> DVec2 {
        (world - self.center) * self.px_per_mm + viewport / 2.0
    }

    /// Сдвигает план на `delta` экранных пикселей.
    pub fn pan(&mut self, delta: DVec2) {
        self.center -= delta / self.px_per_mm;
    }

    /// Масштабирует относительно экранной точки `anchor`, которая остаётся на месте.
    pub fn zoom_at(&mut self, anchor: DVec2, viewport: DVec2, factor: f64) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let before = self.screen_to_world(anchor, viewport);
        self.px_per_mm = (self.px_per_mm * factor).clamp(Self::MIN_PX_PER_MM, Self::MAX_PX_PER_MM);
        let after = self.screen_to_world(anchor, viewport);
        self.center += before - after;
    }
}

impl Default for Camera2D {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEWPORT: DVec2 = DVec2::new(1200.0, 800.0);

    #[test]
    fn screen_world_roundtrip() {
        let camera = Camera2D {
            center: DVec2::new(1500.0, -300.0),
            px_per_mm: 0.37,
        };
        let p = DVec2::new(123.0, 456.0);
        let back = camera.world_to_screen(camera.screen_to_world(p, VIEWPORT), VIEWPORT);
        assert!(back.distance(p) < 1e-9);
    }

    #[test]
    fn zoom_keeps_anchor_in_place() {
        let mut camera = Camera2D::new();
        let anchor = DVec2::new(300.0, 200.0);
        let world = camera.screen_to_world(anchor, VIEWPORT);
        camera.zoom_at(anchor, VIEWPORT, 2.5);
        assert!(camera.world_to_screen(world, VIEWPORT).distance(anchor) < 1e-9);
    }

    #[test]
    fn zoom_is_clamped() {
        let mut camera = Camera2D::new();
        camera.zoom_at(DVec2::ZERO, VIEWPORT, 1e9);
        assert!((camera.px_per_mm - Camera2D::MAX_PX_PER_MM).abs() < f64::EPSILON);
    }

    #[test]
    fn pan_moves_content_with_cursor() {
        let mut camera = Camera2D::new();
        let world = DVec2::new(1000.0, 1000.0);
        let before = camera.world_to_screen(world, VIEWPORT);
        camera.pan(DVec2::new(40.0, -10.0));
        let after = camera.world_to_screen(world, VIEWPORT);
        assert!((after - before - DVec2::new(40.0, -10.0)).length() < 1e-9);
    }
}
