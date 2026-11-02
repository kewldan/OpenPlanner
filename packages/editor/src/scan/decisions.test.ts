import { describe, expect, test } from "bun:test";
import type { ScanResult } from "@openplanner/engine-client";
import { answerList, defaultParams, isAccepted, percent, removedWalls } from "./decisions";

const result: ScanResult = {
  source: { format: "glb", triangles: 10, points: 0, roomData: false },
  ceilingHeight: 2650,
  ceilingDetected: true,
  walls: [],
  openings: [],
  issues: [
    { id: 0, kind: { type: "extraWall", wall: 7 }, confidence: 0.85, at: { x: 0, y: 0 } },
    { id: 1, kind: { type: "doorOrArch", opening: 2 }, confidence: 0.4, at: { x: 0, y: 0 } },
  ],
};

describe("решения по вопросам проверки", () => {
  test("по умолчанию принимается то, в чём уверенность от 50 %", () => {
    const [extra, door] = result.issues;
    if (!extra || !door) throw new Error("нет вопросов");
    expect(isAccepted(extra, {})).toBe(true);
    expect(isAccepted(door, {})).toBe(false);
    expect(answerList(result, { 0: false })).toEqual([
      { issue: 0, accept: false },
      { issue: 1, accept: false },
    ]);
  });

  test("принятая «лишняя стена» убирается из превью", () => {
    expect([...removedWalls(result, {})]).toEqual([7]);
    expect(removedWalls(result, { 0: false }).size).toBe(0);
  });

  test("параметры берут высоту потолка из скана", () => {
    expect(defaultParams(result).ceilingHeight).toBe(2650);
    expect(percent(0.854)).toBe(85);
  });
});
