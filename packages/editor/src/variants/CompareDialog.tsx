import type { ViewSummary, WallMaterial } from "@openplanner/engine-client";
import { Button, Dialog } from "@openplanner/ui";
import { useState } from "react";
import { type MessageKey, t } from "../i18n";
import { formatArea, formatDecimal } from "../inspector/format";
import { useEditor } from "../store/editor";
import { showCompare } from "./actions";
import styles from "./CompareDialog.module.css";

const materialKeys: Record<WallMaterial, MessageKey> = {
  aeratedConcrete: "material.aeratedConcrete",
  gypsum: "material.gypsum",
  brick: "material.brick",
};

const key = (s: ViewSummary) => s.variant ?? "";

/** Сравнение двух видов: площади помещений, изменения, показ рядом. */
export function CompareDialog() {
  const open = useEditor((s) => s.dialog === "compare");
  const summaries = useEditor((s) => s.summaries);
  const setDialog = useEditor((s) => s.setDialog);
  const [left, setLeft] = useState("");
  const [right, setRight] = useState<string | null>(null);
  const rightKey = right ?? key(summaries[1] ?? summaries[0] ?? ({} as ViewSummary));
  const a = summaries.find((s) => key(s) === left) ?? summaries[0];
  const b = summaries.find((s) => key(s) === rightKey) ?? summaries[1] ?? summaries[0];

  const select = (value: string, onChange: (v: string) => void, label: string) => (
    <label className={styles.pick}>
      <span>{label}</span>
      <select value={value} onChange={(event) => onChange(event.target.value)} aria-label={label}>
        {summaries.map((s) => (
          <option key={key(s)} value={key(s)}>
            {s.name}
          </option>
        ))}
      </select>
    </label>
  );

  // Строки — помещения обоих видов по идентификатору.
  const rows = new Map<string, { name: string; a?: number; b?: number }>();
  for (const r of a?.rooms ?? []) rows.set(r.id, { name: r.name, a: r.area });
  for (const r of b?.rooms ?? []) {
    const row = rows.get(r.id) ?? { name: r.name };
    rows.set(r.id, { ...row, b: r.area });
  }
  const area = (v: number | undefined) => (v === undefined ? "—" : formatArea(v));
  const delta = (x: number | undefined, y: number | undefined) => {
    if (x === undefined || y === undefined) return "—";
    const d = y - x;
    if (Math.abs(d) < 0.005) return "0";
    return `${d > 0 ? "+" : "−"}${formatDecimal(Math.abs(d))}`;
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => setDialog(next ? "compare" : null)}
      title={t("compare.title")}
      width={760}
    >
      <div className={styles.root} data-testid="compare">
        <div className={styles.picks}>
          {select(key(a ?? summaries[0] ?? ({} as ViewSummary)), setLeft, t("compare.left"))}
          {select(rightKey, setRight, t("compare.right"))}
        </div>
        {a && b && (
          <>
            <table className={styles.table}>
              <thead>
                <tr>
                  <th>{t("compare.room")}</th>
                  <th>{a.name}</th>
                  <th>{b.name}</th>
                  <th>{t("compare.delta")}</th>
                </tr>
              </thead>
              <tbody>
                {[...rows.entries()].map(([id, row]) => (
                  <tr key={id}>
                    <td>{row.name}</td>
                    <td>{area(row.a)}</td>
                    <td>{area(row.b)}</td>
                    <td>{delta(row.a, row.b)}</td>
                  </tr>
                ))}
              </tbody>
              <tfoot>
                <tr>
                  <th>{t("compare.total")}</th>
                  <th>{formatArea(a.totalArea)}</th>
                  <th>{formatArea(b.totalArea)}</th>
                  <th>{delta(a.totalArea, b.totalArea)}</th>
                </tr>
              </tfoot>
            </table>
            <div className={styles.changes}>
              <Changes summary={a} />
              <Changes summary={b} />
            </div>
          </>
        )}
        <div className={styles.actions}>
          <Button
            variant="primary"
            disabled={!a || !b}
            onClick={() => showCompare(a?.variant ?? null, b?.variant ?? null)}
          >
            {t("compare.show")}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}

function Changes({ summary }: { summary: ViewSummary }) {
  const amount = (length: number, area: number) =>
    t("compare.amount", { length: formatDecimal(length), area: formatDecimal(area) });
  const empty =
    summary.demolishedLength === 0 &&
    summary.newWalls.length === 0 &&
    summary.openingsAdded === 0 &&
    summary.openingsClosed === 0;
  return (
    <section className={styles.changeList}>
      <h3>
        {t("compare.changes")} · {summary.name}
      </h3>
      {empty ? (
        <p>{t("compare.none")}</p>
      ) : (
        <dl>
          {summary.demolishedLength > 0 && (
            <>
              <dt>{t("compare.demolished")}</dt>
              <dd>{amount(summary.demolishedLength, summary.demolishedArea)}</dd>
            </>
          )}
          {summary.newWalls.map((m) => (
            <div key={m.material ?? "none"} className={styles.pair}>
              <dt>
                {t("compare.newWalls")}:{" "}
                {t(m.material ? materialKeys[m.material] : "material.none")}
              </dt>
              <dd>{amount(m.length, m.area)}</dd>
            </div>
          ))}
          {summary.openingsAdded > 0 && (
            <>
              <dt>{t("compare.openingsAdded")}</dt>
              <dd>{summary.openingsAdded}</dd>
            </>
          )}
          {summary.openingsClosed > 0 && (
            <>
              <dt>{t("compare.openingsClosed")}</dt>
              <dd>{summary.openingsClosed}</dd>
            </>
          )}
        </dl>
      )}
    </section>
  );
}
