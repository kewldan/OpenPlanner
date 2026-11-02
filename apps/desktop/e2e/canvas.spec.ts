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

test("стены, заливка помещения и подписи рисуются выбранным бэкендом", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText(/GPU: /)).toBeVisible({ timeout: 30_000 });
  const canvas = page.getByTestId("plan-canvas");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("нет холста");
  // Камера по умолчанию: (4000, 3000) мм в центре, 0,1 px на мм.
  const at = (x: number, y: number): [number, number] => [
    box.x + box.width / 2 + (x - 4000) * 0.1,
    box.y + box.height / 2 + (y - 3000) * 0.1,
  ];
  await page.keyboard.press("KeyW");
  for (const [x, y] of [
    [1000, 1000],
    [4000, 1000],
    [4000, 5000],
    [1000, 5000],
    [1000, 1000],
  ] as const) {
    await page.mouse.click(...at(x, y));
  }
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: /Найти помещения/ }).click();
  await expect(page.getByTestId("object-tree")).toContainText("11,17 м²");

  const sample = async () => {
    const image = decodePng(await canvas.screenshot());
    const k = image.width / box.width;
    const pixel = (x: number, y: number) => {
      const [sx, sy] = at(x, y);
      return image.pixel(Math.round((sx - box.x) * k), Math.round((sy - box.y) * k));
    };
    // Подпись помещения: в полосе текста под центром есть тёмные пиксели.
    let ink = 0;
    for (let x = 2000; x <= 3000; x += 10) {
      for (let y = 2850; y <= 2950; y += 10) {
        const [r, g, b] = pixel(x, y);
        if (r < 0x80 && g < 0x80 && b < 0x80) ink++;
      }
    }
    return { wall: pixel(1000, 3000), room: pixel(1500, 4500), ink };
  };
  await expect.poll(async () => near((await sample()).wall, [0x3b, 0x3b, 0x37], 12)).toBe(true);
  const { room, ink } = await sample();
  expect(near(room, [0xf1, 0xee, 0xe4], 6), `заливка помещения ${room}`).toBe(true);
  expect(ink, "подпись помещения нарисована").toBeGreaterThan(3);
});
