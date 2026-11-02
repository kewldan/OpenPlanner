// Минимальный клиент W3C WebDriver для tauri-driver (без зависимостей).

const BASE = process.env.WEBDRIVER_URL ?? "http://127.0.0.1:4444";

async function request(method, path, body) {
  const response = await fetch(`${BASE}${path}`, {
    method,
    headers: { "content-type": "application/json" },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
  const json = await response.json();
  if (!response.ok || json.value?.error) {
    throw new Error(`${method} ${path}: ${JSON.stringify(json.value ?? json)}`);
  }
  return json.value;
}

export async function newSession(application) {
  const value = await request("POST", "/session", {
    capabilities: { alwaysMatch: { "tauri:options": { application } } },
  });
  return new Session(value.sessionId);
}

export class Session {
  constructor(id) {
    this.id = id;
  }

  execute(script, ...args) {
    return request("POST", `/session/${this.id}/execute/sync`, { script, args });
  }

  async find(xpath) {
    const value = await request("POST", `/session/${this.id}/element`, {
      using: "xpath",
      value: xpath,
    });
    return Object.values(value)[0];
  }

  async type(element, text) {
    await request("POST", `/session/${this.id}/element/${element}/clear`, {});
    await request("POST", `/session/${this.id}/element/${element}/value`, { text });
  }

  text() {
    return this.execute("return document.body.innerText");
  }

  value(xpath) {
    return this.execute(
      "return document.evaluate(arguments[0], document, null, 9, null).singleNodeValue?.value ?? null",
      xpath,
    );
  }

  async close() {
    await request("DELETE", `/session/${this.id}`).catch(() => {});
  }
}

export async function waitFor(what, check, timeout = 30_000) {
  const start = Date.now();
  let last;
  while (Date.now() - start < timeout) {
    try {
      last = await check();
      if (last) return last;
    } catch (error) {
      last = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error(`не дождались: ${what} (последнее: ${last})`);
}
