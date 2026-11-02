import { Button, Dialog } from "@openplanner/ui";
import { useEffect, useState } from "react";
import styles from "../drawings/DrawingsDialog.module.css";
import { callEngine } from "../engine";
import { t } from "../i18n";
import { platform } from "../platform";
import { useEditor } from "../store/editor";
import { GENERATOR } from "../version";
import { guestLink, LONG_LINK, saveWebUrl, webUrl } from "./guest";

/** Окно «Поделиться»: гостевая ссылка на просмотр проекта в веб-версии. */
export function ShareDialog() {
  const open = useEditor((s) => s.dialog === "share");
  const setDialog = useEditor((s) => s.setDialog);
  const document = useEditor((s) => s.document);
  const [base, setBase] = useState(() => webUrl(platform.kind));
  const [project, setProject] = useState<Uint8Array | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!open || !document) return;
    setCopied(false);
    let cancelled = false;
    void callEngine({ type: "exportGuest", generator: GENERATOR }).then((bytes) => {
      if (!cancelled) setProject(bytes);
    });
    return () => {
      cancelled = true;
    };
  }, [open, document]);

  const link = project ? guestLink(base, project) : "";
  const copy = async () => {
    await navigator.clipboard.writeText(link);
    setCopied(true);
  };
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => setDialog(next ? "share" : null)}
      title={t("share.title")}
      width={620}
    >
      <div className={styles.side} style={{ padding: "var(--space-6)" }} data-testid="share">
        <p className={styles.hint}>{t("share.hint")}</p>
        <label className={styles.pick}>
          <span>{t("share.web")}</span>
          <input
            aria-label={t("share.web")}
            value={base}
            onChange={(e) => {
              setBase(e.target.value);
              saveWebUrl(e.target.value);
            }}
          />
        </label>
        <textarea
          aria-label={t("share.link")}
          readOnly
          rows={4}
          value={link}
          onFocus={(e) => e.currentTarget.select()}
        />
        {link.length > LONG_LINK && (
          <p className={styles.warning} role="alert">
            {t("share.long", { kb: Math.round(link.length / 1024) })}
          </p>
        )}
        <Button variant="primary" disabled={!link} onClick={() => void copy()}>
          {copied ? t("share.copied") : t("share.copy")}
        </Button>
      </div>
    </Dialog>
  );
}
