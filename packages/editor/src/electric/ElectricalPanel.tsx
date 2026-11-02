import type { Circuit, CircuitKind, LightScene } from "@openplanner/engine-client";
import { Button } from "@openplanner/ui";
import { sendToEngine } from "../engine";
import { formatNumber, type MessageKey, t } from "../i18n";
import { formatDecimal } from "../inspector/format";
import { useEditor } from "../store/editor";
import { isLight, newCircuit, setCircuits } from "./devices";
import styles from "./ElectricalPanel.module.css";

const BREAKERS = [6, 10, 13, 16, 20, 25, 32, 40, 50];
const SECTIONS = [1.5, 2.5, 4, 6, 10, 16];
const kinds: { value: CircuitKind; label: MessageKey }[] = [
  { value: "lighting", label: "circuit.lighting" },
  { value: "sockets", label: "circuit.sockets" },
  { value: "power", label: "circuit.power" },
];

/** Вкладка «Электрика»: группы щита с расчётом, состав щита, сценарии, освещённость. */
export function ElectricalPanel() {
  const doc = useEditor((s) => s.document);
  const report = useEditor((s) => s.electrical);
  const selection = useEditor((s) => s.selection);
  if (!doc) return null;
  const circuits = doc.circuits ?? [];
  const scenes = doc.scenes ?? [];
  const update = (id: string, patch: Partial<Circuit>) =>
    setCircuits(circuits.map((c) => (c.id === id ? { ...c, ...patch } : c)));
  const selectedLights = (doc.devices ?? [])
    .filter((d) => selection.includes(d.id) && isLight(d.kind))
    .map((d) => d.id);
  const setScenes = (next: LightScene[]) =>
    sendToEngine({ type: "command", command: { type: "setScenes", scenes: next } });

  return (
    <div className={styles.root} data-testid="electrical">
      <section className={styles.section}>
        <h3>{t("electrical.circuits")}</h3>
        <Button variant="outline" onClick={() => sendToEngine({ type: "autoCircuits" })}>
          {t("electrical.auto")}
        </Button>
        {report && report.unassigned > 0 && (
          <p className={styles.warning}>
            {t("electrical.unassigned", { count: report.unassigned })}
          </p>
        )}
        <ul className={styles.list} aria-label={t("electrical.circuits")}>
          {circuits.map((c) => {
            const r = report?.circuits.find((x) => x.id === c.id);
            return (
              <li key={c.id} className={styles.circuit} data-circuit={c.name}>
                <div className={styles.head}>
                  <input
                    className={styles.name}
                    aria-label={t("electrical.name")}
                    defaultValue={c.name}
                    onBlur={(e) => e.target.value.trim() && update(c.id, { name: e.target.value })}
                  />
                  <input
                    className={styles.purpose}
                    aria-label={t("electrical.purpose")}
                    placeholder={t("electrical.purpose")}
                    defaultValue={c.purpose}
                    onBlur={(e) => update(c.id, { purpose: e.target.value })}
                  />
                  <button
                    type="button"
                    className={styles.remove}
                    aria-label={t("electrical.remove", { name: c.name })}
                    onClick={() => setCircuits(circuits.filter((x) => x.id !== c.id))}
                  >
                    ×
                  </button>
                </div>
                <div className={styles.params}>
                  <select
                    aria-label={t("electrical.kind")}
                    value={c.kind}
                    onChange={(e) => update(c.id, { kind: e.target.value as CircuitKind })}
                  >
                    {kinds.map((k) => (
                      <option key={k.value} value={k.value}>
                        {t(k.label)}
                      </option>
                    ))}
                  </select>
                  <select
                    aria-label={t("electrical.breaker")}
                    value={c.breaker}
                    onChange={(e) => update(c.id, { breaker: Number(e.target.value) })}
                  >
                    {BREAKERS.map((b) => (
                      <option key={b} value={b}>
                        C{b}
                      </option>
                    ))}
                  </select>
                  <select
                    aria-label={t("electrical.section")}
                    value={c.section}
                    onChange={(e) => update(c.id, { section: Number(e.target.value) })}
                  >
                    {SECTIONS.map((s) => (
                      <option key={s} value={s}>
                        {formatDecimal(s)} мм²
                      </option>
                    ))}
                  </select>
                  <select
                    aria-label={t("electrical.phases")}
                    value={c.phases}
                    onChange={(e) => update(c.id, { phases: Number(e.target.value) })}
                  >
                    <option value={1}>1Ф</option>
                    <option value={3}>3Ф</option>
                  </select>
                  <label className={styles.check}>
                    <input
                      type="checkbox"
                      checked={c.rcd}
                      onChange={(e) => update(c.id, { rcd: e.target.checked })}
                    />
                    {t("electrical.rcd")}
                  </label>
                  <label className={styles.check}>
                    Кс
                    <input
                      className={styles.demand}
                      aria-label={t("electrical.demand")}
                      type="number"
                      min={0}
                      max={1}
                      step={0.05}
                      defaultValue={c.demand}
                      onBlur={(e) => {
                        const v = Number(e.target.value);
                        if (v >= 0 && v <= 1) update(c.id, { demand: v });
                      }}
                    />
                  </label>
                </div>
                {r && (
                  <p className={styles.calc} data-testid="circuit-calc">
                    {t("electrical.calc", {
                      devices: r.devices,
                      power: formatDecimal(r.demand / 1000),
                      current: formatDecimal(r.current),
                      length: formatDecimal(r.length),
                      drop: formatDecimal(r.drop),
                    })}
                  </p>
                )}
                {r?.warnings.map((w) => (
                  <p key={w} className={styles.warning} role="alert">
                    {w}
                  </p>
                ))}
              </li>
            );
          })}
        </ul>
        <Button variant="outline" onClick={() => setCircuits([...circuits, newCircuit(circuits)])}>
          {t("electrical.add")}
        </Button>
      </section>

      {report && circuits.length > 0 && (
        <section className={styles.section} data-testid="panel">
          <h3>
            {t("electrical.panel", {
              modules: report.panel.modules,
              enclosure: report.panel.enclosure,
            })}
          </h3>
          <ul className={styles.rows}>
            {report.panel.items.map((i) => (
              <li key={i.name}>
                <span>{i.name}</span>
                <span>
                  {i.count} × {i.modules} DIN
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}

      <section className={styles.section}>
        <h3>{t("electrical.scenes")}</h3>
        <Button
          variant="outline"
          disabled={selectedLights.length === 0}
          onClick={() =>
            setScenes([
              ...scenes,
              {
                id: `scene-${Date.now().toString(36)}`,
                name: t("electrical.sceneName", { n: scenes.length + 1 }),
                lights: selectedLights,
              },
            ])
          }
        >
          {t("electrical.sceneFromSelection")}
        </Button>
        <ul className={styles.rows}>
          {scenes.map((s) => (
            <li key={s.id}>
              <button
                type="button"
                className={styles.link}
                onClick={() => sendToEngine({ type: "select", ids: s.lights })}
              >
                {s.name}
              </button>
              <span>
                {t("electrical.lights", { count: s.lights.length })}
                <button
                  type="button"
                  className={styles.remove}
                  aria-label={t("electrical.removeScene", { name: s.name })}
                  onClick={() => setScenes(scenes.filter((x) => x.id !== s.id))}
                >
                  ×
                </button>
              </span>
            </li>
          ))}
        </ul>
      </section>

      {report && report.rooms.length > 0 && (
        <section className={styles.section} data-testid="illuminance">
          <h3>{t("electrical.illuminance")}</h3>
          <ul className={styles.rows}>
            {report.rooms.map((r) => (
              <li key={r.id} data-ok={r.lux >= r.norm}>
                <span>{r.name}</span>
                <span>
                  {t("electrical.lux", { lux: formatNumber(r.lux), norm: formatNumber(r.norm) })}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}
