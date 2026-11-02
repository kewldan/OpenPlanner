import { expect, type Page, test } from "@playwright/test";

// Электрика и освещение (стадия 7): розетки на стене, светильники, выключатель
// с клавишами, группы щита с расчётом, освещённость и листы.
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
const devices = (page: Page) => page.getByTestId("device-catalog");

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

test("электрика: точки, светильники, выключатель, группы щита и листы", async ({ page }) => {
  await openEditor(page);
  await drawRooms(page);

  // «Электрика»: розетка у верхней стены и розетка 380 В у правой.
  await page.getByRole("button", { name: "Электрика", exact: true }).click();
  await expect(devices(page)).toBeVisible();
  await devices(page)
    .getByRole("button", { name: /^Розетка H/ })
    .click();
  await expect(page.getByTestId("tool-hint")).toContainText("поставить точку");
  await click(page, 2000, 1150);
  await devices(page)
    .getByRole("button", { name: /^Розетка 380 В/ })
    .click();
  await click(page, 3900, 4000);
  await page.keyboard.press("Escape");

  // «Освещение»: светильник в каждой комнате и двухклавишный выключатель у левой стены.
  await page.getByRole("button", { name: "Освещение", exact: true }).click();
  await devices(page)
    .getByRole("button", { name: /Светильник потолочный/ })
    .click();
  await click(page, 2500, 2000);
  await click(page, 2500, 4000);
  await devices(page)
    .getByRole("button", { name: /Выключатель двухклавишный/ })
    .click();
  await click(page, 1100, 3600);
  await page.keyboard.press("Escape");

  await page.getByRole("tab", { name: "Объекты" }).click();
  await expect(tree(page).getByRole("heading", { name: /Электрика/ })).toContainText("5");

  // Клавиши выключателя: первая — светильник С1, вторая — С2.
  await tree(page)
    .getByRole("button", { name: /Выключатель 2-кл/ })
    .click();
  const section = inspector(page).getByTestId("device-section");
  await expect(section.getByLabel("Высота H")).toHaveValue("900");
  // Флажки управляемые: состояние приходит из движка после правки.
  const key1 = section.getByRole("group", { name: "Клавиша 1" }).getByLabel(/С1/);
  const key2 = section.getByRole("group", { name: "Клавиша 2" }).getByLabel(/С2/);
  await key1.click();
  await expect(key1).toBeChecked();
  await key2.click();
  await expect(key2).toBeChecked();

  // Группы щита: освещение, розетки, силовая плита; расчёт и состав щита.
  await page.getByRole("tab", { name: "Щит" }).click();
  const panel = page.getByTestId("electrical");
  await panel.getByRole("button", { name: "Распределить по группам" }).click();
  await expect(panel.locator("[data-circuit]")).toHaveCount(3);
  await expect(panel.getByTestId("circuit-calc").first()).toContainText("А");
  await expect(panel.getByTestId("panel")).toContainText("Вводной автомат 3P");
  await expect(panel.getByTestId("illuminance").getByRole("listitem")).toHaveCount(2);

  // Листы электрики в комплекте чертежей.
  await page.getByRole("button", { name: /^Чертежи/ }).click();
  const sheets = page.getByTestId("drawings").getByRole("list", { name: "Листы" });
  for (const title of [
    "План розеток и выводов",
    "План освещения",
    "План выключателей",
    "План электропроводки",
  ]) {
    await expect(sheets.getByRole("button", { name: new RegExp(title) })).toBeVisible();
  }
});
