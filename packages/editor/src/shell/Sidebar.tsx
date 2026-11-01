import { IconButton } from "@openplanner/ui";
import { t } from "../i18n";
import { Icon, type IconName } from "./icons";
import styles from "./Sidebar.module.css";

const tools: readonly { icon: IconName; label: string }[] = [
  { icon: "select", label: "Выбор · V" },
  { icon: "wall", label: "Стена · W" },
  { icon: "door", label: "Проём · D" },
  { icon: "room", label: "Помещение · R" },
  { icon: "ruler", label: "Размер · M" },
];

/** Панель инструментов режима. На стадии 0 инструменты ещё не работают. */
export function Toolbar() {
  return (
    <nav className={styles.toolbar} aria-label="Инструменты">
      {tools.map((tool) => (
        <IconButton key={tool.icon} label={tool.label} disabled>
          <Icon name={tool.icon} />
        </IconButton>
      ))}
    </nav>
  );
}

/** Левая панель: дерево объектов и слои. */
export function Sidebar() {
  return (
    <aside className={styles.panel} aria-label={t("sidebar.objects")}>
      <div className={styles.tabs}>
        <span>{t("sidebar.objects")}</span>
        <span className={styles.inactive}>{t("sidebar.layers")}</span>
      </div>
      <p className={styles.empty}>{t("sidebar.empty")}</p>
    </aside>
  );
}
