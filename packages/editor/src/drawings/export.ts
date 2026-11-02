import type {
  DrawingFormat,
  DrawingRequest,
  DrawingSetInfo,
  SheetPreview,
} from "@openplanner/engine-client";
import { callEngine } from "../engine";
import { platform } from "../platform";

/** Что экспортировать: формат движка или PNG (растр SVG листов). */
export type ExportKind = DrawingFormat | "png";

/** Разрешение PNG, точек на дюйм. */
export const PNG_DPI = 150;

/** Дата для основной надписи: месяц и год, «11.26». */
export function sheetDate(now: Date): string {
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const year = String(now.getFullYear() % 100).padStart(2, "0");
  return `${month}.${year}`;
}

/** Запрос комплекта: вариант (или обмер) и сегодняшняя дата. */
export function drawingRequest(
  variant: string | null,
  now = new Date(),
  level: string | null = null,
): DrawingRequest {
  return { ...(variant ? { variant } : {}), ...(level ? { level } : {}), date: sheetDate(now) };
}

/** Имена PNG: один лист — файл, несколько — архив с файлами по листам. */
export function pngNames(info: Pick<DrawingSetInfo, "name" | "sheets">): {
  file: string;
  entries: string[];
} {
  const entries = info.sheets.map((s, i) => `${String(i + 1).padStart(2, "0")} ${s.title}.png`);
  return {
    file: entries.length === 1 ? `${info.name}.png` : `${info.name} — PNG.zip`,
    entries,
  };
}

/** Размер растра листа в пикселях. */
export function rasterSize(sheet: Pick<SheetPreview, "width" | "height">, dpi = PNG_DPI) {
  const px = (mm: number) => Math.round((mm / 25.4) * dpi);
  return { width: px(sheet.width), height: px(sheet.height) };
}

/** Растеризует SVG листа (текст в нём — контурами) в PNG на белом фоне. */
export async function rasterize(sheet: SheetPreview, dpi = PNG_DPI): Promise<Uint8Array> {
  const url = URL.createObjectURL(new Blob([sheet.svg], { type: "image/svg+xml" }));
  try {
    const image = new Image();
    image.src = url;
    await image.decode();
    const { width, height } = rasterSize(sheet, dpi);
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("не удалось создать растр листа");
    context.fillStyle = "#ffffff";
    context.fillRect(0, 0, width, height);
    context.drawImage(image, 0, 0, width, height);
    const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/png"));
    if (!blob) throw new Error("не удалось записать PNG");
    return new Uint8Array(await blob.arrayBuffer());
  } finally {
    URL.revokeObjectURL(url);
  }
}

/**
 * Собирает файл экспорта и предлагает сохранить его.
 * Возвращает имя сохранённого файла или `null`, если пользователь отменил выбор.
 */
export async function exportDrawings(
  kind: ExportKind,
  request: DrawingRequest,
  info: DrawingSetInfo,
): Promise<string | null> {
  let bytes: Uint8Array;
  let name: string;
  if (kind === "png") {
    const images: Uint8Array[] = [];
    for (const sheet of info.sheets) images.push(await rasterize(sheet));
    const names = pngNames(info);
    name = names.file;
    const [single] = images;
    bytes =
      images.length === 1 && single
        ? single
        : await callEngine({ type: "zipFiles", names: names.entries, files: images });
  } else {
    bytes = await callEngine({ type: "exportDrawings", request, format: kind });
    name = info.fileNames[kind];
  }
  return (await platform.saveExport(bytes, name)) ? name : null;
}
