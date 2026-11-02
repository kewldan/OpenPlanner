import type { Device, Document, Element } from "@openplanner/engine-client";
import { sendToEngine } from "../engine";
import { t } from "../i18n";
import { CommitField } from "../inspector/CommitField";
import { parseNumber } from "../inspector/format";
import inspector from "../inspector/Inspector.module.css";
import { useEditor } from "../store/editor";
import { isLight, lightMarks, toggleKeyLight } from "./devices";
import styles from "./ElectricalPanel.module.css";

function put(device: Device) {
  const element: Element = { type: "device", ...device };
  sendToEngine({ type: "command", command: { type: "put", element } });
}

/** Число ≥ 0 из поля: иначе — ошибка в инспекторе. */
function amount(apply: (value: number) => void) {
  return (raw: string) => {
    const value = parseNumber(raw);
    if (value === null || value < 0)
      useEditor.setState({ lastError: "введите число не меньше нуля" });
    else apply(value);
  };
}

/** Свойства электроточки: назначение, высота, мощность, поток, группа, клавиши выключателя. */
export function DeviceSection({ device, document }: { device: Device; document: Document }) {
  const catalog = useEditor((s) => s.deviceCatalog);
  const entry = catalog.find((c) => c.kind === device.kind);
  const update = (patch: Partial<Device>) => put({ ...device, ...patch });
  const lights = lightMarks(document).filter(
    (l) => l.device.variant === undefined || l.device.variant === device.variant,
  );
  const light = isLight(device.kind);
  // Клавиши позиционные: ключ — выключатель и номер клавиши.
  const switchKeys = (device.keys ?? []).map((key, k) => ({ id: `${device.id}:${k}`, key, k }));
  const loads = (entry?.power ?? 0) > 0 || light;
  return (
    <section className={inspector.section} data-testid="device-section">
      <h2 className={inspector.title}>{entry?.name ?? t("devices.device")}</h2>
      <CommitField
        label={t("devices.label")}
        value={device.label ?? ""}
        onCommit={(label) => update({ label })}
      />
      <CommitField
        label={t("devices.h")}
        unit={t("unit.mm")}
        numeric
        value={String(Math.round(device.height))}
        onCommit={amount((height) => update({ height }))}
      />
      {loads && (
        <CommitField
          label={t("devices.power")}
          unit={t("unit.w")}
          numeric
          value={String(device.power ?? 0)}
          onCommit={amount((power) => update({ power }))}
        />
      )}
      {light && (
        <CommitField
          label={t("devices.flux")}
          unit={t("unit.lm")}
          numeric
          value={String(device.flux ?? 0)}
          onCommit={amount((flux) => update({ flux }))}
        />
      )}
      {loads && (
        <label className={inspector.row}>
          <span className={inspector.label}>{t("devices.circuit")}</span>
          <select
            className={inspector.select}
            aria-label={t("devices.circuit")}
            value={device.circuit ?? ""}
            onChange={(e) => {
              const { circuit: _circuit, ...rest } = device;
              put(e.target.value ? { ...rest, circuit: e.target.value } : rest);
            }}
          >
            <option value="">{t("devices.noCircuit")}</option>
            {(document.circuits ?? []).map((c) => (
              <option key={c.id} value={c.id}>
                {c.name} {c.purpose}
              </option>
            ))}
          </select>
        </label>
      )}
      {switchKeys.map(({ id, key, k }) => (
        <fieldset key={id} className={styles.keys} aria-label={t("devices.key", { n: k + 1 })}>
          <legend>{t("devices.key", { n: k + 1 })}</legend>
          {lights.length === 0 && <p className={styles.calc}>{t("devices.noLights")}</p>}
          {lights.map(({ device: l, mark }) => (
            <label key={l.id} className={styles.check}>
              <input
                type="checkbox"
                checked={key.includes(l.id)}
                onChange={() => update({ keys: toggleKeyLight(device.keys ?? [], k, l.id) })}
              />
              {mark} · {catalog.find((c) => c.kind === l.kind)?.name ?? ""}
            </label>
          ))}
        </fieldset>
      ))}
    </section>
  );
}
