import type { DeviceKind, Document, Layer, OpeningKind, View } from "@openplanner/engine-client";
import { IconButton } from "@openplanner/ui";
import { type KeyboardEvent, type MouseEvent, useState } from "react";
import { addUnderlay, isEnabled, toolActions } from "../actions";
import { format, isMacPlatform } from "../actions/shortcut";
import { ElectricalPanel } from "../electric/ElectricalPanel";
import { sendToEngine } from "../engine";
import { Catalog } from "../furniture/Catalog";
import { formatNumber, type MessageKey, t } from "../i18n";
import { formatArea } from "../inspector/format";
import { openScanWizard } from "../scan/store";
import { type DerivedInfo, type SidebarTab, useEditor } from "../store/editor";
import { demolishedIds, type Status, statusOf, viewVariant, visibleIn } from "../variants/status";
import { Variants } from "../variants/Variants";
import { Icon, type IconName } from "./icons";
import styles from "./Sidebar.module.css";

const toolIcons: Record<string, IconName> = {
  "tool.select": "select",
  "tool.wall": "wall",
  "tool.door": "door",
  "tool.window": "window",
  "tool.arch": "arch",
  "tool.room": "room",
  "tool.dimension": "ruler",
  "tool.leader": "leader",
  "tool.calibrate": "calibrate",
  "tool.demolish": "demolish",
};

/** Панель инструментов режима «Обмер». */
export function Toolbar() {
  const current = useEditor((s) => s.tool);
  const ready = useEditor((s) => s.engine.state === "ready");
  // Подписка на вид и 3D: доступность инструментов зависит от них.
  useEditor((s) => s.view);
  useEditor((s) => s.three.options.enabled);
  return (
    <nav className={styles.toolbar} aria-label={t("tools.label")}>
      {toolActions.map((action) => {
        const key = action.keys?.[0];
        const label = key ? `${action.title} · ${format(key, isMacPlatform)}` : action.title;
        const icon = toolIcons[action.id] ?? "select";
        return (
          <IconButton
            key={action.id}
            label={label}
            aria-pressed={`tool.${current}` === action.id}
            disabled={!ready || !isEnabled(action)}
            onClick={action.run}
            data-tool={action.id.slice(5)}
          >
            <Icon name={icon} />
          </IconButton>
        );
      })}
      <div className={styles.separator} />
      <IconButton label={t("tool.importScan")} disabled={!ready} onClick={openScanWizard}>
        <Icon name="scan" />
      </IconButton>
      <IconButton label={t("tool.underlay")} disabled={!ready} onClick={() => void addUnderlay()}>
        <Icon name="image" />
      </IconButton>
    </nav>
  );
}

const tabTitles: Record<SidebarTab, MessageKey> = {
  objects: "sidebar.objects",
  catalog: "sidebar.catalog",
  layers: "sidebar.layers",
  variants: "sidebar.variants",
  electrical: "sidebar.electrical",
};

/** Левая панель: дерево объектов, каталог мебели, слои, варианты. */
export function Sidebar() {
  const selected = useEditor((s) => s.sidebarTab);
  const setTab = useEditor((s) => s.setSidebarTab);
  const mode = useEditor((s) => s.mode);
  // Вкладка щита нужна только в режимах «Электрика» и «Освещение».
  const electric = mode === "electric" || mode === "light";
  const tabs = (["objects", "catalog", "electrical", "layers", "variants"] as const).filter(
    (value) => value !== "electrical" || electric,
  );
  const tab = selected === "electrical" && !electric ? "objects" : selected;
  return (
    <aside className={styles.panel} aria-label={t("sidebar.objects")}>
      <div className={styles.tabs} role="tablist" aria-label={t("sidebar.tabs")}>
        {tabs.map((value) => (
          <button
            key={value}
            type="button"
            role="tab"
            aria-selected={tab === value}
            className={styles.tab}
            onClick={() => setTab(value)}
          >
            {t(tabTitles[value])}
          </button>
        ))}
      </div>
      <div className={styles.body} role="tabpanel">
        {tab === "objects" ? (
          <ObjectTree />
        ) : tab === "catalog" ? (
          <Catalog />
        ) : tab === "electrical" ? (
          <ElectricalPanel />
        ) : tab === "layers" ? (
          <Layers />
        ) : (
          <Variants />
        )}
      </div>
    </aside>
  );
}

