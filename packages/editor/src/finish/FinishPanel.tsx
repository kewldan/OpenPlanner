import type {
  Finish,
  Material,
  MaterialKind,
  Surface,
  TilePattern,
} from "@openplanner/engine-client";
import { Button } from "@openplanner/ui";
import { useState } from "react";
import styles from "../electric/ElectricalPanel.module.css";
import { sendToEngine } from "../engine";
import { type MessageKey, t } from "../i18n";
import { formatDecimal, parseNumber } from "../inspector/format";
import { useEditor } from "../store/editor";
import {
  formatQuantity,
  isPiecewise,
  materialOptions,
  newFinish,
  newMaterial,
  replaceFinish,
  setFinishes,
  setMaterials,
} from "./finishes";

const surfaces: readonly { value: Surface; label: MessageKey }[] = [
  { value: "floor", label: "finish.floor" },
  { value: "ceiling", label: "finish.ceiling" },
  { value: "walls", label: "finish.walls" },
];

const patterns: readonly { value: TilePattern; label: MessageKey }[] = [
  { value: "straight", label: "finish.straight" },
  { value: "offset", label: "finish.offset" },
  { value: "diagonal", label: "finish.diagonal" },
];

const kinds: readonly { value: MaterialKind; label: MessageKey }[] = [
  { value: "tile", label: "finish.kind.tile" },
  { value: "laminate", label: "finish.kind.laminate" },
  { value: "paint", label: "finish.kind.paint" },
  { value: "wallpaper", label: "finish.kind.wallpaper" },
  { value: "sheet", label: "finish.kind.sheet" },
];

/** Число ≥ 0 из поля; иначе — ошибка. */
function number(raw: string, apply: (value: number) => void) {
  const value = parseNumber(raw);
  if (value === null || value < 0)
    useEditor.setState({ lastError: "введите число не меньше нуля" });
  else apply(value);
}

