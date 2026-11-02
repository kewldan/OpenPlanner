import { expect, type Page, test } from "@playwright/test";

// Перепланировка (стадия 3): варианты на общем обмере, демонтаж, новые перегородки,
// сравнение. Камера по умолчанию: точка плана (4000, 3000) мм в центре холста, 0,1 px на мм.

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

const tree = (page: Page) => page.getByTestId("object-tree");
const inspector = (page: Page) => page.getByTestId("inspector");
const viewName = (page: Page) => page.getByTestId("view-name");

/** Обмер: прямоугольник 3000 × 4000 мм и поперечная перегородка — два помещения. */
async function drawSurvey(page: Page) {
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
  await expect(tree(page).getByRole("heading", { name: /Помещения/ })).toContainText("2");
}

test.beforeEach(async ({ page }) => {
  page.on("dialog", (dialog) => {
    throw new Error(`неожиданный диалог: ${dialog.message()}`);
  });
});

test("вариант: демонтаж, перегородка из ГКЛ, сравнение и общий обмер", async ({ page }) => {
  await openEditor(page);
  await drawSurvey(page);
  await expect(viewName(page)).toHaveAttribute("data-view", "survey");

  // Режим «Перепланировка» создаёт первый вариант.
  await page.getByRole("button", { name: "Перепланировка" }).click();
  await expect(viewName(page)).toContainText("Вариант А");
  await expect(viewName(page)).toHaveAttribute("data-view", "variant");

  // Демонтаж перегородки: стена остаётся в списке со статусом.
  await page.keyboard.press("KeyX");
  await expect(page.getByTestId("tool-hint")).toContainText("демонт");
  await click(page, 2500, 3000);
  await expect(tree(page).locator('[data-status="demolish"]')).toHaveCount(1);

  // Новая перегородка из ГКЛ поперёк.
  await page.keyboard.press("KeyW");
  await inspector(page).getByLabel("Материал").selectOption({ label: "ГКЛ" });
  await expect(inspector(page).getByLabel("Толщина")).toHaveValue("100");
  await click(page, 2000, 1000);
  await click(page, 2000, 5000);
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await expect(tree(page).locator('[data-status="new"]')).toHaveCount(1);
  await tree(page).locator('[data-status="new"]').click();
  await expect(inspector(page).getByTestId("status-new")).toBeVisible();
  await expect(inspector(page).getByLabel("Материал")).toHaveValue("gypsum");
  await page.keyboard.press("Escape");

  // Вкладка вариантов: итог изменений.
  await page.getByRole("tab", { name: "Варианты" }).click();
  const row = page.locator("[data-variant]");
  await expect(row).toHaveCount(1);
  await expect(row).toContainText("−3 м · +4 м стен");

  // Сравнение: обмер слева, вариант справа, затем рядом на плане.
  await page.getByRole("button", { name: "Сравнить варианты…" }).click();
  const compare = page.getByTestId("compare");
  await expect(compare.getByRole("columnheader", { name: "Вариант А" })).toBeVisible();
  await expect(compare).toContainText("Демонтаж стен");
  await expect(compare).toContainText("ГКЛ");
  await compare.getByRole("button", { name: "Показать рядом на плане" }).click();
  await expect(viewName(page)).toHaveAttribute("data-view", "compare");
  await expect(page.getByTestId("compare-banner")).toContainText("Обмер (как есть) ↔ Вариант А");
  // Сравнение только для просмотра: инструменты недоступны.
  await expect(page.getByTestId("tool-hint")).toContainText("только для просмотра");
  await page.keyboard.press("KeyW");
  await expect(page.getByTestId("tool-hint")).toContainText("только для просмотра");
  await page.keyboard.press("Escape");
  await expect(viewName(page)).toContainText("Вариант А");

  // Правка обмера попадает во все варианты: дверь в нижней стене.
  await page.getByRole("button", { name: "Обмер", exact: true }).click();
  await expect(viewName(page)).toHaveAttribute("data-view", "survey");
  await page.getByRole("tab", { name: "Объекты" }).click();
  await expect(tree(page).locator("[data-status]")).toHaveCount(0);
  await page.keyboard.press("KeyD");
  await click(page, 1800, 5000);
  await page.keyboard.press("Escape");
  await expect(tree(page).getByRole("button", { name: "Дверь 800" })).toBeVisible();
  await page.getByRole("button", { name: "Перепланировка" }).click();
  await expect(viewName(page)).toContainText("Вариант А");
  await expect(tree(page).getByRole("button", { name: "Дверь 800" })).toBeVisible();
  await expect(tree(page).locator('[data-status="demolish"]')).toHaveCount(1);

  // Вернуть перегородку: статус «Существующий» в инспекторе.
  await tree(page).locator('[data-status="demolish"]').click();
  await inspector(page).getByRole("button", { name: "Существующий" }).click();
  await expect(tree(page).locator('[data-status="demolish"]')).toHaveCount(0);

  // Отмена возвращает демонтаж.
  await page.locator("body").press("Control+KeyZ");
  await expect(tree(page).locator('[data-status="demolish"]')).toHaveCount(1);
});

test("копия варианта независима, удаление варианта с подтверждением", async ({ page }) => {
  await openEditor(page);
  await drawSurvey(page);
  await page.getByRole("button", { name: "Перепланировка" }).click();
  await expect(viewName(page)).toContainText("Вариант А");
  await page.keyboard.press("KeyX");
  await click(page, 2500, 3000);
  await page.keyboard.press("Escape");

  await page.getByRole("tab", { name: "Варианты" }).click();
  const rows = page.locator("[data-variant]");
  await rows.first().getByRole("button", { name: "Копировать вариант" }).click();
  await expect(rows).toHaveCount(2);
  await expect(rows.nth(1).getByLabel("Название варианта")).toHaveValue("Вариант А (копия)");
  await expect(viewName(page)).toContainText("Вариант А (копия)");
  await expect(rows.nth(1)).toContainText("−3 м ·");

  // Демонтаж в копии отменяется, оригинал не меняется.
  await page.getByRole("tab", { name: "Объекты" }).click();
  await tree(page).locator('[data-status="demolish"]').click();
  await inspector(page).getByRole("button", { name: "Существующий" }).click();
  await page.getByRole("tab", { name: "Варианты" }).click();
  await expect(rows.nth(1)).toContainText("−0 м ·");
  await expect(rows.nth(0)).toContainText("−3 м ·");

  page.removeAllListeners("dialog");
  page.once("dialog", (dialog) => void dialog.accept());
  await rows.nth(1).getByRole("button", { name: "Удалить вариант" }).click();
  await expect(rows).toHaveCount(1);
  await expect(viewName(page)).toHaveAttribute("data-view", "survey");
});
