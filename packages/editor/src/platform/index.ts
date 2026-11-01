import { isTauri } from "@tauri-apps/api/core";
import { browserPlatform } from "./browser";
import { desktopPlatform } from "./desktop";
import type { Platform } from "./types";

export * from "./types";

/** Текущая платформа: Tauri, если приложение запущено в десктопной оболочке. */
export const platform: Platform = isTauri() ? desktopPlatform : browserPlatform;
