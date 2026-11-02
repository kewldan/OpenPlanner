import { expect, type Page, test } from "@playwright/test";

// Этажи (стадия 11): у каждого этажа свой план, переключение, список этажей.
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

async function rectangle(page: Page, w: number, h: number) {
  await page.keyboard.press("KeyW");
  for (const [x, y] of [
    [1000, 1000],
    [1000 + w, 1000],
    [1000 + w, 1000 + h],
    [1000, 1000 + h],
    [1000, 1000],
  ] as const) {
    await click(page, x, y);
  }
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
}

const walls = (page: Page) =>
  page.getByTestId("object-tree").getByRole("heading", { name: /Стены/ });

test("этажи: свой план на каждом этаже и переключение", async ({ page }) => {
  await openEditor(page);
  await rectangle(page, 3000, 2000);
  await expect(walls(page)).toContainText("4");

  await page.getByRole("button", { name: "Добавить этаж" }).click();
  const current = page.getByTestId("levels").getByRole("combobox", { name: "Этаж" });
  await expect(current).toHaveValue(/.+/);
  await expect(page.getByTestId("object-tree")).toHaveCount(0);

  // На втором этаже — свой контур из двух комнат.
  await rectangle(page, 4000, 3000);
  await page.keyboard.press("KeyW");
  await click(page, 3000, 1000);
  await click(page, 3000, 4000);
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await expect(walls(page)).toContainText("5");

  await current.selectOption({ label: "Этаж 1" });
  await expect(walls(page)).toContainText("4");

  await page.getByRole("tab", { name: "Слои" }).click();
  const list = page.getByTestId("level-list");
  await expect(list.getByLabel("Название этажа")).toHaveValue("Этаж 2");
  await expect(list).toContainText("±3 000");
});
