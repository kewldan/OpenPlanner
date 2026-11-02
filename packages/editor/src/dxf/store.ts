import type { DxfInfo } from "@openplanner/engine-client";
import { create } from "zustand";
import { callEngine } from "../engine";
import { platform } from "../platform";
import { useEditor } from "../store/editor";

/** Выбранный чертёж DXF, ждущий параметров импорта. */
export interface DxfFile {
  name: string;
  bytes: Uint8Array;
  info: DxfInfo;
}

export const useDxf = create<{ file: DxfFile | null }>()(() => ({ file: null }));

/** Слои, похожие на стены (по названию): их предлагаем стенами. */
export function wallLayers(info: DxfInfo): string[] {
  const walls = info.layers.filter((l) => /стен|перегор|wall|a-wall/i.test(l.name));
  return (walls.length > 0 ? walls : info.layers).map((l) => l.name);
}

/** Выбирает чертёж DXF и открывает окно импорта. */
export async function pickDxf(): Promise<void> {
  try {
    const picked = await platform.openDrawing();
    if (!picked) return;
    const info = await callEngine({ type: "dxfInfo", bytes: picked.bytes });
    useDxf.setState({ file: { name: picked.name, bytes: picked.bytes, info } });
  } catch (error) {
    useEditor.setState({ lastError: error instanceof Error ? error.message : String(error) });
  }
}
