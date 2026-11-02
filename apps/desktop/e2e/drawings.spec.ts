import { readFile } from "node:fs/promises";
import { expect, type Locator, type Page, test } from "@playwright/test";

// Чертежи (стадия 4): комплект листов из модели, основная надпись, формат и экспорт.
// Камера по умолчанию: точка плана (4000, 3000) мм в центре холста, 0,1 px на мм.

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
const dialog = (page: Page) => page.getByTestId("drawings");
const sheets = (page: Page) =>
  dialog(page).getByRole("list", { name: "Листы" }).getByRole("button");

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
  // Найденные помещения выделены; щелчок по пустому месту снимает выделение.
  await click(page, 7000, 5500);
  await expect(inspector(page).getByLabel("Шифр")).toBeVisible();
}

/** Нажимает кнопку экспорта и возвращает имя и байты скачанного файла. */
async function download(page: Page, button: Locator) {
  const downloading = page.waitForEvent("download");
  await button.click();
  const file = await downloading;
  const path = await file.path();
  return { name: file.suggestedFilename(), bytes: await readFile(path) };
}

test.beforeEach(async ({ page }) => {
  page.on("dialog", (d) => {
    throw new Error(`неожиданный диалог: ${d.message()}`);
  });
});

test("комплект листов: основная надпись, формат, экспорт PDF, PNG и пакета", async ({ page }) => {
  await openEditor(page);
  await drawSurvey(page);

  // Графы основной надписи — в панели проекта.
  const designation = inspector(page).getByLabel("Шифр");
  await designation.fill("2026-01-АР");
  await designation.press("Enter");
  await expect(designation).toHaveValue("2026-01-АР");
  await expect(inspector(page).getByLabel("Стадия")).toHaveValue("Р");

  // Обмер: один лист в 1:50.
  await page.getByRole("button", { name: /^Чертежи/ }).click();
  await expect(dialog(page)).toBeVisible();
  await expect(sheets(page)).toHaveCount(1);
  await expect(sheets(page).first()).toContainText("Обмерный план");
  await expect(dialog(page).getByLabel("Масштаб")).toContainText("Авто (1:50)");
  await expect(dialog(page).getByTestId("sheet-preview")).toHaveAttribute("style", /420 \/ 297/);

  const pdf = await download(page, dialog(page).getByRole("button", { name: "PDF" }));
  expect(pdf.name).toBe("Новый проект — Обмер (как есть).pdf");
  expect(pdf.bytes.subarray(0, 4).toString()).toBe("%PDF");
  await expect(dialog(page).getByTestId("drawings-status")).toContainText("Сохранено");

  // Формат листа сохраняется в проекте и сразу меняет предпросмотр.
  await dialog(page).getByLabel("Формат").selectOption({ label: "A4 книжный" });
  await expect(dialog(page).getByTestId("sheet-preview")).toHaveAttribute("style", /210 \/ 297/);
  await page.keyboard.press("Escape");
  await expect(dialog(page)).toBeHidden();
  await page.locator("body").press("ControlOrMeta+KeyZ");
  await page.getByRole("button", { name: "Экспорт", exact: true }).click();
  await expect(dialog(page).getByTestId("sheet-preview")).toHaveAttribute("style", /420 \/ 297/);
  await page.keyboard.press("Escape");

  // Вариант: демонтаж перегородки — листы демонтажа и помещений.
  await page.getByRole("button", { name: "Перепланировка" }).click();
  await expect(page.getByTestId("view-name")).toContainText("Вариант А");
  await page.keyboard.press("KeyX");
  await click(page, 2500, 3000);
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: /^Чертежи/ }).click();
  await expect(dialog(page).getByLabel("Вид")).toHaveValue(/.+/);
  await expect(sheets(page)).toHaveCount(3);
  await expect(sheets(page).nth(1)).toContainText("План демонтажа");
  await expect(sheets(page).nth(2)).toContainText("План помещений");
  await sheets(page).nth(1).click();
  await expect(dialog(page).getByTestId("sheet-preview")).toHaveAttribute("alt", "План демонтажа");

  const bundle = await download(
    page,
    dialog(page).getByRole("button", { name: "Пакет для бригады" }),
  );
  expect(bundle.name).toBe("Новый проект — Вариант А — пакет для бригады.zip");
  expect(bundle.bytes.subarray(0, 2).toString()).toBe("PK");
  expect(bundle.bytes.includes(Buffer.from("Новый проект — Вариант А.pdf"))).toBe(true);

  // PNG растеризуется в UI: три листа — архив.
  const png = await download(page, dialog(page).getByRole("button", { name: "PNG" }));
  expect(png.name).toBe("Новый проект — Вариант А — PNG.zip");
  expect(png.bytes.includes(Buffer.from("02 План демонтажа.png"))).toBe(true);

  // Обратно на обмер — снова один лист.
  await dialog(page).getByLabel("Вид").selectOption({ label: "Обмер (как есть)" });
  await expect(sheets(page)).toHaveCount(1);
});
