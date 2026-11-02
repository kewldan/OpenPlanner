import { type PointerEvent, useEffect, useRef, useState } from "react";
import { sendToEngine, startEngine } from "../engine";
import { t } from "../i18n";
import { useEditor } from "../store/editor";
import styles from "./PlanCanvas.module.css";

/** Холст плана. Рисует движок в воркере; здесь — только ввод и размеры. */
export function PlanCanvas() {
  const rootRef = useRef<HTMLDivElement>(null);
  const [panning, setPanning] = useState(false);
  const engine = useEditor((s) => s.engine);

  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;
    // Холст создаётся здесь, а не в JSX: управление им передаётся воркеру
    // (transferControlToOffscreen) и вернуть его нельзя. Новый элемент на каждый
    // запуск эффекта позволяет перезапускать движок (в том числе в StrictMode).
    const canvas = document.createElement("canvas");
    canvas.className = styles.canvas ?? "";
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

    return () => {
      canvas.removeEventListener("wheel", onWheel);
      observer.disconnect();
      stop();
      canvas.remove();
    };
  }, []);

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 1) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    setPanning(true);
  };

  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    if (!panning) return;
    sendToEngine({ type: "pan", dx: event.movementX, dy: event.movementY });
  };

  const onPointerUp = (event: PointerEvent<HTMLDivElement>) => {
    if (!panning) return;
    event.currentTarget.releasePointerCapture(event.pointerId);
    setPanning(false);
  };

  return (
    <div
      ref={rootRef}
      className={[styles.root, panning && styles.panning].filter(Boolean).join(" ")}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
    >
      {engine.state === "starting" && <div className={styles.overlay}>{t("status.starting")}</div>}
      {engine.state === "failed" && (
        <div className={[styles.overlay, styles.error].join(" ")} role="alert">
          {t("error.engine", { message: engine.message })}
        </div>
      )}
    </div>
  );
}
