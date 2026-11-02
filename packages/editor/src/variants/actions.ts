import type { View } from "@openplanner/engine-client";
import { sendToEngine } from "../engine";
import { t } from "../i18n";
import { platform } from "../platform";
import { useEditor } from "../store/editor";

const LETTERS = "АБВГДЕЖЗИКЛМНОПРСТУФХЦЧШЭЮЯ";

/** Свободное название нового варианта: «Вариант А», «Вариант Б»… */
export function nextVariantName(taken: readonly string[]): string {
  for (const letter of LETTERS) {
    const name = t("variants.newName", { letter });
    if (!taken.includes(name)) return name;
  }
  return t("variants.newName", { letter: String(taken.length + 1) });
}

function variantNames(): string[] {
  return useEditor
    .getState()
    .summaries.filter((s) => s.variant)
    .map((s) => s.name);
}

export function setView(view: View) {
  sendToEngine({ type: "setView", view });
}

/** Режим «Перепланировка»: последний вариант, первый вариант или новый. */
export function enterReplan() {
  const { view, summaries, lastVariant } = useEditor.getState();
  if (view.type === "variant") return;
  const variants = summaries.filter((s) => s.variant).map((s) => s.variant as string);
  const target = lastVariant && variants.includes(lastVariant) ? lastVariant : variants[0];
  if (target) setView({ type: "variant", id: target });
  else createVariant();
}

/** Режим «Обмер». */
export function enterSurvey() {
  setView({ type: "survey" });
}

export function createVariant(copyOf?: string) {
  const names = variantNames();
  const source = copyOf
    ? useEditor.getState().summaries.find((s) => s.variant === copyOf)?.name
    : undefined;
  const name = source ? t("variants.copyName", { name: source }) : nextVariantName(names);
  sendToEngine(copyOf ? { type: "createVariant", name, copyOf } : { type: "createVariant", name });
}

export async function deleteVariant(id: string) {
  const name = useEditor.getState().summaries.find((s) => s.variant === id)?.name ?? "";
  const ok = await platform.confirm(t("variants.confirmRemove", { name }), {
    title: t("variants.remove"),
    okLabel: t("variants.remove"),
    cancelLabel: t("confirm.cancel"),
  });
  if (ok) sendToEngine({ type: "deleteVariant", id });
}

/** Два вида рядом на плане. */
export function showCompare(left: string | null, right: string | null) {
  const { view } = useEditor.getState();
  if (view.type !== "compare") useEditor.setState({ previousView: view });
  setView({
    type: "compare",
    ...(left ? { left } : {}),
    ...(right ? { right } : {}),
  });
  useEditor.getState().setDialog(null);
}

export function exitCompare() {
  setView(useEditor.getState().previousView);
}
