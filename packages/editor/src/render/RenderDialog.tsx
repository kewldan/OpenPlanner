import type { RenderShot } from "@openplanner/engine-client";
import { Button, Dialog } from "@openplanner/ui";
import { useEffect, useState } from "react";
import styles from "../drawings/DrawingsDialog.module.css";
import { callEngine, sendToEngine } from "../engine";
import { type MessageKey, t } from "../i18n";
import { platform } from "../platform";
import { useEditor } from "../store/editor";
import { cancel, clearFinished, enqueue, QUALITIES, SIZES, useRenderQueue } from "./queue";

/** Окно «Рендер»: постановка кадров в очередь, ход трассировки, готовые кадры проекта. */
export function RenderDialog() {
  const open = useEditor((s) => s.dialog === "render");
  const setDialog = useEditor((s) => s.setDialog);
  const doc = useEditor((s) => s.document);
  const jobs = useRenderQueue((s) => s.jobs);
  const [view, setView] = useState("");
  const [size, setSize] = useState(0);
  const [quality, setQuality] = useState(1);
  if (!doc) return null;
  const views = doc.views ?? [];
  const add = () => {
    const s = SIZES[size] ?? SIZES[0];
    const q = QUALITIES[quality] ?? QUALITIES[0];
    const name = views.find((v) => v.id === view)?.name ?? t("render.camera");
    enqueue({
      name,
      ...(view ? { view } : {}),
      width: s.width,
      height: s.height,
      passes: q.passes,
    });
  };
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => setDialog(next ? "render" : null)}
      title={t("render.title")}
      width={900}
    >
      <div className={styles.root} data-testid="render">
        <div className={styles.side}>
          <label className={styles.pick}>
            <span>{t("render.view")}</span>
            <select
              aria-label={t("render.view")}
              value={view}
              onChange={(e) => setView(e.target.value)}
            >
              <option value="">{t("render.camera")}</option>
              {views.map((v) => (
                <option key={v.id} value={v.id}>
                  {v.name}
                </option>
              ))}
            </select>
          </label>
          <label className={styles.pick}>
            <span>{t("render.size")}</span>
            <select
              aria-label={t("render.size")}
              value={size}
              onChange={(e) => setSize(Number(e.target.value))}
            >
              {SIZES.map((s, i) => (
                <option key={s.label} value={i}>
                  {s.label}
                </option>
              ))}
            </select>
          </label>
          <label className={styles.pick}>
            <span>{t("render.quality")}</span>
            <select
              aria-label={t("render.quality")}
              value={quality}
              onChange={(e) => setQuality(Number(e.target.value))}
            >
              {QUALITIES.map((q, i) => (
                <option key={q.label} value={i}>
                  {t(q.label as MessageKey)}
                </option>
              ))}
            </select>
          </label>
          <Button variant="primary" onClick={add}>
            {t("render.enqueue")}
          </Button>
          <p className={styles.hint}>{t("render.hint")}</p>
          <h3 className={styles.heading}>{t("render.queue")}</h3>
          <ul className={styles.side} aria-label={t("render.queue")} data-testid="render-queue">
            {jobs.length === 0 && <li className={styles.hint}>{t("render.empty")}</li>}
            {jobs.map((j) => (
              <li key={j.id} data-status={j.status}>
                <span>
                  {j.name} · {j.width}×{j.height} ·{" "}
                  {t(`render.status.${j.status}` as MessageKey, { pass: j.pass, passes: j.passes })}
                </span>
                {(j.status === "waiting" || j.status === "running") && (
                  <button
                    type="button"
                    onClick={() => cancel(j.id)}
                    aria-label={t("render.cancel")}
                  >
                    ×
                  </button>
                )}
                {j.error && (
                  <p className={styles.warning} role="alert">
                    {j.error}
                  </p>
                )}
              </li>
            ))}
          </ul>
          {jobs.some((j) => j.status !== "waiting" && j.status !== "running") && (
            <Button variant="outline" onClick={clearFinished}>
              {t("render.clear")}
            </Button>
          )}
        </div>
        <div className={styles.preview}>
          <Gallery shots={doc.renders ?? []} />
        </div>
      </div>
    </Dialog>
  );
}

/** Готовые кадры проекта. */
function Gallery({ shots }: { shots: RenderShot[] }) {
  const [urls, setUrls] = useState<Record<string, string>>({});
  useEffect(() => {
    let cancelled = false;
    const made: string[] = [];
    void Promise.all(
      shots.map(async (s) => {
        const bytes = await callEngine({ type: "assetBytes", path: s.asset });
        if (!bytes) return null;
        const url = URL.createObjectURL(new Blob([bytes as BlobPart], { type: "image/png" }));
        made.push(url);
        return [s.id, url] as const;
      }),
    ).then((pairs) => {
      if (!cancelled)
        setUrls(
          Object.fromEntries(pairs.filter((p): p is readonly [string, string] => p !== null)),
        );
    });
    return () => {
      cancelled = true;
      for (const u of made) URL.revokeObjectURL(u);
    };
  }, [shots]);
  if (shots.length === 0) return <p className={styles.hint}>{t("render.noShots")}</p>;
  const save = async (s: RenderShot) => {
    const bytes = await callEngine({ type: "assetBytes", path: s.asset });
    if (bytes) await platform.saveExport(bytes, `${s.name} — ${s.width}×${s.height}.png`);
  };
  return (
    <ul className={styles.side} data-testid="render-gallery">
      {[...shots].reverse().map((s) => (
        <li key={s.id} data-shot={s.id}>
          {urls[s.id] && (
            <img
              className={styles.page}
              src={urls[s.id]}
              alt={s.name}
              style={{ aspectRatio: `${s.width} / ${s.height}` }}
            />
          )}
          <div className={styles.formats}>
            <span className={styles.hint}>
              {s.name} · {s.width}×{s.height} · {s.date}
            </span>
            <Button variant="outline" onClick={() => void save(s)}>
              {t("render.save")}
            </Button>
            <Button
              variant="outline"
              onClick={() => sendToEngine({ type: "deleteRender", id: s.id })}
            >
              {t("render.delete")}
            </Button>
          </div>
        </li>
      ))}
    </ul>
  );
}
