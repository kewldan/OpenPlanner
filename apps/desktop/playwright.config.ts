import { defineConfig, devices } from "@playwright/test";

// E2E по веб-сборке: тот же фронтенд и WASM-движок, что в Tauri,
// но с браузерной платформой (файлы — скачивание и выбор файла, журнал — localStorage).
// Перед запуском: `bun run build:wasm` и `bunx vite build`.
export default defineConfig({
  testDir: "e2e",
  timeout: 60_000,
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: "http://localhost:4173",
    viewport: { width: 1440, height: 900 },
    trace: "retain-on-failure",
    launchOptions: {
      // В CI и контейнерах нет GPU: WebGL2 через программный SwiftShader.
      args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"],
      // Без UTF-8-локали Chromium заменяет кириллические имена скачиваемых файлов на «download».
      env: { ...process.env, LANG: "C.UTF-8", LC_ALL: "C.UTF-8" },
    },
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"], viewport: { width: 1440, height: 900 } },
    },
  ],
  webServer: {
    command: "bunx vite preview --port 4173 --strictPort",
    url: "http://localhost:4173",
    reuseExistingServer: !process.env.CI,
  },
});
