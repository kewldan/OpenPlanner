//! Камера 3D-вида: орбита вокруг точки и прогулка на высоте глаз.
//!
//! Координаты — 3D-сцены (X вправо по плану, Y вверх по плану, Z вверх, мм);
//! наружу (UI, сохранённые ракурсы) точки отдаются в координатах плана (Y вниз).

use glam::{DVec2, DVec3};
use op_model::SavedView;
use op_render::three::Camera3D;

use crate::protocol::{CameraAction, CameraInfo, CameraMode};

/// Высота глаз при прогулке, мм.
pub const EYE_HEIGHT: f64 = 1600.0;
/// Вертикальный угол обзора, градусы.
const FOV_DEG: f64 = 55.0;
/// Наклон орбиты, градусы: от почти горизонтального до взгляда сверху.
const PITCH_ORBIT: (f64, f64) = (5.0, 89.0);
/// Наклон взгляда при прогулке, градусы.
const PITCH_WALK: (f64, f64) = (-80.0, 80.0);
/// Расстояние орбиты, мм.
const DISTANCE: (f64, f64) = (800.0, 300_000.0);
/// Высота глаз при прогулке, мм.
const WALK_HEIGHT: (f64, f64) = (300.0, 3000.0);

/// Камера 3D-вида.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera3d {
    /// Режим.
    pub mode: CameraMode,
    /// Точка орбиты.
    pub target: DVec3,
    /// Азимут глаза орбиты от оси X против часовой стрелки, радианы.
    pub yaw: f64,
    /// Наклон орбиты, радианы.
    pub pitch: f64,
    /// Расстояние орбиты, мм.
    pub distance: f64,
    /// Глаз при прогулке.
    pub walker: DVec3,
    /// Направление взгляда при прогулке от оси X против часовой стрелки, радианы.
    pub heading: f64,
    /// Наклон взгляда при прогулке, радианы.
    pub look: f64,
}

impl Default for Camera3d {
    fn default() -> Self {
        Self {
            mode: CameraMode::Orbit,
            target: DVec3::new(4000.0, -3000.0, 1000.0),
            yaw: -std::f64::consts::FRAC_PI_2,
            pitch: 50f64.to_radians(),
            distance: 14_000.0,
            walker: DVec3::new(4000.0, -3000.0, EYE_HEIGHT),
            heading: std::f64::consts::FRAC_PI_2,
            look: 0.0,
        }
    }
}

/// Точка 3D-сцены в координатах плана: x, y (вниз), высота.
fn to_plan(p: DVec3) -> [f64; 3] {
    [p.x, -p.y, p.z]
}

fn from_plan(p: [f64; 3]) -> DVec3 {
    DVec3::new(p[0], -p[1], p[2])
}

fn dir(yaw: f64, pitch: f64) -> DVec3 {
    DVec3::new(
        pitch.cos() * yaw.cos(),
        pitch.cos() * yaw.sin(),
        pitch.sin(),
    )
}

fn clamp_deg(v: f64, (lo, hi): (f64, f64)) -> f64 {
    v.clamp(lo.to_radians(), hi.to_radians())
}

impl Camera3d {
    /// Глаз и точка взгляда.
    #[must_use]
    pub fn eye_target(&self) -> (DVec3, DVec3) {
        match self.mode {
            CameraMode::Orbit => (
                self.target + dir(self.yaw, self.pitch) * self.distance,
                self.target,
            ),
            CameraMode::Walk => (
                self.walker,
                self.walker + dir(self.heading, self.look) * 1000.0,
            ),
        }
    }

    /// Камера рендерера.
    #[must_use]
    pub fn render_camera(&self) -> Camera3D {
        let (eye, target) = self.eye_target();
        Camera3D {
            eye,
            target,
            fov_y: FOV_DEG.to_radians(),
        }
    }

    /// Сведения для UI.
    #[must_use]
    pub fn info(&self) -> CameraInfo {
        let (eye, target) = self.eye_target();
        let d = target - eye;
        // От верха листа (+Y) по часовой стрелке.
        let heading = d.x.atan2(d.y).to_degrees().rem_euclid(360.0);
        CameraInfo {
            mode: self.mode,
            eye: to_plan(eye),
            target: to_plan(target),
            heading,
        }
    }

