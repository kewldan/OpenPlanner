import type { ToEngine } from "@openplanner/engine-client";
import { EngineClient } from "@openplanner/engine-client";
import { useEditor } from "./store/editor";

let client: EngineClient | null = null;

/** Запускает движок на холсте. Повторный вызов перезапускает его. */
export function startEngine(canvas: HTMLCanvasElement): () => void {
  client?.dispose();
  const { setEngine, applyEvents } = useEditor.getState();
  setEngine({ state: "starting" });
  const current = new EngineClient(canvas, {
    onReady: (backend) => setEngine({ state: "ready", backend }),
    onEvents: applyEvents,
    onFatal: (message) => setEngine({ state: "failed", message }),
  });
  client = current;
  return () => {
    current.dispose();
    if (client === current) client = null;
  };
}

/** Отправляет сообщение движку. До запуска движка сообщения теряются. */
export function sendToEngine(message: ToEngine) {
  client?.send(message);
}
