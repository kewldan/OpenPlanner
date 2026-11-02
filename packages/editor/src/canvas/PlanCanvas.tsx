import type { PointerPhase } from "@openplanner/engine-client";
import { type PointerEvent, useEffect, useRef, useState } from "react";
import { sendToEngine, startEngine } from "../engine";
import { t } from "../i18n";
import { useEditor } from "../store/editor";
import { exitCompare } from "../variants/actions";
import { NumericInput } from "./NumericInput";
import styles from "./PlanCanvas.module.css";

/** Инструменты, которые рисуют: курсор — перекрестие. */
const drawingTools = new Set([
  "wall",
  "door",
  "window",
  "arch",
  "room",
  "dimension",
  "leader",
  "calibrate",
]);

/** Холст плана. Рисует движок в воркере; здесь — ввод и размеры. */
export function PlanCanvas() {
  const rootRef = useRef<HTMLDivElement>(null);
  const [panning, setPanning] = useState(false);
  const space = useRef(false);
  const engine = useEditor((s) => s.engine);
  const tool = useEditor((s) => s.tool);

  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;
    // Холст создаётся здесь, а не в JSX: управление им передаётся воркеру
    // (transferControlToOffscreen) и вернуть его нельзя. Новый элемент на каждый
    // запуск эффекта позволяет перезапускать движок (в том числе в StrictMode).
    const canvas = document.createElement("canvas");
    canvas.className = styles.canvas ?? "";
    canvas.dataset.testid = "plan-canvas";
    root.prepend(canvas);
    const stop = startEngine(canvas);

    const observer = new ResizeObserver(([entry]) => {
      if (!entry) return;
      const { width, height } = entry.contentRect;
      useEditor.getState().setViewport(width, height);
      sendToEngine({ type: "resize", width, height, devicePixelRatio: window.devicePixelRatio });
    });
    observer.observe(root);

    // Прокрутка — сдвиг, ⌘/Ctrl + прокрутка (и щипок на тачпаде) — масштаб.
    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      const rect = canvas.getBoundingClientRect();
      if (event.ctrlKey || event.metaKey) {
        // Щелчок колеса мыши (~100 px) — ×1,22; щипок тачпада даёт мелкие шаги.
        const delta =
          event.deltaMode === WheelEvent.DOM_DELTA_LINE ? event.deltaY * 33 : event.deltaY;
        const factor = Math.exp(-Math.max(-300, Math.min(300, delta)) * 0.002);
        sendToEngine({
          type: "zoom",
          x: event.clientX - rect.left,
          y: event.clientY - rect.top,
          factor,
        });
      } else {
        sendToEngine({ type: "pan", dx: -event.deltaX, dy: -event.deltaY });
      }
    };
    canvas.addEventListener("wheel", onWheel, { passive: false });

    // Пробел — временное панорамирование левой кнопкой.
    const isTyping = (target: EventTarget | null) =>
      target instanceof HTMLElement && Boolean(target.closest("input, textarea, select"));
    const onKey = (event: KeyboardEvent) => {
      if (event.code !== "Space" || isTyping(event.target)) return;
      space.current = event.type === "keydown";
      if (event.type === "keydown") event.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("keyup", onKey);

    return () => {
      canvas.removeEventListener("wheel", onWheel);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("keyup", onKey);
      observer.disconnect();
      stop();
      canvas.remove();
    };
  }, []);

  const point = (event: PointerEvent<HTMLDivElement>, phase: PointerPhase) => {
    const rect = event.currentTarget.getBoundingClientRect();
    sendToEngine({
      type: "pointer",
      phase,
      x: event.clientX - rect.left,
      y: event.clientY - rect.top,
      shift: event.shiftKey,
      alt: event.altKey,
    });
  };

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget.querySelector("canvas")) return;
    // Фокус уходит с полей инспектора: правка в поле применяется до действия на холсте.
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
    if (event.button === 1 || (event.button === 0 && space.current)) {
      event.preventDefault();
      event.currentTarget.setPointerCapture(event.pointerId);
      setPanning(true);
      return;
    }
    if (event.button === 2) {
      // Правая кнопка заканчивает цепочку или прерывает шаг инструмента.
      sendToEngine({ type: useEditor.getState().step === "wallNext" ? "finish" : "cancel" });
      return;
    }
    if (event.button !== 0) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    point(event, "down");
  };

  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    if (panning) {
      sendToEngine({ type: "pan", dx: event.movementX, dy: event.movementY });
      return;
    }
    point(event, "move");
  };

  const onPointerUp = (event: PointerEvent<HTMLDivElement>) => {
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    if (panning) {
      setPanning(false);
      return;
    }
    if (event.button === 0) point(event, "up");
  };

  return (
    <div
      ref={rootRef}
      role="application"
      aria-label={t("canvas.label")}
      className={[styles.root, panning && styles.panning, drawingTools.has(tool) && styles.drawing]
        .filter(Boolean)
        .join(" ")}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onPointerLeave={(event) => point(event, "leave")}
      onDoubleClick={(event) => {
        if (event.target === event.currentTarget.querySelector("canvas")) {
          const rect = event.currentTarget.getBoundingClientRect();
          sendToEngine({
            type: "pointer",
            phase: "doubleClick",
            x: event.clientX - rect.left,
            y: event.clientY - rect.top,
            shift: event.shiftKey,
            alt: event.altKey,
          });
        }
      }}
      onContextMenu={(event) => event.preventDefault()}
    >
      <NumericInput />
      <CompareBanner />
      {engine.state === "starting" && <div className={styles.overlay}>{t("status.starting")}</div>}
      {engine.state === "failed" && (
        <div className={[styles.overlay, styles.error].join(" ")} role="alert">
          {t("error.engine", { message: engine.message })}
        </div>
      )}
    </div>
  );
}

/** Плашка синхронного просмотра двух вариантов. */
function CompareBanner() {
  const label = useEditor((s) => {
    if (s.view.type !== "compare") return null;
    const name = (id: string | undefined) =>
      s.summaries.find((x) => (x.variant ?? undefined) === id)?.name ?? "";
    return t("compare.banner", { left: name(s.view.left), right: name(s.view.right) });
  });
  if (!label) return null;
  return (
    <div className={styles.banner} data-testid="compare-banner">
      <span>{label}</span>
      <button type="button" onClick={exitCompare}>
        {t("compare.exit")}
      </button>
    </div>
  );
}
