import type {
  Device,
  Dimension,
  Document,
  Element,
  Furniture,
  Justification,
  Leader,
  Opening,
  OpeningKind,
  Pipe,
  PlumbingPoint,
  ProjectMeta,
  Room,
  Underlay,
  Wall,
  WallMaterial,
} from "@openplanner/engine-client";
import { Button, Segmented } from "@openplanner/ui";
import type { ReactNode } from "react";
import { DeviceSection } from "../electric/DeviceSection";
import { sendToEngine } from "../engine";
import { formatNumber, type MessageKey, t } from "../i18n";
import { PipeSection, PlumbingPointSection } from "../plumbing/PlumbingSection";
import { useEditor } from "../store/editor";
import { demolishedIds, statusOf, viewVariant } from "../variants/status";
import { View3dPanel } from "../view3d/Panel";
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

type MaterialChoice = WallMaterial | "none";

const materials: readonly { value: MaterialChoice; label: string }[] = [
  { value: "none", label: t("material.none") },
  { value: "aeratedConcrete", label: t("material.aeratedConcrete") },
  { value: "gypsum", label: t("material.gypsum") },
  { value: "brick", label: t("material.brick") },
];

/** Толщина перегородки по умолчанию для материала, мм. */
const materialThickness: Record<WallMaterial, number> = {
  aeratedConcrete: 100,
  gypsum: 100,
  brick: 120,
};

function MaterialSelect({
  value,
  onChange,
}: {
  value: WallMaterial | undefined;
  onChange: (m: WallMaterial | undefined) => void;
}) {
  return (
    <Row label={t("material.label")}>
      <select
        className={styles.select}
        aria-label={t("material.label")}
        value={value ?? "none"}
        onChange={(event) => {
          const next = event.target.value as MaterialChoice;
          onChange(next === "none" ? undefined : next);
        }}
      >
        {materials.map((m) => (
          <option key={m.value} value={m.value}>
            {m.label}
          </option>
        ))}
      </select>
    </Row>
  );
}

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
  | { type: "underlay"; element: Underlay }
  | { type: "furniture"; element: Furniture }
  | { type: "device"; element: Device }
  | { type: "plumbingPoint"; element: PlumbingPoint }
  | { type: "pipe"; element: Pipe };

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
  const furniture = (doc.furniture ?? []).find((e) => e.id === id);
  if (furniture) return { type: "furniture", element: furniture };
  const device = (doc.devices ?? []).find((e) => e.id === id);
  if (device) return { type: "device", element: device };
  const point = (doc.plumbing ?? []).find((e) => e.id === id);
  if (point) return { type: "plumbingPoint", element: point };
  const pipe = (doc.pipes ?? []).find((e) => e.id === id);
  if (pipe) return { type: "pipe", element: pipe };
  return null;
}

/** Правая панель: свойства выделенного, параметры инструмента, проект. */
export function Inspector() {
  const document = useEditor((s) => s.document);
  const selection = useEditor((s) => s.selection);
  const tool = useEditor((s) => s.tool);
  const lastError = useEditor((s) => s.lastError);
  const freeRegions = useEditor((s) => s.derived.freeRegions);
  const in3d = useEditor((s) => s.three.options.enabled);

  if (in3d && document) {
    return (
      <aside className={styles.root} aria-label={t("view3d.title")} data-testid="inspector">
        <View3dPanel document={document} />
        {lastError && (
          <p className={styles.error} role="alert">
            {lastError}
          </p>
        )}
      </aside>
    );
  }

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
      {selection.length === 0 && document && <TitleBlockSection meta={document.meta} />}
      {lastError && (
        <p className={styles.error} role="alert">
          {lastError}
        </p>
      )}
    </aside>
  );
}

const titleBlockFields = [
  ["designation", "inspector.designation"],
  ["stage", "inspector.stage"],
  ["organization", "inspector.organization"],
  ["developer", "inspector.developer"],
  ["checker", "inspector.checker"],
] as const satisfies readonly (readonly [keyof ProjectMeta, MessageKey])[];

