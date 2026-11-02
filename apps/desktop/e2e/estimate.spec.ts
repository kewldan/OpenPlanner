import { expect, type Page, test } from "@playwright/test";

// Смета (стадия 10): типовой прайс, объёмы из модели, ручной объём, группировка
// по помещениям, закупки, история цен, экспорт сметы и импорт прайса.
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
const finish = (page: Page) => page.getByTestId("finish");
const estimate = (page: Page) => page.getByTestId("estimate");

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

test("смета: прайс, объёмы из модели, ручной объём, закупки, экспорт и импорт", async ({
  page,
}) => {
  await openEditor(page);
  await drawRooms(page);

  // Пол первой комнаты — керамогранит: объём сметы берётся из отделки.
  await page.getByRole("button", { name: "Отделка", exact: true }).click();
  await finish(page).getByRole("button", { name: "+ Пол" }).click();
  await expect(finish(page).getByTestId("finish-calc")).toContainText("Плитка");

  await page.getByRole("button", { name: /^Смета/ }).click();
  await expect(estimate(page)).toContainText("Прайс проекта пуст");
  await estimate(page).getByRole("button", { name: "Типовой прайс" }).click();

  const lines = estimate(page).getByTestId("estimate-lines");
  await expect(lines.locator("tbody[data-group='Отделка']")).toBeVisible();
  const tiles = lines.locator("tr[data-line='О-01']");
  const quantity = tiles.getByLabel(/Количество: Укладка плитки на пол/);
  await expect(quantity).toHaveValue("5,41");
  const total = estimate(page).getByTestId("estimate-total");
  const before = await total.textContent();

  // Ручной объём меняет сумму; «вернуть» — снова по модели.
  await quantity.fill("10");
  await quantity.press("Enter");
  await expect(total).not.toHaveText(before ?? "");
  await tiles.getByRole("button", { name: /Вернуть объём по модели/ }).click();
  await expect(total).toHaveText(before ?? "");

  // По помещениям и закупки.
  await estimate(page).getByRole("button", { name: "По помещениям", exact: true }).click();
  await expect(lines.locator("tbody[data-group='Помещение 1']")).toBeVisible();
  await estimate(page).getByRole("button", { name: "Закупки", exact: true }).click();
  await expect(estimate(page).getByTestId("purchases")).toContainText("Плиточный клей");

  // Экспорт сметы в XLSX.
  await estimate(page).getByRole("button", { name: "Смета", exact: true }).click();
  const download = page.waitForEvent("download");
  await estimate(page).getByRole("button", { name: "Смета в XLSX" }).click();
  expect((await download).suggestedFilename()).toMatch(/смета\.xlsx$/);

  // Прайс: новая цена уходит в историю; импорт CSV обновляет и добавляет позиции.
  await estimate(page).getByRole("button", { name: "Прайс", exact: true }).click();
  const prices = estimate(page).getByTestId("prices");
  const price = prices.getByLabel("Цена: Укладка плитки на пол");
  await price.fill("2000");
  await price.press("Tab");
  await expect(prices.locator("tr[data-price='О-01']")).toContainText("было");
  const chooser = page.waitForEvent("filechooser");
  await estimate(page).getByRole("button", { name: "Импорт прайса…" }).click();
  await (await chooser).setFiles({
    name: "прайс.csv",
    mimeType: "text/csv",
    buffer: Buffer.from("Код;Наименование;Цена\nХ-01;Доставка материалов;3000\n", "utf8"),
  });
  await expect(prices.locator("tr[data-price='Х-01']")).toBeVisible();
});