    /// Горизонтальные «вперёд» и «вправо» для текущего взгляда.
    fn ground_axes(&self) -> (DVec3, DVec3) {
        let (eye, target) = self.eye_target();
        let f = (target - eye).truncate().normalize_or(DVec2::Y).extend(0.0);
        (f, DVec3::new(f.y, -f.x, 0.0))
    }

    /// Меняет режим. Прогулка начинается там, куда смотрела орбита; орбита
    /// после прогулки возвращается на прежнее место.
    pub fn set_mode(&mut self, mode: CameraMode) {
        if mode == self.mode {
            return;
        }
        if mode == CameraMode::Walk {
            self.walker = DVec3::new(self.target.x, self.target.y, EYE_HEIGHT);
            self.heading = self.yaw + std::f64::consts::PI;
            self.look = 0.0;
        }
        self.mode = mode;
    }

    /// Выполняет действие.
    pub fn apply(&mut self, action: CameraAction) {
        match (action, self.mode) {
            (CameraAction::Rotate { dx, dy }, CameraMode::Orbit) => {
                self.yaw -= dx * 0.006;
                self.pitch = clamp_deg(self.pitch + dy * 0.006, PITCH_ORBIT);
            }
            (CameraAction::Rotate { dx, dy }, CameraMode::Walk) => {
                self.heading -= dx * 0.004;
                self.look = clamp_deg(self.look - dy * 0.004, PITCH_WALK);
            }
            (CameraAction::Pan { dx, dy }, CameraMode::Orbit) => {
                let (eye, target) = self.eye_target();
                let forward = (target - eye).normalize_or(DVec3::Y);
                let right = forward.cross(DVec3::Z).normalize_or(DVec3::X);
                let up = right.cross(forward);
                let k = self.distance * 0.0012;
                self.target += (-right * dx + up * dy) * k;
            }
            (CameraAction::Pan { dx, dy }, CameraMode::Walk) => {
                let (_, right) = self.ground_axes();
                self.walker += right * (-dx * 8.0);
                self.walker.z = (self.walker.z + dy * 8.0).clamp(WALK_HEIGHT.0, WALK_HEIGHT.1);
            }
            (CameraAction::Zoom { factor }, mode) => {
                if !factor.is_finite() || factor <= 0.0 {
                    return;
                }
                match mode {
                    CameraMode::Orbit => {
                        self.distance = (self.distance / factor).clamp(DISTANCE.0, DISTANCE.1);
                    }
                    CameraMode::Walk => {
                        let (forward, _) = self.ground_axes();
                        self.walker += forward * factor.ln() * 3000.0;
                    }
                }
            }
            (CameraAction::Move { forward, right, up }, mode) => {
                let (f, r) = self.ground_axes();
                let delta = f * forward + r * right;
                match mode {
                    CameraMode::Orbit => self.target += delta + DVec3::Z * up,
                    CameraMode::Walk => {
                        self.walker += delta;
                        self.walker.z = (self.walker.z + up).clamp(WALK_HEIGHT.0, WALK_HEIGHT.1);
                    }
                }
            }
        }
    }

    /// Орбита на всю модель с габаритом `bounds`: взгляд с юга листа сверху под 50°.
    pub fn fit(&mut self, bounds: (DVec3, DVec3)) {
        let (lo, hi) = bounds;
        let center = lo.midpoint(hi);
        let radius = ((hi - lo).length() / 2.0).max(1000.0);
        self.mode = CameraMode::Orbit;
        self.target = DVec3::new(center.x, center.y, (hi.z * 0.3).max(0.0));
        self.yaw = -std::f64::consts::FRAC_PI_2;
        self.pitch = 50f64.to_radians();
        self.distance =
            (radius / (FOV_DEG.to_radians() / 2.0).sin() * 1.05).clamp(DISTANCE.0, DISTANCE.1);
        self.walker = DVec3::new(center.x, center.y, EYE_HEIGHT);
        self.heading = std::f64::consts::FRAC_PI_2;
        self.look = 0.0;
    }

