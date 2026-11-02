import { expect, test } from "@playwright/test";
import { decodePng } from "./png";

const BACKGROUND: [number, number, number] = [0xfb, 0xfb, 0xf8];

const near = (a: [number, number, number], b: [number, number, number], tolerance = 4) =>
  a.every((value, i) => Math.abs(value - (b[i] ?? 0)) <= tolerance);

test("холст рисует сетку плана выбранным бэкендом", async ({ page }, testInfo) => {
  await page.goto("/");
  const status = page.getByText(/GPU: /);
  await expect(status).toBeVisible({ timeout: 30_000 });
  const backend = (await status.textContent())?.replace("GPU: ", "").trim();
  const expected = testInfo.project.name === "webgpu" ? "BrowserWebGpu" : "Gl";
  expect(backend).toBe(expected);

  // Камера по умолчанию: метровые линии сетки видны, фон — цвет листа.
  // Под холстом тот же цвет фона (CSS), поэтому ждём именно линий — первого кадра движка.
  const canvas = page.locator("canvas");
  const measure = async () => {
    const image = decodePng(await canvas.screenshot());
    let background = 0;
    let lines = 0;
    for (let y = 0; y < image.height; y++) {
      for (let x = 0; x < image.width; x++) {
        const color = image.pixel(x, y);
        if (near(color, BACKGROUND)) background++;
        // Линии сетки — заметно темнее фона, но остаются светло-серыми.
        else if (color.every((value) => value < 0xf2 && value > 0xb0)) lines++;
      }
    }
    const total = image.width * image.height;
    return { background: background / total, lines: lines / total };
  };
  await expect.poll(async () => (await measure()).lines, { timeout: 15_000 }).toBeGreaterThan(0.01);
  expect((await measure()).background, "большая часть холста — фон листа").toBeGreaterThan(0.5);
});
