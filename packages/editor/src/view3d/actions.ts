import type { CameraAction, Options3d } from "@openplanner/engine-client";
import { sendToEngine } from "../engine";
import { useEditor } from "../store/editor";

/**
 * Отправляет настройки 3D-вида и сразу кладёт их в стор: следующая правка,
 * сделанная до ответа движка, строится уже от них, а не от старых.
 */
function send(options: Options3d) {
  useEditor.setState((s) => ({ three: { ...s.three, options } }));
  sendToEngine({ type: "set3d", options });
}

/** Меняет настройки 3D-вида. */
export function set3d(change: Partial<Options3d>) {
  send({ ...useEditor.getState().three.options, ...change });
}

/** Убирает необязательную настройку (срез, солнце). */
export function clear3d(key: "cut" | "sun") {
  const { [key]: _removed, ...rest } = useEditor.getState().three.options;
  send(rest);
}

/** Включает или выключает 3D-вид. В сравнении вариантов 3D недоступен. */
export function toggle3d(enabled = !useEditor.getState().three.options.enabled) {
  if (enabled && useEditor.getState().view.type === "compare") return;
  set3d({ enabled });
}

export function camera(action: CameraAction) {
  sendToEngine({ type: "camera3d", action });
}

/** Свободное название ракурса: «Ракурс 1», «Ракурс 2»… */
export function nextViewName(taken: readonly string[], base: string): string {
  for (let n = 1; ; n++) {
    const name = `${base} ${n}`;
    if (!taken.includes(name)) return name;
  }
}

const DAYS = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/** День года 1–365 по месяцу (1–12) и дню; год невисокосный, как в движке. */
export function dayOfYear(month: number, day: number): number {
  const m = Math.min(12, Math.max(1, month));
  const before = DAYS.slice(0, m - 1).reduce((a, b) => a + b, 0);
  return before + Math.min(DAYS[m - 1] ?? 31, Math.max(1, day));
}

/** Дата `YYYY-MM-DD` (для поля даты) по дню года. */
export function dateOfDay(day: number, year: number): string {
  let rest = Math.min(365, Math.max(1, Math.round(day)));
  let month = 0;
  while (month < 11 && rest > (DAYS[month] ?? 31)) {
    rest -= DAYS[month] ?? 31;
    month++;
  }
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${year}-${pad(month + 1)}-${pad(rest)}`;
}

/** День года из значения поля даты; `null`, если поле пустое. */
export function dayOfDate(value: string): number | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) return null;
  return dayOfYear(Number(match[2]), Number(match[3]));
}

/** Время из часов: 13,5 → «13:30». */
export function formatHour(hour: number): string {
  const total = Math.round(hour * 60);
  const h = Math.floor(total / 60) % 24;
  const m = total % 60;
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;
}
