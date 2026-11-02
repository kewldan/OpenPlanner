import type {
  EstimateLine,
  EstimateReport,
  PriceItem,
  PriceKind,
  TableFormat,
} from "@openplanner/engine-client";
import { Button, Dialog, Segmented } from "@openplanner/ui";
import { useState } from "react";
import { sendToEngine } from "../engine";
import { t } from "../i18n";
import { formatDecimal, parseNumber } from "../inspector/format";
import { useProject } from "../project";
import { useEditor } from "../store/editor";
import styles from "./EstimateDialog.module.css";
import {
  formatMoney,
  importPrices,
  measureKey,
  measurePresets,
  newPriceItem,
  saveTable,
  setOverrides,
  setPrices,
  today,
  withOverride,
  withPrice,
} from "./prices";

type Tab = "estimate" | "purchases" | "prices";
type Grouping = "sections" | "rooms";

/** Число ≥ 0 из поля; иначе — ошибка и `null`. */
function amount(raw: string): number | null {
  const value = parseNumber(raw);
  if (value === null || value < 0) {
    useEditor.setState({ lastError: "введите число не меньше нуля" });
    return null;
  }
  return value;
}

/** Окно «Смета»: смета текущего вида по разделам или помещениям, закупки, прайс. */
export function EstimateDialog() {
  const open = useEditor((s) => s.dialog === "estimate");
  const setDialog = useEditor((s) => s.setDialog);
  const document = useEditor((s) => s.document);
  const report = useEditor((s) => s.estimate);
  const viewName = useEditor((s) => {
    const v = s.view;
    const id = v.type === "variant" ? v.id : undefined;
    return s.summaries.find((x) => x.variant === id)?.name ?? "";
  });
  const project = useProject((s) => s.name) ?? t("project.untitled");
  const [tab, setTab] = useState<Tab>("estimate");
  const [status, setStatus] = useState<{ text: string; error: boolean } | null>(null);

  if (!document) return null;
  const prices = document.prices ?? [];

  const run = async (action: () => Promise<string | null | boolean>) => {
    setStatus(null);
    try {
      const result = await action();
      if (typeof result === "string")
        setStatus({ text: t("estimate.saved", { name: result }), error: false });
    } catch (error) {
      setStatus({ text: error instanceof Error ? error.message : String(error), error: true });
    }
  };
  const save = (what: "estimate" | "prices", format: TableFormat) =>
    void run(() => saveTable(what, format, project));

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => setDialog(next ? "estimate" : null)}
      title={t("estimate.title")}
      width={1100}
    >
      <div className={styles.root} data-testid="estimate">
        <div className={styles.bar}>
          <Segmented
            label={t("estimate.tabs")}
            value={tab}
            options={[
              { value: "estimate", label: t("estimate.tab.estimate"), disabled: false },
              { value: "purchases", label: t("estimate.tab.purchases"), disabled: false },
              { value: "prices", label: t("estimate.tab.prices"), disabled: false },
            ]}
            onChange={setTab}
          />
          <span className={styles.view}>{viewName}</span>
          {report && (
            <strong className={styles.total} data-testid="estimate-total">
              {formatMoney(report.total)}
            </strong>
          )}
        </div>
        {status && (
          <p
            className={status.error ? styles.warning : styles.status}
            role={status.error ? "alert" : "status"}
          >
            {status.text}
          </p>
        )}
        <div className={styles.body}>
          {tab === "prices" ? (
            <Prices
              prices={prices}
              onImport={() => void run(importPrices)}
              onSave={(format) => save("prices", format)}
            />
          ) : prices.length === 0 ? (
            <div className={styles.empty}>
              <p>{t("estimate.empty")}</p>
              <Button variant="primary" onClick={() => sendToEngine({ type: "defaultPrices" })}>
                {t("estimate.defaultPrices")}
              </Button>
              <Button variant="outline" onClick={() => void run(importPrices)}>
                {t("estimate.import")}
              </Button>
            </div>
          ) : report && tab === "purchases" ? (
            <Purchases report={report} />
          ) : report ? (
            <Estimate report={report} onSave={(format) => save("estimate", format)} />
          ) : null}
        </div>
      </div>
    </Dialog>
  );
}

