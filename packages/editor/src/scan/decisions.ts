import type { ScanIssue, ScanParams, ScanResult } from "@openplanner/engine-client";

/** Ответы пользователя на вопросы проверки: номер вопроса → принять предложение. */
export type Answers = Record<number, boolean>;

/** Принято ли предложение: ответ пользователя или по умолчанию — уверенность от 50 %. */
export function isAccepted(issue: ScanIssue, answers: Answers): boolean {
  return answers[issue.id] ?? issue.confidence >= 0.5;
}

/** Ответы для движка — по всем вопросам, с учётом умолчаний. */
export function answerList(result: ScanResult, answers: Answers) {
  return result.issues.map((issue) => ({ issue: issue.id, accept: isAccepted(issue, answers) }));
}

/** Стены, убранные принятыми вопросами «лишняя стена?». */
export function removedWalls(result: ScanResult, answers: Answers): Set<number> {
  const removed = new Set<number>();
  for (const issue of result.issues) {
    if (issue.kind.type === "extraWall" && isAccepted(issue, answers)) removed.add(issue.kind.wall);
  }
  return removed;
}

/** Параметры по умолчанию для результата: высота потолка — из скана. */
export function defaultParams(result: ScanResult): ScanParams {
  return {
    ceilingHeight: result.ceilingHeight,
    singleThickness: 200,
    addedThickness: 120,
    createRooms: true,
    underlay: true,
    keepSource: true,
  };
}

/** Уверенность в процентах. */
export function percent(confidence: number): number {
  return Math.round(Math.min(1, Math.max(0, confidence)) * 100);
}
