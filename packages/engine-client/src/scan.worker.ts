/// <reference lib="webworker" />
// Web Worker распознавания скана: тот же WASM-модуль, что у движка, но в
// отдельном потоке — редактор не замирает, обработку можно отменить.

import init, { analyzeScan } from "../wasm/op_engine";
import type { ScanWorkerRequest, ScanWorkerResponse } from "./messages";

declare const self: DedicatedWorkerGlobalScope;

function post(response: ScanWorkerResponse) {
  self.postMessage(response);
}

self.onmessage = async (event: MessageEvent<ScanWorkerRequest>) => {
  try {
    await init();
    const result = analyzeScan(event.data.bytes, (stage: number, count: number) =>
      post({ kind: "progress", stage, count }),
    );
    post({ kind: "result", result });
  } catch (error) {
    post({ kind: "error", message: error instanceof Error ? error.message : String(error) });
  }
};
