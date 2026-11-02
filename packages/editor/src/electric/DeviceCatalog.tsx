import { useState } from "react";
import { sendToEngine } from "../engine";
import styles from "../furniture/Catalog.module.css";
import { DRAG_TYPE } from "../furniture/items";
import { t } from "../i18n";
import { useEditor } from "../store/editor";
import { deviceEntries, deviceGroups } from "./devices";

/** Каталог электроточек: «Электрика» — розетки, выводы, щит; «Освещение» — светильники и выключатели. */
export function DeviceCatalog({ lighting }: { lighting: boolean }) {
  const catalog = useEditor((s) => s.deviceCatalog);
  const tool = useEditor((s) => s.tool);
  const [active, setActive] = useState<string | null>(null);
  const groups = deviceGroups(deviceEntries(catalog, lighting));
  const pick = (kind: (typeof catalog)[number]["kind"]) => {
    setActive(kind);
    sendToEngine({ type: "deviceTool", kind });
  };
  return (
    <div className={styles.root} data-testid="device-catalog">
      <p className={styles.hint}>{t("devices.hint")}</p>
      {groups.map(([group, items]) => (
        <section key={group} className={styles.group}>
          <h3 className={styles.title}>{group}</h3>
          <ul className={styles.grid}>
            {items.map((item) => (
              <li key={item.kind}>
                <button
                  type="button"
                  className={styles.item}
                  draggable
                  aria-pressed={tool === "device" && active === item.kind}
                  onClick={() => pick(item.kind)}
                  onDragStart={(event) => {
                    pick(item.kind);
                    event.dataTransfer.setData(DRAG_TYPE, item.kind);
                    event.dataTransfer.effectAllowed = "copy";
                  }}
                >
                  <svg viewBox="0 0 100 100" aria-hidden="true">
                    <path d={item.icon} />
                  </svg>
                  <span className={styles.name}>{item.name}</span>
                  {item.onWall && (
                    <span className={styles.size}>{t("devices.height", { h: item.height })}</span>
                  )}
                </button>
              </li>
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}
