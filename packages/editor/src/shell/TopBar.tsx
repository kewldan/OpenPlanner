import { Button, IconButton, Kbd, Segmented } from "@openplanner/ui";
import { actions, isEnabled } from "../actions";
import { format, isMacPlatform } from "../actions/shortcut";
import { sendToEngine } from "../engine";
import { t } from "../i18n";
import { LevelPicker } from "../levels/LevelPicker";
import { useProject } from "../project";
import { catalogModes, type EditorMode, type SidebarTab, useEditor } from "../store/editor";
import { enterReplan, enterSurvey } from "../variants/actions";
import { toggle3d } from "../view3d/actions";
import { Icon } from "./icons";
import styles from "./TopBar.module.css";

const modes: readonly { value: EditorMode; label: string; disabled: boolean }[] = [
  { value: "measure", label: t("mode.measure"), disabled: false },
  { value: "replan", label: t("mode.replan"), disabled: false },
  { value: "furniture", label: t("mode.furniture"), disabled: false },
  { value: "electric", label: t("mode.electric"), disabled: false },
  { value: "light", label: t("mode.light"), disabled: false },
  { value: "plumbing", label: t("mode.plumbing"), disabled: false },
  { value: "finish", label: t("mode.finish"), disabled: false },
];

type ViewKind = "2d" | "3d";

function action(id: string) {
  const found = actions.find((a) => a.id === id);
  if (!found) throw new Error(`нет команды ${id}`);
  return found;
}

const undo = action("edit.undo");
const redo = action("edit.redo");
const palette = action("help.palette");
const drawings = action("file.drawings");
const estimate = action("file.estimate");

function label(id: string): string {
  const found = action(id);
  const key = found.keys?.[0];
  return key ? `${found.title} (${format(key, isMacPlatform)})` : found.title;
}

export function TopBar() {
  const fileName = useProject((s) => s.name) ?? t("project.untitled");
  const dirty = useEditor((s) => s.dirty);
  const mode = useEditor((s) => s.mode);
  const view = useEditor((s) => s.view);
  const viewName = useEditor((s) => {
    const v = s.view;
    if (v.type === "compare") return t("compare.title");
    const id = v.type === "variant" ? v.id : undefined;
    return s.summaries.find((x) => x.variant === id)?.name ?? t("variants.survey");
  });
  const setMode = (next: EditorMode) => {
    // Раскладка пола на плане видна только в режиме «Отделка».
    if ((next === "finish") !== (useEditor.getState().mode === "finish"))
      sendToEngine({ type: "showFinishes", show: next === "finish" });
    // Режимы каталогов не меняют вид (обмер или вариант): открывают каталог или расчёт.
    if (catalogModes.includes(next)) {
      const tab: SidebarTab = next === "finish" ? "finish" : "catalog";
      useEditor.getState().setMode(next);
      useEditor.getState().setSidebarTab(tab);
    } else {
      useEditor.getState().setMode(next);
      useEditor.getState().setSidebarTab("objects");
      if (next === "replan") enterReplan();
      else enterSurvey();
    }
  };
  const in3d = useEditor((s) => s.three.options.enabled);
  const comparing = view.type === "compare";
  const views: readonly { value: ViewKind; label: string; disabled: boolean }[] = [
    { value: "2d", label: t("view.2d"), disabled: false },
    { value: "3d", label: t("view.3d"), disabled: comparing },
  ];
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
        <span className={styles.variant} data-testid="view-name" data-view={view.type}>
          / {viewName}
        </span>
      </div>
      <div className={styles.group}>
        <IconButton label={label(undo.id)} disabled={!isEnabled(undo)} onClick={undo.run}>
          <Icon name="undo" />
        </IconButton>
        <IconButton label={label(redo.id)} disabled={!isEnabled(redo)} onClick={redo.run}>
          <Icon name="redo" />
        </IconButton>
      </div>
      <LevelPicker />
      <div className={styles.spacer} />
      <Segmented label={t("mode.label")} value={mode} options={modes} onChange={setMode} />
      <div className={styles.spacer} />
      <Segmented
        label={t("view.label")}
        value={in3d ? "3d" : "2d"}
        options={views}
        onChange={(next: ViewKind) => toggle3d(next === "3d")}
      />
      <div className={styles.group}>
        <Button onClick={palette.run} aria-label={label(palette.id)}>
          Команды
          <Kbd>{format(palette.keys?.[0] ?? { code: "KeyK", mod: true }, isMacPlatform)}</Kbd>
        </Button>
        <Button onClick={drawings.run} aria-label={label(drawings.id)}>
          {t("action.drawings")}
        </Button>
        <Button onClick={estimate.run} aria-label={label(estimate.id)}>
          {t("action.estimate")}
        </Button>
        <Button variant="primary" onClick={drawings.run}>
          {t("action.export")}
        </Button>
      </div>
    </header>
  );
}
