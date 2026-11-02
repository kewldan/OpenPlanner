import { expect, type Page, test } from "@playwright/test";

// Сантехника (стадия 8): приборы и выводы под них, стояки, прокладка труб,
// ручная труба, трубы и фитинги, листы водоснабжения и канализации.
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
const tree = (page: Page) => page.getByTestId("object-tree");
const catalog = (page: Page) => page.getByTestId("catalog");
const plumbing = (page: Page) => page.getByTestId("plumbing-catalog");

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

test("сантехника: выводы под приборы, стояки, трубы, проверки и листы", async ({ page }) => {
  await openEditor(page);
  await drawRooms(page);

  // Приборы: раковина и унитаз у верхней стены верхней комнаты.
  await page.getByRole("button", { name: "Мебель", exact: true }).click();
  await catalog(page).getByLabel("Найти в каталоге").fill("раковина");
  await catalog(page)
    .getByRole("button", { name: /^Раковина/ })
    .click();
  await click(page, 1800, 1300);
  await catalog(page).getByLabel("Найти в каталоге").fill("унитаз");
  await catalog(page)
    .getByRole("button", { name: /^Унитаз/ })
    .click();
  await click(page, 2700, 1400);
  await page.keyboard.press("Escape");

  // «Сантехника»: стояки канализации, ХВС и ГВС в углу комнаты.
  await page.getByRole("button", { name: "Сантехника", exact: true }).click();
  await expect(plumbing(page)).toBeVisible();
  for (const [name, x] of [
    ["Стояк канализации", 3700],
    ["Стояк ХВС", 3500],
    ["Стояк ГВС", 3300],
  ] as const) {
    await plumbing(page)
      .getByRole("button", { name: new RegExp(`^${name}`) })
      .click();
    await expect(page.getByTestId("tool-hint")).toContainText("поставить точку");
    await click(page, x, 2700);
  }
  await page.keyboard.press("Escape");

  // «Трубы»: выводы под приборы и прокладка.
  await page.getByRole("tab", { name: "Трубы" }).click();
  const panel = page.getByTestId("plumbing");
  await panel.getByRole("button", { name: "Выводы для приборов" }).click();
  await expect(panel).toContainText("Выводов без трубы: 5");
  await panel.getByRole("button", { name: "Проложить трубы" }).click();
  await expect(panel.locator("[data-pipe]")).toHaveCount(5);
  await expect(panel.getByTestId("pipe-calc").first()).toContainText("м");
  await expect(panel.getByTestId("plumbing-bill")).toContainText("Водорозетка");
  await expect(panel.getByTestId("plumbing-bill")).toContainText("Тройник канализационный");

  // Труба канализации из списка — в инспекторе с уклоном.
  await panel.locator("[data-pipe='drain']").first().getByRole("button").click();
  await expect(page.getByTestId("pipe-section").getByLabel("Уклон")).not.toHaveValue("");

  // Ручная труба вершинами, Enter заканчивает.
  await page.getByRole("tab", { name: "Каталог" }).click();
  await plumbing(page)
    .getByRole("button", { name: /Труба К1 Ø50/ })
    .click();
  await expect(page.getByTestId("tool-hint")).toContainText("Первая вершина");
  await click(page, 1500, 2500);
  await click(page, 2500, 2500);
  await page.keyboard.press("Enter");
  await page.keyboard.press("Escape");
  await page.getByRole("tab", { name: "Объекты" }).click();
  await expect(tree(page).getByRole("heading", { name: /Трубы/ })).toContainText("6");
  await expect(tree(page).getByRole("heading", { name: /Сантехника/ })).toContainText("8");

  // Листы сантехники в комплекте.
  await page.getByRole("button", { name: /^Чертежи/ }).click();
  const sheets = page.getByTestId("drawings").getByRole("list", { name: "Листы" });
  for (const title of ["План водоснабжения", "План канализации"]) {
    await expect(sheets.getByRole("button", { name: new RegExp(title) })).toBeVisible();
  }
});
