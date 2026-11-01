use glam::DVec2;
use op_model::{Document, History};
use op_render::Camera2D;

use crate::protocol::{EngineEvents, FromEngine, ToEngine};

/// CSS-пикселей в миллиметре экрана (96 dpi).
const CSS_PX_PER_MM: f64 = 96.0 / 25.4;
/// Знаменатель масштаба, который считается 100 %.
const REFERENCE_SCALE: f64 = 50.0;

/// Состояние движка без привязки к GPU: документ, история, камера.
///
/// Вынесено отдельно от WASM-обёртки, чтобы протокол тестировался нативно.
#[derive(Debug)]
pub struct EngineState {
    history: History,
    camera: Camera2D,
    viewport: DVec2,
    device_pixel_ratio: f64,
}

impl EngineState {
    /// Новое состояние с пустым документом.
    #[must_use]
    pub fn new(viewport: DVec2, device_pixel_ratio: f64) -> Self {
        Self {
            history: History::new(Document::new("Новый проект")),
            camera: Camera2D::new(),
            viewport,
            device_pixel_ratio,
        }
    }

    /// Камера плана.
    #[must_use]
    pub fn camera(&self) -> &Camera2D {
        &self.camera
    }

    /// Размер холста в CSS-пикселях.
    #[must_use]
    pub fn viewport(&self) -> DVec2 {
        self.viewport
    }

    /// Отношение физических пикселей к CSS-пикселям.
    #[must_use]
    pub fn device_pixel_ratio(&self) -> f64 {
        self.device_pixel_ratio
    }

    /// События, описывающие текущее состояние целиком (после запуска).
    #[must_use]
    pub fn snapshot(&self) -> EngineEvents {
        EngineEvents {
            events: vec![
                self.document_event(),
                self.history_event(),
                self.view_event(),
            ],
        }
    }

    /// Обрабатывает сообщение UI.
    pub fn handle(&mut self, message: ToEngine) -> EngineEvents {
        let events = match message {
            ToEngine::Resize {
                width,
                height,
                device_pixel_ratio,
            } => {
                self.viewport = DVec2::new(width.max(1.0), height.max(1.0));
                self.device_pixel_ratio = device_pixel_ratio.max(0.1);
                vec![]
            }
            ToEngine::Pan { dx, dy } => {
                self.camera.pan(DVec2::new(dx, dy));
                vec![]
            }
            ToEngine::Zoom { x, y, factor } => {
                self.camera.zoom_at(DVec2::new(x, y), self.viewport, factor);
                vec![self.view_event()]
            }
            ToEngine::ZoomToFit => {
                self.camera = Camera2D::new();
                vec![self.view_event()]
            }
            ToEngine::Command { command } => match self.history.execute(command) {
                Ok(()) => vec![self.document_event(), self.history_event()],
                Err(err) => vec![FromEngine::Error {
                    message: err.to_string(),
                }],
            },
            ToEngine::Undo => self.step(History::undo),
            ToEngine::Redo => self.step(History::redo),
        };
        EngineEvents { events }
    }

    fn step(
        &mut self,
        op: fn(&mut History) -> Result<bool, op_model::CommandError>,
    ) -> Vec<FromEngine> {
        match op(&mut self.history) {
            Ok(true) => vec![self.document_event(), self.history_event()],
            Ok(false) => vec![],
            Err(err) => vec![FromEngine::Error {
                message: err.to_string(),
            }],
        }
    }

    fn document_event(&self) -> FromEngine {
        FromEngine::DocumentChanged {
            document: self.history.document().clone(),
        }
    }

    fn history_event(&self) -> FromEngine {
        FromEngine::HistoryChanged {
            can_undo: self.history.can_undo(),
            can_redo: self.history.can_redo(),
        }
    }

    fn view_event(&self) -> FromEngine {
        let scale_denominator = CSS_PX_PER_MM / self.camera.px_per_mm;
        FromEngine::ViewChanged {
            scale_denominator,
            zoom_percent: REFERENCE_SCALE / scale_denominator * 100.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use op_model::Command;

    use super::*;

    fn state() -> EngineState {
        EngineState::new(DVec2::new(1000.0, 800.0), 2.0)
    }

    #[test]
    fn command_then_undo() {
        let mut s = state();
        let events = s.handle(ToEngine::Command {
            command: Command::SetTitle {
                title: "Комендантский пр.".into(),
            },
        });
        assert!(events.events.contains(&FromEngine::HistoryChanged {
            can_undo: true,
            can_redo: false
        }));

        let events = s.handle(ToEngine::Undo);
        let FromEngine::DocumentChanged { document } = &events.events[0] else {
            panic!("ожидалось DocumentChanged");
        };
        assert_eq!(document.meta.title, "Новый проект");
    }

    #[test]
    fn invalid_command_reports_error() {
        let mut s = state();
        let events = s.handle(ToEngine::Command {
            command: Command::SetDefaultCeilingHeight { height: 0.0 },
        });
        assert!(matches!(
            events.events.as_slice(),
            [FromEngine::Error { .. }]
        ));
    }

    #[test]
    fn zoom_reports_view() {
        let mut s = state();
        let events = s.handle(ToEngine::Zoom {
            x: 500.0,
            y: 400.0,
            factor: 2.0,
        });
        let [FromEngine::ViewChanged { zoom_percent, .. }] = events.events.as_slice() else {
            panic!("ожидалось ViewChanged");
        };
        let before = REFERENCE_SCALE / (CSS_PX_PER_MM / Camera2D::new().px_per_mm) * 100.0;
        assert!((zoom_percent / before - 2.0).abs() < 1e-9);
    }

    #[test]
    fn protocol_json_shape() {
        let json = serde_json_shape(&ToEngine::Zoom {
            x: 1.0,
            y: 2.0,
            factor: 1.1,
        });
        assert_eq!(json, r#"{"type":"zoom","x":1.0,"y":2.0,"factor":1.1}"#);
    }

    fn serde_json_shape<T: serde::Serialize>(value: &T) -> String {
        serde_json::to_string(value).unwrap()
    }
}
