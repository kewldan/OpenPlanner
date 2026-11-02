import { Button } from "@openplanner/ui";
import { useState } from "react";
import { DeviceCatalog } from "../electric/DeviceCatalog";
import { formatNumber, t } from "../i18n";
import { useEditor } from "../store/editor";
import styles from "./Catalog.module.css";
import { byCategory, DRAG_TYPE, importModel, pickItem, searchCatalog } from "./items";

/** Каталог мебели: поиск, разделы, позиции с обозначениями; щелчок или перетаскивание на план. */
export function Catalog() {
  const mode = useEditor((s) => s.mode);
  if (mode === "electric" || mode === "light") return <DeviceCatalog lighting={mode === "light"} />;
  return <FurnitureCatalog />;
}

function FurnitureCatalog() {
  const catalog = useEditor((s) => s.catalog);
  const tool = useEditor((s) => s.tool);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState<string | null>(null);
  const groups = byCategory(searchCatalog(catalog, query));

  return (
    <div className={styles.root} data-testid="catalog">
      <input
        className={styles.search}
        type="search"
        placeholder={t("catalog.search")}
        aria-label={t("catalog.search")}
        value={query}
        onChange={(event) => setQuery(event.target.value)}
      />
      <p className={styles.hint}>{t("catalog.hint")}</p>
      {groups.length === 0 && <p className={styles.hint}>{t("catalog.empty")}</p>}
      {groups.map(([category, items]) => (
        <section key={category} className={styles.group}>
          <h3 className={styles.title}>{category}</h3>
          <ul className={styles.grid}>
            {items.map((item) => (
              <li key={item.id}>
                <button
                  type="button"
                  className={styles.item}
                  draggable
                  aria-pressed={tool === "furniture" && active === item.id}
                  onClick={() => {
                    setActive(item.id);
                    pickItem(item.id);
                  }}
                  onDragStart={(event) => {
                    setActive(item.id);
                    pickItem(item.id);
                    event.dataTransfer.setData(DRAG_TYPE, item.id);
                    event.dataTransfer.effectAllowed = "copy";
                  }}
                >
                  <svg viewBox="0 0 100 100" aria-hidden="true">
                    <path d={item.icon} />
                  </svg>
                  <span className={styles.name}>{item.name}</span>
                  <span className={styles.size}>
                    {t("catalog.size", {
                      width: formatNumber(item.width),
                      depth: formatNumber(item.depth),
                    })}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      ))}
      <Button variant="outline" onClick={() => void importModel()}>
        {t("catalog.import")}
      </Button>
    </div>
  );
}
