import type { RenderWorkerResponse } from "./messages";

/** Трассировка кадра в отдельном воркере. */
export interface RenderJob {
  /** PNG готового кадра. */
  result: Promise<Uint8Array>;
  /** Прерывает трассировку (воркер останавливается). */
  cancel: () => void;
}

/** Запускает трассировку сцены из `renderInput`; `onPass(n)` — после каждого прохода. */
export function traceImage(
  input: Uint8Array,
  size: { width: number; height: number; passes: number },
  onPass: (pass: number) => void,
): RenderJob {
  const worker = new Worker(new URL("./render.worker.ts", import.meta.url), {
    type: "module",
    name: "openplanner-render",
  });
  let cancel = () => {};
  const result = new Promise<Uint8Array>((resolve, reject) => {
    cancel = () => {
      worker.terminate();
      reject(new Error("отменено"));
    };
    worker.onmessage = (event: MessageEvent<RenderWorkerResponse>) => {
      const response = event.data;
      if (response.kind === "progress") {
        onPass(response.pass);
        return;
      }
      worker.terminate();
      if (response.kind === "result") resolve(response.png);
      else reject(new Error(response.message));
    };
    worker.onerror = (event) => {
      worker.terminate();
      reject(new Error(event.message));
    };
  });
  worker.postMessage({ input, ...size }, [input.buffer]);
  return { result, cancel: () => cancel() };
}