/** Смета: строки по разделам или помещениям, ручные объёмы, итоги, экспорт. */
function Estimate({
  report,
  onSave,
}: {
  report: EstimateReport;
  onSave: (format: TableFormat) => void;
}) {
  const overrides = useEditor((s) => s.document?.overrides ?? []);
  const [grouping, setGrouping] = useState<Grouping>("sections");
  const groups = grouping === "sections" ? report.sections : report.rooms;
  const of = (l: EstimateLine) => (grouping === "sections" ? l.section : l.roomName);
  return (
    <>
      <div className={styles.bar}>
        <Segmented
          label={t("estimate.grouping")}
          value={grouping}
          options={[
            { value: "sections", label: t("estimate.bySection"), disabled: false },
            { value: "rooms", label: t("estimate.byRoom"), disabled: false },
          ]}
          onChange={setGrouping}
        />
        <div className={styles.spacer} />
        <Button variant="outline" onClick={() => onSave("xlsx")}>
          {t("estimate.exportXlsx")}
        </Button>
        <Button variant="outline" onClick={() => onSave("csv")}>
          {t("estimate.exportCsv")}
        </Button>
      </div>
      <table className={styles.table} data-testid="estimate-lines">
        <thead>
          <tr>
            <th>{t("estimate.name")}</th>
            <th>{grouping === "sections" ? t("estimate.room") : t("estimate.section")}</th>
            <th>{t("estimate.unit")}</th>
            <th className={styles.num}>{t("estimate.quantity")}</th>
            <th className={styles.num}>{t("estimate.price")}</th>
            <th className={styles.num}>{t("estimate.cost")}</th>
          </tr>
        </thead>
        {groups.map((g) => (
          <tbody key={g.name} data-group={g.name}>
            <tr className={styles.group}>
              <th colSpan={5}>{g.name}</th>
              <th className={styles.num}>{formatMoney(g.total)}</th>
            </tr>
            {report.lines
              .filter((l) => of(l) === g.name)
              .map((l) => (
                <tr key={`${l.item}:${l.room ?? ""}`} data-line={l.code || l.name}>
                  <td>
                    {l.name}
                    {l.kind === "material" && (
                      <span className={styles.tag}>{t("estimate.material")}</span>
                    )}
                  </td>
                  <td>{grouping === "sections" ? l.roomName : l.section}</td>
                  <td>{l.unit}</td>
                  <td className={styles.num}>
                    <input
                      className={styles.input}
                      aria-label={t("estimate.quantityOf", { name: l.name })}
                      data-overridden={l.overridden}
                      title={t("estimate.measured", { value: formatDecimal(l.measured) })}
                      defaultValue={formatDecimal(l.quantity)}
                      key={`${l.quantity}`}
                      onBlur={(e) => {
                        const value = amount(e.target.value);
                        if (value !== null && value !== l.quantity)
                          setOverrides(withOverride(overrides, l.item, l.room, value));
                      }}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") e.currentTarget.blur();
                      }}
                    />
                    {l.overridden && (
                      <button
                        type="button"
                        className={styles.reset}
                        aria-label={t("estimate.reset", { name: l.name })}
                        title={t("estimate.reset", { name: l.name })}
                        onClick={() => setOverrides(withOverride(overrides, l.item, l.room, null))}
                      >
                        ↺
                      </button>
                    )}
                  </td>
                  <td className={styles.num}>{formatMoney(l.price)}</td>
                  <td className={styles.num}>{formatMoney(l.cost)}</td>
                </tr>
              ))}
          </tbody>
        ))}
        <tfoot>
          <tr>
            <th colSpan={5}>{t("estimate.works")}</th>
            <th className={styles.num}>{formatMoney(report.works)}</th>
          </tr>
          <tr>
            <th colSpan={5}>{t("estimate.materials")}</th>
            <th className={styles.num}>{formatMoney(report.materials)}</th>
          </tr>
          <tr>
            <th colSpan={5}>{t("estimate.totalLabel")}</th>
            <th className={styles.num}>{formatMoney(report.total)}</th>
          </tr>
        </tfoot>
      </table>
    </>
  );
}

/** Список закупок: материалы сметы по позициям. */
function Purchases({ report }: { report: EstimateReport }) {
  if (report.purchases.length === 0)
    return <p className={styles.hint}>{t("estimate.noPurchases")}</p>;
  const total = report.purchases.reduce((sum, p) => sum + p.cost, 0);
  return (
    <table className={styles.table} data-testid="purchases">
      <thead>
        <tr>
          <th>{t("estimate.code")}</th>
          <th>{t("estimate.name")}</th>
          <th>{t("estimate.unit")}</th>
          <th className={styles.num}>{t("estimate.quantity")}</th>
          <th className={styles.num}>{t("estimate.price")}</th>
          <th className={styles.num}>{t("estimate.cost")}</th>
        </tr>
      </thead>
      <tbody>
        {report.purchases.map((p) => (
          <tr key={p.item}>
            <td>{p.code}</td>
            <td>{p.name}</td>
            <td>{p.unit}</td>
            <td className={styles.num}>{formatDecimal(p.quantity)}</td>
            <td className={styles.num}>{formatMoney(p.price)}</td>
            <td className={styles.num}>{formatMoney(p.cost)}</td>
          </tr>
        ))}
      </tbody>
      <tfoot>
        <tr>
          <th colSpan={5}>{t("estimate.totalLabel")}</th>
          <th className={styles.num}>{formatMoney(total)}</th>
        </tr>
      </tfoot>
    </table>
  );
}

