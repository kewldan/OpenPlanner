use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Command, CommandError, Document};

/// Операция над историей. Последовательность таких операций — журнал
/// автосохранения: воспроизведение журнала поверх исходного документа
/// восстанавливает состояние, включая стеки отмены и повтора.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
// Операции короткоживущие и по одной на действие: упаковка команды в Box
// не окупает лишнего выделения памяти.
#[allow(clippy::large_enum_variant)]
pub enum HistoryOp {
    /// Выполнить команду.
    Execute {
        /// Команда.
        command: Command,
    },
    /// Отменить последнюю команду.
    Undo,
    /// Повторить отменённую команду.
    Redo,
}

/// Идентификатор состояния документа. Нужен, чтобы понимать, совпадает ли
/// текущее состояние с сохранённым (в том числе после отмены и повтора).
type StateId = u64;

/// Документ вместе со стеком отмены и повтора.
#[derive(Debug, Clone, Default)]
pub struct History {
    doc: Document,
    /// Обратные команды и состояние, в которое они возвращают.
    undo: Vec<(Command, StateId)>,
    redo: Vec<(Command, StateId)>,
    current: StateId,
    next_id: StateId,
    saved: Option<StateId>,
}

impl History {
    /// Оборачивает документ в историю с пустыми стеками. Документ считается сохранённым.
    #[must_use]
    pub fn new(doc: Document) -> Self {
        Self {
            doc,
            undo: Vec::new(),
            redo: Vec::new(),
            current: 0,
            next_id: 1,
            saved: Some(0),
        }
    }

    /// Текущее состояние документа.
    #[must_use]
    pub fn document(&self) -> &Document {
        &self.doc
    }

    /// Выполняет команду. Стек повтора очищается.
    ///
    /// # Errors
    ///
    /// Пробрасывает ошибку команды; история при этом не меняется.
    pub fn execute(&mut self, command: Command) -> Result<(), CommandError> {
        let inverse = command.apply(&mut self.doc)?;
        self.undo.push((inverse, self.current));
        self.redo.clear();
        self.current = self.next_id;
        self.next_id += 1;
        Ok(())
    }

    /// Отменяет последнюю команду. Возвращает `false`, если отменять нечего.
    ///
    /// # Errors
    ///
    /// Ошибка означает нарушение инварианта: обратная команда должна
    /// применяться всегда.
    pub fn undo(&mut self) -> Result<bool, CommandError> {
        Self::step(
            &mut self.doc,
            &mut self.current,
            &mut self.undo,
            &mut self.redo,
        )
    }

    /// Повторяет последнюю отменённую команду. Возвращает `false`, если повторять нечего.
    ///
    /// # Errors
    ///
    /// См. [`History::undo`].
    pub fn redo(&mut self) -> Result<bool, CommandError> {
        Self::step(
            &mut self.doc,
            &mut self.current,
            &mut self.redo,
            &mut self.undo,
        )
    }

    /// Применяет операцию журнала. Возвращает `true`, если документ изменился.
    ///
    /// # Errors
    ///
    /// Пробрасывает ошибку команды.
    pub fn apply(&mut self, op: HistoryOp) -> Result<bool, CommandError> {
        match op {
            HistoryOp::Execute { command } => self.execute(command).map(|()| true),
            HistoryOp::Undo => self.undo(),
            HistoryOp::Redo => self.redo(),
        }
    }

