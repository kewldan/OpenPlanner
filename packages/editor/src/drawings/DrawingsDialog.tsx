import type { DrawingSetInfo, DrawingSettings, PaperFormat } from "@openplanner/engine-client";
import { Button, Dialog } from "@openplanner/ui";
import { useEffect, useMemo, useState } from "react";
import { callEngine, sendToEngine } from "../engine";
import { type MessageKey, t } from "../i18n";
import { useEditor } from "../store/editor";
import styles from "./DrawingsDialog.module.css";
import { drawingRequest, type ExportKind, exportDrawings } from "./export";

const papers: readonly PaperFormat[] = ["a3Landscape", "a3Portrait", "a4Landscape", "a4Portrait"];

/** Масштабы, которые можно выбрать вручную (ГОСТ 2.302). */
const scales = [20, 25, 50, 75, 100, 200] as const;

const exports: readonly { kind: ExportKind; label: MessageKey }[] = [
  { kind: "pdf", label: "drawings.pdf" },
  { kind: "dxf", label: "drawings.dxf" },
  { kind: "svg", label: "drawings.svg" },
  { kind: "png", label: "drawings.png" },
  { kind: "xlsx", label: "drawings.xlsx" },
  { kind: "ifc", label: "drawings.ifc" },
];

/** Вариант, с которого открывается окно: текущий, последний открытый или обмер. */
function initialVariant(): string | null {
  const { view, lastVariant, summaries } = useEditor.getState();
  if (view.type === "variant") return view.id;
  return summaries.some((s) => s.variant === lastVariant) ? lastVariant : null;
}

function setDrawing(drawing: DrawingSettings) {
  sendToEngine({ type: "command", command: { type: "setDrawing", drawing } });
}