/** Короткие названия электроточек для дерева объектов. */
const deviceNames: Record<DeviceKind, string> = {
  socket: "Розетка",
  socketDouble: "Розетка двойная",
  socketWaterproof: "Розетка IP44",
  socketPower: "Розетка 380 В",
  socketData: "Розетка LAN",
  socketTv: "Розетка ТВ",
  cableOutlet: "Вывод кабеля",
  floorOutlet: "Напольный лючок",
  ceilingOutlet: "Вывод на потолке",
  switch1: "Выключатель",
  switch2: "Выключатель 2-кл.",
  switch3: "Выключатель 3-кл.",
  passSwitch: "Проходной выключатель",
  crossSwitch: "Перекрёстный выключатель",
  ceilingLight: "Светильник",
  pendant: "Подвес",
  spot: "Точечный светильник",
  wallLight: "Бра",
  ledStrip: "Светодиодная лента",
  junctionBox: "Распредкоробка",
  panel: "Электрощит",
};

interface TreeItem {
  id: string;
  label: string;
  meta?: string;
  status?: Status | null;
}

const openingKeys: Record<OpeningKind, MessageKey> = {
  door: "item.door",
  window: "item.window",
  arch: "item.arch",
};

function mm(value: number | undefined): string {
  return value === undefined ? "—" : formatNumber(Math.round(value));
}

/** Группы дерева объектов в порядке показа. */
export function treeGroups(
  doc: Document,
  derived: DerivedInfo,
  view: View = { type: "survey" },
): { key: MessageKey; items: TreeItem[] }[] {
  const demolished = demolishedIds(doc, viewVariant(view));
  const visible = <T extends { id: string; variant?: string }>(list: T[]) =>
    list.filter((e) => visibleIn(view, e.variant));
  const status = (e: { id: string; variant?: string }) =>
    statusOf(view, demolished, e.id, e.variant);
  const groups = [
    {
      key: "group.rooms",
      items: visible(doc.rooms).map((r) => {
        const info = derived.rooms[r.id];
        return {
          id: r.id,
          label: r.name,
          meta: info?.closed ? formatArea(info.area) : "—",
          status: status(r),
        };
      }),
    },
    {
      key: "group.walls",
      items: visible(doc.walls).map((w) => ({
        id: w.id,
        label: t("item.wall", { length: mm(derived.walls[w.id]) }),
        meta: formatNumber(w.thickness),
        status: status(w),
      })),
    },
    {
      key: "group.openings",
      items: visible(doc.openings).map((o) => ({
        id: o.id,
        label: t(openingKeys[o.kind], { width: formatNumber(o.width) }),
        status: status(o),
      })),
    },
    {
      key: "group.dimensions",
      items: visible(doc.dimensions).map((d) => ({
        id: d.id,
        label: t("item.dimension", { length: mm(derived.dimensions[d.id]) }),
      })),
    },
    {
      key: "group.leaders",
      items: visible(doc.leaders).map((l) => ({ id: l.id, label: l.text || "—" })),
    },
    {
      key: "group.furniture",
      items: visible(doc.furniture ?? []).map((f) => ({
        id: f.id,
        label: f.name,
        meta: `${formatNumber(f.width)}×${formatNumber(f.depth)}`,
        status: status(f),
      })),
    },
    {
      key: "group.devices",
      items: visible(doc.devices ?? []).map((d) => ({
        id: d.id,
        label: d.label ? `${deviceNames[d.kind]} · ${d.label}` : deviceNames[d.kind],
        meta: `H ${formatNumber(d.height)}`,
        status: status(d),
      })),
    },
    {
      key: "group.underlays",
      items: doc.underlays.map((u) => ({ id: u.id, label: u.name })),
    },
  ];
  return groups.filter((group) => group.items.length > 0) as {
    key: MessageKey;
    items: TreeItem[];
  }[];
}