/** Вкладка «Отделка»: поверхности помещения, отделка с раскладкой и расходом, материалы. */
export function FinishPanel() {
  const doc = useEditor((s) => s.document);
  const report = useEditor((s) => s.finishes);
  const selection = useEditor((s) => s.selection);
  const library = useEditor((s) => s.materialLibrary);
  const [chosen, setChosen] = useState<string | null>(null);
  if (!doc || !report) return null;
  const rooms = report.rooms;
  const selectedRoom = rooms.find((r) => selection.includes(r.id))?.id;
  const roomId = selectedRoom ?? (rooms.some((r) => r.id === chosen) ? chosen : rooms[0]?.id);
  const room = rooms.find((r) => r.id === roomId);
  const finishes = doc.finishes ?? [];
  const own = doc.materials ?? [];
  const options = materialOptions(library, own);
  const update = (next: Finish) => setFinishes(replaceFinish(finishes, next));

  return (
    <div className={styles.root} data-testid="finish">
      <section className={styles.section}>
        <h3>{t("finish.room")}</h3>
        {rooms.length === 0 ? (
          <p className={styles.calc}>{t("finish.noRooms")}</p>
        ) : (
          <select
            className={styles.demand}
            style={{ width: "100%" }}
            aria-label={t("finish.room")}
            value={roomId ?? ""}
            onChange={(e) => {
              setChosen(e.target.value);
              sendToEngine({ type: "select", ids: [e.target.value] });
            }}
          >
            {rooms.map((r) => (
              <option key={r.id} value={r.id}>
                {r.name}
              </option>
            ))}
          </select>
        )}
        {room && (
          <ul className={styles.rows} data-testid="surfaces">
            <li>
              <span>{t("finish.floor")}</span>
              <span>{formatDecimal(room.floor)} м²</span>
            </li>
            <li>
              <span>{t("finish.ceiling")}</span>
              <span>{formatDecimal(room.ceiling)} м²</span>
            </li>
            <li>
              <span>{t("finish.wallsNet")}</span>
              <span>{formatDecimal(room.wallsNet)} м²</span>
            </li>
            <li>
              <span>{t("finish.perimeter")}</span>
              <span>
                {formatDecimal(room.perimeter)} м × {formatDecimal(room.height)} м
              </span>
            </li>
          </ul>
        )}
      </section>

      {room && (
        <section className={styles.section}>
          <h3>{t("finish.list")}</h3>
          <ul className={styles.list} aria-label={t("finish.list")}>
            {finishes
              .filter((f) => f.room === room.id)
              .map((f) => {
                const info = report.finishes.find((i) => i.id === f.id);
                const material = options.find((m) => m.id === f.material);
                const surface =
                  f.surface === "walls" && f.face !== undefined
                    ? t("finish.face", { letter: room.faces[f.face]?.letter ?? "?" })
                    : t(surfaces.find((s) => s.value === f.surface)?.label ?? "finish.walls");
                return (
                  <li key={f.id} className={styles.circuit} data-finish={f.surface}>
                    <div className={styles.head}>
                      <strong className={styles.purpose}>{surface}</strong>
                      <button
                        type="button"
                        className={styles.remove}
                        aria-label={t("finish.remove", { surface })}
                        onClick={() => setFinishes(finishes.filter((x) => x.id !== f.id))}
                      >
                        ×
                      </button>
                    </div>
                    <div className={styles.params}>
                      <select
                        className={styles.purpose}
                        aria-label={t("finish.material")}
                        value={f.material}
                        onChange={(e) => update({ ...f, material: e.target.value })}
                      >
                        {options.map((m) => (
                          <option key={m.id} value={m.id}>
                            {m.name}
                          </option>
                        ))}
                      </select>
                    </div>
                    {f.surface === "walls" && (
                      <div className={styles.params}>
                        <select
                          aria-label={t("finish.faceLabel")}
                          value={f.face ?? ""}
                          onChange={(e) => {
                            const { face: _face, ...rest } = f;
                            update(
                              e.target.value === ""
                                ? rest
                                : { ...rest, face: Number(e.target.value) },
                            );
                          }}
                        >
                          <option value="">{t("finish.allWalls")}</option>
                          {room.faces.map((face, i) => (
                            <option key={face.letter} value={i}>
                              {t("finish.face", { letter: face.letter })}
                            </option>
                          ))}
                        </select>
                        <label className={styles.check}>
                          {t("finish.from")}
                          <input
                            className={styles.demand}
                            aria-label={t("finish.bottom")}
                            defaultValue={f.bottom}
                            key={`b${f.bottom}`}
                            onBlur={(e) =>
                              number(e.target.value, (bottom) => update({ ...f, bottom }))
                            }
                          />
                        </label>
                        <label className={styles.check}>
                          {t("finish.to")}
                          <input
                            className={styles.demand}
                            aria-label={t("finish.top")}
                            placeholder={t("finish.ceilingShort")}
                            defaultValue={f.top ?? ""}
                            key={`t${f.top}`}
                            onBlur={(e) => {
                              const { top: _top, ...rest } = f;
                              if (e.target.value.trim() === "") update(rest);
                              else number(e.target.value, (top) => update({ ...rest, top }));
                            }}
                          />
                        </label>
                      </div>
                    )}
                    {material && isPiecewise(material.kind) && (
                      <div className={styles.params}>
                        <select
                          aria-label={t("finish.pattern")}
                          value={f.layout.pattern ?? "straight"}
                          onChange={(e) =>
                            update({
                              ...f,
                              layout: { ...f.layout, pattern: e.target.value as TilePattern },
                            })
                          }
                        >
                          {patterns.map((p) => (
                            <option key={p.value} value={p.value}>
                              {t(p.label)}
                            </option>
                          ))}
                        </select>
                        {material.kind === "tile" && (
                          <label className={styles.check}>
                            {t("finish.joint")}
                            <input
                              className={styles.demand}
                              aria-label={t("finish.joint")}
                              defaultValue={f.layout.joint}
                              key={`j${f.layout.joint}`}
                              onBlur={(e) =>
                                number(e.target.value, (joint) =>
                                  update({ ...f, layout: { ...f.layout, joint } }),
                                )
                              }
                            />
                          </label>
                        )}
                        <Button
                          variant="outline"
                          onClick={() => sendToEngine({ type: "optimizeLayout", id: f.id })}
                        >
                          {t("finish.optimize")}
                        </Button>
                      </div>
                    )}
                    {info && (
                      <p className={styles.calc} data-testid="finish-calc">
                        {formatDecimal(info.area)} м²
                        {info.whole + info.cut > 0 &&
                          ` · ${t("finish.pieces", { whole: info.whole, cut: info.cut })}`}
                        {info.quantities.length > 0 &&
                          ` · ${info.quantities.map(formatQuantity).join(" · ")}`}
                      </p>
                    )}
                    {info?.warnings.map((w) => (
                      <p key={w} className={styles.warning} role="alert">
                        {w}
                      </p>
                    ))}
                  </li>
                );
              })}
          </ul>
          <div className={styles.params}>
            {surfaces.map((s) => (
              <Button
                key={s.value}
                variant="outline"
                onClick={() => setFinishes([...finishes, newFinish(room.id, s.value)])}
              >
                + {t(s.label)}
              </Button>
            ))}
          </div>
        </section>
      )}

      <Materials own={own} />

      {report.totals.length > 0 && (
        <section className={styles.section} data-testid="finish-totals">
          <h3>{t("finish.totals")}</h3>
          <ul className={styles.rows}>
            {report.totals.map((total) => (
              <li key={total.material}>
                <span>{total.name}</span>
                <span>{total.quantities.map(formatQuantity).join(" · ")}</span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}

/** Материалы проекта: название, вид, размеры, упаковка, цвет. */
function Materials({ own }: { own: Material[] }) {
  const update = (next: Material) => setMaterials(own.map((m) => (m.id === next.id ? next : m)));
  const size = (m: Material, field: "width" | "length" | "pack", label: MessageKey) => (
    <label className={styles.check}>
      {t(label)}
      <input
        className={styles.demand}
        aria-label={t(label)}
        defaultValue={m[field]}
        key={`${field}${m[field]}`}
        onBlur={(e) => number(e.target.value, (value) => update({ ...m, [field]: value }))}
      />
    </label>
  );
  return (
    <section className={styles.section} data-testid="materials">
      <h3>{t("finish.materials")}</h3>
      <ul className={styles.list}>
        {own.map((m) => (
          <li key={m.id} className={styles.circuit}>
            <div className={styles.head}>
              <input
                type="color"
                aria-label={t("finish.color")}
                value={m.color || "#d8d2c4"}
                onChange={(e) => update({ ...m, color: e.target.value })}
              />
              <input
                className={styles.purpose}
                aria-label={t("finish.materialName")}
                defaultValue={m.name}
                onBlur={(e) => e.target.value.trim() && update({ ...m, name: e.target.value })}
              />
              <button
                type="button"
                className={styles.remove}
                aria-label={t("finish.removeMaterial", { name: m.name })}
                onClick={() => setMaterials(own.filter((x) => x.id !== m.id))}
              >
                ×
              </button>
            </div>
            <div className={styles.params}>
              <select
                aria-label={t("finish.kind")}
                value={m.kind}
                onChange={(e) => update({ ...m, kind: e.target.value as MaterialKind })}
              >
                {kinds.map((k) => (
                  <option key={k.value} value={k.value}>
                    {t(k.label)}
                  </option>
                ))}
              </select>
              {size(m, "width", "finish.width")}
              {size(m, "length", "finish.length")}
              {size(m, "pack", "finish.pack")}
            </div>
          </li>
        ))}
      </ul>
      <Button variant="outline" onClick={() => setMaterials([...own, newMaterial(own.length + 1)])}>
        {t("finish.addMaterial")}
      </Button>
    </section>
  );
}
