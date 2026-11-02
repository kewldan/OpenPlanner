import { Field } from "@openplanner/ui";
import { type KeyboardEvent, useEffect, useRef, useState } from "react";
import { useEditor } from "../store/editor";

interface CommitFieldProps {
  label: string;
  value: string;
  unit?: string;
  numeric?: boolean;
  placeholder?: string;
  readOnly?: boolean;
  /** Поставить фокус и выделить текст при появлении. */
  autoSelect?: boolean;
  onCommit: (value: string) => void;
}

/** Поле, которое применяет значение по Enter или при потере фокуса; Esc — отмена правки. */
export function CommitField({
  label,
  value,
  unit,
  numeric,
  placeholder,
  readOnly,
  autoSelect,
  onCommit,
}: CommitFieldProps) {
  const [draft, setDraft] = useState(value);
  const editing = useRef(false);
  const input = useRef<HTMLInputElement | null>(null);
  // Значение из документа подхватывается, только когда поле не редактируется:
  // подтверждение прошлой правки не должно затирать то, что пользователь набирает сейчас.
  useEffect(() => {
    if (!editing.current) setDraft(value);
  }, [value]);

  useEffect(() => {
    if (autoSelect && input.current) {
      input.current.focus();
      input.current.select();
      useEditor.getState().clearEditText();
    }
  }, [autoSelect]);

  const commit = () => {
    editing.current = false;
    if (draft !== value) onCommit(draft);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    const target = event.currentTarget;
    if (event.key === "Enter") target.blur();
    if (event.key === "Escape") {
      setDraft(value);
      editing.current = false;
      // Сначала React применит откат черновика, потом поле потеряет фокус без коммита.
      requestAnimationFrame(() => target.blur());
    }
  };

  return (
    <Field
      ref={input}
      label={label}
      value={draft}
      {...(unit ? { unit } : {})}
      {...(placeholder ? { placeholder } : {})}
      readOnly={readOnly ?? false}
      numeric={numeric ?? false}
      inputMode={numeric ? "decimal" : "text"}
      onChange={(event) => setDraft(event.target.value)}
      onFocus={() => {
        editing.current = true;
      }}
      onBlur={commit}
      onKeyDown={onKeyDown}
    />
  );
}
