import { Button, IconButton, Segmented } from "@openplanner/ui";
import { sendToEngine } from "../engine";
import { t } from "../i18n";
import { type EditorMode, useEditor } from "../store/editor";
import { Icon } from "./icons";
import styles from "./TopBar.module.css";

const modes: readonly { value: EditorMode; label: string; disabled: boolean }[] = [
  { value: "measure", label: t("mode.measure"), disabled: false },
  { value: "replan", label: t("mode.replan"), disabled: true },
  { value: "furniture", label: t("mode.furniture"), disabled: true },
  { value: "electric", label: t("mode.electric"), disabled: true },
  { value: "light", label: t("mode.light"), disabled: true },
  { value: "plumbing", label: t("mode.plumbing"), disabled: true },
  { value: "finish", label: t("mode.finish"), disabled: true },
];

const views = [
  { value: "2d", label: t("view.2d") },
  { value: "3d", label: t("view.3d"), disabled: true },
] as const;

export function TopBar() {
  const title = useEditor((s) => s.document?.meta.title ?? t("app.name"));
  const mode = useEditor((s) => s.mode);
  const setMode = useEditor((s) => s.setMode);
  const canUndo = useEditor((s) => s.canUndo);
  const canRedo = useEditor((s) => s.canRedo);

  return (
    <header className={styles.root}>
      <div className={styles.project}>
        <span className={styles.title}>{title}</span>
        <span className={styles.variant}>/ {t("project.untitledVariant")}</span>
      </div>
      <div className={styles.group}>
        <IconButton
          label={`${t("action.undo")} (⌘Z)`}
          disabled={!canUndo}
          onClick={() => sendToEngine({ type: "undo" })}
        >
          <Icon name="undo" />
        </IconButton>
        <IconButton
          label={`${t("action.redo")} (⇧⌘Z)`}
          disabled={!canRedo}
          onClick={() => sendToEngine({ type: "redo" })}
        >
          <Icon name="redo" />
        </IconButton>
      </div>
      <div className={styles.spacer} />
      <Segmented label={t("mode.label")} value={mode} options={modes} onChange={setMode} />
      <div className={styles.spacer} />
      <Segmented label={t("view.label")} value="2d" options={views} onChange={() => {}} />
      <div className={styles.group}>
        <Button disabled>{t("action.drawings")}</Button>
        <Button disabled>{t("action.estimate")}</Button>
        <Button variant="primary" disabled>
          {t("action.export")}
        </Button>
      </div>
    </header>
  );
}
