import { expect, type Page, test } from "@playwright/test";

// Климат (стадия 11): терморегулятор и тёплый пол с площадью обогрева, бризер
// и воздухообмен, лист тёплых полов и климата.
// Камера по умолчанию: (4000, 3000) мм в центре, 0,1 px на мм.

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

const inspector = (page: Page) => page.getByTestId("inspector");
const devices = (page: Page) => page.getByTestId("device-catalog");
const climate = (page: Page) => page.getByTestId("climate");

/** Две комнаты 3000 × 2000 мм: прямоугольник 1000–4000 × 1000–5000 и перегородка y = 3000. */
async function drawRooms(page: Page) {
  await page.keyboard.press("KeyW");
  for (const [x, y] of [
    [1000, 1000],
    [4000, 1000],
    [4000, 5000],
    [1000, 5000],
    [1000, 1000],
  ] as const) {
    await click(page, x, y);
  }
  await page.keyboard.press("Escape");
  await page.keyboard.press("KeyW");
  await click(page, 1000, 3000);
  await click(page, 4000, 3000);
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await inspector(page)
    .getByRole("button", { name: /Найти помещения/ })
    .click();
  await click(page, 6000, 5500);
}

test.beforeEach(async ({ page }) => {
  page.on("dialog", (d) => {
    throw new Error(`неожиданный диалог: ${d.message()}`);
  });
});

test("климат: тёплый пол, бризер, воздухообмен и лист", async ({ page }) => {
  await openEditor(page);
  await drawRooms(page);

  await page.getByRole("button", { name: "Климат", exact: true }).click();
  await expect(devices(page)).toBeVisible();
  await devices(page)
    .getByRole("button", { name: /Терморегулятор тёплого пола/ })
    .click();
  await click(page, 1100, 2000);
  await devices(page)
    .getByRole("button", { name: /^Бризер/ })
    .click();
  await click(page, 2500, 1150);
  await page.keyboard.press("Escape");

  await page.getByRole("tab", { name: "Климат" }).click();
  await climate(page)
    .getByRole("button", { name: /^Тёплый пол: Помещение 1/ })
    .click();
  const zone = climate(page).locator("[data-zone='Помещение 1']");
  await expect(zone.getByTestId("zone-calc")).toContainText("Вт");
  await expect(zone.getByLabel("Терморегулятор")).not.toHaveValue("");
  await expect(climate(page).getByTestId("air")).toContainText("120 из");

  await page.getByRole("button", { name: /^Чертежи/ }).click();
  const sheets = page.getByTestId("drawings").getByRole("list", { name: "Листы" });
  await expect(sheets.getByRole("button", { name: /План тёплых полов и климата/ })).toBeVisible();
});
