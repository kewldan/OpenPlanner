import { Dialog, Kbd } from "@openplanner/ui";
import { type ActionGroup, actions, groupTitles } from "../actions";
import { format, isMacPlatform } from "../actions/shortcut";
import { useEditor } from "../store/editor";
import styles from "./ShortcutsDialog.module.css";

const canvasHints: readonly [string, string][] = [
  ["Сдвиг плана", "Прокрутка · средняя кнопка"],
  ["Масштаб", isMacPlatform ? "⌘ + прокрутка · щипок" : "Ctrl + прокрутка"],
];

/** Шпаргалка: все команды с сочетаниями клавиш. */
export function ShortcutsDialog() {
  const open = useEditor((s) => s.dialog === "shortcuts");
  const setDialog = useEditor((s) => s.setDialog);
  const groups = (Object.keys(groupTitles) as ActionGroup[]).map((group) => ({
    group,
    items: actions.filter((action) => action.group === group && action.keys?.length),
  }));

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => setDialog(next ? "shortcuts" : null)}
      title="Горячие клавиши"
      width={640}
    >
      <div className={styles.grid}>
        {groups.map(({ group, items }) =>
          items.length === 0 ? null : (
            <section key={group} className={styles.section}>
              <h3 className={styles.heading}>{groupTitles[group]}</h3>
              {items.map((action) => (
                <div key={action.id} className={styles.row}>
                  <span>{action.title}</span>
                  <Kbd>
                    {(action.keys ?? []).map((key) => format(key, isMacPlatform)).join(" · ")}
                  </Kbd>
                </div>
              ))}
            </section>
          ),
        )}
        <section className={styles.section}>
          <h3 className={styles.heading}>Холст</h3>
          {canvasHints.map(([title, keys]) => (
            <div key={title} className={styles.row}>
              <span>{title}</span>
              <Kbd>{keys}</Kbd>
            </div>
          ))}
        </section>
      </div>
    </Dialog>
  );
}
