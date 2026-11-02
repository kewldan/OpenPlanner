import type { Variant } from "@openplanner/engine-client";
import { Button, IconButton } from "@openplanner/ui";
import { type KeyboardEvent, useEffect, useState } from "react";
import { sendToEngine } from "../engine";
import { formatMoney } from "../estimate/prices";
import { t } from "../i18n";
import { formatDecimal } from "../inspector/format";
import { Icon } from "../shell/icons";
import styles from "../shell/Sidebar.module.css";
import { useEditor } from "../store/editor";
import { createVariant, deleteVariant, enterSurvey, setView } from "./actions";

/** Вкладка «Варианты»: обмер и варианты перепланировки. */
export function Variants() {
  const doc = useEditor((s) => s.document);
  const view = useEditor((s) => s.view);
  if (!doc) return null;
  return (
    <div className={styles.tree} data-testid="variants">
      <ul className={styles.list}>
        <li>
          <button
            type="button"
            className={styles.item}
            aria-pressed={view.type === "survey"}
            onClick={enterSurvey}
          >
            <span className={styles.itemLabel}>{t("variants.survey")}</span>
          </button>
        </li>
        {(doc.variants ?? []).map((variant) => (
          <VariantRow
            key={variant.id}
            variant={variant}
            active={view.type === "variant" && view.id === variant.id}
          />
        ))}
      </ul>
      <button type="button" className={styles.add} onClick={() => createVariant()}>
        <Icon name="plus" />
        {t("variants.add")}
      </button>
      <div className={styles.footer}>
        <Button
          variant="outline"
          disabled={(doc.variants ?? []).length === 0}
          onClick={() => useEditor.getState().setDialog("compare")}
        >
          {t("variants.compare")}
        </Button>
        <p className={styles.empty}>{t("variants.hint")}</p>
      </div>
    </div>
  );
}

function VariantRow({ variant, active }: { variant: Variant; active: boolean }) {
  const [draft, setDraft] = useState(variant.name);
  // Переименование отменили или повторили — поле показывает название из документа.
  useEffect(() => setDraft(variant.name), [variant.name]);
  const summary = useEditor((s) => s.summaries.find((x) => x.variant === variant.id));
  const commit = () => {
    const name = draft.trim();
    if (name && name !== variant.name) {
      sendToEngine({
        type: "command",
        command: { type: "put", element: { type: "variant", ...variant, name } },
      });
    } else setDraft(variant.name);
  };
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") event.currentTarget.blur();
    if (event.key === "Escape") {
      setDraft(variant.name);
      const input = event.currentTarget;
      requestAnimationFrame(() => input.blur());
    }
  };
  const added = summary?.newWalls.reduce((sum, m) => sum + m.length, 0) ?? 0;
  return (
    <li className={styles.variant} data-variant={variant.id} data-active={active}>
      <IconButton
        label={variant.name}
        aria-pressed={active}
        onClick={() => setView({ type: "variant", id: variant.id })}
      >
        <Icon name={active ? "eye" : "rooms"} />
      </IconButton>
      <div className={styles.variantBody}>
        <input
          className={styles.layerName}
          aria-label={t("variants.rename")}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onBlur={commit}
          onKeyDown={onKeyDown}
          onFocus={() => {
            if (!active) setView({ type: "variant", id: variant.id });
          }}
        />
        <span className={styles.count} title={t("compare.changes")}>
          {t("variants.changes", {
            demolished: formatDecimal(summary?.demolishedLength ?? 0),
            added: formatDecimal(added),
          })}
          {summary && summary.cost > 0 && ` · ${formatMoney(summary.cost)}`}
        </span>
      </div>
      <IconButton label={t("variants.copy")} onClick={() => createVariant(variant.id)}>
        <Icon name="plus" />
      </IconButton>
      <IconButton label={t("variants.remove")} onClick={() => void deleteVariant(variant.id)}>
        <Icon name="trash" />
      </IconButton>
    </li>
  );
}
