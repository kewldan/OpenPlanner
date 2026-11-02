import type { ScanIssue, ScanResult } from "@openplanner/engine-client";
import { Button, Dialog, Field } from "@openplanner/ui";
import { useEffect, useMemo, useRef, useState } from "react";
import { formatNumber, type MessageKey, t } from "../i18n";
import { parseNumber } from "../inspector/format";
import { useEditor } from "../store/editor";
import { type Answers, isAccepted, percent, removedWalls } from "./decisions";
import styles from "./ScanWizard.module.css";
import {
  cancelScan,
  closeScanWizard,
  importScan,
  pickScanFile,
  runScan,
  type ScanStep,
  setAnswer,
  setParams,
  setStep,
  startEmpty,
  useScan,
} from "./store";

const steps: readonly { step: ScanStep; key: MessageKey }[] = [
  { step: "file", key: "scan.stepFile" },
  { step: "processing", key: "scan.stepProcessing" },
  { step: "review", key: "scan.stepReview" },
  { step: "params", key: "scan.stepParams" },
];

const stageKeys: readonly MessageKey[] = [
  "scan.stageReading",
  "scan.stageSurfaces",
  "scan.stageLevels",
  "scan.stageWalls",
  "scan.stageOpenings",
  "scan.stageRaster",
];

/** Мастер импорта скана: Файл → Обработка → Проверка → Параметры. */
export function ScanWizard() {
  const open = useEditor((s) => s.dialog === "scan");
  const step = useScan((s) => s.step);
  const current = step === "failure" ? "review" : step;
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) closeScanWizard();
      }}
      title={t("scan.title")}
      width={880}
    >
      <ol className={styles.steps} aria-label={t("scan.steps")}>
        {steps.map((s, i) => (
          <li key={s.step} aria-current={s.step === current ? "step" : undefined}>
            <span className={styles.stepNumber}>{i + 1}</span>
            {t(s.key)}
          </li>
        ))}
      </ol>
      <div className={styles.content} data-testid="scan-wizard">
        {step === "file" && <FileStep />}
        {step === "processing" && <ProcessingStep />}
        {step === "review" && <ReviewStep />}
        {step === "params" && <ParamsStep />}
        {step === "failure" && <FailureStep />}
      </div>
    </Dialog>
  );
}

function ErrorText() {
  const error = useScan((s) => s.error);
  return error ? (
    <p className={styles.error} role="alert">
      {error}
    </p>
  ) : null;
}

function FileStep() {
  const [over, setOver] = useState(false);
  const zone = useRef<HTMLDivElement>(null);
  // Перетаскивание файла: обработчики на DOM-узле зоны.
  useEffect(() => {
    const element = zone.current;
    if (!element) return;
    const onOver = (event: DragEvent) => {
      event.preventDefault();
      setOver(true);
    };
    const onLeave = () => setOver(false);
    const onDrop = async (event: DragEvent) => {
      event.preventDefault();
      setOver(false);
      const file = event.dataTransfer?.files[0];
      if (file) runScan(new Uint8Array(await file.arrayBuffer()), file.name);
    };
    element.addEventListener("dragover", onOver);
    element.addEventListener("dragleave", onLeave);
    element.addEventListener("drop", onDrop);
    return () => {
      element.removeEventListener("dragover", onOver);
      element.removeEventListener("dragleave", onLeave);
      element.removeEventListener("drop", onDrop);
    };
  }, []);
  return (
    <div ref={zone} className={[styles.drop, over && styles.dropOver].filter(Boolean).join(" ")}>
      <p className={styles.lead}>{t("scan.fileLead")}</p>
      <p className={styles.muted}>{t("scan.fileFormats")}</p>
      <Button variant="primary" size="lg" onClick={() => void pickScanFile()}>
        {t("scan.pick")}
      </Button>
      <ErrorText />
    </div>
  );
}

function ProcessingStep() {
  const name = useScan((s) => s.name);
  const stage = useScan((s) => s.stage);
  const count = useScan((s) => s.count);
  const key = stageKeys[Math.min(stage, stageKeys.length - 1)] ?? "scan.stageReading";
  return (
    <div className={styles.center}>
      <p className={styles.lead}>{name}</p>
      <progress className={styles.progress} max={count} value={stage + 0.5} aria-label={t(key)} />
      <p className={styles.muted} data-testid="scan-stage">
        {t(key)}
      </p>
      <Button variant="outline" onClick={cancelScan}>
        {t("scan.cancel")}
      </Button>
    </div>
  );
}

