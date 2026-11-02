import { describe, expect, test } from "bun:test";
import type { EngineCall } from "@openplanner/engine-client";
import { handleMcp, type McpHost } from "./relay";

function host(overrides: Partial<McpHost> = {}) {
  const calls: EngineCall[] = [];
  const written: string[] = [];
  const h: McpHost = {
    callEngine: (async (call: EngineCall) => {
      calls.push(call);
      return {
        response: '{"jsonrpc":"2.0","id":1,"result":{}}',
        files: [{ path: "/tmp/кв.pdf", bytes: new Uint8Array([37, 80]) }],
      };
    }) as McpHost["callEngine"],
    writeExport: async (_b, path) => {
      written.push(path);
    },
    saveProject: async (path) => ({ ok: path !== undefined, path: path ?? null }),
    openProject: async () => true,
    now: () => new Date(2026, 10, 2),
    ...overrides,
  };
  return { h, calls, written };
}

const call = (name: string, args: object) =>
  JSON.stringify({
    jsonrpc: "2.0",
    id: 5,
    method: "tools/call",
    params: { name, arguments: args },
  });

describe("MCP в приложении", () => {
  test("сохранение и открытие выполняет приложение", async () => {
    const { h, calls } = host();
    const saved = JSON.parse(
      (await handleMcp(call("save_project", { path: "/p/кв.oplan" }), h)) ?? "",
    );
    expect(saved.result.isError).toBe(false);
    const unsaved = JSON.parse((await handleMcp(call("save_project", {}), h)) ?? "");
    expect(unsaved.result.isError).toBe(true);
    const opened = JSON.parse(
      (await handleMcp(call("open_project", { path: "/p/a.oplan" }), h)) ?? "",
    );
    expect(opened.result.isError).toBe(false);
    expect(calls).toHaveLength(0);
  });

  test("экспорт: дата штампа и запись файла по пути агента", async () => {
    const { h, calls, written } = host();
    const reply = await handleMcp(call("export", { format: "pdf", path: "/tmp/кв.pdf" }), h);
    expect(reply).toContain('"id":1');
    const sent = JSON.parse((calls[0] as { line: string }).line);
    expect(sent.params.arguments.date).toBe("11.26");
    expect(written).toEqual(["/tmp/кв.pdf"]);
  });

  test("ошибка записи — ошибкой инструмента", async () => {
    const { h } = host({
      writeExport: async () => {
        throw new Error("нет доступа");
      },
    });
    const reply = JSON.parse(
      (await handleMcp(call("export", { format: "pdf", path: "/x.pdf" }), h)) ?? "",
    );
    expect(reply.result.isError).toBe(true);
    expect(reply.result.content[0].text).toContain("нет доступа");
  });
});
