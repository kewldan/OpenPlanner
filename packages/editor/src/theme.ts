export type ThemeChoice = "auto" | "light" | "dark";

const KEY = "openplanner.theme";

/** Сохранённый выбор темы (или «как в системе»). */
export function loadTheme(): ThemeChoice {
  try {
    const value = window.localStorage.getItem(KEY);
    return value === "light" || value === "dark" ? value : "auto";
  } catch {
    return "auto";
  }
}

/** Применяет тему к документу и запоминает выбор. */
export function applyTheme(choice: ThemeChoice) {
  const root = document.documentElement;
  if (choice === "auto") root.removeAttribute("data-theme");
  else root.dataset.theme = choice;
  try {
    window.localStorage.setItem(KEY, choice);
  } catch {
    // Хранилище недоступно — тема просто не запомнится.
  }
}