    /// Можно ли отменить действие.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Можно ли повторить действие.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Есть ли изменения после последнего сохранения.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.saved != Some(self.current)
    }

    /// Отмечает текущее состояние как сохранённое.
    pub fn mark_saved(&mut self) {
        self.saved = Some(self.current);
    }

    /// Отмечает документ как несохранённый (например, после восстановления из журнала
    /// или для нового проекта, у которого ещё нет файла).
    pub fn mark_unsaved(&mut self) {
        self.saved = None;
    }

    fn step(
        doc: &mut Document,
        current: &mut StateId,
        from: &mut Vec<(Command, StateId)>,
        to: &mut Vec<(Command, StateId)>,
    ) -> Result<bool, CommandError> {
        let Some((command, target)) = from.pop() else {
            return Ok(false);
        };
        match command.clone().apply(doc) {
            Ok(inverse) => {
                to.push((inverse, *current));
                *current = target;
                Ok(true)
            }
            Err(err) => {
                from.push((command, target));
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn title(title: &str) -> Command {
        Command::SetTitle {
            title: title.into(),
        }
    }

    #[test]
    fn undo_redo_restores_states() {
        let mut history = History::new(Document::new("Квартира"));
        history.execute(title("Комендантский пр.")).unwrap();
        history
            .execute(Command::SetDefaultCeilingHeight { height: 2720.0 })
            .unwrap();

        assert!(history.undo().unwrap());
        let height = history.document().default_ceiling_height;
        assert!((height - Document::DEFAULT_CEILING_HEIGHT).abs() < f64::EPSILON);
        assert!(history.undo().unwrap());
        assert_eq!(history.document().meta.title, "Квартира");
        assert!(!history.undo().unwrap());

        assert!(history.redo().unwrap());
        assert_eq!(history.document().meta.title, "Комендантский пр.");
        assert!(history.can_redo());
    }

    #[test]
    fn invalid_command_leaves_history_untouched() {
        let mut history = History::new(Document::new("Квартира"));
        let err = history.execute(Command::SetDefaultCeilingHeight { height: -1.0 });
        assert!(err.is_err());
        assert!(!history.can_undo());
        assert!(!history.is_dirty());
        assert_eq!(history.document(), &Document::new("Квартира"));
    }

    #[test]
    fn new_command_clears_redo() {
        let mut history = History::new(Document::new("А"));
        history.execute(title("Б")).unwrap();
        history.undo().unwrap();
        history
            .execute(Command::SetAddress {
                address: "СПб".into(),
            })
            .unwrap();
        assert!(!history.can_redo());
    }

    #[test]
    fn dirty_tracks_saved_state_through_undo_redo() {
        let mut history = History::new(Document::new("А"));
        assert!(!history.is_dirty());
        history.execute(title("Б")).unwrap();
        assert!(history.is_dirty());
        history.undo().unwrap();
        assert!(!history.is_dirty(), "отмена вернула сохранённое состояние");
        history.redo().unwrap();
        history.mark_saved();
        history.execute(title("В")).unwrap();
        history.undo().unwrap();
        assert!(!history.is_dirty());
        history.undo().unwrap();
        assert!(history.is_dirty());
    }

    #[test]
    fn diverged_branch_is_dirty() {
        let mut history = History::new(Document::new("А"));
        history.execute(title("Б")).unwrap();
        history.mark_saved();
        history.undo().unwrap();
        history.execute(title("Б")).unwrap();
        assert!(
            history.is_dirty(),
            "новая ветка — новое состояние, даже с тем же содержимым"
        );
    }

    #[test]
    fn replaying_ops_reproduces_state() {
        let ops = vec![
            HistoryOp::Execute {
                command: title("Б"),
            },
            HistoryOp::Execute {
                command: Command::SetAddress {
                    address: "СПб".into(),
                },
            },
            HistoryOp::Undo,
            HistoryOp::Execute {
                command: title("В"),
            },
            HistoryOp::Undo,
            HistoryOp::Redo,
        ];
        let mut live = History::new(Document::new("А"));
        let mut replayed = History::new(Document::new("А"));
        for op in &ops {
            live.apply(op.clone()).unwrap();
        }
        for op in ops {
            replayed.apply(op).unwrap();
        }
        assert_eq!(live.document(), replayed.document());
        assert_eq!(live.can_undo(), replayed.can_undo());
        assert_eq!(live.can_redo(), replayed.can_redo());
    }

    #[test]
    fn history_op_json_shape() {
        let json = serde_json::to_value(HistoryOp::Execute {
            command: title("А"),
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "op": "execute", "command": { "type": "setTitle", "title": "А" } })
        );
        assert_eq!(
            serde_json::to_value(HistoryOp::Undo).unwrap(),
            serde_json::json!({ "op": "undo" })
        );
    }
}
