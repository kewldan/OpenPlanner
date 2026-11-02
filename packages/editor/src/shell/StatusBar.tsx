import { IconButton } from "@openplanner/ui";
import { sendToEngine } from "../engine";
import { formatNumber, t } from "../i18n";
import { useProject } from "../project";
import { useEditor } from "../store/editor";
import { Icon } from "./icons";
import styles from "./StatusBar.module.css";

const timeFormat = new Intl.DateTimeFormat("ru-RU", { hour: "2-digit", minute: "2-digit" });
const angleFormat = new Intl.NumberFormat("ru-RU", { maximumFractionDigits: 1 });

function CursorInfo() {
  const cursor = useEditor((s) => s.cursor);
  if (!cursor) return null;
  return (
    <>
      <span className={styles.mono} data-testid="cursor">
        {t("status.cursor", { x: formatNumber(cursor.x), y: formatNumber(cursor.y) })}
      </span>
      {cursor.length !== null && cursor.angle !== null && (
        <span className={styles.mono} data-testid="segment">
          {t("status.segment", {
            length: formatNumber(cursor.length),
            angle: angleFormat.format(cursor.angle),
          })}
        </span>
      )}
      {cursor.snap && <span className={styles.snap}>{t(`snap.${cursor.snap}`)}</span>}
    </>
  );
}

export function StatusBar() {
  const engine = useEditor((s) => s.engine);
  const scale = useEditor((s) => s.scaleDenominator);
  const zoom = useEditor((s) => s.zoomPercent);
  const dirty = useEditor((s) => s.dirty);
  const step = useEditor((s) => s.step);
  const savedAt = useProject((s) => s.savedAt);
  const saveStatus = dirty
    ? t("project.unsaved")
    : savedAt
      ? t("project.saved", { time: timeFormat.format(savedAt) })
      : t("project.notSavedYet");

  return (
    <footer className={styles.root}>
      <span className={styles.hint} data-testid="tool-hint">
        {step === "idle" ? t("status.hint") : t(`step.${step}`)}
      </span>
      <CursorInfo />
      <button
        type="button"
        className={[styles.mono, styles.link].join(" ")}
        title={t("status.scaleTo50")}
        onClick={() => sendToEngine({ type: "zoomToScale", denominator: 50 })}
      >
        {t("status.scale", { denominator: formatNumber(scale) })}
      </button>
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
