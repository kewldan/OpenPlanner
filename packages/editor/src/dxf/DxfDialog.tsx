import type { DxfMode } from "@openplanner/engine-client";
import { Button, Dialog, Segmented } from "@openplanner/ui";
import { useEffect, useState } from "react";
import styles from "../drawings/DrawingsDialog.module.css";
import { callEngine } from "../engine";
import { t } from "../i18n";
import { formatDecimal, parseNumber } from "../inspector/format";
import { useEditor } from "../store/editor";
import { useDxf, wallLayers } from "./store";

/** Окно импорта DXF: подложкой или стенами по линиям выбранных слоёв. */
export function DxfDialog() {
  const file = useDxf((s) => s.file);
  const wall = useEditor((s) => s.toolOptions.wallThickness);
  const [mode, setMode] = useState<DxfMode>("underlay");
  const [layers, setLayers] = useState<string[]>([]);
  const [thickness, setThickness] = useState(String(wall));
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!file) return;
    setMode("underlay");
    setLayers(file.info.layers.map((l) => l.name));
    setError(null);
  }, [file]);

  if (!file) return null;
  const close = () => useDxf.setState({ file: null });
  const switchMode = (next: DxfMode) => {
    setMode(next);
    setLayers(next === "walls" ? wallLayers(file.info) : file.info.layers.map((l) => l.name));
  };
  const run = async () => {
    const value = parseNumber(thickness);
    if (mode === "walls" && (value === null || value <= 0)) {
      setError(t("dxf.badThickness"));
      return;
    }
    try {
      await callEngine({
        type: "importDxf",
        bytes: file.bytes,
        name: file.name,
        options: { mode, layers, thickness: value ?? wall },
      });
      close();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <Dialog open onOpenChange={(next) => !next && close()} title={t("dxf.title")} width={520}>
      <div className={styles.side} style={{ padding: "var(--space-6)" }} data-testid="dxf">
        <p className={styles.hint}>
          {t("dxf.size", {
            name: file.name,
            width: formatDecimal(Math.round(file.info.width / 10) / 100),
            height: formatDecimal(Math.round(file.info.height / 10) / 100),
          })}
        </p>
        <Segmented
          label={t("dxf.mode")}
          value={mode}
          options={[
            { value: "underlay", label: t("dxf.underlay"), disabled: false },
            { value: "walls", label: t("dxf.walls"), disabled: false },
          ]}
          onChange={switchMode}
        />
        <p className={styles.hint}>{t(mode === "walls" ? "dxf.wallsHint" : "dxf.underlayHint")}</p>
        <fieldset className={styles.side} aria-label={t("dxf.layers")}>
          <legend className={styles.heading}>{t("dxf.layers")}</legend>
          {file.info.layers.map((l) => (
            <label key={l.name}>
              <input
                type="checkbox"
                checked={layers.includes(l.name)}
                onChange={(e) =>
                  setLayers(
                    e.target.checked ? [...layers, l.name] : layers.filter((x) => x !== l.name),
                  )
                }
              />{" "}
              {l.name} · {t("dxf.paths", { count: l.paths })}
            </label>
          ))}
        </fieldset>
        {mode === "walls" && (
          <label className={styles.pick}>
            <span>{t("dxf.thickness")}</span>
            <input
              aria-label={t("dxf.thickness")}
              value={thickness}
              onChange={(e) => setThickness(e.target.value)}
            />
          </label>
        )}
        {error && (
          <p className={styles.warning} role="alert">
            {error}
          </p>
        )}
        <Button variant="primary" disabled={layers.length === 0} onClick={() => void run()}>
          {t("dxf.import")}
        </Button>
      </div>
    </Dialog>
  );
}
