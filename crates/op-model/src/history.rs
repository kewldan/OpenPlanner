use crate::{Command, CommandError, Document};

/// Документ вместе со стеком отмены и повтора.
#[derive(Debug, Clone, Default)]
pub struct History {
    doc: Document,
    undo: Vec<Command>,
    redo: Vec<Command>,
}

impl History {
    /// Оборачивает документ в историю с пустыми стеками.
    #[must_use]
    pub fn new(doc: Document) -> Self {
        Self {
            doc,
            undo: Vec::new(),
            redo: Vec::new(),
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
        self.undo.push(inverse);
        self.redo.clear();
        Ok(())
    }

    /// Отменяет последнюю команду. Возвращает `false`, если отменять нечего.
    ///
    /// # Errors
    ///
    /// Ошибка означает нарушение инварианта: обратная команда должна
    /// применяться всегда.
    pub fn undo(&mut self) -> Result<bool, CommandError> {
        Self::step(&mut self.doc, &mut self.undo, &mut self.redo)
    }

    /// Повторяет последнюю отменённую команду. Возвращает `false`, если повторять нечего.
    ///
    /// # Errors
    ///
    /// См. [`History::undo`].
    pub fn redo(&mut self) -> Result<bool, CommandError> {
        Self::step(&mut self.doc, &mut self.redo, &mut self.undo)
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

    fn step(
        doc: &mut Document,
        from: &mut Vec<Command>,
        to: &mut Vec<Command>,
    ) -> Result<bool, CommandError> {
        let Some(command) = from.pop() else {
            return Ok(false);
        };
        match command.clone().apply(doc) {
            Ok(inverse) => {
                to.push(inverse);
                Ok(true)
            }
            Err(err) => {
                from.push(command);
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_restores_states() {
        let mut history = History::new(Document::new("Квартира"));
        history
            .execute(Command::SetTitle {
                title: "Комендантский пр.".into(),
            })
            .unwrap();
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
        assert_eq!(history.document(), &Document::new("Квартира"));
    }

    #[test]
    fn new_command_clears_redo() {
        let mut history = History::new(Document::new("А"));
        history
            .execute(Command::SetTitle { title: "Б".into() })
            .unwrap();
        history.undo().unwrap();
        history
            .execute(Command::SetAddress {
                address: "СПб".into(),
            })
            .unwrap();
        assert!(!history.can_redo());
    }

    #[test]
    fn command_json_shape() {
        let json =
            serde_json::to_value(Command::SetDefaultCeilingHeight { height: 2700.0 }).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "type": "setDefaultCeilingHeight", "height": 2700.0 })
        );
    }
}
