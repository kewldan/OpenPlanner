import type {
  EngineCall,
  EngineCallResult,
  FromEngine,
  ToEngine,
} from "@openplanner/engine-client";
import { EngineClient } from "@openplanner/engine-client";
import { useEditor } from "./store/editor";

let client: EngineClient | null = null;
const listeners = new Set<(events: FromEngine[]) => void>();
const readyListeners = new Set<() => void>();

/** Запускает движок на холсте. Возвращает функцию остановки. */
export function startEngine(canvas: HTMLCanvasElement): () => void {
  client?.dispose();
  const { setEngine, applyEvents } = useEditor.getState();
  setEngine({ state: "starting" });
  const current = new EngineClient(canvas, {
    onReady: (backend) => {
      setEngine({ state: "ready", backend });
      for (const listener of readyListeners) listener();
    },
    onEvents: (events) => {
      applyEvents(events);
      for (const listener of listeners) listener(events);
    },
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

/** Вызов движка с ответом. */
export function callEngine<C extends EngineCall>(call: C): Promise<EngineCallResult[C["type"]]> {
  if (!client) return Promise.reject(new Error("движок не запущен"));
  return client.call(call);
}

/** Подписка на события движка (после применения к стору). */
export function onEngineEvents(listener: (events: FromEngine[]) => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Подписка на запуск движка. */
export function onEngineReady(listener: () => void): () => void {
  readyListeners.add(listener);
  return () => readyListeners.delete(listener);
}
