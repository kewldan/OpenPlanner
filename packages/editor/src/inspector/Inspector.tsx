import type { Command } from "@openplanner/engine-client";
import { Field } from "@openplanner/ui";
import { type KeyboardEvent, useEffect, useState } from "react";
import { sendToEngine } from "../engine";
import { t } from "../i18n";
import { useEditor } from "../store/editor";
import styles from "./Inspector.module.css";

/** Правая панель. Пока — свойства проекта; правки идут командами в движок. */
export function Inspector() {
  const document = useEditor((s) => s.document);
  const lastError = useEditor((s) => s.lastError);

  return (
    <aside className={styles.root} aria-label={t("inspector.project")}>
      <section className={styles.section}>
        <h2 className={styles.title}>{t("inspector.project")}</h2>
        {document && (
          <>
            <CommitField
              label={t("inspector.title")}
              value={document.meta.title}
              toCommand={(title) => ({ type: "setTitle", title })}
            />
            <CommitField
              label={t("inspector.address")}
              value={document.meta.address}
              toCommand={(address) => ({ type: "setAddress", address })}
            />
            <CommitField
              label={t("inspector.ceiling")}
              unit={t("unit.mm")}
              numeric
              value={String(document.defaultCeilingHeight)}
              toCommand={(raw) => ({
                type: "setDefaultCeilingHeight",
                height: Number(raw.replace(",", ".").replace(/\s/g, "")),
              })}
            />
          </>
        )}
        {lastError && (
          <p className={styles.error} role="alert">
            {lastError}
          </p>
        )}
      </section>
    </aside>
  );
}

interface CommitFieldProps {
  label: string;
  value: string;
  unit?: string;
  numeric?: boolean;
  toCommand: (value: string) => Command;
}

/** Поле, которое отправляет команду по Enter или при потере фокуса; Esc — отмена правки. */
function CommitField({ label, value, unit, numeric, toCommand }: CommitFieldProps) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);

  const commit = () => {
    if (draft !== value) sendToEngine({ type: "command", command: toCommand(draft) });
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    const input = event.currentTarget;
    if (event.key === "Enter") input.blur();
    if (event.key === "Escape") {
      setDraft(value);
      // Сначала React применит откат черновика, потом поле потеряет фокус без коммита.
      requestAnimationFrame(() => input.blur());
    }
  };

  return (
    <Field
      label={label}
      value={draft}
      {...(unit ? { unit } : {})}
      numeric={numeric ?? false}
      inputMode={numeric ? "decimal" : "text"}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={commit}
      onKeyDown={onKeyDown}
    />
  );
}
