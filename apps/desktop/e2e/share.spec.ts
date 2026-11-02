import { expect, type Page, test } from "@playwright/test";

// Гостевая ссылка (стадия 11): проект в адресе открывается в веб-версии только
// для просмотра.

const CENTER: [number, number] = [4000, 3000];
const SCALE = 0.1;

async function openEditor(page: Page, url = "/") {
  await page.goto(url);
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

const walls = (page: Page) =>
  page.getByTestId("object-tree").getByRole("heading", { name: /Стены/ });

test("гостевая ссылка: проект из адреса только для просмотра", async ({ page, context }) => {
  await openEditor(page);
  await page.keyboard.press("KeyW");
  for (const [x, y] of [
    [1000, 1000],
    [4000, 1000],
    [4000, 4000],
    [1000, 4000],
    [1000, 1000],
  ] as const) {
    await click(page, x, y);
  }
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await expect(walls(page)).toContainText("4");

  await page.getByRole("button", { name: /^Поделиться/ }).click();
  const share = page.getByTestId("share");
  const link = share.getByLabel("Гостевая ссылка");
  await expect(link).toHaveValue(/#guest=/);
  const url = await link.inputValue();

  const guest = await context.newPage();
  await openEditor(guest, url);
  await expect(guest.getByTestId("guest")).toHaveText("Только просмотр");
  await expect(walls(guest)).toContainText("4");
  // Правки отклоняются.
  await guest.keyboard.press("KeyW");
  await click(guest, 6000, 5000);
  await click(guest, 7000, 5000);
  await guest.keyboard.press("Escape");
  await expect(walls(guest)).toContainText("4");
});
