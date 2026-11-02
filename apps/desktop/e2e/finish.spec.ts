import { expect, type Page, test } from "@playwright/test";

// Отделка (стадия 9): поверхности помещения с площадями, плитка с раскладкой и
// подбором старта, краска и обои, материалы проекта, расход и листы.
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

test("отделка: поверхности, раскладка, расход, материалы и листы", async ({ page }) => {
  await openEditor(page);
  await drawRooms(page);

  await page.getByRole("button", { name: "Отделка", exact: true }).click();
  await expect(finish(page)).toBeVisible();
  await expect(finish(page).getByLabel("Помещение")).toBeVisible();
  // Комната 3000 × 2000 по осям, стены 120 мм: пол 2,88 × 1,88 м.
  await expect(finish(page).getByTestId("surfaces")).toContainText("5,41 м²");

  // Пол — керамогранит: штуки, клей, затирка.
  await finish(page).getByRole("button", { name: "+ Пол" }).click();
  const floor = finish(page).locator("[data-finish='floor']");
  await expect(floor.getByTestId("finish-calc")).toContainText("Плитка");
  await expect(floor.getByTestId("finish-calc")).toContainText("Затирка");
  await floor.getByRole("button", { name: "Подобрать старт" }).click();
  await expect(floor.getByRole("alert")).toHaveCount(0);

  // Стены — краска, затем обои.
  await finish(page).getByRole("button", { name: "+ Стены" }).click();
  const walls = finish(page).locator("[data-finish='walls']");
  await expect(walls.getByTestId("finish-calc")).toContainText("Краска");
  await walls.getByLabel("Материал").selectOption({ label: "Обои флизелиновые 1,06 × 10,05 м" });
  await expect(walls.getByTestId("finish-calc")).toContainText("Обои");

  // Свой материал проекта и итог расхода.
  await finish(page).getByRole("button", { name: "Добавить материал" }).click();
  await expect(finish(page).getByLabel("Название материала")).toHaveValue("Материал 1");
  await expect(finish(page).getByTestId("finish-totals")).toContainText("Керамогранит 600×600");

  // Листы отделки и развёртки.
  await page.getByRole("button", { name: /^Чертежи/ }).click();
  const sheets = page.getByTestId("drawings").getByRole("list", { name: "Листы" });
  for (const title of ["План полов", "Развёртки стен"]) {
    await expect(sheets.getByRole("button", { name: new RegExp(title) }).first()).toBeVisible();
  }
});
