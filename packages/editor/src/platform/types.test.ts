import { describe, expect, test } from "bun:test";
import { baseName, exportExtension, fileNameFor } from "./types";

describe("имена файлов", () => {
  test("baseName убирает папки и расширение", () => {
    expect(baseName("/Users/a/Ремонт/Комендантский пр.oplan")).toBe("Комендантский пр");
    expect(baseName("C:\\Проекты\\кв.oplan")).toBe("кв");
    expect(baseName("кв.zip")).toBe("кв.zip");
  });

  test("fileNameFor чистит запрещённые символы", () => {
    expect(fileNameFor('Кв. 5/12: "черновик"')).toBe("Кв. 5 12 черновик.oplan");
    expect(fileNameFor("   ")).toBe("Проект.oplan");
    expect(fileNameFor("Комендантский пр.")).toBe("Комендантский пр.oplan");
    expect(fileNameFor("...")).toBe("Проект.oplan");
  });

  test("exportExtension пропускает только форматы экспорта", () => {
    expect(exportExtension("Кв — Вариант А.PDF")).toBe("pdf");
    expect(exportExtension("листы.zip")).toBe("zip");
    expect(exportExtension("кв.oplan")).toBeNull();
    expect(exportExtension("без расширения")).toBeNull();
  });
});
