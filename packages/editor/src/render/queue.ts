import { type RenderJob, traceImage } from "@openplanner/engine-client";
import { create } from "zustand";
import { callEngine } from "../engine";
import { today } from "../estimate/prices";

/** Размеры кадра. */
export const SIZES = [
  { label: "1280 × 720", width: 1280, height: 720 },
  { label: "1920 × 1080", width: 1920, height: 1080 },
  { label: "800 × 600", width: 800, height: 600 },
  { label: "640 × 360", width: 640, height: 360 },
] as const;

/** Качество: проходов трассировки (лучей на пиксель). */
export const QUALITIES = [
  { label: "render.draft", passes: 8 },
  { label: "render.good", passes: 32 },
  { label: "render.best", passes: 128 },
] as const;

export type JobStatus = "waiting" | "running" | "done" | "failed" | "cancelled";

/** Задание очереди рендера. */
export interface Job {
  id: string;
  name: string;
  /** Ракурс (`undefined` — текущая камера 3D-вида). */
  view?: string;
  width: number;
  height: number;
  passes: number;
  status: JobStatus;
  pass: number;
  error?: string;
}

interface Queue {
  jobs: Job[];
}

export const useRenderQueue = create<Queue>()(() => ({ jobs: [] }));

let running: { id: string; job: RenderJob } | null = null;

function patch(id: string, next: Partial<Job>) {
  useRenderQueue.setState((s) => ({
    jobs: s.jobs.map((j) => (j.id === id ? { ...j, ...next } : j)),
  }));
}

/** Запускает следующее ожидающее задание, если ничего не идёт. */
async function pump(): Promise<void> {
  if (running) return;
  const next = useRenderQueue.getState().jobs.find((j) => j.status === "waiting");
  if (!next) return;
  patch(next.id, { status: "running", pass: 0 });
  try {
    const input = await callEngine(
      next.view ? { type: "renderInput", view: next.view } : { type: "renderInput" },
    );
    const job = traceImage(
      input,
      { width: next.width, height: next.height, passes: next.passes },
      (pass) => patch(next.id, { pass }),
    );
    running = { id: next.id, job };
    const png = await job.result;
    await callEngine({
      type: "addRender",
      bytes: png,
      name: next.name,
      passes: next.passes,
      date: today(),
    });
    patch(next.id, { status: "done", pass: next.passes });
  } catch (error) {
    const current = useRenderQueue.getState().jobs.find((j) => j.id === next.id);
    if (current?.status !== "cancelled")
      patch(next.id, {
        status: "failed",
        error: error instanceof Error ? error.message : String(error),
      });
  } finally {
    running = null;
    void pump();
  }
}

/** Ставит кадр в очередь. */
export function enqueue(job: Omit<Job, "id" | "status" | "pass">) {
  const id = `job-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
  useRenderQueue.setState((s) => ({
    jobs: [...s.jobs, { ...job, id, status: "waiting", pass: 0 }],
  }));
  void pump();
}

/** Отменяет задание: идущее — останавливает воркер. */
export function cancel(id: string) {
  patch(id, { status: "cancelled" });
  if (running?.id === id) running.job.cancel();
}

/** Убирает завершённые задания из списка. */
export function clearFinished() {
  useRenderQueue.setState((s) => ({
    jobs: s.jobs.filter((j) => j.status === "waiting" || j.status === "running"),
  }));
}
