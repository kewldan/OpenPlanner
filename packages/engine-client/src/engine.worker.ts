/// <reference lib="webworker" />
// Web Worker: WASM-движок (ядро модели + рендерер) рисует в OffscreenCanvas.

import init, { Engine } from "../wasm/op_engine";
import type { EngineCall, WorkerRequest, WorkerResponse } from "./messages";

declare const self: DedicatedWorkerGlobalScope;

let engine: Engine | undefined;
let framePending = false;
const queue: WorkerRequest[] = [];

function post(response: WorkerResponse, transfer: Transferable[] = []) {
  self.postMessage(response, transfer);
}

function requestFrame() {
  if (framePending || !engine) return;
  framePending = true;
  const draw = () => {
    framePending = false;
    engine?.render();
  };
  if (typeof self.requestAnimationFrame === "function") {
    self.requestAnimationFrame(draw);
  } else {
    setTimeout(draw, 0);
  }
}

type GpuNavigator = WorkerNavigator & { gpu?: { requestAdapter(): Promise<unknown> } };

/** Диагностика WebGPU в воркере: почему движок выбрал тот или иной бэкенд. */
async function probeWebGpu(): Promise<string> {
  const gpu = (self.navigator as GpuNavigator).gpu;
  if (!gpu) return "нет navigator.gpu";
  try {
    return (await gpu.requestAdapter()) ? "адаптер доступен" : "адаптер не выдан";
  } catch (error) {
    return `ошибка: ${errorMessage(error)}`;
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** Выполняет вызов; события изменения документа отправляет отдельно, до ответа. */
function call(current: Engine, request: EngineCall): { value: unknown; transfer: Transferable[] } {
  switch (request.type) {
    case "exportFile": {
      const bytes = current.exportFile(request.generator);
      return { value: bytes, transfer: [bytes.buffer] };
    }
    case "importFile":
      post({ kind: "events", events: current.importFile(request.bytes).events });
      return { value: undefined, transfer: [] };
    case "addUnderlay":
      post({ kind: "events", events: current.addUnderlay(request.bytes, request.name).events });
      return { value: undefined, transfer: [] };
    case "replayJournal":
      post({ kind: "events", events: current.replayJournal(request.text).events });
      return { value: undefined, transfer: [] };
    case "journalHeader":
      return { value: Engine.journalHeader(), transfer: [] };
    case "documentTitle": {
      // Ответ идёт после всех ранее отправленных команд — название всегда актуально.
      const changed = current.snapshot().events.find((event) => event.type === "documentChanged");
      return { value: changed?.document.meta.title ?? "", transfer: [] };
    }
  }
}

function handle(request: WorkerRequest) {
  if (!engine) return;
  if (request.kind === "message") {
    const { events } = engine.handle(request.message);
    if (events.length > 0) post({ kind: "events", events });
  } else if (request.kind === "call") {
    try {
      const { value, transfer } = call(engine, request.call);
      post({ kind: "result", id: request.id, value }, transfer);
    } catch (error) {
      post({ kind: "failure", id: request.id, message: errorMessage(error) });
    }
  }
  requestFrame();
}

async function start(request: Extract<WorkerRequest, { kind: "init" }>) {
  try {
    await init();
    engine = await Engine.create(
      request.canvas,
      request.width,
      request.height,
      request.devicePixelRatio,
    );
    post({ kind: "ready", backend: engine.backend, webgpu: await probeWebGpu() });
    post({ kind: "events", events: engine.snapshot().events });
    for (const pending of queue.splice(0)) handle(pending);
    requestFrame();
  } catch (error) {
    post({ kind: "fatal", message: errorMessage(error) });
  }
}

self.onmessage = (event: MessageEvent<WorkerRequest>) => {
  const request = event.data;
  if (request.kind === "init") {
    void start(request);
  } else if (engine) {
    handle(request);
  } else {
    queue.push(request);
  }
};
