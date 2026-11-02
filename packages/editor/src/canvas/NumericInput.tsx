import { type FormEvent, type KeyboardEvent, useEffect, useRef, useState } from "react";
import { sendToEngine } from "../engine";
import { t } from "../i18n";
import { parseNumber } from "../inspector/format";
import { useEditor } from "../store/editor";
import styles from "./PlanCanvas.module.css";

/**
 * Числовой ввод при рисовании: длина и угол следующей стены или настоящая
 * длина отрезка при калибровке. Открывается первой цифрой с клавиатуры.
 */
export function NumericInput() {
  const step = useEditor((s) => s.step);
  const [open, setOpen] = useState(false);
  const [length, setLength] = useState("");
  const [angle, setAngle] = useState("");
  const lengthRef = useRef<HTMLInputElement>(null);
  const wall = step === "wallNext";
  const calibrate = step === "calibrateDistance";
  const visible = calibrate || (wall && open);

  // Цифра на холсте открывает поле длины с этой цифрой.
  useEffect(() => {
    if (!wall || open) return;
    const onKeyDown = (event: KeyboardEvent | globalThis.KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.closest("input, textarea, select")) return;
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      if (!/^[0-9]$/.test(event.key)) return;
      event.preventDefault();
      setLength(event.key);
      setAngle("");
      setOpen(true);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [wall, open]);

  useEffect(() => {
    if (!wall && !calibrate) setOpen(false);
  }, [wall, calibrate]);

  useEffect(() => {
    if (!visible) return;
    const input = lengthRef.current;
    if (!input) return;
    input.focus();
    // Курсор в конец уже введённой цифры.
    input.setSelectionRange(input.value.length, input.value.length);
  }, [visible]);

  if (!visible) return null;

  const close = () => {
    setOpen(false);
    setLength("");
    setAngle("");
  };

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const value = parseNumber(length);
    if (value === null || value <= 0) return;
    const degrees = wall ? parseNumber(angle) : null;
    sendToEngine(
      degrees === null
        ? { type: "numericInput", value }
        : { type: "numericInput", value, angle: degrees },
    );
    close();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLFormElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      close();
      if (calibrate) sendToEngine({ type: "cancel" });
    }
  };

  return (
    <form
      className={styles.numeric}
      onSubmit={submit}
      onKeyDown={onKeyDown}
      data-testid="numeric-input"
    >
      <label className={styles.numericField}>
        <span>{t(calibrate ? "numeric.distance" : "numeric.length")}</span>
        <input
          ref={lengthRef}
          value={length}
          inputMode="decimal"
          aria-label={t(calibrate ? "numeric.distance" : "numeric.length")}
          onChange={(event) => setLength(event.target.value)}
        />
        <span>{t("unit.mm")}</span>
      </label>
      {wall && (
        <label className={styles.numericField}>
          <span>{t("numeric.angle")}</span>
          <input
            value={angle}
            inputMode="decimal"
            aria-label={t("numeric.angle")}
            onChange={(event) => setAngle(event.target.value)}
          />
          <span>{t("unit.deg")}</span>
        </label>
      )}
      <button type="submit" hidden />
      <span className={styles.numericHint}>{t("numeric.hint")}</span>
    </form>
  );
}