/** Окно «Чертежи»: комплект листов из модели, формат, масштаб и экспорт. */
export function DrawingsDialog() {
  const open = useEditor((s) => s.dialog === "drawings");
  const setDialog = useEditor((s) => s.setDialog);
  const document = useEditor((s) => s.document);
  const summaries = useEditor((s) => s.summaries);
  const [variant, setVariant] = useState<string | null>(null);
  const [info, setInfo] = useState<DrawingSetInfo | null>(null);
  const [selected, setSelected] = useState(0);
  const [busy, setBusy] = useState<ExportKind | null>(null);
  const [status, setStatus] = useState<{ text: string; error: boolean } | null>(null);

  useEffect(() => {
    if (!open) return;
    setVariant(initialVariant());
    setSelected(0);
    setStatus(null);
  }, [open]);

  // Листы пересобираются при каждом изменении документа и выбранного вида.
  useEffect(() => {
    if (!open || !document) return;
    let cancelled = false;
    callEngine({ type: "drawingSet", request: drawingRequest(variant) })
      .then((next) => {
        if (!cancelled) setInfo(next);
      })
      .catch((error: unknown) => {
        if (!cancelled) setStatus({ text: String(error), error: true });
      });
    return () => {
      cancelled = true;
    };
  }, [open, document, variant]);

  const urls = useMemo(
    () =>
      (info?.sheets ?? []).map((s) =>
        URL.createObjectURL(new Blob([s.svg], { type: "image/svg+xml" })),
      ),
    [info],
  );
  useEffect(
    () => () => {
      for (const url of urls) URL.revokeObjectURL(url);
    },
    [urls],
  );

  if (!document) return null;
  const drawing: DrawingSettings = document.drawing ?? { paper: "a3Landscape" };
  const sheets = info?.sheets ?? [];
  const current = Math.min(selected, Math.max(sheets.length - 1, 0));
  const sheet = sheets[current];

  const run = async (kind: ExportKind) => {
    if (!info) return;
    setBusy(kind);
    setStatus(null);
    try {
      const name = await exportDrawings(kind, drawingRequest(variant), info);
      if (name) setStatus({ text: t("drawings.saved", { name }), error: false });
    } catch (error) {
      setStatus({ text: error instanceof Error ? error.message : String(error), error: true });
    } finally {
      setBusy(null);
    }
  };

  const setScale = (value: string) => {
    const { scale: _scale, ...rest } = drawing;
    setDrawing(value === "auto" ? rest : { ...rest, scale: Number(value) });
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => setDialog(next ? "drawings" : null)}
      title={t("drawings.title")}
      width={1040}
    >
      <div className={styles.root} data-testid="drawings">
        <div className={styles.side}>
          <label className={styles.pick}>
            <span>{t("drawings.view")}</span>
            <select
              aria-label={t("drawings.view")}
              value={variant ?? ""}
              onChange={(event) => {
                setVariant(event.target.value || null);
                setSelected(0);
              }}
            >
              {summaries.map((s) => (
                <option key={s.variant ?? ""} value={s.variant ?? ""}>
                  {s.name}
                </option>
              ))}
            </select>
          </label>
          <label className={styles.pick}>
            <span>{t("drawings.paper")}</span>
            <select
              aria-label={t("drawings.paper")}
              value={drawing.paper}
              onChange={(event) =>
                setDrawing({ ...drawing, paper: event.target.value as PaperFormat })
              }
            >
              {papers.map((p) => (
                <option key={p} value={p}>
                  {t(`drawings.paper.${p}`)}
                </option>
              ))}
            </select>
          </label>
          <label className={styles.pick}>
            <span>{t("drawings.scale")}</span>
            <select
              aria-label={t("drawings.scale")}
              value={drawing.scale === undefined ? "auto" : String(drawing.scale)}
              onChange={(event) => setScale(event.target.value)}
            >
              <option value="auto">
                {info?.autoScale
                  ? t("drawings.scaleAutoValue", { denominator: info.scale })
                  : t("drawings.scaleAuto")}
              </option>
              {scales.map((s) => (
                <option key={s} value={s}>
                  1:{s}
                </option>
              ))}
            </select>
          </label>
          <p className={styles.hint}>
            {variant === null ? t("drawings.surveyOnly") : null} {t("drawings.titleBlockHint")}
          </p>
          {info?.warnings.map((w) => (
            <p key={w} className={styles.warning} role="alert">
              {w}
            </p>
          ))}

          <h3 className={styles.heading}>{t("drawings.export")}</h3>
          <Button
            variant="primary"
            size="lg"
            disabled={!info || busy !== null}
            onClick={() => void run("bundle")}
          >
            {busy === "bundle" ? t("drawings.exporting") : t("drawings.bundle")}
          </Button>
          <p className={styles.hint}>{t("drawings.bundleHint")}</p>
          <div className={styles.formats}>
            {exports.map((e) => (
              <Button
                key={e.kind}
                variant="outline"
                disabled={!info || busy !== null}
                onClick={() => void run(e.kind)}
              >
                {busy === e.kind ? t("drawings.exporting") : t(e.label)}
              </Button>
            ))}
          </div>
          {status && (
            <p
              className={status.error ? styles.warning : styles.status}
              role={status.error ? "alert" : "status"}
              data-testid="drawings-status"
            >
              {status.text}
            </p>
          )}
        </div>

        <div className={styles.preview}>
          {sheet ? (
            <img
              className={styles.page}
              src={urls[current]}
              alt={sheet.title}
              style={{ aspectRatio: `${sheet.width} / ${sheet.height}` }}
              data-testid="sheet-preview"
            />
          ) : (
            <p className={styles.hint}>{t("drawings.loading")}</p>
          )}
          <h3 className={styles.heading}>
            {t("drawings.sheets")} · {sheets.length}
          </h3>
          <ol className={styles.thumbs} aria-label={t("drawings.sheets")}>
            {sheets.map((s, i) => (
              <li key={s.title}>
                <button
                  type="button"
                  className={styles.thumb}
                  aria-current={i === current}
                  onClick={() => setSelected(i)}
                >
                  <img src={urls[i]} alt="" />
                  <span>{t("drawings.sheetN", { n: i + 1, title: s.title })}</span>
                </button>
              </li>
            ))}
          </ol>
        </div>
      </div>
    </Dialog>
  );
}
