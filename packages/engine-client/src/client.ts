import type { FromEngine, ToEngine, WorkerRequest, WorkerResponse } from "./messages";

export interface EngineClientHandlers {
  onReady: (backend: string) => void;
  onEvents: (events: FromEngine[]) => void;
  onFatal: (message: string) => void;
}

/** Клиент движка в UI-потоке: создаёт воркер и передаёт ему холст. */
export class EngineClient {
  readonly #worker: Worker;

  constructor(canvas: HTMLCanvasElement, handlers: EngineClientHandlers) {
    this.#worker = new Worker(new URL("./engine.worker.ts", import.meta.url), {
      type: "module",
      name: "openplanner-engine",
    });
    this.#worker.onmessage = (event: MessageEvent<WorkerResponse>) => {
      const response = event.data;
      switch (response.kind) {
        case "ready":
          handlers.onReady(response.backend);
          break;
        case "events":
          handlers.onEvents(response.events);
          break;
        case "fatal":
          handlers.onFatal(response.message);
          break;
      }
    };
    this.#worker.onerror = (event) => handlers.onFatal(event.message);

    const offscreen = canvas.transferControlToOffscreen();
    const rect = canvas.getBoundingClientRect();
    this.#post(
      {
        kind: "init",
        canvas: offscreen,
        width: rect.width,
        height: rect.height,
        devicePixelRatio: window.devicePixelRatio,
      },
      [offscreen],
    );
  }

  /** Отправляет сообщение движку. */
  send(message: ToEngine) {
    this.#post({ kind: "message", message });
  }

  /** Останавливает воркер. */
  dispose() {
    this.#worker.terminate();
  }

  #post(request: WorkerRequest, transfer: Transferable[] = []) {
    this.#worker.postMessage(request, transfer);
  }
}
