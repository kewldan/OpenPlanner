use glam::DVec2;
use op_format::{FormatError, Manifest, journal};
use op_model::{Document, History, HistoryOp};
use op_render::Camera2D;

use crate::protocol::{EngineEvents, FromEngine, ToEngine};

/// CSS-пикселей в миллиметре экрана (96 dpi).
const CSS_PX_PER_MM: f64 = 96.0 / 25.4;
/// Знаменатель масштаба, который считается 100 %.
const REFERENCE_SCALE: f64 = 50.0;
/// Название нового проекта.
pub const UNTITLED: &str = "Новый проект";

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
            history: History::new(Document::new(UNTITLED)),
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

    /// События, описывающие текущее состояние целиком.
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
            ToEngine::Command { command } => self.apply(HistoryOp::Execute { command }),
            ToEngine::Undo => self.apply(HistoryOp::Undo),
            ToEngine::Redo => self.apply(HistoryOp::Redo),
            ToEngine::NewDocument => {
                self.reset(Document::new(UNTITLED));
                return self.snapshot();
            }
            ToEngine::MarkSaved => {
                self.history.mark_saved();
                vec![self.history_event()]
            }
        };
        EngineEvents { events }
    }

    /// Записывает текущий документ в контейнер `.oplan`.
    ///
    /// # Errors
    ///
    /// Ошибка сериализации или упаковки.
    pub fn export_file(&self, generator: &str) -> Result<Vec<u8>, FormatError> {
        let cursor = std::io::Cursor::new(Vec::new());
        let cursor = op_format::write(
            cursor,
            &Manifest::current(generator),
            self.history.document(),
        )?;
        Ok(cursor.into_inner())
    }

    /// Открывает проект из контейнера `.oplan`. История начинается заново.
    ///
    /// # Errors
    ///
    /// Ошибка, если файл повреждён или создан более новой версией.
    pub fn import_file(&mut self, bytes: &[u8]) -> Result<EngineEvents, FormatError> {
        let (_manifest, document): (Manifest, Document) =
            op_format::read(std::io::Cursor::new(bytes))?;
        self.reset(document);
        Ok(self.snapshot())
    }

    /// Воспроизводит журнал автосохранения поверх текущего документа.
    ///
    /// Восстановленные изменения считаются несохранёнными. Строки журнала
    /// повторно не выдаются: журнал продолжает дописываться.
    ///
    /// # Errors
    ///
    /// Ошибка разбора журнала. Операции, которые не применились, пропускаются.
    pub fn replay_journal(&mut self, text: &str) -> Result<EngineEvents, FormatError> {
        let ops: Vec<HistoryOp> = journal::parse(text)?;
        let mut changed = false;
        for op in ops {
            changed |= self.history.apply(op).unwrap_or(false);
        }
        if changed {
            self.history.mark_unsaved();
        }
        Ok(self.snapshot())
    }

    fn reset(&mut self, document: Document) {
        self.history = History::new(document);
        self.camera = Camera2D::new();
    }

    fn apply(&mut self, op: HistoryOp) -> Vec<FromEngine> {
        let line = journal::entry_line(&op);
        match self.history.apply(op) {
            Ok(true) => {
                let mut events = vec![self.document_event(), self.history_event()];
                match line {
                    Ok(line) => events.push(FromEngine::Journal { line }),
                    Err(err) => events.push(FromEngine::Error {
                        message: err.to_string(),
                    }),
                }
                events
            }
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
            dirty: self.history.is_dirty(),
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

    fn rename(s: &mut EngineState, title: &str) -> EngineEvents {
        s.handle(ToEngine::Command {
            command: Command::SetTitle {
                title: title.into(),
            },
        })
    }

    fn title(events: &EngineEvents) -> &str {
        events
            .events
            .iter()
            .find_map(|e| match e {
                FromEngine::DocumentChanged { document } => Some(document.meta.title.as_str()),
                _ => None,
            })
            .unwrap()
    }

    fn journal_of(events: &[EngineEvents]) -> String {
        let mut text = journal::header_line().unwrap();
        for batch in events {
            for event in &batch.events {
                if let FromEngine::Journal { line } = event {
                    text += line;
                }
            }
        }
        text
    }

    #[test]
    fn command_then_undo() {
        let mut s = state();
        let events = rename(&mut s, "Комендантский пр.");
        assert!(events.events.contains(&FromEngine::HistoryChanged {
            can_undo: true,
            can_redo: false,
            dirty: true
        }));
        let events = s.handle(ToEngine::Undo);
        assert_eq!(title(&events), UNTITLED);
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
    fn nothing_to_undo_emits_nothing() {
        assert!(state().handle(ToEngine::Undo).events.is_empty());
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
    fn file_roundtrip_resets_history() {
        let mut s = state();
        rename(&mut s, "Комендантский пр.");
        let bytes = s.export_file("test").unwrap();

        let mut other = state();
        let events = other.import_file(&bytes).unwrap();
        assert_eq!(title(&events), "Комендантский пр.");
        assert!(events.events.contains(&FromEngine::HistoryChanged {
            can_undo: false,
            can_redo: false,
            dirty: false
        }));
    }

    #[test]
    fn journal_replay_restores_unsaved_changes() {
        let mut s = state();
        let batches = vec![
            rename(&mut s, "А"),
            rename(&mut s, "Б"),
            s.handle(ToEngine::Undo),
        ];
        let journal = journal_of(&batches);

        let mut recovered = state();
        let events = recovered.replay_journal(&journal).unwrap();
        assert_eq!(title(&events), "А");
        assert!(events.events.contains(&FromEngine::HistoryChanged {
            can_undo: true,
            can_redo: true,
            dirty: true
        }));
        assert!(
            !events
                .events
                .iter()
                .any(|e| matches!(e, FromEngine::Journal { .. }))
        );
    }

    #[test]
    fn mark_saved_clears_dirty() {
        let mut s = state();
        rename(&mut s, "А");
        let events = s.handle(ToEngine::MarkSaved);
        assert_eq!(
            events.events,
            vec![FromEngine::HistoryChanged {
                can_undo: true,
                can_redo: false,
                dirty: false
            }]
        );
    }

    #[test]
    fn protocol_json_shape() {
        let json = serde_json::to_string(&ToEngine::Zoom {
            x: 1.0,
            y: 2.0,
            factor: 1.1,
        })
        .unwrap();
        assert_eq!(json, r#"{"type":"zoom","x":1.0,"y":2.0,"factor":1.1}"#);
    }
}
