import type {
  Dimension,
  Document,
  Element,
  Justification,
  Leader,
  Opening,
  OpeningKind,
  Room,
  Underlay,
  Wall,
} from "@openplanner/engine-client";
import { Button, Segmented } from "@openplanner/ui";
import type { ReactNode } from "react";
import { sendToEngine } from "../engine";
import { formatNumber, t } from "../i18n";
import { useEditor } from "../store/editor";
import { CommitField } from "./CommitField";
import { formatArea, formatDecimal, parseNumber } from "./format";
import styles from "./Inspector.module.css";

function put(element: Element) {
  sendToEngine({ type: "command", command: { type: "put", element } });
}

/** Число из поля: отправляет правку, только если введено число. */
function numberCommit(apply: (value: number) => void) {
  return (raw: string) => {
    const value = parseNumber(raw);
    if (value === null) {
      useEditor.setState({ lastError: "введите число" });
      return;
    }
    apply(value);
  };
}

const justifications: readonly { value: Justification; label: string }[] = [
  { value: "center", label: t("just.center") },
  { value: "left", label: t("just.left") },
  { value: "right", label: t("just.right") },
];

const openingKinds: readonly { value: OpeningKind; label: string }[] = [
  { value: "door", label: t("opening.door") },
  { value: "window", label: t("opening.window") },
  { value: "arch", label: t("opening.arch") },
];

type Selected =
  | { type: "wall"; element: Wall }
  | { type: "opening"; element: Opening }
  | { type: "room"; element: Room }
  | { type: "dimension"; element: Dimension }
  | { type: "leader"; element: Leader }
  | { type: "underlay"; element: Underlay };

/** Элемент документа по идентификатору. */
export function findElement(doc: Document, id: string): Selected | null {
  const wall = doc.walls.find((e) => e.id === id);
  if (wall) return { type: "wall", element: wall };
  const opening = doc.openings.find((e) => e.id === id);
  if (opening) return { type: "opening", element: opening };
  const room = doc.rooms.find((e) => e.id === id);
  if (room) return { type: "room", element: room };
  const dimension = doc.dimensions.find((e) => e.id === id);
  if (dimension) return { type: "dimension", element: dimension };
  const leader = doc.leaders.find((e) => e.id === id);
  if (leader) return { type: "leader", element: leader };
  const underlay = doc.underlays.find((e) => e.id === id);
  if (underlay) return { type: "underlay", element: underlay };
  return null;
}

/** Правая панель: свойства выделенного, параметры инструмента, проект. */
export function Inspector() {
  const document = useEditor((s) => s.document);
  const selection = useEditor((s) => s.selection);
  const tool = useEditor((s) => s.tool);
  const lastError = useEditor((s) => s.lastError);
  const freeRegions = useEditor((s) => s.derived.freeRegions);

  const single =
    document && selection.length === 1 ? findElement(document, selection[0] ?? "") : null;

  return (
    <aside className={styles.root} aria-label={t("inspector.project")} data-testid="inspector">
      {tool === "wall" && <WallToolSection />}
      {single && document && (
        <ElementSection key={single.element.id} selected={single} document={document} />
      )}
      {selection.length > 1 && (
        <Section title={t("inspector.selection", { count: selection.length })}>
          <DeleteButton />
        </Section>
      )}
      {selection.length === 0 && document && (
        <Section title={t("inspector.project")}>
          <CommitField
            label={t("inspector.title")}
            value={document.meta.title}
            onCommit={(title) =>
              sendToEngine({ type: "command", command: { type: "setTitle", title } })
            }
          />
          <CommitField
            label={t("inspector.address")}
            value={document.meta.address}
            onCommit={(address) =>
              sendToEngine({ type: "command", command: { type: "setAddress", address } })
            }
          />
          <CommitField
            label={t("inspector.ceiling")}
            unit={t("unit.mm")}
            numeric
            value={String(document.defaultCeilingHeight)}
            onCommit={numberCommit((height) =>
              sendToEngine({
                type: "command",
                command: { type: "setDefaultCeilingHeight", height },
              }),
            )}
          />
          {freeRegions > 0 && (
            <Button variant="outline" onClick={() => sendToEngine({ type: "findRooms" })}>
              {t("inspector.findRooms", { count: freeRegions })}
            </Button>
          )}
        </Section>
      )}
      {lastError && (
        <p className={styles.error} role="alert">
          {lastError}
        </p>
      )}
    </aside>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className={styles.section}>
      <h2 className={styles.title}>{title}</h2>
      {children}
    </section>
  );
}

function DeleteButton() {
  return (
    <Button variant="outline" onClick={() => sendToEngine({ type: "deleteSelection" })}>
      {t("inspector.delete")}
    </Button>
  );
}

