/// <reference lib="webworker" />
// Web Worker фотореалистичного рендера: тот же WASM-модуль, трассировка путей
// на CPU в отдельном потоке — редактор не замирает, задание можно отменить.

import init, { Engine } from "../wasm/op_engine";
import type { RenderWorkerRequest, RenderWorkerResponse } from "./messages";

declare const self: DedicatedWorkerGlobalScope;

function post(response: RenderWorkerResponse, transfer: Transferable[] = []) {
  self.postMessage(response, transfer);
}

self.onmessage = async (event: MessageEvent<RenderWorkerRequest>) => {
  try {
    await init();
    const { input, width, height, passes } = event.data;
    const png = Engine.traceImage(input, width, height, passes, (pass: number) =>
      post({ kind: "progress", pass }),
    );
    post({ kind: "result", png }, [png.buffer]);
  } catch (error) {
    post({ kind: "error", message: error instanceof Error ? error.message : String(error) });
  }
};
