import type { PointerPhase } from "@openplanner/engine-client";
import { type MouseEvent, type PointerEvent, useEffect, useRef, useState } from "react";
import { sendToEngine, startEngine } from "../engine";
import { DRAG_TYPE } from "../furniture/items";
import { t } from "../i18n";
import { useEditor } from "../store/editor";
import { exitCompare } from "../variants/actions";
import { camera } from "../view3d/actions";
import { NumericInput } from "./NumericInput";
import styles from "./PlanCanvas.module.css";

/** Инструменты, которые рисуют: курсор — перекрестие. */
const drawingTools = new Set([
  "furniture",
  "wall",
  "door",
  "window",
  "arch",
  "room",
  "dimension",
  "leader",
  "calibrate",
]);

/** Клавиши движения в 3D: вперёд, вправо, вверх. */
const moveKeys: Record<string, [number, number, number]> = {
  KeyW: [1, 0, 0],
  ArrowUp: [1, 0, 0],
  KeyS: [-1, 0, 0],
  ArrowDown: [-1, 0, 0],
  KeyD: [0, 1, 0],
  ArrowRight: [0, 1, 0],
  KeyA: [0, -1, 0],
  ArrowLeft: [0, -1, 0],
  KeyE: [0, 0, 1],
  KeyQ: [0, 0, -1],
};

/** Скорость движения в 3D, мм/с (с Shift — быстрее). */
const SPEED = 2500;

const in3d = () => useEditor.getState().three.options.enabled;

const isTyping = (target: EventTarget | null) =>
  target instanceof HTMLElement && Boolean(target.closest("input, textarea, select"));

/** WASD в 3D: пока клавиши нажаты, каждый кадр камера сдвигается. */
function useMoveKeys3d() {
  const enabled = useEditor((s) => s.three.options.enabled);
  useEffect(() => {
    if (!enabled) return;
    const pressed = new Set<string>();
    let fast = false;
    let frame = 0;
    let last = 0;
    const tick = (time: number) => {
      const dt = Math.min(0.1, (time - last) / 1000);
      last = time;
      const v = [0, 0, 0];
      for (const code of pressed) {
        const d = moveKeys[code];
        if (d) for (let i = 0; i < 3; i++) v[i] = (v[i] ?? 0) + (d[i] ?? 0);
      }
      const step = SPEED * (fast ? 2.5 : 1) * dt;
      if (v.some((x) => x !== 0)) {
        camera({
          type: "move",
          forward: (v[0] ?? 0) * step,
          right: (v[1] ?? 0) * step,
          up: (v[2] ?? 0) * step,
        });
      }
      frame = pressed.size > 0 ? requestAnimationFrame(tick) : 0;
    };
    const onKey = (event: KeyboardEvent) => {
      fast = event.shiftKey;
      if (!(event.code in moveKeys) || isTyping(event.target)) return;
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      if (useEditor.getState().dialog !== null) return;
      event.preventDefault();
      if (event.type === "keydown") pressed.add(event.code);
      else pressed.delete(event.code);
      if (pressed.size > 0 && frame === 0) {
        last = performance.now();
        frame = requestAnimationFrame(tick);
      }
    };
    const onBlur = () => pressed.clear();
    window.addEventListener("keydown", onKey);
    window.addEventListener("keyup", onKey);
    window.addEventListener("blur", onBlur);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("keyup", onKey);
      window.removeEventListener("blur", onBlur);
    };
  }, [enabled]);
}

/** Холст плана и 3D-вида. Рисует движок в воркере; здесь — ввод и размеры. */
export function PlanCanvas() {
  const rootRef = useRef<HTMLDivElement>(null);
  const [panning, setPanning] = useState(false);
  const space = useRef(false);
  /** Перетаскивание в 3D: поворот или сдвиг. */
  const drag3d = useRef<"rotate" | "pan" | null>(null);
  const engine = useEditor((s) => s.engine);
  const tool = useEditor((s) => s.tool);
  const three = useEditor((s) => s.three.options.enabled);
  useMoveKeys3d();

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
      if (in3d()) {
        const delta =
          event.deltaMode === WheelEvent.DOM_DELTA_LINE ? event.deltaY * 33 : event.deltaY;
        camera({ type: "zoom", factor: Math.exp(-Math.max(-300, Math.min(300, delta)) * 0.002) });
        return;
      }
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

  const point = (event: MouseEvent<HTMLDivElement>, phase: PointerPhase) => {
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
    if (in3d()) {
      event.preventDefault();
      event.currentTarget.setPointerCapture(event.pointerId);
      drag3d.current = event.button === 0 && !event.shiftKey ? "rotate" : "pan";
      return;
    }
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
    if (in3d()) {
      if (drag3d.current) {
        camera({ type: drag3d.current, dx: event.movementX, dy: event.movementY });
      }
      return;
    }
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
    if (drag3d.current || in3d()) {
      drag3d.current = null;
      return;
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
      data-view={three ? "3d" : "2d"}
      className={[
        styles.root,
        panning && styles.panning,
        three && styles.orbit,
        !three && drawingTools.has(tool) && styles.drawing,
      ]
        .filter(Boolean)
        .join(" ")}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onPointerLeave={(event) => {
        if (!in3d()) point(event, "leave");
      }}
      onDoubleClick={(event) => {
        if (!in3d() && event.target === event.currentTarget.querySelector("canvas")) {
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
      // Позиция каталога, перетащенная на план: превью у курсора, отпускание ставит предмет.
      onDragOver={(event) => {
        if (in3d() || !event.dataTransfer.types.includes(DRAG_TYPE)) return;
        event.preventDefault();
        event.dataTransfer.dropEffect = "copy";
        point(event, "move");
      }}
      onDrop={(event) => {
        if (in3d() || !event.dataTransfer.types.includes(DRAG_TYPE)) return;
        event.preventDefault();
        point(event, "down");
        point(event, "up");
      }}
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
