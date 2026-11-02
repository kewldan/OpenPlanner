import {
  analyzeScan,
  type ScanImportMode,
  type ScanJob,
  type ScanParams,
  type ScanResult,
} from "@openplanner/engine-client";
import { create } from "zustand";
import { callEngine, sendToEngine } from "../engine";
import { platform } from "../platform";
import { project } from "../project";
import { useEditor } from "../store/editor";
import { type Answers, answerList, defaultParams } from "./decisions";

export type ScanStep = "file" | "processing" | "review" | "params" | "failure";

export interface ScanState {
  step: ScanStep;
  name: string | null;
  bytes: Uint8Array | null;
  stage: number;
  count: number;
  result: ScanResult | null;
  answers: Answers;
  params: ScanParams | null;
  error: string | null;
  /** Вопрос, подсвеченный на превью. */
  highlight: number | null;
  busy: boolean;
}

const initial: ScanState = {
  step: "file",
  name: null,
  bytes: null,
  stage: 0,
  count: 1,
  result: null,
  answers: {},
  params: null,
  error: null,
  highlight: null,
  busy: false,
};

/** Состояние мастера импорта скана. */
export const useScan = create<ScanState>()(() => initial);

let job: ScanJob | null = null;

/** Открывает мастер с начала. */
export function openScanWizard() {
  job?.cancel();
  job = null;
  useScan.setState(initial);
  useEditor.getState().setDialog("scan");
}

/** Закрывает мастер; обработка прерывается. */
export function closeScanWizard() {
  job?.cancel();
  job = null;
  useScan.setState(initial);
  if (useEditor.getState().dialog === "scan") useEditor.getState().setDialog(null);
}

/** Выбор файла через платформу. */
export async function pickScanFile() {
  try {
    const picked = await platform.openScan();
    if (picked) runScan(picked.bytes, picked.name);
  } catch (error) {
    useScan.setState({ error: error instanceof Error ? error.message : String(error) });
  }
}

/** Распознаёт файл в воркере. */
export function runScan(bytes: Uint8Array, name: string) {
  job?.cancel();
  useScan.setState({ ...initial, step: "processing", name, bytes, stage: 0, count: 6 });
  const current = analyzeScan(bytes, (stage, count) => {
    if (job === current) useScan.setState({ stage, count });
  });
  job = current;
  current.result.then(
    (result) => {
      if (job !== current) return;
      job = null;
      useScan.setState({
        result,
        params: defaultParams(result),
        step: result.failure ? "failure" : "review",
      });
    },
    (error: unknown) => {
      if (job !== current) return;
      job = null;
      useScan.setState({
        step: "file",
        error: error instanceof Error ? error.message : String(error),
      });
    },
  );
}

/** Прерывает обработку и возвращает к выбору файла. */
export function cancelScan() {
  job?.cancel();
  job = null;
  useScan.setState({ step: "file", stage: 0 });
}

export function setStep(step: ScanStep) {
  useScan.setState({ step });
}

export function setAnswer(issue: number, accept: boolean) {
  useScan.setState((s) => ({ answers: { ...s.answers, [issue]: accept } }));
}

export function setParams(patch: Partial<ScanParams>) {
  useScan.setState((s) => (s.params ? { params: { ...s.params, ...patch } } : {}));
}

/** Импортирует план (или только подложку) в проект и закрывает мастер. */
export async function importScan(mode: ScanImportMode) {
  const { result, answers, params, name, bytes } = useScan.getState();
  if (!result || !params) return;
  useScan.setState({ busy: true });
  try {
    const source = params.keepSource && bytes ? bytes : undefined;
    await callEngine({
      type: "importScan",
      request: { result, answers: answerList(result, answers), params, mode, name: name ?? "скан" },
      ...(source ? { source } : {}),
    });
    closeScanWizard();
    sendToEngine({ type: "setTool", tool: mode === "underlayOnly" ? "wall" : "select" });
  } catch (error) {
    useScan.setState({
      busy: false,
      error: error instanceof Error ? error.message : String(error),
    });
  }
}

/** «Начать с пустого проекта» на экране ошибки. */
export async function startEmpty() {
  closeScanWizard();
  await project.newProject();
}
