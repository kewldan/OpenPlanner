import { IconButton } from "@openplanner/ui";
import { sendToEngine } from "../engine";
import { formatNumber, t } from "../i18n";
import { useEditor } from "../store/editor";
import { Icon } from "./icons";
import styles from "./StatusBar.module.css";

export function StatusBar() {
  const engine = useEditor((s) => s.engine);
  const scale = useEditor((s) => s.scaleDenominator);
  const zoom = useEditor((s) => s.zoomPercent);

  return (
    <footer className={styles.root}>
      <span className={styles.hint}>{t("status.hint")}</span>
      <span className={styles.mono}>{t("status.scale", { denominator: formatNumber(scale) })}</span>
      <span className={styles.mono}>{t("status.zoom", { percent: formatNumber(zoom) })}</span>
      <IconButton label={t("action.zoomToFit")} onClick={() => sendToEngine({ type: "zoomToFit" })}>
        <Icon name="fit" />
      </IconButton>
      {engine.state === "ready" && (
        <span className={styles.mono}>{t("status.backend", { backend: engine.backend })}</span>
      )}
    </footer>
  );
}