    /// Сохранённый ракурс.
    #[must_use]
    pub fn saved(&self, id: String, name: String) -> SavedView {
        let info = self.info();
        SavedView {
            id,
            name,
            walk: self.mode == CameraMode::Walk,
            eye: info.eye,
            target: info.target,
        }
    }

    /// Переходит к сохранённому ракурсу.
    pub fn restore(&mut self, view: &SavedView) {
        let (eye, target) = (from_plan(view.eye), from_plan(view.target));
        let d = eye - target;
        if view.walk {
            self.mode = CameraMode::Walk;
            self.walker = eye;
            let look = target - eye;
            self.heading = look.y.atan2(look.x);
            self.look = look.z.atan2(look.truncate().length());
        } else {
            self.mode = CameraMode::Orbit;
            self.target = target;
            self.distance = d.length().clamp(DISTANCE.0, DISTANCE.1);
            self.yaw = d.y.atan2(d.x);
            self.pitch = d.z.atan2(d.truncate().length());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbit_keeps_distance_and_clamps_pitch() {
        let mut c = Camera3d::default();
        let (eye, target) = c.eye_target();
        c.apply(CameraAction::Rotate { dx: 120.0, dy: 0.0 });
        let (eye2, _) = c.eye_target();
        assert!((eye2.distance(target) - eye.distance(target)).abs() < 1e-6);
        assert!(eye2.distance(eye) > 100.0);
        c.apply(CameraAction::Rotate { dx: 0.0, dy: 1e6 });
        assert!((c.pitch - 89f64.to_radians()).abs() < 1e-12);
    }

    #[test]
    fn default_view_looks_from_the_bottom_of_the_plan() {
        let info = Camera3d::default().info();
        // Глаз ниже точки на листе (больше Y плана) и выше пола; взгляд — вверх листа.
        assert!(info.eye[1] > info.target[1] && info.eye[2] > info.target[2]);
        assert!(info.heading.abs() < 1e-6 || (info.heading - 360.0).abs() < 1e-6);
    }

    #[test]
    fn walk_moves_forward_along_the_view() {
        let mut c = Camera3d::default();
        c.set_mode(CameraMode::Walk);
        let before = c.info().eye;
        // Взгляд вверх листа: шаг вперёд уменьшает Y плана.
        c.apply(CameraAction::Move {
            forward: 500.0,
            right: 0.0,
            up: 0.0,
        });
        let after = c.info().eye;
        assert!(
            (before[1] - after[1] - 500.0).abs() < 1e-6,
            "{before:?} {after:?}"
        );
        assert!((after[2] - EYE_HEIGHT).abs() < 1e-9);
        // Вправо — по +X плана.
        c.apply(CameraAction::Move {
            forward: 0.0,
            right: 300.0,
            up: 0.0,
        });
        assert!((c.info().eye[0] - after[0] - 300.0).abs() < 1e-6);
    }

    #[test]
    fn saved_views_round_trip() {
        let mut c = Camera3d::default();
        c.apply(CameraAction::Rotate {
            dx: 50.0,
            dy: -30.0,
        });
        c.apply(CameraAction::Zoom { factor: 1.7 });
        let view = c.saved("v".into(), "Кухня".into());
        let mut d = Camera3d::default();
        d.set_mode(CameraMode::Walk);
        d.restore(&view);
        for (a, b) in c.info().eye.iter().zip(d.info().eye) {
            assert!((a - b).abs() < 1e-6);
        }
        let mut c = Camera3d::default();
        c.set_mode(CameraMode::Walk);
        c.apply(CameraAction::Rotate { dx: 80.0, dy: 40.0 });
        let view = c.saved("w".into(), "Прогулка".into());
        assert!(view.walk);
        let mut d = Camera3d::default();
        d.restore(&view);
        assert_eq!(d.mode, CameraMode::Walk);
        for (a, b) in c.info().target.iter().zip(d.info().target) {
            assert!((a - b).abs() < 1e-6);
        }
    }
}
