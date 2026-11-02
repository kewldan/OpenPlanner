import type { Pipe, PlumbingPoint } from "@openplanner/engine-client";
import { t } from "../i18n";
import { CommitField } from "../inspector/CommitField";
import { formatDecimal, parseNumber } from "../inspector/format";
import inspector from "../inspector/Inspector.module.css";
import { useEditor } from "../store/editor";
import { pipeSlope, putPipe, putPoint, systemCodes, withSlope } from "./points";

function amount(apply: (value: number) => void, positive = false) {
  return (raw: string) => {
    const value = parseNumber(raw);
    if (value === null || value < 0 || (positive && value === 0))
      useEditor.setState({
        lastError: positive ? "значение должно быть больше нуля" : "введите число не меньше нуля",
      });
    else apply(value);
  };
}

const kindNames = {
  outlet: "plumbing.outlet",
  riser: "plumbing.riser",
  manifold: "plumbing.manifold",
} as const;

/** Свойства сантехнической точки: назначение, отметка, диаметр. */
export function PlumbingPointSection({ point }: { point: PlumbingPoint }) {
  const update = (patch: Partial<PlumbingPoint>) => putPoint({ ...point, ...patch });
  return (
    <section className={inspector.section} data-testid="plumbing-section">
      <h2 className={inspector.title}>
        {t(kindNames[point.kind])} {systemCodes[point.system]}
      </h2>
      <CommitField
        label={t("plumbing.label")}
        value={point.label ?? ""}
        onCommit={(label) => update({ label })}
      />
      <CommitField
        label={t("plumbing.h")}
        unit={t("unit.mm")}
        numeric
        value={String(Math.round(point.height))}
        onCommit={amount((height) => update({ height }))}
      />
      <CommitField
        label={t("plumbing.diameter")}
        unit={t("unit.mm")}
        numeric
        value={String(point.diameter)}
        onCommit={amount((diameter) => update({ diameter }), true)}
      />
    </section>
  );
}

/** Свойства трубы: диаметр, длина, уклон (канализация — правится, отметки пересчитываются). */
export function PipeSection({ pipe }: { pipe: Pipe }) {
  const report = useEditor((s) => s.plumbing?.pipes.find((p) => p.id === pipe.id));
  const slope = pipeSlope(pipe.points);
  return (
    <section className={inspector.section} data-testid="pipe-section">
      <h2 className={inspector.title}>
        {t("plumbing.pipeTitle", { code: systemCodes[pipe.system] })}
      </h2>
      <CommitField
        label={t("plumbing.diameter")}
        unit={t("unit.mm")}
        numeric
        value={String(pipe.diameter)}
        onCommit={amount((diameter) => putPipe({ ...pipe, diameter }), true)}
      />
      <CommitField
        label={t("plumbing.length")}
        unit={t("unit.m")}
        readOnly
        numeric
        value={formatDecimal(report?.length ?? 0)}
        onCommit={() => {}}
      />
      {pipe.system === "drain" && (
        <CommitField
          label={t("plumbing.slope")}
          unit={t("unit.percent")}
          numeric
          value={slope === null ? "" : formatDecimal(Math.round(slope * 1000) / 10)}
          onCommit={amount((percent) =>
            putPipe({ ...pipe, points: withSlope(pipe.points, percent / 100) }),
          )}
        />
      )}
      {report?.warnings.map((w) => (
        <p key={w} className={inspector.error} role="alert">
          {w}
        </p>
      ))}
    </section>
  );
}
