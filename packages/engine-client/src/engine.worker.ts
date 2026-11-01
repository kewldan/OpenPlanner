/// <reference lib="webworker" />
// Web Worker: WASM-движок (ядро модели + рендерер) рисует в OffscreenCanvas.

import init, { Engine } from "../wasm/op_engine";
import type { WorkerRequest, WorkerResponse } from "./messages";

declare const self: DedicatedWorkerGlobalScope;

let engine: Engine | undefined;
let framePending = false;
const queue: WorkerRequest[] = [];

function post(response: WorkerResponse) {
  self.postMessage(response);
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

function handle(request: WorkerRequest) {
  if (request.kind !== "message" || !engine) return;
  const { events } = engine.handle(request.message);
  if (events.length > 0) post({ kind: "events", events });
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
    post({ kind: "ready", backend: engine.backend });
    post({ kind: "events", events: engine.snapshot().events });
    for (const pending of queue.splice(0)) handle(pending);
    requestFrame();
  } catch (error) {
    post({ kind: "fatal", message: error instanceof Error ? error.message : String(error) });
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
