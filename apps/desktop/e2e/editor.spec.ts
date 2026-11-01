import { readFile } from "node:fs/promises";
import { expect, type Page, test } from "@playwright/test";

async function openEditor(page: Page) {
  await page.goto("/");
  await expect(page.getByText(/GPU: /)).toBeVisible({ timeout: 30_000 });
}

async function rename(page: Page, title: string) {
  const field = page.getByLabel("Название");
  await field.fill(title);
  await field.press("Enter");
}

test.beforeEach(async ({ page }) => {
  page.on("dialog", (dialog) => {
    throw new Error(`неожиданный диалог: ${dialog.message()}`);
  });
});

test("холст запускается, команда меняет документ, Ctrl+Z отменяет", async ({ page }) => {
  await openEditor(page);
  await rename(page, "Комендантский пр.");
  await expect(page.getByLabel("Название")).toHaveValue("Комендантский пр.");
  await expect(page.getByTestId("save-status")).toHaveText("Не сохранено");

  await page.locator("body").click({ position: { x: 700, y: 400 } });
  await page.keyboard.press("Control+KeyZ");
  await expect(page.getByLabel("Название")).toHaveValue("Новый проект");
  await page.keyboard.press("Control+Shift+KeyZ");
  await expect(page.getByLabel("Название")).toHaveValue("Комендантский пр.");
});

test("палитра команд находит и выполняет команду", async ({ page }) => {
  await openEditor(page);
  await page.keyboard.press("Control+KeyK");
  const search = page.getByRole("combobox", { name: "Найти команду" });
  await expect(search).toBeFocused();
  await search.fill("тёмн");
  await expect(page.getByRole("option")).toHaveCount(1);
  await search.press("Enter");
  await expect(page.getByRole("dialog")).toBeHidden();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
});

test("шпаргалка горячих клавиш", async ({ page }) => {
  await openEditor(page);
  await page.keyboard.press("Control+Slash");
  const dialog = page.getByRole("dialog", { name: "Горячие клавиши" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText("Сохранить как…")).toBeVisible();
  await expect(dialog.getByText("Ctrl+Shift+S")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
});

test("сохранение и открытие .oplan", async ({ page }) => {
  await openEditor(page);
  await rename(page, "Комендантский пр.");

  const downloading = page.waitForEvent("download");
  await page.keyboard.press("Control+KeyS");
  const download = await downloading;
  expect(download.suggestedFilename()).toBe("Комендантский пр.oplan");
  await expect(page.getByTestId("save-status")).toHaveText(/^Сохранено/);
  await expect(page.getByTestId("project-name")).toHaveText("Комендантский пр");
  const saved = await download.path();

  // Новый проект, затем открыть сохранённый файл.
  await page.keyboard.press("Control+KeyN");
  await expect(page.getByLabel("Название")).toHaveValue("Новый проект");

  const choosing = page.waitForEvent("filechooser");
  await page.keyboard.press("Control+KeyO");
  const chooser = await choosing;
  await chooser.setFiles({
    name: "Комендантский пр.oplan",
    mimeType: "application/zip",
    buffer: await readFile(saved),
  });
  await expect(page.getByLabel("Название")).toHaveValue("Комендантский пр.");
  await expect(page.getByTestId("project-name")).toHaveText("Комендантский пр");
});

test("несохранённые изменения восстанавливаются после перезагрузки", async ({ page }) => {
  await openEditor(page);
  await rename(page, "Черновик");
  await rename(page, "Черновик 2");
  // Журнал дописывается с небольшой задержкой.
  await page.waitForTimeout(300);

  page.removeAllListeners("dialog");
  page.once("dialog", (dialog) => {
    expect(dialog.message()).toContain("несохранённые изменения");
    void dialog.accept();
  });
  await page.reload();
  await expect(page.getByText(/GPU: /)).toBeVisible({ timeout: 30_000 });
  await expect(page.getByLabel("Название")).toHaveValue("Черновик 2");
  await expect(page.getByTestId("save-status")).toHaveText("Не сохранено");

  // Отмена работает и для восстановленной истории.
  await page.locator("body").click({ position: { x: 700, y: 400 } });
  await page.keyboard.press("Control+KeyZ");
  await expect(page.getByLabel("Название")).toHaveValue("Черновик");
});
