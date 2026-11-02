import { expect, type Page, test } from "@playwright/test";

// Очередь фотореалистичного рендера (стадия 11): кадр текущей камеры трассируется
// в отдельном воркере и сохраняется в проекте.

const CENTER: [number, number] = [4000, 3000];
const SCALE = 0.1;

async function openEditor(page: Page) {
  await page.goto("/");
  await expect(page.getByText(/GPU: /)).toBeVisible({ timeout: 30_000 });
}

async function click(page: Page, x: number, y: number) {
  const box = await page.getByTestId("plan-canvas").boundingBox();
  if (!box) throw new Error("нет холста");
  const sx = box.x + box.width / 2 + (x - CENTER[0]) * SCALE;
  const sy = box.y + box.height / 2 + (y - CENTER[1]) * SCALE;
  await page.mouse.move(sx, sy);
  await page.mouse.click(sx, sy);
}

test("рендер: кадр в очереди, прогресс и сохранение в проекте", async ({ page }) => {
  await openEditor(page);
  await page.keyboard.press("KeyW");
  for (const [x, y] of [
    [2000, 1500],
    [6000, 1500],
    [6000, 4500],
    [2000, 4500],
    [2000, 1500],
  ] as const) {
    await click(page, x, y);
  }
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");

  await page.getByRole("button", { name: "3D", exact: true }).click();
  await page.getByRole("button", { name: "Фотореалистичный рендер…" }).click();
  const dialog = page.getByTestId("render");
  await dialog.getByLabel("Размер").selectOption({ label: "640 × 360" });
  await dialog.getByLabel("Качество").selectOption({ label: "Черновик (8 проходов)" });
  await dialog.getByRole("button", { name: "В очередь" }).click();
  const job = dialog.getByTestId("render-queue").getByRole("listitem").first();
  await expect(job).toHaveAttribute("data-status", "done", { timeout: 60_000 });
  const shot = dialog.getByTestId("render-gallery").getByRole("img");
  await expect(shot).toHaveCount(1);
  await expect(shot).toHaveJSProperty("naturalWidth", 640);
});
