import { expect, type Page, test } from "@playwright/test";

// Импорт чертежа DXF (стадия 11): слои файла, стены по линиям выбранного слоя.

const ROOM = [
  "0",
  "SECTION",
  "2",
  "HEADER",
  "9",
  "$INSUNITS",
  "70",
  "4",
  "0",
  "ENDSEC",
  "0",
  "SECTION",
  "2",
  "ENTITIES",
  "0",
  "LWPOLYLINE",
  "8",
  "Стены",
  "90",
  "4",
  "70",
  "1",
  "10",
  "0",
  "20",
  "0",
  "10",
  "4000",
  "20",
  "0",
  "10",
  "4000",
  "20",
  "3000",
  "10",
  "0",
  "20",
  "3000",
  "0",
  "LINE",
  "8",
  "Мебель",
  "10",
  "100",
  "20",
  "100",
  "11",
  "900",
  "21",
  "100",
  "0",
  "ENDSEC",
  "0",
  "EOF",
].join("\n");

async function openEditor(page: Page) {
  await page.goto("/");
  await expect(page.getByText(/GPU: /)).toBeVisible({ timeout: 30_000 });
}

test("DXF: стены по линиям слоя и помещение", async ({ page }) => {
  await openEditor(page);
  const chooser = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Импорт DXF" }).click();
  await (await chooser).setFiles({
    name: "план.dxf",
    mimeType: "application/dxf",
    buffer: Buffer.from(ROOM, "utf8"),
  });
  const dialog = page.getByTestId("dxf");
  await expect(dialog).toContainText("4 × 3 м");
  await dialog.getByRole("button", { name: "Стенами", exact: true }).click();
  // Предлагается только слой стен.
  await expect(dialog.getByRole("checkbox", { name: /Стены/ })).toBeChecked();
  await expect(dialog.getByRole("checkbox", { name: /Мебель/ })).not.toBeChecked();
  await dialog.getByRole("button", { name: "Импортировать" }).click();
  await expect(dialog).toBeHidden();

  const tree = page.getByTestId("object-tree");
  await expect(tree.getByRole("heading", { name: /Стены/ })).toContainText("4");
  await page
    .getByTestId("inspector")
    .getByRole("button", { name: /Найти помещения/ })
    .click();
  await expect(tree.getByRole("heading", { name: /Помещения/ })).toContainText("1");
});
