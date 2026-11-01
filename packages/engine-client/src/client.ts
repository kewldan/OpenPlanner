import type {
  EngineCall,
  EngineCallResult,
  FromEngine,
  ToEngine,
  WorkerRequest,
  WorkerResponse,
} from "./messages";

export interface EngineClientHandlers {
  onReady: (backend: string) => void;
  onEvents: (events: FromEngine[]) => void;
  onFatal: (message: string) => void;
}

interface Pending {
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
}

/** Клиент движка в UI-потоке: создаёт воркер и передаёт ему холст. */
export class EngineClient {
  readonly #worker: Worker;
  readonly #pending = new Map<number, Pending>();
  #nextId = 1;

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
        case "result":
          this.#settle(response.id)?.resolve(response.value);
          break;
        case "failure":
          this.#settle(response.id)?.reject(new Error(response.message));
          break;
        case "fatal":
          this.#rejectAll(response.message);
          handlers.onFatal(response.message);
          break;
      }
    };
    this.#worker.onerror = (event) => {
      this.#rejectAll(event.message);
      handlers.onFatal(event.message);
    };

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

  /** Отправляет сообщение движку. Ответные события придут в `onEvents`. */
  send(message: ToEngine) {
    this.#post({ kind: "message", message });
  }

  /** Вызов с ответом. Сообщения и вызовы выполняются строго по порядку отправки. */
  call<C extends EngineCall>(call: C): Promise<EngineCallResult[C["type"]]> {
    const id = this.#nextId++;
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve: resolve as (value: unknown) => void, reject });
      this.#post({ kind: "call", id, call });
    });
  }

  /** Останавливает воркер. */
  dispose() {
    this.#rejectAll("движок остановлен");
    this.#worker.terminate();
  }

  #settle(id: number): Pending | undefined {
    const pending = this.#pending.get(id);
    this.#pending.delete(id);
    return pending;
  }

  #rejectAll(message: string) {
    for (const pending of this.#pending.values()) pending.reject(new Error(message));
    this.#pending.clear();
  }

  #post(request: WorkerRequest, transfer: Transferable[] = []) {
    this.#worker.postMessage(request, transfer);
  }
}