/** Прайс проекта: позиции, правила объёма, нормы, запас, история цен, импорт и экспорт. */
function Prices({
  prices,
  onImport,
  onSave,
}: {
  prices: PriceItem[];
  onImport: () => void;
  onSave: (format: TableFormat) => void;
}) {
  const update = (next: PriceItem) => setPrices(prices.map((p) => (p.id === next.id ? next : p)));
  const text = (p: PriceItem, field: "code" | "section" | "name" | "unit", width: string) => (
    <input
      className={styles.input}
      style={{ width }}
      aria-label={t(`estimate.${field}`)}
      defaultValue={p[field]}
      key={`${field}${p[field]}`}
      onBlur={(e) => {
        const value = e.target.value.trim();
        if (value !== p[field] && (value || field === "code")) update({ ...p, [field]: value });
      }}
    />
  );
  const number = (value: number, apply: (v: number) => PriceItem, label: string) => (
    <input
      className={styles.input}
      style={{ width: "64px" }}
      aria-label={label}
      defaultValue={formatDecimal(value)}
      key={`${label}${value}`}
      onBlur={(e) => {
        const v = amount(e.target.value);
        if (v !== null && v !== value) update(apply(v));
      }}
    />
  );
  return (
    <>
      <div className={styles.bar}>
        <Button
          variant="outline"
          onClick={() => setPrices([...prices, newPriceItem(prices.length + 1)])}
        >
          {t("estimate.addItem")}
        </Button>
        <Button variant="outline" onClick={() => sendToEngine({ type: "defaultPrices" })}>
          {t("estimate.defaultPrices")}
        </Button>
        <div className={styles.spacer} />
        <Button variant="outline" onClick={onImport}>
          {t("estimate.import")}
        </Button>
        <Button variant="outline" onClick={() => onSave("xlsx")}>
          {t("estimate.pricesXlsx")}
        </Button>
        <Button variant="outline" onClick={() => onSave("csv")}>
          {t("estimate.pricesCsv")}
        </Button>
      </div>
      <p className={styles.hint}>{t("estimate.pricesHint")}</p>
      <table className={styles.table} data-testid="prices">
        <thead>
          <tr>
            <th>{t("estimate.code")}</th>
            <th>{t("estimate.section")}</th>
            <th>{t("estimate.name")}</th>
            <th>{t("estimate.kind")}</th>
            <th>{t("estimate.unit")}</th>
            <th>{t("estimate.price")}</th>
            <th>{t("estimate.measure")}</th>
            <th>{t("estimate.rate")}</th>
            <th>{t("estimate.reserve")}</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {prices.map((p) => {
            const key = measureKey(p.measure);
            const known = measurePresets.some((m) => measureKey(m.measure) === key);
            const last = p.history?.at(-1);
            return (
              <tr key={p.id} data-price={p.code || p.name}>
                <td>{text(p, "code", "56px")}</td>
                <td>{text(p, "section", "96px")}</td>
                <td>{text(p, "name", "240px")}</td>
                <td>
                  <select
                    className={styles.input}
                    aria-label={t("estimate.kind")}
                    value={p.kind}
                    onChange={(e) => update({ ...p, kind: e.target.value as PriceKind })}
                  >
                    <option value="work">{t("estimate.work")}</option>
                    <option value="material">{t("estimate.material")}</option>
                  </select>
                </td>
                <td>{text(p, "unit", "44px")}</td>
                <td>
                  {number(
                    p.price,
                    (v) => withPrice(p, v, today()),
                    t("estimate.priceOf", { name: p.name }),
                  )}
                  {last && (
                    <span
                      className={styles.history}
                      title={(p.history ?? [])
                        .map((h) => `${h.date}: ${formatMoney(h.price)}`)
                        .join("\n")}
                    >
                      {t("estimate.was", { price: formatMoney(last.price), date: last.date })}
                    </span>
                  )}
                </td>
                <td>
                  <select
                    className={styles.input}
                    aria-label={t("estimate.measure")}
                    value={key}
                    onChange={(e) => {
                      const preset = measurePresets.find(
                        (m) => measureKey(m.measure) === e.target.value,
                      );
                      if (preset) update({ ...p, measure: preset.measure });
                    }}
                  >
                    {!known && <option value={key}>{key}</option>}
                    {measurePresets.map((m) => (
                      <option key={measureKey(m.measure)} value={measureKey(m.measure)}>
                        {m.label}
                      </option>
                    ))}
                  </select>
                </td>
                <td>{number(p.rate, (rate) => ({ ...p, rate }), t("estimate.rate"))}</td>
                <td>
                  {number(
                    Math.round((p.reserve ?? 0) * 1000) / 10,
                    (v) => ({ ...p, reserve: v / 100 }),
                    t("estimate.reserve"),
                  )}
                </td>
                <td>
                  <button
                    type="button"
                    className={styles.reset}
                    aria-label={t("estimate.removeItem", { name: p.name })}
                    onClick={() => setPrices(prices.filter((x) => x.id !== p.id))}
                  >
                    ×
                  </button>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </>
  );
}
