import type { HeatingZone } from "@openplanner/engine-client";
import { Button } from "@openplanner/ui";
import styles from "../electric/ElectricalPanel.module.css";
import { sendToEngine } from "../engine";
import { t } from "../i18n";
import { formatDecimal, parseNumber } from "../inspector/format";
import { useEditor } from "../store/editor";

/** Удельная мощность тёплого пола, Вт/м². */
const DENSITIES = [120, 150, 180, 200];
/** Типовые сплит-системы по холоду, кВт (как в расчёте движка). */
const AC_SIZES = [2.0, 2.5, 3.5, 5.0, 7.0, 8.0];

export function setHeating(heating: HeatingZone[]) {
  sendToEngine({ type: "command", command: { type: "setHeating", heating } });
}

/** Ближайшая типовая сплит-система не меньше нужной мощности. */
export function acSize(kw: number): number {
  return AC_SIZES.find((s) => s >= kw - 1e-9) ?? AC_SIZES[AC_SIZES.length - 1] ?? kw;
}

/** Вкладка «Климат»: зоны тёплого пола, воздухообмен и охлаждение по помещениям. */
export function ClimatePanel() {
  const doc = useEditor((s) => s.document);
  const report = useEditor((s) => s.climate);
  const selection = useEditor((s) => s.selection);
  if (!doc || !report) return null;
  const heating = doc.heating ?? [];
  const thermostats = (doc.devices ?? []).filter((d) => d.kind === "thermostat");
  const free = report.rooms.filter((r) => !heating.some((z) => z.room === r.id));
  const target = free.find((r) => selection.includes(r.id)) ?? free[0];
  const update = (id: string, patch: Partial<HeatingZone>) =>
    setHeating(heating.map((z) => (z.id === id ? { ...z, ...patch } : z)));

  return (
    <div className={styles.root} data-testid="climate">
      <section className={styles.section}>
        <h3>{t("climate.heating")}</h3>
        <ul className={styles.list} aria-label={t("climate.heating")}>
          {heating.map((z) => {
            const info = report.zones.find((i) => i.id === z.id);
            return (
              <li key={z.id} className={styles.circuit} data-zone={info?.roomName ?? z.room}>
                <div className={styles.head}>
                  <strong className={styles.purpose}>{info?.roomName ?? "—"}</strong>
                  <button
                    type="button"
                    className={styles.remove}
                    aria-label={t("climate.removeZone", { name: info?.roomName ?? "" })}
                    onClick={() => setHeating(heating.filter((x) => x.id !== z.id))}
                  >
                    ×
                  </button>
                </div>
                <div className={styles.params}>
                  <select
                    aria-label={t("climate.density")}
                    value={z.density}
                    onChange={(e) => update(z.id, { density: Number(e.target.value) })}
                  >
                    {DENSITIES.map((d) => (
                      <option key={d} value={d}>
                        {d} Вт/м²
                      </option>
                    ))}
                  </select>
                  <label className={styles.check}>
                    {t("climate.margin")}
                    <input
                      className={styles.demand}
                      aria-label={t("climate.margin")}
                      defaultValue={z.margin}
                      key={`m${z.margin}`}
                      onBlur={(e) => {
                        const v = parseNumber(e.target.value);
                        if (v !== null && v >= 0) update(z.id, { margin: v });
                      }}
                    />
                  </label>
                  <select
                    aria-label={t("climate.thermostat")}
                    value={z.thermostat ?? ""}
                    onChange={(e) => {
                      const { thermostat: _t, ...rest } = z;
                      setHeating(
                        heating.map((x) =>
                          x.id === z.id
                            ? e.target.value
                              ? { ...rest, thermostat: e.target.value }
                              : rest
                            : x,
                        ),
                      );
                    }}
                  >
                    <option value="">{t("climate.noThermostat")}</option>
                    {thermostats.map((d, i) => (
                      <option key={d.id} value={d.id}>
                        {t("climate.thermostatN", { n: i + 1, h: Math.round(d.height) })}
                      </option>
                    ))}
                  </select>
                </div>
                {info && (
                  <p className={styles.calc} data-testid="zone-calc">
                    {t("climate.zoneCalc", {
                      heated: formatDecimal(info.heated),
                      area: formatDecimal(info.area),
                      power: info.power,
                    })}
                  </p>
                )}
              </li>
            );
          })}
        </ul>
        <Button
          variant="outline"
          disabled={!target}
          onClick={() =>
            target &&
            setHeating([
              ...heating,
              {
                id: `zone-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`,
                room: target.id,
                density: 150,
                margin: 100,
                ...(thermostats[0] ? { thermostat: thermostats[0].id } : {}),
              },
            ])
          }
        >
          {target ? t("climate.addZone", { name: target.name }) : t("climate.noRooms")}
        </Button>
      </section>

      <section className={styles.section} data-testid="air">
        <h3>{t("climate.air")}</h3>
        <ul className={styles.rows}>
          {report.rooms.map((r) => (
            <li key={r.id} data-ok={r.airProvided >= r.airRequired || r.airRequired === 0}>
              <span>{r.name}</span>
              <span>
                {t("climate.airCalc", {
                  provided: Math.round(r.airProvided),
                  required: Math.round(r.airRequired),
                  cooling: formatDecimal(acSize(r.cooling)),
                  units: r.acUnits,
                })}
              </span>
            </li>
          ))}
        </ul>
        {report.warnings.map((w) => (
          <p key={w} className={styles.warning} role="alert">
            {w}
          </p>
        ))}
      </section>
    </div>
  );
}
