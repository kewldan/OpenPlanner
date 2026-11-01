import { describe, expect, test } from "bun:test";
import { baseName, fileNameFor } from "./types";

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
});
