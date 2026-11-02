import { IconButton } from "@openplanner/ui";
import { sendToEngine } from "../engine";
import { formatNumber, t } from "../i18n";
import { useProject } from "../project";
import { useEditor } from "../store/editor";
import { Icon } from "./icons";
import styles from "./StatusBar.module.css";

const timeFormat = new Intl.DateTimeFormat("ru-RU", { hour: "2-digit", minute: "2-digit" });

export function StatusBar() {
  const engine = useEditor((s) => s.engine);
  const scale = useEditor((s) => s.scaleDenominator);
  const zoom = useEditor((s) => s.zoomPercent);
  const dirty = useEditor((s) => s.dirty);
  const savedAt = useProject((s) => s.savedAt);
  const saveStatus = dirty
    ? t("project.unsaved")
    : savedAt
      ? t("project.saved", { time: timeFormat.format(savedAt) })
      : t("project.notSavedYet");

  return (
    <footer className={styles.root}>
      <span className={styles.hint}>{t("status.hint")}</span>
      <span className={styles.mono}>{t("status.scale", { denominator: formatNumber(scale) })}</span>
      <span className={styles.mono}>{t("status.zoom", { percent: formatNumber(zoom) })}</span>
      <IconButton label={t("action.zoomToFit")} onClick={() => sendToEngine({ type: "zoomToFit" })}>
        <Icon name="fit" />
      </IconButton>
      <span data-testid="save-status">{saveStatus}</span>
      {engine.state === "ready" && (
        <span className={styles.mono} title={`WebGPU: ${engine.webgpu}`}>
          {t("status.backend", { backend: engine.backend })}
        </span>
      )}
    </footer>
  );
}