function WallToolSection() {
  const options = useEditor((s) => s.toolOptions);
  return (
    <Section title={t("inspector.newWalls")}>
      <CommitField
        label={t("inspector.thickness")}
        unit={t("unit.mm")}
        numeric
        value={String(options.wallThickness)}
        onCommit={numberCommit((wallThickness) =>
          sendToEngine({ type: "setToolOptions", options: { ...options, wallThickness } }),
        )}
      />
      <Row label={t("inspector.justification")} stack>
        <Segmented
          label={t("inspector.justification")}
          value={options.justification}
          options={justifications}
          onChange={(justification) =>
            sendToEngine({ type: "setToolOptions", options: { ...options, justification } })
          }
        />
      </Row>
    </Section>
  );
}

function Row({ label, children, stack }: { label: string; children: ReactNode; stack?: boolean }) {
  return (
    <div className={[styles.row, stack && styles.stack].filter(Boolean).join(" ")}>
      <span className={styles.label}>{label}</span>
      <div className={styles.control}>{children}</div>
    </div>
  );
}

function LayerSelect({
  document,
  value,
  onChange,
}: {
  document: Document;
  value: string;
  onChange: (layer: string) => void;
}) {
  return (
    <Row label={t("inspector.layer")}>
      <select
        className={styles.select}
        aria-label={t("inspector.layer")}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      >
        {document.layers.map((layer) => (
          <option key={layer.id} value={layer.id}>
            {layer.name}
          </option>
        ))}
      </select>
    </Row>
  );
}

function ElementSection({ selected, document }: { selected: Selected; document: Document }) {
  switch (selected.type) {
    case "wall":
      return <WallSection wall={selected.element} document={document} />;
    case "opening":
      return <OpeningSection opening={selected.element} document={document} />;
    case "room":
      return <RoomSection room={selected.element} document={document} />;
    case "dimension":
      return <DimensionSection dimension={selected.element} document={document} />;
    case "leader":
      return <LeaderSection leader={selected.element} document={document} />;
    case "underlay":
      return <UnderlaySection underlay={selected.element} document={document} />;
  }
}

function WallSection({ wall, document }: { wall: Wall; document: Document }) {
  const length = useEditor((s) => s.derived.walls[wall.id]);
  const { height: _height, ...withoutHeight } = wall;
  return (
    <Section title={t("inspector.wall")}>
      <CommitField
        label={t("inspector.length")}
        unit={t("unit.mm")}
        numeric
        value={length === undefined ? "" : String(Math.round(length))}
        onCommit={numberCommit((value) =>
          sendToEngine({ type: "setWallLength", id: wall.id, length: value }),
        )}
      />
      <CommitField
        label={t("inspector.thickness")}
        unit={t("unit.mm")}
        numeric
        value={String(wall.thickness)}
        onCommit={numberCommit((thickness) => put({ type: "wall", ...wall, thickness }))}
      />
      <Row label={t("inspector.justification")} stack>
        <Segmented
          label={t("inspector.justification")}
          value={wall.justification}
          options={justifications}
          onChange={(justification) => put({ type: "wall", ...wall, justification })}
        />
      </Row>
      <CommitField
        label={t("inspector.height")}
        unit={t("unit.mm")}
        numeric
        placeholder={t("inspector.toCeiling")}
        value={wall.height === undefined ? "" : String(wall.height)}
        onCommit={(raw) => {
          if (raw.trim() === "") put({ type: "wall", ...withoutHeight });
          else numberCommit((height) => put({ type: "wall", ...wall, height }))(raw);
        }}
      />
      <LayerSelect
        document={document}
        value={wall.layer}
        onChange={(layer) => put({ type: "wall", ...wall, layer })}
      />
      <DeleteButton />
    </Section>
  );
}

function OpeningSection({ opening, document }: { opening: Opening; document: Document }) {
  const update = (patch: Partial<Opening>) => put({ type: "opening", ...opening, ...patch });
  const fromStart = opening.offset - opening.width / 2;
  return (
    <Section title={t(`opening.${opening.kind}`)}>
      <Row label={t("inspector.kind")} stack>
        <Segmented
          label={t("inspector.kind")}
          value={opening.kind}
          options={openingKinds}
          onChange={(kind) => update({ kind })}
        />
      </Row>
      <CommitField
        label={t("inspector.width")}
        unit={t("unit.mm")}
        numeric
        value={String(opening.width)}
        onCommit={numberCommit((width) => update({ width, offset: fromStart + width / 2 }))}
      />
      <CommitField
        label={t("inspector.height")}
        unit={t("unit.mm")}
        numeric
        value={String(opening.height)}
        onCommit={numberCommit((height) => update({ height }))}
      />
      <CommitField
        label={t("inspector.sill")}
        unit={t("unit.mm")}
        numeric
        value={String(opening.sill)}
        onCommit={numberCommit((sill) => update({ sill }))}
      />
      <CommitField
        label={t("inspector.fromStart")}
        unit={t("unit.mm")}
        numeric
        value={String(Math.round(fromStart))}
        onCommit={numberCommit((value) => update({ offset: value + opening.width / 2 }))}
      />
      {opening.kind === "door" && (
        <div className={styles.buttons}>
          <Button variant="outline" onClick={() => update({ hingeAtEnd: !opening.hingeAtEnd })}>
            {t("inspector.flipHinge")}
          </Button>
          <Button
            variant="outline"
            onClick={() => update({ swing: opening.swing === "left" ? "right" : "left" })}
          >
            {t("inspector.flipSwing")}
          </Button>
        </div>
      )}
      <LayerSelect
        document={document}
        value={opening.layer}
        onChange={(layer) => update({ layer })}
      />
      <DeleteButton />
    </Section>
  );
}

