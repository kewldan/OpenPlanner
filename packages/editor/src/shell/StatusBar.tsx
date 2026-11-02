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

/** Камера 3D-вида: режим, положение глаза и его высота. */
function CameraInfo3d() {
  const camera = useEditor((s) => s.three.camera);
  if (!camera) return null;
  const [x, y, h] = camera.eye;
  return (
    <span className={styles.mono} data-testid="camera-3d">
      {t("view3d.camera", {
        mode: camera.mode === "walk" ? t("view3d.walk") : t("view3d.orbit"),
        x: formatNumber(x),
        y: formatNumber(y),
        h: formatNumber(h),
      })}
    </span>
  );
}

export function StatusBar() {
  const engine = useEditor((s) => s.engine);
  const in3d = useEditor((s) => s.three.options.enabled);
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
      {in3d ? (
        <CameraInfo3d />
      ) : (
        <>
          <span className={styles.hint} data-testid="tool-hint">
            {step === "idle" ? t("status.hint") : t(`step.${step}`)}
          </span>
          <CursorInfo />
        </>
      )}
      {in3d ? (
        <>
          <span className={styles.spacer} />
          <IconButton
            label={t("view3d.reset")}
            onClick={() => sendToEngine({ type: "resetCamera3d" })}
          >
            <Icon name="fit" />
          </IconButton>
        </>
      ) : (
        <>
          <button
            type="button"
            className={[styles.mono, styles.link].join(" ")}
            title={t("status.scaleTo50")}
            onClick={() => sendToEngine({ type: "zoomToScale", denominator: 50 })}
          >
            {t("status.scale", { denominator: formatNumber(scale) })}
          </button>
          <span className={styles.mono}>{t("status.zoom", { percent: formatNumber(zoom) })}</span>
          <IconButton
            label={t("action.zoomToFit")}
            onClick={() => sendToEngine({ type: "zoomToFit" })}
          >
            <Icon name="fit" />
          </IconButton>
        </>
      )}
      <span data-testid="save-status">{saveStatus}</span>
      {engine.state === "ready" && (
        <span className={styles.mono} title={`WebGPU: ${engine.webgpu}`}>
          {t("status.backend", { backend: engine.backend })}
        </span>
      )}
    </footer>
  );
}
