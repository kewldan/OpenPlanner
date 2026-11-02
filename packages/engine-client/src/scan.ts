import type { ScanResult, ScanWorkerResponse } from "./messages";

/** Распознавание скана в отдельном воркере. */
export interface ScanJob {
  result: Promise<ScanResult>;
  /** Прерывает обработку (воркер останавливается). */
  cancel: () => void;
}

/** Запускает распознавание. `onProgress(stage, count)` — номер этапа из `count`. */
export function analyzeScan(
  bytes: Uint8Array,
  onProgress: (stage: number, count: number) => void,
): ScanJob {
  const worker = new Worker(new URL("./scan.worker.ts", import.meta.url), {
    type: "module",
    name: "openplanner-scan",
  });
  let cancel = () => {};
  const result = new Promise<ScanResult>((resolve, reject) => {
    cancel = () => {
      worker.terminate();
      reject(new Error("отменено"));
    };
    worker.onmessage = (event: MessageEvent<ScanWorkerResponse>) => {
      const response = event.data;
      if (response.kind === "progress") {
        onProgress(response.stage, response.count);
        return;
      }
      worker.terminate();
      if (response.kind === "result") resolve(response.result);
      else reject(new Error(response.message));
    };
    worker.onerror = (event) => {
      worker.terminate();
      reject(new Error(event.message));
    };
  });
  // Байты копируются: исходный файл ещё нужен для сохранения в проекте.
  worker.postMessage({ bytes: bytes.slice() });
  return { result, cancel: () => cancel() };
}
