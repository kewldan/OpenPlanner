import type { EngineCall, EngineCallResult } from "@openplanner/engine-client";
import { sheetDate } from "../drawings/export";

/** Инструменты, которые выполняет приложение, а не движок (файлы проекта). */
export const HOST_TOOLS = ["save_project", "open_project"] as const;

/** Что ретрансляции нужно от приложения. */
export interface McpHost {
  callEngine<C extends EngineCall>(call: C): Promise<EngineCallResult[C["type"]]>;
  writeExport(bytes: Uint8Array, path: string): Promise<void>;
  saveProject(path: string | undefined): Promise<{ ok: boolean; path: string | null }>;
  openProject(path: string): Promise<boolean>;
  now?: () => Date;
}

type Json = Record<string, unknown>;

function result(id: unknown, text: string, isError: boolean): string {
  return JSON.stringify({
    jsonrpc: "2.0",
    id,
    result: { content: [{ type: "text", text }], isError },
  });
}

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * Одна строка JSON-RPC от агента → строка ответа (или `null` для уведомлений).
 * Сохранение и открытие проекта выполняет приложение; остальное — движок,
 * файлы экспорта записываются по пути, который указал агент.
 */
export async function handleMcp(line: string, host: McpHost): Promise<string | null> {
  let request: Json | null = null;
  try {
    request = JSON.parse(line) as Json;
  } catch {
    // Разбор и ответ об ошибке — в движке.
  }
  const params = (request?.params ?? {}) as Json;
  const name = typeof params.name === "string" ? params.name : "";
  const args = (params.arguments ?? {}) as Json;
  const id = request?.id;
  if (request?.method === "tools/call" && (HOST_TOOLS as readonly string[]).includes(name)) {
    try {
      if (name === "save_project") {
        const path = typeof args.path === "string" ? args.path : undefined;
        const saved = await host.saveProject(path);
        return saved.ok
          ? result(id, JSON.stringify({ path: saved.path }), false)
          : result(id, "проект не сохранён: у нового проекта укажите path", true);
      }
      const path = typeof args.path === "string" ? args.path : "";
      if (!path) return result(id, "нужен параметр «path»", true);
      const opened = await host.openProject(path);
      return result(id, opened ? JSON.stringify({ path }) : "проект не открыт", !opened);
    } catch (error) {
      return result(id, message(error), true);
    }
  }
  // Дата основной надписи — сегодняшняя.
  let forwarded = line;
  if (request?.method === "tools/call" && name === "export") {
    forwarded = JSON.stringify({
      ...request,
      params: { ...params, arguments: { ...args, date: sheetDate(host.now?.() ?? new Date()) } },
    });
  }
  const reply = await host.callEngine({ type: "mcpMessage", line: forwarded });
  try {
    for (const f of reply.files) await host.writeExport(f.bytes, f.path);
  } catch (error) {
    return result(id, `файл не записан: ${message(error)}`, true);
  }
  return reply.response ?? null;
}
