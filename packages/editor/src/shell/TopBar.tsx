import { Button, IconButton, Kbd, Segmented } from "@openplanner/ui";
import { actions, isEnabled } from "../actions";
import { format, isMacPlatform } from "../actions/shortcut";
import { t } from "../i18n";
import { useProject } from "../project";
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

function action(id: string) {
  const found = actions.find((a) => a.id === id);
  if (!found) throw new Error(`нет команды ${id}`);
  return found;
}

const undo = action("edit.undo");
const redo = action("edit.redo");
const palette = action("help.palette");

function label(id: string): string {
  const found = action(id);
  const key = found.keys?.[0];
  return key ? `${found.title} (${format(key, isMacPlatform)})` : found.title;
}

export function TopBar() {
  const fileName = useProject((s) => s.name) ?? t("project.untitled");
  const dirty = useEditor((s) => s.dirty);
  const mode = useEditor((s) => s.mode);
  const setMode = useEditor((s) => s.setMode);
  // Подписка на историю, чтобы кнопки обновляли доступность.
  useEditor((s) => s.canUndo && s.canRedo);

  return (
    <header className={styles.root}>
      <div className={styles.project}>
        <span className={styles.title} data-testid="project-name">
          {fileName}
        </span>
        {dirty && (
          <span
            className={styles.dirty}
            role="img"
            aria-label={t("project.unsaved")}
            title={t("project.unsaved")}
          />
        )}
        <span className={styles.variant}>/ {t("project.untitledVariant")}</span>
      </div>
      <div className={styles.group}>
        <IconButton label={label(undo.id)} disabled={!isEnabled(undo)} onClick={undo.run}>
          <Icon name="undo" />
        </IconButton>
        <IconButton label={label(redo.id)} disabled={!isEnabled(redo)} onClick={redo.run}>
          <Icon name="redo" />
        </IconButton>
      </div>
      <div className={styles.spacer} />
      <Segmented label={t("mode.label")} value={mode} options={modes} onChange={setMode} />
      <div className={styles.spacer} />
      <Segmented label={t("view.label")} value="2d" options={views} onChange={() => {}} />
      <div className={styles.group}>
        <Button onClick={palette.run} aria-label={label(palette.id)}>
          Команды
          <Kbd>{format(palette.keys?.[0] ?? { code: "KeyK", mod: true }, isMacPlatform)}</Kbd>
        </Button>
        <Button disabled>{t("action.drawings")}</Button>
        <Button disabled>{t("action.estimate")}</Button>
        <Button variant="primary" disabled>
          {t("action.export")}
        </Button>
      </div>
    </header>
  );
}
