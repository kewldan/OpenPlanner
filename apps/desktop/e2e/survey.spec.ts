import { readFile } from "node:fs/promises";
import { expect, type Page, test } from "@playwright/test";
import { decodePng, encodePng } from "./png";

// Обмер (стадия 1) на веб-сборке: тот же WASM-движок, что в Tauri.
// Камера по умолчанию: точка плана (4000, 3000) мм в центре холста, 0,1 px на мм.

async function openEditor(page: Page) {
  await page.goto("/");
  await expect(page.getByText(/GPU: /)).toBeVisible({ timeout: 30_000 });
}

/** Камера: точка плана в центре холста и масштаб, px на мм. */
interface View {
  center: [number, number];
  scale: number;
}

const DEFAULT_VIEW: View = { center: [4000, 3000], scale: 0.1 };
let view = DEFAULT_VIEW;

async function at(page: Page, x: number, y: number): Promise<[number, number]> {
  const box = await page.getByTestId("plan-canvas").boundingBox();
  if (!box) throw new Error("нет холста");
  return [
    box.x + box.width / 2 + (x - view.center[0]) * view.scale,
    box.y + box.height / 2 + (y - view.center[1]) * view.scale,
  ];
}

/** Камера после «Показать весь план» для содержимого в прямоугольнике `lo`–`hi` (как в движке). */
async function fitView(page: Page, lo: [number, number], hi: [number, number]): Promise<View> {
  const box = await page.getByTestId("plan-canvas").boundingBox();
  if (!box) throw new Error("нет холста");
  const size = [Math.max(hi[0] - lo[0], 1000), Math.max(hi[1] - lo[1], 1000)];
  const scale = Math.min(
    Math.max(box.width - 80, 50) / (size[0] ?? 1),
    Math.max(box.height - 80, 50) / (size[1] ?? 1),
  );
  return { center: [(lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2], scale };
}

async function click(page: Page, x: number, y: number) {
  const [sx, sy] = await at(page, x, y);
  await page.mouse.move(sx, sy);
  await page.mouse.click(sx, sy);
}

const tree = (page: Page) => page.getByTestId("object-tree");
const inspector = (page: Page) => page.getByTestId("inspector");

/** Прямоугольник 3000 × 4000 мм по осям стен; последняя сторона — длиной с клавиатуры. */
async function drawRoom(page: Page) {
  await page.keyboard.press("KeyW");
  await expect(page.getByTestId("tool-hint")).toHaveText("Щёлкните начало стены");
  await click(page, 1000, 1000);
  await click(page, 4000, 1000);
  await click(page, 4000, 5000);
  const [sx, sy] = await at(page, 2000, 5000);
  await page.mouse.move(sx, sy);
  await expect(page.getByTestId("segment")).toContainText("180");
  await page.keyboard.type("3000");
  await expect(page.getByTestId("numeric-input")).toBeVisible();
  await page.keyboard.press("Enter");
  await click(page, 1000, 1000);
  await expect(tree(page).getByRole("heading", { name: /Стены/ })).toContainText("4");
  await page.keyboard.press("Escape");
}

async function findRooms(page: Page) {
  await page.keyboard.press("Escape");
  await inspector(page)
    .getByRole("button", { name: /Найти помещения/ })
    .click();
}

test.beforeEach(async ({ page }) => {
  view = DEFAULT_VIEW;
  page.on("dialog", (dialog) => {
    throw new Error(`неожиданный диалог: ${dialog.message()}`);
  });
});

test("стены кликами и с клавиатуры, помещение с площадью", async ({ page }) => {
  await openEditor(page);
  await drawRoom(page);
  await findRooms(page);
  // Толщина 120 мм по оси: 2,88 × 3,88 м по внутренним поверхностям.
  await expect(tree(page).getByRole("button", { name: /Помещение 1/ })).toContainText("11,17 м²");
  await expect(inspector(page).getByLabel("Площадь")).toHaveValue("11,17 м²");

  const name = inspector(page).getByLabel("Название");
  await name.fill("Кухня");
  await name.press("Enter");
  await expect(tree(page).getByRole("button", { name: /Кухня/ })).toBeVisible();

  // Отмена: переименование, поиск помещений, затем последняя стена — отдельными шагами.
  await page.keyboard.press("Escape");
  await page.keyboard.press("Control+KeyZ");
  await expect(tree(page).getByRole("button", { name: /Помещение 1/ })).toBeVisible();
  await page.keyboard.press("Control+KeyZ");
  await expect(tree(page).getByRole("heading", { name: /Помещения/ })).toBeHidden();
  await page.keyboard.press("Control+KeyZ");
  await expect(tree(page).getByRole("heading", { name: /Стены/ })).toContainText("3");
});

test("проёмы, размер следует за стеной, удаление и отмена", async ({ page }) => {
  await openEditor(page);
  await drawRoom(page);
  await page.keyboard.press("KeyD");
  await click(page, 2000, 1000);
  await page.keyboard.press("KeyO");
  await click(page, 2500, 5000);
  await expect(tree(page).getByRole("button", { name: "Дверь 800" })).toBeVisible();
  await expect(tree(page).getByRole("button", { name: "Окно 1 200" })).toBeVisible();

  await page.keyboard.press("KeyM");
  await click(page, 1000, 1000);
  await click(page, 4000, 1000);
  await click(page, 2500, 600);
  await expect(tree(page).getByRole("button", { name: "Размер 3 000" })).toBeVisible();

  // Длина верхней стены в инспекторе — размер, привязанный к её узлам, меняется.
  await page.keyboard.press("KeyV");
  await click(page, 3000, 1000);
  const length = inspector(page).getByLabel("Длина");
  await expect(length).toHaveValue("3000");
  await length.fill("3600");
  await length.press("Enter");
  await expect(tree(page).getByRole("button", { name: "Размер 3 600" })).toBeVisible();

  await tree(page).getByRole("button", { name: "Дверь 800" }).click();
  await expect(inspector(page).getByLabel("Ширина")).toHaveValue("800");
  await page.locator("body").press("Delete");
  await expect(tree(page).getByRole("button", { name: "Дверь 800" })).toBeHidden();
  await page.keyboard.press("Control+KeyZ");
  await expect(tree(page).getByRole("button", { name: "Дверь 800" })).toBeVisible();
});

const WALL: [number, number, number] = [0x3b, 0x3b, 0x37];

async function wallPixel(page: Page): Promise<[number, number, number]> {
  const canvas = page.getByTestId("plan-canvas");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("нет холста");
  const image = decodePng(await canvas.screenshot());
  // Середина левой стены (x = 1000 мм), с учётом масштаба скриншота.
  const k = image.width / box.width;
  const [sx, sy] = await at(page, 1000, 3000);
  return image.pixel(Math.round((sx - box.x) * k), Math.round((sy - box.y) * k));
}

const near = (a: [number, number, number], b: [number, number, number]) =>
  a.every((v, i) => Math.abs(v - (b[i] ?? 0)) <= 12);

test("скрытый слой не рисуется, заблокированный не выделяется", async ({ page }) => {
  await openEditor(page);
  await drawRoom(page);
  await expect.poll(async () => near(await wallPixel(page), WALL)).toBe(true);

  await page.getByRole("tab", { name: "Слои" }).click();
  const walls = page.locator('[data-layer="layer-walls"]');
  await walls.getByRole("button", { name: "Скрыть слой" }).click();
  await expect.poll(async () => near(await wallPixel(page), WALL)).toBe(false);
  await walls.getByRole("button", { name: "Показать слой" }).click();
  await expect.poll(async () => near(await wallPixel(page), WALL)).toBe(true);

  await walls.getByRole("button", { name: "Заблокировать слой" }).click();
  await click(page, 1000, 3000);
  await expect(inspector(page).getByText("Стена", { exact: true })).toBeHidden();
  await walls.getByRole("button", { name: "Разблокировать слой" }).click();
  await click(page, 1000, 3000);
  await expect(inspector(page).getByText("Стена", { exact: true })).toBeVisible();
});

test("подложка: добавление и калибровка по отрезку", async ({ page }) => {
  await openEditor(page);
  const choosing = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Добавить подложку" }).click();
  await (await choosing).setFiles({
    name: "план БТИ.png",
    mimeType: "image/png",
    buffer: encodePng(200, 100, (x, y) =>
      x % 20 === 0 || y % 20 === 0 ? [40, 40, 160] : [240, 240, 232],
    ),
  });
  await expect(tree(page).getByRole("button", { name: "план БТИ.png" })).toBeVisible();
  const scale = inspector(page).getByLabel("Масштаб");
  const before = Number((await scale.inputValue()).replace(",", "."));

  await inspector(page).getByRole("button", { name: "Калибровать по отрезку" }).click();
  await click(page, 2000, 3000);
  await click(page, 4000, 3000);
  const input = page.getByTestId("numeric-input").getByLabel("Настоящая длина");
  await expect(input).toBeFocused();
  await input.fill("5000");
  await input.press("Enter");
  // Отрезок 2000 мм на подложке оказался 5000 мм: масштаб ×2,5.
  await expect
    .poll(async () => Number((await scale.inputValue()).replace(",", ".")))
    .toBeCloseTo(before * 2.5, 1);
});

test("выноска: текст вводится сразу после создания", async ({ page }) => {
  await openEditor(page);
  await page.keyboard.press("KeyL");
  await click(page, 2000, 2000);
  await click(page, 2600, 1600);
  const text = inspector(page).getByLabel("Текст");
  await expect(text).toBeFocused();
  await page.keyboard.type("Ниша под шкаф");
  await page.keyboard.press("Enter");
  await expect(tree(page).getByRole("button", { name: "Ниша под шкаф" })).toBeVisible();
});

test("обмер сохраняется в .oplan и открывается", async ({ page }) => {
  await openEditor(page);
  await drawRoom(page);
  await findRooms(page);

  const downloading = page.waitForEvent("download");
  await page.keyboard.press("Control+KeyS");
  const saved = await (await downloading).path();
  await expect(page.getByTestId("save-status")).toHaveText(/^Сохранено/);

  await page.keyboard.press("Control+KeyN");
  await expect(page.getByText(/^Пусто/)).toBeVisible();
  const choosing = page.waitForEvent("filechooser");
  await page.keyboard.press("Control+KeyO");
  await (await choosing).setFiles({
    name: "Новый проект.oplan",
    mimeType: "application/zip",
    buffer: await readFile(saved),
  });
  await expect(tree(page).getByRole("button", { name: /Помещение 1/ })).toContainText("11,17 м²");

  // Открытый проект показан целиком: стены по осям от (1000, 1000) до (4000, 5000).
  view = await fitView(page, [1000, 1000], [4000, 5000]);
  await page.keyboard.press("KeyD");
  await click(page, 2000, 1000);
  await expect(tree(page).getByRole("button", { name: "Дверь 800" })).toBeVisible();
});

test("несохранённый обмер восстанавливается после перезагрузки", async ({ page }) => {
  await openEditor(page);
  await drawRoom(page);
  await page.keyboard.press("KeyD");
  await click(page, 2000, 1000);
  await expect(tree(page).getByRole("button", { name: "Дверь 800" })).toBeVisible();
  await expect
    .poll(() =>
      page.evaluate(() =>
        Object.keys(localStorage)
          .filter((key) => key.startsWith("openplanner.journal:"))
          .map((key) => localStorage.getItem(key))
          .join(""),
      ),
    )
    .toContain('"opening"');
  page.removeAllListeners("dialog");
  page.once("dialog", (dialog) => void dialog.accept());
  await page.reload();
  await expect(page.getByText(/GPU: /)).toBeVisible({ timeout: 30_000 });
  await expect(tree(page).getByRole("button", { name: "Дверь 800" })).toBeVisible();
  await expect(tree(page).getByRole("heading", { name: /Стены/ })).toContainText("4");
});
