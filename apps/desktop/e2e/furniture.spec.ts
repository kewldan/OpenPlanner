import { expect, type Page, test } from "@playwright/test";
import { pyramidGlb } from "./glbModel";

// Мебель (стадия 6): каталог, установка с привязкой к стене, поворот, перетаскивание
// на план, своя модель GLB, лист расстановки. Камера по умолчанию: (4000, 3000) мм в центре, 0,1 px на мм.

const CENTER: [number, number] = [4000, 3000];
const SCALE = 0.1;

async function openEditor(page: Page) {
  await page.goto("/");
  await expect(page.getByText(/GPU: /)).toBeVisible({ timeout: 30_000 });
}

async function screen(page: Page, x: number, y: number): Promise<[number, number]> {
  const box = await page.getByTestId("plan-canvas").boundingBox();
  if (!box) throw new Error("нет холста");
  return [
    box.x + box.width / 2 + (x - CENTER[0]) * SCALE,
    box.y + box.height / 2 + (y - CENTER[1]) * SCALE,
  ];
}

async function click(page: Page, x: number, y: number) {
  const [sx, sy] = await screen(page, x, y);
  await page.mouse.move(sx, sy);
  await page.mouse.click(sx, sy);
}

const inspector = (page: Page) => page.getByTestId("inspector");
const tree = (page: Page) => page.getByTestId("object-tree");
const catalog = (page: Page) => page.getByTestId("catalog");

/** Комната 5000 × 4000 мм: оси x 1500–6500, y 1000–5000, стены 120 мм. */
async function drawRoom(page: Page) {
  await page.keyboard.press("KeyW");
  for (const [x, y] of [
    [1500, 1000],
    [6500, 1000],
    [6500, 5000],
    [1500, 5000],
    [1500, 1000],
  ] as const) {
    await click(page, x, y);
  }
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await inspector(page)
    .getByRole("button", { name: /Найти помещения/ })
    .click();
  await click(page, 8000, 6000);
}

test.beforeEach(async ({ page }) => {
  page.on("dialog", (d) => {
    throw new Error(`неожиданный диалог: ${d.message()}`);
  });
});

test("мебель: каталог, привязка к стене, поворот, перетаскивание, своя модель и лист", async ({
  page,
}) => {
  await openEditor(page);
  await drawRoom(page);

  // Режим «Мебель» открывает каталог; поиск по названию.
  await page.getByRole("button", { name: "Мебель", exact: true }).click();
  await expect(catalog(page)).toBeVisible();
  await catalog(page).getByLabel("Найти в каталоге").fill("кровать");
  await expect(catalog(page).getByRole("button")).toHaveCount(3); // две кровати и «Своя модель»
  await catalog(page)
    .getByRole("button", { name: /Кровать двуспальная/ })
    .click();
  await expect(page.getByTestId("tool-hint")).toContainText("поставить предмет");

  // Спинкой к верхней стене (грань y = 1060): центр встаёт на y = 2060.
  await click(page, 3000, 2150);
  await page.keyboard.press("Escape");
  await page.getByRole("tab", { name: "Объекты" }).click();
  const bed = tree(page).getByRole("button", { name: /Кровать двуспальная/ });
  await expect(bed).toBeVisible();
  await bed.click();
  await expect(inspector(page).getByLabel("Ширина")).toHaveValue("1600");
  await expect(inspector(page).getByLabel("Поворот")).toHaveValue("0");

  // ] — поворот на 90°, правка размера — из инспектора.
  await page.keyboard.press("BracketRight");
  await expect(inspector(page).getByLabel("Поворот")).toHaveValue("90");
  const width = inspector(page).getByLabel("Ширина");
  await width.fill("1400");
  await width.press("Enter");
  await expect(tree(page).getByRole("button", { name: /Кровать двуспальная/ })).toContainText(
    "1 400",
  );

  // Перетаскивание дивана из каталога на план — к нижней стене (грань y = 4940).
  await page.getByRole("tab", { name: "Каталог" }).click();
  await catalog(page).getByLabel("Найти в каталоге").fill("диван");
  const [tx, ty] = await screen(page, 4200, 4400);
  const canvasBox = await page.getByTestId("plan-canvas").boundingBox();
  if (!canvasBox) throw new Error("нет холста");
  await catalog(page)
    .getByRole("button", { name: /^Диван/ })
    .first()
    .dragTo(page.getByTestId("plan-canvas"), {
      targetPosition: { x: tx - canvasBox.x, y: ty - canvasBox.y },
    });
  await page.keyboard.press("Escape");
  await page.getByRole("tab", { name: "Объекты" }).click();
  const sofa = tree(page).getByRole("button", { name: /^Диван/ });
  await expect(sofa).toBeVisible();
  await sofa.click();
  await expect(inspector(page).getByLabel("Поворот")).toHaveValue("180");

  // Своя модель GLB.
  await page.getByRole("tab", { name: "Каталог" }).click();
  const chooser = page.waitForEvent("filechooser");
  await catalog(page).getByRole("button", { name: "Своя модель GLB…" }).click();
  await (await chooser).setFiles({
    name: "Пуф.glb",
    mimeType: "model/gltf-binary",
    buffer: pyramidGlb(),
  });
  await expect(inspector(page).getByLabel("Позиция")).toHaveValue("Своя модель");
  await expect(inspector(page).getByLabel("Высота")).toHaveValue("1200");
  await page.getByRole("tab", { name: "Объекты" }).click();
  await expect(tree(page).getByRole("heading", { name: /Мебель/ })).toContainText("3");

  // 3D-вид и обратно — мебель в модели, ошибок нет.
  await page.getByRole("button", { name: "3D", exact: true }).click();
  await expect(page.getByTestId("camera-3d")).toBeVisible();
  await page.getByRole("button", { name: "2D", exact: true }).click();

  // Лист расстановки в комплекте чертежей.
  await page.getByRole("button", { name: /^Чертежи/ }).click();
  const sheets = page.getByTestId("drawings").getByRole("list", { name: "Листы" });
  await expect(sheets.getByRole("button", { name: /План расстановки мебели/ })).toBeVisible();
  await page.keyboard.press("Escape");

  // Отмена убирает последнюю правку (свою модель).
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  await page.locator("body").press("ControlOrMeta+KeyZ");
  await expect(tree(page).getByRole("heading", { name: /Мебель/ })).toContainText("2");
});