function issueText(issue: ScanIssue, result: ScanResult): { title: string; detail: string } {
  switch (issue.kind.type) {
    case "extraWall": {
      const id = issue.kind.wall;
      const wall = result.walls.find((w) => w.id === id);
      const length = wall ? Math.hypot(wall.b.x - wall.a.x, wall.b.y - wall.a.y) : 0;
      return {
        title: t("scan.issueExtra"),
        detail: t("scan.issueExtraDetail", { length: formatNumber(length) }),
      };
    }
    case "missingWall": {
      const { a, b } = issue.kind;
      return {
        title: t("scan.issueMissing"),
        detail: t("scan.issueMissingDetail", {
          length: formatNumber(Math.hypot(b.x - a.x, b.y - a.y)),
        }),
      };
    }
    case "doorOrArch": {
      const id = issue.kind.opening;
      const opening = result.openings.find((o) => o.id === id);
      return {
        title: t("scan.issueDoor"),
        detail: t("scan.issueDoorDetail", {
          width: formatNumber(opening?.width ?? 0),
          height: formatNumber(opening?.height ?? 0),
        }),
      };
    }
  }
}

const choiceKeys: Record<ScanIssue["kind"]["type"], [MessageKey, MessageKey]> = {
  extraWall: ["scan.remove", "scan.keep"],
  missingWall: ["scan.add", "scan.skip"],
  doorOrArch: ["scan.door", "scan.arch"],
};

function ReviewStep() {
  const result = useScan((s) => s.result);
  const answers = useScan((s) => s.answers);
  const highlight = useScan((s) => s.highlight);
  if (!result) return null;
  return (
    <div className={styles.review}>
      <Preview result={result} answers={answers} highlight={highlight} />
      <div className={styles.side}>
        <p className={styles.summary} data-testid="scan-summary">
          {t("scan.summary", {
            walls: result.walls.length - removedWalls(result, answers).size,
            openings: result.openings.length,
          })}
        </p>
        {result.issues.length === 0 ? (
          <p className={styles.muted}>{t("scan.noIssues")}</p>
        ) : (
          <ul className={styles.issues} aria-label={t("scan.issues")}>
            {result.issues.map((issue) => {
              const { title, detail } = issueText(issue, result);
              const accepted = isAccepted(issue, answers);
              const [yes, no] = choiceKeys[issue.kind.type];
              return (
                <li
                  key={issue.id}
                  className={styles.issue}
                  data-highlight={highlight === issue.id}
                  onPointerEnter={() => useScan.setState({ highlight: issue.id })}
                  onPointerLeave={() => useScan.setState({ highlight: null })}
                >
                  <div className={styles.issueHead}>
                    <span className={styles.issueNumber}>{issue.id + 1}</span>
                    <span className={styles.issueTitle}>{title}</span>
                    <span className={styles.confidence}>
                      {t("scan.confidence", { percent: percent(issue.confidence) })}
                    </span>
                  </div>
                  <p className={styles.muted}>{detail}</p>
                  <fieldset className={styles.choice} aria-label={title}>
                    <button
                      type="button"
                      aria-pressed={accepted}
                      onClick={() => setAnswer(issue.id, true)}
                    >
                      {t(yes)}
                    </button>
                    <button
                      type="button"
                      aria-pressed={!accepted}
                      onClick={() => setAnswer(issue.id, false)}
                    >
                      {t(no)}
                    </button>
                  </fieldset>
                </li>
              );
            })}
          </ul>
        )}
        <div className={styles.actions}>
          <Button variant="outline" onClick={() => setStep("file")}>
            {t("scan.back")}
          </Button>
          <Button variant="primary" onClick={() => setStep("params")}>
            {t("scan.next")}
          </Button>
        </div>
      </div>
    </div>
  );
}

