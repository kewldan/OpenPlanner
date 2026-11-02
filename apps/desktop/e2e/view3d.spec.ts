import { expect, type Page, test } from "@playwright/test";
import { decodePng } from "./png";

// 3D-вид (стадия 5): орбита, прогулка, срез, солнце, сохранённые ракурсы.
// Камера плана по умолчанию: (4000, 3000) мм в центре холста, 0,1 px на мм.

const CENTER: [number, number] = [4000, 3000];
const SCALE = 0.1;
/** Цвет неба за моделью (`op-render`, `gpu3d.rs`). */
const SKY: [number, number, number] = [233, 238, 243];

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
const camera = (page: Page) => page.getByTestId("camera-3d");

/** Две комнаты 3000 × 2000 мм с дверью в перегородке и окном. */
async function drawApartment(page: Page) {
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
  await page.keyboard.press("KeyD");
  await click(page, 2500, 3000);
  await page.keyboard.press("KeyO");
  await click(page, 2500, 1000);
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await inspector(page)
    .getByRole("button", { name: /Найти помещения/ })
    .click();
  await click(page, 7000, 5500);
}

const near = (a: [number, number, number], b: [number, number, number], tolerance = 6) =>
  a.every((value, i) => Math.abs(value - (b[i] ?? 0)) <= tolerance);

test.beforeEach(async ({ page }) => {
  page.on("dialog", (d) => {
    throw new Error(`неожиданный диалог: ${d.message()}`);
  });
});

test("3D: модель на холсте, орбита, прогулка, срез, солнце и ракурсы", async ({ page }) => {
  await openEditor(page);
  await drawApartment(page);

  await page.getByRole("button", { name: "3D", exact: true }).click();
  await expect(camera(page)).toContainText("Орбита");
  await expect(page.getByTestId("view3d")).toBeVisible();
  // Инструменты плана в 3D недоступны.
  await expect(page.locator('[data-tool="wall"]')).toBeDisabled();

  // Кадр: по краям небо, в середине — модель.
  const canvas = page.getByTestId("plan-canvas");
  const sample = async () => {
    const image = decodePng(await canvas.screenshot());
    return {
      corner: image.pixel(4, 4),
      center: image.pixel(Math.round(image.width / 2), Math.round(image.height / 2)),
    };
  };
  await expect.poll(async () => near((await sample()).corner, SKY), { timeout: 15_000 }).toBe(true);
  expect(near((await sample()).center, SKY), "в середине кадра — модель").toBe(false);

  // Перетаскивание вращает орбиту.
  const before = await camera(page).textContent();
  const box = await canvas.boundingBox();
  if (!box) throw new Error("нет холста");
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 + 150, box.y + box.height / 2, { steps: 5 });
  await page.mouse.up();
  await expect(camera(page)).not.toHaveText(before ?? "");

  // Ракурс сохраняется в проекте и восстанавливается.
  await inspector(page).getByRole("button", { name: "Сохранить ракурс" }).click();
  const views = inspector(page).getByRole("list", { name: "Ракурсы" });
  await expect(views.getByRole("button", { name: /^Ракурс 1/ })).toBeVisible();
  const saved = await camera(page).textContent();

  // Прогулка: W ведёт вперёд на высоте глаз.
  await inspector(page).getByRole("button", { name: "Прогулка", exact: true }).click();
  await expect(camera(page)).toContainText("Прогулка");
  await expect(camera(page)).toContainText("H 1 600");
  const walking = await camera(page).textContent();
  await page.keyboard.down("KeyW");
  await page.waitForTimeout(400);
  await page.keyboard.up("KeyW");
  await expect(camera(page)).not.toHaveText(walking ?? "");
  await expect(camera(page)).toContainText("H 1 600");

  await views.getByRole("button", { name: /^Ракурс 1/ }).click();
  await expect(camera(page)).toHaveText(saved ?? "");

  // Срез, белая модель, солнце.
  await inspector(page).getByText("Срез стен по высоте").click();
  await expect(inspector(page).getByLabel("Высота среза")).toHaveValue("1200");
  await inspector(page).getByText("Белая модель").click();
  await inspector(page).getByText("Солнце и тени").click();
  await inspector(page).getByLabel("Дата").fill("2026-06-21");
  await expect(page.getByTestId("sun-position")).toContainText(/Азимут \d+°, высота (5\d|6\d)°/);
  await expect(camera(page)).toContainText("Орбита");

  // Сохранение ракурса — правка проекта: отмена убирает его. Фокус уходит из
  // поля даты, иначе ⌘Z отменит ввод в поле.
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  await page.locator("body").press("ControlOrMeta+KeyZ");
  await expect(views.getByRole("button", { name: /^Ракурс 1/ })).toBeHidden();

  // Обратно в 2D: снова инструменты плана.
  await page.getByRole("button", { name: "2D", exact: true }).click();
  await expect(page.getByTestId("tool-hint")).toBeVisible();
  await expect(page.locator('[data-tool="wall"]')).toBeEnabled();
});