function ObjectTree() {
  const doc = useEditor((s) => s.document);
  const derived = useEditor((s) => s.derived);
  const selection = useEditor((s) => s.selection);
  const view = useEditor((s) => s.view);
  if (!doc) return null;
  const groups = treeGroups(doc, derived, view);
  if (groups.length === 0) return <p className={styles.empty}>{t("sidebar.empty")}</p>;

  const pick = (event: MouseEvent, id: string) => {
    const additive = event.shiftKey || event.metaKey || event.ctrlKey;
    const ids = additive
      ? selection.includes(id)
        ? selection.filter((s) => s !== id)
        : [...selection, id]
      : [id];
    sendToEngine({ type: "select", ids });
  };

  return (
    <div className={styles.tree} data-testid="object-tree">
      {groups.map((group) => (
        <section key={group.key} className={styles.group}>
          <h3 className={styles.groupTitle}>
            {t(group.key)}
            <span className={styles.count}>{group.items.length}</span>
          </h3>
          <ul className={styles.list}>
            {group.items.map((item) => (
              <li key={item.id}>
                <button
                  type="button"
                  className={styles.item}
                  aria-pressed={selection.includes(item.id)}
                  onClick={(event) => pick(event, item.id)}
                >
                  <span className={styles.itemLabel}>{item.label}</span>
                  {item.status && item.status !== "existing" && (
                    <span className={styles.status} data-status={item.status}>
                      {t(item.status === "new" ? "status.new" : "status.demolish")}
                    </span>
                  )}
                  {item.meta && <span className={styles.meta}>{item.meta}</span>}
                </button>
              </li>
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}

function putLayer(layer: Layer) {
  sendToEngine({
    type: "command",
    command: { type: "put", element: { type: "layer", ...layer } },
  });
}

function layerUsage(doc: Document, id: string): number {
  return [
    doc.walls,
    doc.openings,
    doc.rooms,
    doc.dimensions,
    doc.leaders,
    doc.underlays,
    doc.furniture ?? [],
    doc.devices ?? [],
  ].reduce((sum, list) => sum + list.filter((e) => e.layer === id).length, 0);
}

function Layers() {
  const doc = useEditor((s) => s.document);
  if (!doc) return null;

  const add = () => {
    const taken = new Set(doc.layers.map((l) => l.name));
    let n = doc.layers.filter((l) => !l.system).length + 1;
    while (taken.has(t("layers.newName", { n }))) n++;
    putLayer({
      id: `layer-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`,
      name: t("layers.newName", { n }),
      visible: true,
      locked: false,
    });
  };

  return (
    <div className={styles.tree} data-testid="layers">
      <ul className={styles.list}>
        {[...doc.layers].reverse().map((layer) => (
          <LayerRow key={layer.id} layer={layer} count={layerUsage(doc, layer.id)} />
        ))}
      </ul>
      <button type="button" className={styles.add} onClick={add}>
        <Icon name="plus" />
        {t("layers.add")}
      </button>
    </div>
  );
}

function LayerRow({ layer, count }: { layer: Layer; count: number }) {
  const [draft, setDraft] = useState(layer.name);
  const commit = () => {
    const name = draft.trim();
    if (name && name !== layer.name) putLayer({ ...layer, name });
    else setDraft(layer.name);
  };
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") event.currentTarget.blur();
    if (event.key === "Escape") {
      setDraft(layer.name);
      const input = event.currentTarget;
      requestAnimationFrame(() => input.blur());
    }
  };
  return (
    <li className={styles.layer} data-layer={layer.id}>
      <IconButton
        label={t(layer.visible ? "layers.hide" : "layers.show")}
        aria-pressed={!layer.visible}
        onClick={() => putLayer({ ...layer, visible: !layer.visible })}
      >
        <Icon name={layer.visible ? "eye" : "eyeOff"} />
      </IconButton>
      <IconButton
        label={t(layer.locked ? "layers.unlock" : "layers.lock")}
        aria-pressed={layer.locked}
        onClick={() => putLayer({ ...layer, locked: !layer.locked })}
      >
        <Icon name={layer.locked ? "lock" : "unlock"} />
      </IconButton>
      <input
        className={styles.layerName}
        aria-label={t("layers.rename")}
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
        onKeyDown={onKeyDown}
      />
      <span className={styles.count}>{count}</span>
      {!layer.system && count === 0 && (
        <IconButton
          label={t("layers.remove")}
          onClick={() =>
            sendToEngine({ type: "command", command: { type: "remove", id: layer.id } })
          }
        >
          <Icon name="trash" />
        </IconButton>
      )}
    </li>
  );
}
