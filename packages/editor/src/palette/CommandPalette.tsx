import { Dialog, Kbd } from "@openplanner/ui";
import { type KeyboardEvent, useId, useMemo, useRef, useState } from "react";
import { type Action, groupTitles, isEnabled, searchActions } from "../actions";
import { format, isMacPlatform } from "../actions/shortcut";
import { useEditor } from "../store/editor";
import styles from "./CommandPalette.module.css";

/** Палитра команд ⌘K: поиск по всем командам приложения. */
export function CommandPalette() {
  const open = useEditor((s) => s.dialog === "palette");
  const setDialog = useEditor((s) => s.setDialog);
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => setDialog(next ? "palette" : null)}
      title="Палитра команд"
      hideHeader
      width={600}
    >
      {open && <PaletteBody onDone={() => setDialog(null)} />}
    </Dialog>
  );
}

function PaletteBody({ onDone }: { onDone: () => void }) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const listId = useId();
  const listRef = useRef<HTMLDivElement>(null);
  const results = useMemo(() => searchActions(query), [query]);
  const current = Math.min(active, Math.max(results.length - 1, 0));

  const run = (action: Action | undefined) => {
    if (!action || !isEnabled(action)) return;
    onDone();
    // Команда может открыть другое окно — запускаем после закрытия палитры.
    queueMicrotask(() => action.run());
  };

  const move = (delta: number) => {
    if (results.length === 0) return;
    const next = (current + delta + results.length) % results.length;
    setActive(next);
    listRef.current?.children[next]?.scrollIntoView({ block: "nearest" });
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      move(1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      move(-1);
    } else if (event.key === "Enter") {
      event.preventDefault();
      run(results[current]);
    }
  };

  return (
    <>
      <div className={styles.search}>
        <svg width="16" height="16" viewBox="0 0 20 20" aria-hidden="true">
          <path
            d="M8.5 3.5a5 5 0 1 0 0 10 5 5 0 1 0 0-10M12.3 12.3l4.2 4.2"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.4"
            strokeLinecap="round"
          />
        </svg>
        <input
          className={styles.input}
          role="combobox"
          aria-expanded="true"
          aria-controls={listId}
          aria-activedescendant={results[current] ? `${listId}-${current}` : undefined}
          aria-label="Найти команду"
          placeholder="Найти команду…"
          value={query}
          // biome-ignore lint/a11y/noAutofocus: палитра открывается ради ввода
          autoFocus
          onChange={(event) => {
            setQuery(event.target.value);
            setActive(0);
          }}
          onKeyDown={onKeyDown}
        />
      </div>
      {results.length === 0 ? (
        <p className={styles.empty}>Ничего не найдено</p>
      ) : (
        <div id={listId} ref={listRef} role="listbox" aria-label="Команды" className={styles.list}>
          {results.map((action, index) => {
            const enabled = isEnabled(action);
            const showGroup = index === 0 || results[index - 1]?.group !== action.group;
            const shortcut = action.keys?.[0];
            return (
              // biome-ignore lint/a11y/useKeyWithClickEvents: клавиатура обрабатывается полем поиска (aria-activedescendant)
              <div
                key={action.id}
                tabIndex={-1}
                id={`${listId}-${index}`}
                role="option"
                aria-selected={index === current}
                aria-disabled={!enabled}
                className={styles.item}
                onMouseMove={() => setActive(index)}
                onClick={() => run(action)}
              >
                <span className={styles.label}>
                  {showGroup && !query ? `${groupTitles[action.group]} · ` : ""}
                  {action.title}
                </span>
                {shortcut && <Kbd>{format(shortcut, isMacPlatform)}</Kbd>}
              </div>
            );
          })}
        </div>
      )}
      <div className={styles.footer}>
        <span>↑ ↓ выбрать</span>
        <span>↵ выполнить</span>
        <span>Esc закрыть</span>
      </div>
    </>
  );
}
