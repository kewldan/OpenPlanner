import { Button } from "@openplanner/ui";
import styles from "../electric/ElectricalPanel.module.css";
import { sendToEngine } from "../engine";
import { t } from "../i18n";
import { formatDecimal } from "../inspector/format";
import { useEditor } from "../store/editor";
import { systemCodes } from "./points";

/** Вкладка «Трубы»: выводы под приборы, прокладка, проверки, трубы и фитинги. */
export function PlumbingPanel() {
  const report = useEditor((s) => s.plumbing);
  return (
    <div className={styles.root} data-testid="plumbing">
      <section className={styles.section}>
        <h3>{t("plumbing.routing")}</h3>
        <Button variant="outline" onClick={() => sendToEngine({ type: "fixtureOutlets" })}>
          {t("plumbing.fixtureOutlets")}
        </Button>
        <Button variant="outline" onClick={() => sendToEngine({ type: "routePipes" })}>
          {t("plumbing.route")}
        </Button>
        {report && report.unconnected > 0 && (
          <p className={styles.warning}>
            {t("plumbing.unconnected", { count: report.unconnected })}
          </p>
        )}
        {report?.warnings.map((w) => (
          <p key={w} className={styles.warning} role="alert">
            {w}
          </p>
        ))}
      </section>
      {report && report.pipes.length > 0 && (
        <section className={styles.section}>
          <h3>{t("plumbing.pipeList")}</h3>
          <ul className={styles.list} aria-label={t("plumbing.pipeList")}>
            {report.pipes.map((p) => (
              <li key={p.id} className={styles.circuit} data-pipe={p.system}>
                <button
                  type="button"
                  className={styles.link}
                  onClick={() => sendToEngine({ type: "select", ids: [p.id] })}
                >
                  {systemCodes[p.system]} Ø{p.diameter} · {p.label || t("plumbing.unnamed")}
                </button>
                <p className={styles.calc} data-testid="pipe-calc">
                  {t("plumbing.calc", {
                    length: formatDecimal(p.length),
                    slope:
                      p.slope === undefined
                        ? "—"
                        : `${formatDecimal(Math.round(p.slope * 1000) / 10)} %`,
                  })}
                </p>
                {p.warnings.map((w) => (
                  <p key={w} className={styles.warning} role="alert">
                    {w}
                  </p>
                ))}
              </li>
            ))}
          </ul>
        </section>
      )}
      {report && report.bill.length > 0 && (
        <section className={styles.section} data-testid="plumbing-bill">
          <h3>{t("plumbing.bill")}</h3>
          <ul className={styles.rows}>
            {report.bill.map((r) => (
              <li key={r.name}>
                <span>{r.name}</span>
                <span>
                  {formatDecimal(r.amount)} {r.unit}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}