/** Графы основной надписи листов: правка — команда `setMeta` со всеми сведениями. */
function TitleBlockSection({ meta }: { meta: ProjectMeta }) {
  return (
    <Section title={t("inspector.titleBlock")}>
      {titleBlockFields.map(([field, label]) => (
        <CommitField
          key={field}
          label={t(label)}
          value={meta[field] ?? ""}
          onCommit={(value) =>
            sendToEngine({
              type: "command",
              command: { type: "setMeta", meta: { ...meta, [field]: value } },
            })
          }
        />
      ))}
    </Section>
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
  const replan = useEditor((s) => s.view.type === "variant");
  return (
    <Section title={t("inspector.newWalls")}>
      {replan && (
        <MaterialSelect
          value={options.material}
          onChange={(material) =>
            sendToEngine({
              type: "setToolOptions",
              options: material
                ? { ...options, material, wallThickness: materialThickness[material] }
                : {
                    wallThickness: options.wallThickness,
                    justification: options.justification,
                  },
            })
          }
        />
      )}
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
  const view = useEditor((s) => s.view);
  const variant = viewVariant(view);
  const element = selected.element as { id: string; variant?: string };
  const status = statusOf(view, demolishedIds(document, variant), element.id, element.variant);
  // В варианте элементы обмера не правятся — только статус.
  if (status === "existing" || status === "demolish") {
    return <StatusSection selected={selected} demolished={status === "demolish"} />;
  }
  return (
    <>
      {status === "new" && (
        <p className={styles.badge} data-testid="status-new">
          {t("status.new")}
        </p>
      )}
      <ElementFields selected={selected} document={document} />
    </>
  );
}

const sectionTitles: Record<Selected["type"], () => string> = {
  wall: () => t("inspector.wall"),
  opening: () => t("inspector.kind"),
  room: () => t("inspector.room"),
  dimension: () => t("inspector.dimension"),
  leader: () => t("inspector.leader"),
  underlay: () => t("inspector.underlay"),
  furniture: () => t("inspector.furniture"),
  device: () => t("devices.device"),
  plumbingPoint: () => t("inspector.plumbing"),
  pipe: () => t("inspector.pipe"),
};

function StatusSection({ selected, demolished }: { selected: Selected; demolished: boolean }) {
  const length = useEditor((s) =>
    selected.type === "wall" ? s.derived.walls[selected.element.id] : undefined,
  );
  const area = useEditor((s) =>
    selected.type === "room" ? s.derived.rooms[selected.element.id] : undefined,
  );
  const title =
    selected.type === "opening"
      ? t(`opening.${selected.element.kind}`)
      : sectionTitles[selected.type]();
  const demolishable =
    selected.type === "wall" ||
    selected.type === "opening" ||
    selected.type === "room" ||
    selected.type === "furniture" ||
    selected.type === "device" ||
    selected.type === "plumbingPoint" ||
    selected.type === "pipe";
  return (
    <Section title={title}>
      {selected.type === "wall" && length !== undefined && (
        <CommitField
          label={t("inspector.length")}
          readOnly
          numeric
          unit={t("unit.mm")}
          value={String(Math.round(length))}
          onCommit={() => {}}
        />
      )}
      {selected.type === "room" && (
        <CommitField
          label={t("inspector.name")}
          readOnly
          value={selected.element.name}
          onCommit={() => {}}
        />
      )}
      {selected.type === "room" && area?.closed && !demolished && (
        <CommitField
          label={t("inspector.area")}
          readOnly
          numeric
          value={formatArea(area.area)}
          onCommit={() => {}}
        />
      )}
      {demolishable && (
        <Row label={t("status.label")} stack>
          <Segmented
            label={t("status.label")}
            value={demolished ? "demolish" : "existing"}
            options={[
              { value: "existing", label: t("status.existing") },
              { value: "demolish", label: t("status.demolish") },
            ]}
            onChange={(next) =>
              sendToEngine({
                type: "setDemolished",
                ids: [selected.element.id],
                demolished: next === "demolish",
              })
            }
          />
        </Row>
      )}
    </Section>
  );
}

function ElementFields({ selected, document }: { selected: Selected; document: Document }) {
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
    case "furniture":
      return <FurnitureSection furniture={selected.element} document={document} />;
    case "device":
      return (
        <>
          <DeviceSection device={selected.element} document={document} />
          <Section title="">
            <LayerSelect
              document={document}
              value={selected.element.layer}
              onChange={(layer) => put({ type: "device", ...selected.element, layer })}
            />
            <DeleteButton />
          </Section>
        </>
      );
    case "plumbingPoint":
      return (
        <>
          <PlumbingPointSection point={selected.element} />
          <Section title="">
            <LayerSelect
              document={document}
              value={selected.element.layer}
              onChange={(layer) => put({ type: "plumbingPoint", ...selected.element, layer })}
            />
            <DeleteButton />
          </Section>
        </>
      );
    case "pipe":
      return (
        <>
          <PipeSection pipe={selected.element} />
          <Section title="">
            <LayerSelect
              document={document}
              value={selected.element.layer}
              onChange={(layer) => put({ type: "pipe", ...selected.element, layer })}
            />
            <DeleteButton />
          </Section>
        </>
      );
  }
}

function FurnitureSection({ furniture, document }: { furniture: Furniture; document: Document }) {
  const catalog = useEditor((s) => s.catalog);
  const update = (patch: Partial<Furniture>) => put({ type: "furniture", ...furniture, ...patch });
  const positive = (apply: (value: number) => void) =>
    numberCommit((value) => {
      if (value > 0) apply(value);
      else useEditor.setState({ lastError: "значение должно быть больше нуля" });
    });
  const item = catalog.find((c) => c.id === furniture.item)?.name ?? t("catalog.custom");
  return (
    <Section title={t("inspector.furniture")}>
      <CommitField
        label={t("inspector.name")}
        value={furniture.name}
        onCommit={(name) => update({ name: name.trim() || furniture.name })}
      />
      <CommitField label={t("inspector.catalogItem")} readOnly value={item} onCommit={() => {}} />
      <CommitField
        label={t("inspector.width")}
        unit={t("unit.mm")}
        numeric
        value={String(Math.round(furniture.width))}
        onCommit={positive((width) => update({ width }))}
      />
      <CommitField
        label={t("inspector.depth")}
        unit={t("unit.mm")}
        numeric
        value={String(Math.round(furniture.depth))}
        onCommit={positive((depth) => update({ depth }))}
      />
      <CommitField
        label={t("inspector.height")}
        unit={t("unit.mm")}
        numeric
        value={String(Math.round(furniture.height))}
        onCommit={positive((height) => update({ height }))}
      />
      <CommitField
        label={t("inspector.elevation")}
        unit={t("unit.mm")}
        numeric
        value={String(Math.round(furniture.elevation ?? 0))}
        onCommit={numberCommit((elevation) => update({ elevation: Math.max(0, elevation) }))}
      />
      <CommitField
        label={t("inspector.rotation")}
        unit={t("unit.deg")}
        numeric
        value={formatNumber(furniture.rotation ?? 0)}
        onCommit={numberCommit((rotation) => update({ rotation: ((rotation % 360) + 360) % 360 }))}
      />
      <div className={styles.buttons}>
        <Button
          variant="outline"
          onClick={() => sendToEngine({ type: "transformFurniture", rotate: 90, mirror: false })}
        >
          {t("inspector.rotate")}
        </Button>
        <Button
          variant="outline"
          onClick={() => sendToEngine({ type: "transformFurniture", rotate: 0, mirror: true })}
        >
          {t("inspector.mirror")}
        </Button>
      </div>
      <LayerSelect
        document={document}
        value={furniture.layer}
        onChange={(layer) => update({ layer })}
      />
      <DeleteButton />
    </Section>
  );
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
      {wall.variant !== undefined && (
        <MaterialSelect
          value={wall.material}
          onChange={(material) => {
            const { material: _old, ...rest } = wall;
            put(
              material
                ? { type: "wall", ...rest, material, thickness: materialThickness[material] }
                : { type: "wall", ...rest },
            );
          }}
        />
      )}
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
