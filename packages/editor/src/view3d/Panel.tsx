import type { Document, SavedView, Site } from "@openplanner/engine-client";
import { Button, Segmented } from "@openplanner/ui";
import type { ReactNode } from "react";
import { sendToEngine } from "../engine";
import { formatNumber, t } from "../i18n";
import { CommitField } from "../inspector/CommitField";
import { parseNumber } from "../inspector/format";
import inspector from "../inspector/Inspector.module.css";
import { useEditor } from "../store/editor";
import {
  clear3d,
  dateOfDay,
  dayOfDate,
  dayOfYear,
  formatHour,
  nextViewName,
  set3d,
} from "./actions";
import styles from "./Panel.module.css";

/** Срез по умолчанию, мм: чуть выше подоконника. */
const DEFAULT_CUT = 1200;

const modes = [
  { value: "orbit", label: t("view3d.orbit") },
  { value: "walk", label: t("view3d.walk") },
] as const;

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className={inspector.section}>
      <h2 className={inspector.title}>{title}</h2>
      {children}
    </section>
  );
}

function Check({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className={styles.check}>
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span>{label}</span>
    </label>
  );
}

/** Число из поля: применяется, только если введено число. */
function numberField(apply: (v: number) => void) {
  return (raw: string) => {
    const v = parseNumber(raw);
    if (v === null) useEditor.setState({ lastError: "введите число" });
    else apply(v);
  };
}

/** Инспектор 3D-вида: камера, отображение, солнце, ориентация, ракурсы. */
export function View3dPanel({ document }: { document: Document }) {
  const three = useEditor((s) => s.three);
  const { options, sun } = three;
  const site: Site = document.site ?? {
    north: 0,
    latitude: 55.75,
    longitude: 37.62,
    utcOffset: 3,
  };
  const views: SavedView[] = document.views ?? [];
  const setSite = (change: Partial<Site>) =>
    sendToEngine({ type: "command", command: { type: "setSite", site: { ...site, ...change } } });
  const setViews = (next: SavedView[]) =>
    sendToEngine({ type: "command", command: { type: "setViews", views: next } });
  const year = new Date().getFullYear();

  return (
    <div data-testid="view3d">
      <Section title={t("view3d.title")}>
        <Segmented
          label={t("view3d.mode")}
          value={options.mode}
          options={modes}
          onChange={(mode) => set3d({ mode })}
        />
        <p className={styles.hint}>
          {options.mode === "walk" ? t("view3d.hintWalk") : t("view3d.hintOrbit")}
        </p>
        <Button variant="outline" onClick={() => sendToEngine({ type: "resetCamera3d" })}>
          {t("view3d.reset")}
        </Button>
        <Check
          label={t("view3d.ceiling")}
          checked={options.ceiling}
          onChange={(ceiling) => set3d({ ceiling })}
        />
        <Check
          label={t("view3d.white")}
          checked={options.white}
          onChange={(white) => set3d({ white })}
        />
        <Check
          label={t("view3d.cut")}
          checked={options.cut !== undefined}
          onChange={(on) => (on ? set3d({ cut: DEFAULT_CUT }) : clear3d("cut"))}
        />
        {options.cut !== undefined && (
          <CommitField
            label={t("view3d.cutHeight")}
            unit={t("unit.mm")}
            numeric
            value={String(options.cut)}
            onCommit={numberField((cut) => cut > 0 && set3d({ cut }))}
          />
        )}
      </Section>

      <Section title={t("view3d.sun")}>
        <Check
          label={t("view3d.sunOn")}
          checked={options.sun !== undefined}
          onChange={(on) =>
            on
              ? set3d({
                  sun: {
                    day: dayOfYear(new Date().getMonth() + 1, new Date().getDate()),
                    hour: 12,
                  },
                })
              : clear3d("sun")
          }
        />
        {options.sun !== undefined && (
          <>
            <label className={inspector.row}>
              <span className={inspector.label}>{t("view3d.date")}</span>
              <input
                className={inspector.select}
                type="date"
                aria-label={t("view3d.date")}
                value={dateOfDay(options.sun.day, year)}
                onChange={(e) => {
                  const day = dayOfDate(e.target.value);
                  if (day !== null && options.sun) set3d({ sun: { ...options.sun, day } });
                }}
              />
            </label>
            <label className={inspector.row}>
              <span className={inspector.label}>
                {t("view3d.time")} {formatHour(options.sun.hour)}
              </span>
              <input
                type="range"
                min={0}
                max={24}
                step={0.25}
                aria-label={t("view3d.time")}
                value={options.sun.hour}
                onChange={(e) => {
                  if (options.sun) set3d({ sun: { ...options.sun, hour: Number(e.target.value) } });
                }}
              />
            </label>
            {sun && (
              <p className={styles.hint} data-testid="sun-position">
                {sun.altitude > 0
                  ? t("view3d.sunPosition", {
                      azimuth: formatNumber(sun.azimuth),
                      altitude: formatNumber(sun.altitude),
                    })
                  : t("view3d.sunBelow")}
              </p>
            )}
          </>
        )}
        <CommitField
          label={t("view3d.north")}
          unit={t("unit.deg")}
          numeric
          value={String(site.north)}
          onCommit={numberField((north) => setSite({ north }))}
        />
        <CommitField
          label={t("view3d.latitude")}
          unit={t("unit.deg")}
          numeric
          value={String(site.latitude)}
          onCommit={numberField((latitude) => setSite({ latitude }))}
        />
        <CommitField
          label={t("view3d.longitude")}
          unit={t("unit.deg")}
          numeric
          value={String(site.longitude)}
          onCommit={numberField((longitude) => setSite({ longitude }))}
        />
        <CommitField
          label={t("view3d.utcOffset")}
          numeric
          value={String(site.utcOffset)}
          onCommit={numberField((utcOffset) => setSite({ utcOffset }))}
        />
      </Section>

      <Section title={t("view3d.views")}>
        <Button
          variant="outline"
          onClick={() =>
            sendToEngine({
              type: "saveView3d",
              name: nextViewName(
                views.map((v) => v.name),
                t("view3d.viewName"),
              ),
            })
          }
        >
          {t("view3d.save")}
        </Button>
        {views.length === 0 ? (
          <p className={styles.hint}>{t("view3d.noViews")}</p>
        ) : (
          <ul className={styles.views} aria-label={t("view3d.views")}>
            {views.map((v) => (
              <li key={v.id}>
                <button
                  type="button"
                  className={styles.view}
                  onClick={() => sendToEngine({ type: "applyView3d", id: v.id })}
                >
                  {v.name}
                  <span>{v.walk ? t("view3d.walk") : t("view3d.orbit")}</span>
                </button>
                <button
                  type="button"
                  className={styles.remove}
                  aria-label={t("view3d.remove", { name: v.name })}
                  onClick={() => setViews(views.filter((x) => x.id !== v.id))}
                >
                  ×
                </button>
              </li>
            ))}
          </ul>
        )}
      </Section>
    </div>
  );
}
