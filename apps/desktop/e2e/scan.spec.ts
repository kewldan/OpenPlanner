import { readFile } from "node:fs/promises";
import { expect, type Page, test } from "@playwright/test";
import { floorOnly } from "./glb";

// Импорт скана (стадия 2) на веб-сборке. Фикстура — синтетический скан
// квартиры (crates/op-scan/examples/synthetic_scan.rs): комнаты 3500 × 4000
// и 3380 × 4000 мм, перегородка с дверью, окна, входная дверь, шкаф у стены.

const FIXTURE = new URL("./fixtures/apartment.glb", import.meta.url);

async function openWizard(page: Page) {
  await page.goto("/");
  await expect(page.getByText(/GPU: /)).toBeVisible({ timeout: 30_000 });
  await page.keyboard.press("Control+KeyI");
  await expect(page.getByRole("dialog", { name: "Импорт скана" })).toBeVisible();
}

async function choose(page: Page, name: string, buffer: Buffer) {
  const choosing = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Выбрать файл…" }).click();
  await (await choosing).setFiles({ name, mimeType: "application/octet-stream", buffer });
}

const tree = (page: Page) => page.getByTestId("object-tree");

test.beforeEach(async ({ page }) => {
  page.on("dialog", (dialog) => {
    throw new Error(`неожиданный диалог: ${dialog.message()}`);
  });
});

test("скан квартиры: проверка, параметры, импорт одним шагом", async ({ page }) => {
  await openWizard(page);
  await choose(page, "квартира.glb", await readFile(FIXTURE));

  const summary = page.getByTestId("scan-summary");
  await expect(summary).toHaveText("Стен: 5, проёмов: 4", { timeout: 60_000 });
  // Шкаф у стены — «лишняя стена?» с уверенностью; по умолчанию убирается.
  const issues = page.getByRole("list", { name: "Сомнительные места" }).getByRole("listitem");
  await expect(issues.first()).toContainText("Лишняя стена?");
  await expect(issues.first()).toContainText("85 %");
  await issues.first().getByRole("button", { name: "Оставить" }).click();
  await expect(summary).toHaveText("Стен: 6, проёмов: 4");
  await issues.first().getByRole("button", { name: "Убрать" }).click();
  await expect(summary).toHaveText("Стен: 5, проёмов: 4");

  await page.getByRole("button", { name: "Далее" }).click();
  await expect(page.getByLabel("Высота потолка")).toHaveValue("2700");
  await page.getByRole("button", { name: "Импортировать" }).click();
  await expect(page.getByRole("dialog", { name: "Импорт скана" })).toBeHidden();

  await expect(tree(page).getByRole("button", { name: /Помещение 1/ })).toContainText("14,00 м²");
  await expect(tree(page).getByRole("button", { name: /Помещение 2/ })).toContainText("13,52 м²");
  await expect(tree(page).getByRole("heading", { name: /Проёмы/ })).toContainText("4");
  await expect(tree(page).getByRole("button", { name: "квартира.glb" })).toBeVisible();

  await page.getByRole("tab", { name: "Слои" }).click();
  await expect(page.locator('[data-layer="layer-scan"]')).toContainText("1");
  await page.getByRole("tab", { name: "Объекты" }).click();

  // Весь импорт — один шаг отмены.
  await page.locator("body").press("Control+KeyZ");
  await expect(page.getByText(/^Пусто/)).toBeVisible();
});

test("нераспознанный скан: экран ошибки и обводка по подложке", async ({ page }) => {
  await openWizard(page);
  await choose(page, "пол.glb", floorOnly());
  await expect(page.getByText("Стены не распознаны")).toBeVisible({ timeout: 60_000 });
  await expect(page.getByRole("button", { name: "Пересканировать" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Начать с пустого проекта" })).toBeVisible();
  await page.getByRole("button", { name: "Обвести по скану" }).click();
  await expect(page.getByRole("dialog", { name: "Импорт скана" })).toBeHidden();
  await expect(tree(page).getByRole("button", { name: "пол.glb" })).toBeVisible();
  // Сразу инструмент «Стена» — обводить.
  await expect(page.getByTestId("tool-hint")).toHaveText("Щёлкните начало стены");
});

test("нечитаемый файл — понятная ошибка на шаге выбора", async ({ page }) => {
  await openWizard(page);
  await choose(page, "заметки.json", Buffer.from('{"hello": 1}'));
  await expect(page.getByRole("alert")).toContainText("повреждён");
  await expect(page.getByRole("button", { name: "Выбрать файл…" })).toBeVisible();
});
