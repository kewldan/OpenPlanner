import { useState } from "react";
import { sendToEngine } from "../engine";
import styles from "../furniture/Catalog.module.css";
import { t } from "../i18n";
import { useEditor } from "../store/editor";
import { pipeTools, plumbingGroups, systemCodes } from "./points";

/** Каталог сантехники: выводы, стояки, коллекторы и трубы по системам. */
export function PlumbingCatalog() {
  const catalog = useEditor((s) => s.plumbingCatalog);
  const tool = useEditor((s) => s.tool);
  const [active, setActive] = useState<string | null>(null);
  return (
    <div className={styles.root} data-testid="plumbing-catalog">
      <p className={styles.hint}>{t("plumbing.hint")}</p>
      {plumbingGroups(catalog).map(([group, items]) => (
        <section key={group} className={styles.group}>
          <h3 className={styles.title}>{group}</h3>
          <ul className={styles.grid}>
            {items.map((item) => (
              <li key={item.id}>
                <button
                  type="button"
                  className={styles.item}
                  data-system={item.system}
                  aria-pressed={tool === "plumbing" && active === item.id}
                  onClick={() => {
                    setActive(item.id);
                    sendToEngine({ type: "plumbingTool", item: item.id });
                  }}
                >
                  <svg viewBox="0 0 100 100" aria-hidden="true">
                    <path d={item.icon} />
                  </svg>
                  <span className={styles.name}>{item.name}</span>
                  <span className={styles.size}>
                    {t("plumbing.height", { h: item.height, d: item.diameter })}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      ))}
      <section className={styles.group}>
        <h3 className={styles.title}>{t("plumbing.pipes")}</h3>
        <ul className={styles.grid}>
          {pipeTools.map((p) => {
            const id = `${p.system}-${p.diameter}`;
            return (
              <li key={id}>
                <button
                  type="button"
                  className={styles.item}
                  data-system={p.system}
                  aria-pressed={tool === "pipe" && active === id}
                  onClick={() => {
                    setActive(id);
                    sendToEngine({ type: "pipeTool", system: p.system, diameter: p.diameter });
                  }}
                >
                  <svg viewBox="0 0 100 100" aria-hidden="true">
                    <path
                      d="M15 70L50 70L50 30L85 30"
                      strokeDasharray={p.system === "drain" ? "10 6" : undefined}
                    />
                  </svg>
                  <span className={styles.name}>
                    {t("plumbing.pipe", { code: systemCodes[p.system], d: p.diameter })}
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      </section>
    </div>
  );
}
