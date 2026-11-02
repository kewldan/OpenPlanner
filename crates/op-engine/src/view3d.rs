//! 3D-вид движка: настройки, камера, сохранённые ракурсы, сборка сцены.

use op_model::Command;
use op_render::three::{Camera3D, Scene3D};

use crate::{
    EngineState, model3d,
    protocol::{CameraMode, FromEngine, Options3d, SunInfo, View},
};

impl EngineState {
    /// Холст показывает 3D.
    #[must_use]
    pub fn three_d(&self) -> bool {
        self.options_3d.enabled
    }

    /// Камера 3D-вида для рендерера.
    #[must_use]
    pub fn camera_3d(&self) -> Camera3D {
        self.camera_3d.render_camera()
    }

    /// 3D-сцена текущего вида (обмер или вариант).
    #[must_use]
    pub fn scene_3d(&self) -> Scene3D {
        model3d::build(
            &self.projected,
            &self.derived,
            &self.options_3d,
            &self.models,
        )
    }

    /// 3D-сцена, если она изменилась с прошлого вызова и 3D включён.
    pub fn take_scene_3d(&mut self) -> Option<Scene3D> {
        if !self.options_3d.enabled || !self.scene_3d_dirty {
            return None;
        }
        self.scene_3d_dirty = false;
        Some(self.scene_3d())
    }

    /// Наводит орбиту на всю модель.
    pub(crate) fn fit_camera_3d(&mut self) {
        if let Some(bounds) = self.scene_3d().opaque.bounds() {
            self.camera_3d.fit(bounds);
        }
        self.camera_3d_fitted = true;
    }

    pub(crate) fn view_3d_event(&self) -> FromEngine {
        FromEngine::View3dChanged {
            options: self.options_3d,
            camera: self.camera_3d.info(),
            sun: model3d::sun(self.history.document(), &self.options_3d).map(|(p, _)| SunInfo {
                azimuth: p.azimuth,
                altitude: p.altitude,
            }),
        }
    }

    pub(crate) fn set_3d(&mut self, mut options: Options3d) -> Vec<FromEngine> {
        if options.cut.is_some_and(|c| !c.is_finite() || c <= 0.0)
            || options
                .sun
                .is_some_and(|s| !(1..=366).contains(&s.day) || !(0.0..=24.0).contains(&s.hour))
        {
            return vec![FromEngine::Error {
                message: "неверные настройки 3D-вида".into(),
            }];
        }
        // Сравнение — только на плане.
        if matches!(self.view, View::Compare { .. }) {
            options.enabled = false;
        }
        let previous = self.options_3d;
        self.options_3d = options;
        let to_walk = previous.mode != CameraMode::Walk && options.mode == CameraMode::Walk;
        self.camera_3d.set_mode(options.mode);
        // Прогулка начинается в помещении, ближайшем к точке обзора, а не в стене.
        if to_walk {
            let target = self.camera_3d.target;
            let here = glam::DVec2::new(target.x, -target.y);
            let room = self
                .projected
                .rooms
                .iter()
                .filter(|r| self.derived.room_regions.contains_key(&r.id))
                .map(|r| glam::DVec2::new(r.x, r.y))
                .min_by(|a, b| a.distance(here).total_cmp(&b.distance(here)));
            if let Some(p) = room {
                self.camera_3d.walker = glam::DVec3::new(p.x, -p.y, crate::camera3d::EYE_HEIGHT);
            }
        }
        if options.enabled && !self.camera_3d_fitted {
            self.fit_camera_3d();
        }
        let scene = |o: &Options3d| {
            (
                o.ceiling,
                o.white,
                o.cut.map(f64::to_bits),
                o.sun.map(|s| (s.day, s.hour.to_bits())),
            )
        };
        if options.enabled != previous.enabled || scene(&options) != scene(&previous) {
            self.scene_3d_dirty = true;
        }
        // Холст 2D перерисуется при возврате.
        self.document_dirty = true;
        self.overlay_dirty = true;
        vec![self.view_3d_event()]
    }

    pub(crate) fn save_view_3d(&mut self, name: String) -> Vec<FromEngine> {
        let id = self.ids.generate();
        let mut views = self.history.document().views.clone();
        views.push(self.camera_3d.saved(id, name));
        self.execute_all(vec![Command::SetViews { views }])
    }

    pub(crate) fn apply_view_3d(&mut self, id: &str) -> Vec<FromEngine> {
        let Some(view) = self
            .history
            .document()
            .views
            .iter()
            .find(|v| v.id == id)
            .cloned()
        else {
            return vec![FromEngine::Error {
                message: "Такого ракурса нет".into(),
            }];
        };
        self.camera_3d.restore(&view);
        self.options_3d.mode = self.camera_3d.mode;
        vec![self.view_3d_event()]
    }
}