/** План распознанного скана поверх вида сверху. */
function Preview({
  result,
  answers,
  highlight,
}: {
  result: ScanResult;
  answers: Answers;
  highlight: number | null;
}) {
  const raster = result.raster;
  const url = useMemo(
    () =>
      raster
        ? URL.createObjectURL(new Blob([raster.png as BlobPart], { type: "image/png" }))
        : null,
    [raster],
  );
  useEffect(
    () => () => {
      if (url) URL.revokeObjectURL(url);
    },
    [url],
  );
  const removed = removedWalls(result, answers);
  const points = result.walls.flatMap((w) => [w.a, w.b]);
  const box = raster
    ? {
        x: raster.x,
        y: raster.y,
        w: raster.widthPx * raster.mmPerPx,
        h: raster.heightPx * raster.mmPerPx,
      }
    : (() => {
        const xs = points.map((p) => p.x);
        const ys = points.map((p) => p.y);
        const x = Math.min(...xs, 0) - 500;
        const y = Math.min(...ys, 0) - 500;
        return { x, y, w: Math.max(...xs, 1000) - x + 500, h: Math.max(...ys, 1000) - y + 500 };
      })();
  return (
    <svg
      className={styles.preview}
      viewBox={`${box.x} ${box.y} ${box.w} ${box.h}`}
      role="img"
      aria-label={t("scan.preview")}
      data-testid="scan-preview"
    >
      {url && raster && (
        <image href={url} x={raster.x} y={raster.y} width={box.w} height={box.h} opacity={0.75} />
      )}
      {result.walls.map((w) => (
        <line
          key={w.id}
          x1={w.a.x}
          y1={w.a.y}
          x2={w.b.x}
          y2={w.b.y}
          className={removed.has(w.id) ? styles.removed : styles.wall}
          strokeWidth={Math.max(w.thickness, 60)}
        />
      ))}
      {result.openings.map((o) => (
        <circle
          key={o.id}
          cx={o.center.x}
          cy={o.center.y}
          r={Math.max(o.width / 2, 200)}
          className={styles.opening}
        />
      ))}
      {result.issues.map((issue) =>
        issue.kind.type === "missingWall" && isAccepted(issue, answers) ? (
          <line
            key={`m${issue.id}`}
            x1={issue.kind.a.x}
            y1={issue.kind.a.y}
            x2={issue.kind.b.x}
            y2={issue.kind.b.y}
            className={styles.added}
            strokeWidth={120}
          />
        ) : null,
      )}
      {result.issues.map((issue) => (
        <g key={`i${issue.id}`} className={styles.marker} data-highlight={highlight === issue.id}>
          <circle cx={issue.at.x} cy={issue.at.y} r={260} />
          <text x={issue.at.x} y={issue.at.y} dy="0.35em" textAnchor="middle">
            {issue.id + 1}
          </text>
        </g>
      ))}
    </svg>
  );
}

function NumberField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  const [draft, setDraft] = useState(String(value));
  return (
    <Field
      label={label}
      unit={t("unit.mm")}
      numeric
      inputMode="decimal"
      value={draft}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={() => {
        const parsed = parseNumber(draft);
        if (parsed !== null && parsed > 0) onChange(parsed);
        else setDraft(String(value));
      }}
    />
  );
}

function Check({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <label className={styles.check}>
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
      {label}
    </label>
  );
}

function ParamsStep() {
  const params = useScan((s) => s.params);
  const result = useScan((s) => s.result);
  const busy = useScan((s) => s.busy);
  if (!params || !result) return null;
  return (
    <div className={styles.params}>
      <NumberField
        label={t("scan.ceiling")}
        value={params.ceilingHeight}
        onChange={(ceilingHeight) => setParams({ ceilingHeight })}
      />
      {!result.ceilingDetected && <p className={styles.muted}>{t("scan.ceilingDefault")}</p>}
      <NumberField
        label={t("scan.singleThickness")}
        value={params.singleThickness}
        onChange={(singleThickness) => setParams({ singleThickness })}
      />
      <p className={styles.muted}>{t("scan.singleHint")}</p>
      <NumberField
        label={t("scan.addedThickness")}
        value={params.addedThickness}
        onChange={(addedThickness) => setParams({ addedThickness })}
      />
      <Check
        label={t("scan.createRooms")}
        checked={params.createRooms}
        onChange={(createRooms) => setParams({ createRooms })}
      />
      <Check
        label={t("scan.underlay")}
        checked={params.underlay}
        onChange={(underlay) => setParams({ underlay })}
      />
      <Check
        label={t("scan.keepSource")}
        checked={params.keepSource}
        onChange={(keepSource) => setParams({ keepSource })}
      />
      <ErrorText />
      <div className={styles.actions}>
        <Button variant="outline" onClick={() => setStep("review")}>
          {t("scan.back")}
        </Button>
        <Button variant="primary" disabled={busy} onClick={() => void importScan("plan")}>
          {t("scan.import")}
        </Button>
      </div>
    </div>
  );
}

function FailureStep() {
  const result = useScan((s) => s.result);
  const busy = useScan((s) => s.busy);
  if (!result) return null;
  return (
    <div className={styles.failure} role="alert">
      <p className={styles.lead}>
        {t(result.failure === "noFloor" ? "scan.failNoFloor" : "scan.failWalls")}
      </p>
      <p className={styles.muted}>{t("scan.failHint")}</p>
      <div className={styles.failureActions}>
        <Button variant="outline" onClick={closeScanWizard}>
          {t("scan.rescan")}
        </Button>
        {result.raster && (
          <Button variant="primary" disabled={busy} onClick={() => void importScan("underlayOnly")}>
            {t("scan.trace")}
          </Button>
        )}
        <Button variant="outline" onClick={() => void startEmpty()}>
          {t("scan.empty")}
        </Button>
      </div>
      <ErrorText />
    </div>
  );
}