function RoomSection({ room, document }: { room: Room; document: Document }) {
  const info = useEditor((s) => s.derived.rooms[room.id]);
  const { ceilingHeight: _ceiling, ...withoutCeiling } = room;
  return (
    <Section title={t("inspector.room")}>
      <CommitField
        label={t("inspector.name")}
        value={room.name}
        onCommit={(name) => put({ type: "room", ...room, name })}
      />
      {info?.closed ? (
        <>
          <CommitField
            label={t("inspector.area")}
            readOnly
            numeric
            value={formatArea(info.area)}
            onCommit={() => {}}
          />
          <CommitField
            label={t("inspector.perimeter")}
            readOnly
            numeric
            unit={t("unit.m")}
            value={formatDecimal(info.perimeter)}
            onCommit={() => {}}
          />
        </>
      ) : (
        <p className={styles.error}>{t("inspector.notClosed")}</p>
      )}
      <CommitField
        label={t("inspector.ceiling")}
        unit={t("unit.mm")}
        numeric
        placeholder={formatNumber(document.defaultCeilingHeight)}
        value={room.ceilingHeight === undefined ? "" : String(room.ceilingHeight)}
        onCommit={(raw) => {
          if (raw.trim() === "") put({ type: "room", ...withoutCeiling });
          else numberCommit((ceilingHeight) => put({ type: "room", ...room, ceilingHeight }))(raw);
        }}
      />
      <LayerSelect
        document={document}
        value={room.layer}
        onChange={(layer) => put({ type: "room", ...room, layer })}
      />
      <DeleteButton />
    </Section>
  );
}

function DimensionSection({ dimension, document }: { dimension: Dimension; document: Document }) {
  const length = useEditor((s) => s.derived.dimensions[dimension.id]);
  return (
    <Section title={t("inspector.dimension")}>
      <CommitField
        label={t("inspector.length")}
        readOnly
        numeric
        unit={t("unit.mm")}
        value={length === undefined ? "—" : String(Math.round(length))}
        onCommit={() => {}}
      />
      <CommitField
        label={t("inspector.offset")}
        unit={t("unit.mm")}
        numeric
        value={String(Math.round(dimension.offset))}
        onCommit={numberCommit((offset) => put({ type: "dimension", ...dimension, offset }))}
      />
      <LayerSelect
        document={document}
        value={dimension.layer}
        onChange={(layer) => put({ type: "dimension", ...dimension, layer })}
      />
      <DeleteButton />
    </Section>
  );
}

function LeaderSection({ leader, document }: { leader: Leader; document: Document }) {
  const editText = useEditor((s) => s.editText);
  return (
    <Section title={t("inspector.leader")}>
      <CommitField
        label={t("inspector.text")}
        value={leader.text}
        autoSelect={editText === leader.id}
        onCommit={(text) => put({ type: "leader", ...leader, text })}
      />
      <LayerSelect
        document={document}
        value={leader.layer}
        onChange={(layer) => put({ type: "leader", ...leader, layer })}
      />
      <DeleteButton />
    </Section>
  );
}

function UnderlaySection({ underlay, document }: { underlay: Underlay; document: Document }) {
  const update = (patch: Partial<Underlay>) => put({ type: "underlay", ...underlay, ...patch });
  return (
    <Section title={t("inspector.underlay")}>
      <CommitField label={t("inspector.file")} readOnly value={underlay.name} onCommit={() => {}} />
      <CommitField
        label={t("inspector.scale")}
        unit={t("unit.mmPerPx")}
        numeric
        value={formatDecimal(underlay.mmPerPx)}
        onCommit={numberCommit((mmPerPx) => update({ mmPerPx }))}
      />
      <CommitField
        label={t("inspector.opacity")}
        unit={t("unit.percent")}
        numeric
        value={String(Math.round((1 - underlay.opacity) * 100))}
        onCommit={numberCommit((percent) =>
          update({ opacity: Math.min(1, Math.max(0, 1 - percent / 100)) }),
        )}
      />
      <Button
        variant="outline"
        onClick={() => sendToEngine({ type: "setTool", tool: "calibrate" })}
      >
        {t("inspector.calibrate")}
      </Button>
      <LayerSelect
        document={document}
        value={underlay.layer}
        onChange={(layer) => update({ layer })}
      />
      <DeleteButton />
    </Section>
  );
}
