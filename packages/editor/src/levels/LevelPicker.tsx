import { IconButton } from "@openplanner/ui";
import { sendToEngine } from "../engine";
import { t } from "../i18n";
import { Icon } from "../shell/icons";
import styles from "../shell/TopBar.module.css";
import { useEditor } from "../store/editor";
import { levelList, setLevel } from "./levels";

/** Этаж в верхней панели: выбор и добавление нового этажа. */
export function LevelPicker() {
  const doc = useEditor((s) => s.document);
  const level = useEditor((s) => s.level);
  if (!doc) return null;
  const levels = levelList(doc);
  return (
    <div className={styles.group} data-testid="levels">
      {levels.length > 1 && (
        <select
          className={styles.level}
          aria-label={t("levels.current")}
          value={level ?? ""}
          onChange={(e) => setLevel(e.target.value || null)}
        >
          {levels.map((l) => (
            <option key={l.id ?? ""} value={l.id ?? ""}>
              {l.name}
            </option>
          ))}
        </select>
      )}
      <IconButton
        label={t("levels.add")}
        onClick={() => sendToEngine({ type: "addLevel", name: "" })}
      >
        <Icon name="plus" />
      </IconButton>
    </div>
  );
}
