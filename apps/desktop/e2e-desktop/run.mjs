// E2E настоящего десктопного приложения (Linux, WebKitGTK) под Xvfb.
//
// Проверяет то, что не видит e2e веб-сборки: нативные диалоги файлов (GTK),
// файловые команды Tauri, журнал на диске и восстановление после аварийного
// завершения процесса.
//
// Окружение: Xvfb + оконный менеджер, xdotool, tauri-driver и WebKitWebDriver.
// Запуск: scripts/desktop-e2e.sh (собирает приложение и поднимает всё нужное).

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { inflateRawSync } from "node:zlib";
import { newSession, waitFor } from "./wd.mjs";

const APP = process.env.APP;
const WORK = process.env.WORK;
const SHOTS = process.env.SHOTS;
if (!APP || !WORK || !SHOTS) throw new Error("нужны переменные APP, WORK, SHOTS");

const TITLE = "//label[text()='Название']/following::input[1]";
const PROJECT = join(WORK, "kv.oplan");
const JOURNAL = `${PROJECT}.journal`;
const SCAN = new URL("../e2e/fixtures/apartment.glb", import.meta.url).pathname;

rmSync(WORK, { recursive: true, force: true });
mkdirSync(WORK, { recursive: true });
mkdirSync(SHOTS, { recursive: true });

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const xdo = (...args) => execFileSync("xdotool", args, { encoding: "utf8" }).trim();
const shot = (name) => execFileSync("import", ["-window", "root", join(SHOTS, `${name}.png`)]);
const step = (text) => console.log(`• ${text}`);

/** Нажатие в активном окне — настоящие события X, как от клавиатуры. */
const key = (...keys) => xdo("key", "--clearmodifiers", ...keys);

/** Видимые окна верхнего уровня. */
function windows() {
  try {
    return new Set(xdo("search", "--onlyvisible", "--name", ".*").split("\n"));
  } catch {
    return new Set();
  }
}

/** Ждёт появления нового окна (нативного диалога) и возвращает его id. */
async function waitNewWindow(what, before) {
  const id = await waitFor(what, () => [...windows()].find((w) => !before.has(w)) ?? null);
  return { id };
}

/** Подтверждает диалог GTK: Enter, пока окно не закроется (первый Enter может уйти автодополнению). */
async function confirmDialog(id) {
  for (let attempt = 0; attempt < 4; attempt++) {
    await sleep(400);
    key("Return");
    const closed = await waitFor("диалог закрылся", () => !windows().has(id), 1500).catch(
      () => false,
    );
    if (closed) return;
  }
  throw new Error("диалог не закрылся");
}

/** Читает файл из ZIP без зависимостей (только deflate и stored). */
function readZipEntry(path, entry) {
  const zip = readFileSync(path);
  let offset = 0;
  while (offset < zip.length - 4 && zip.readUInt32LE(offset) === 0x04034b50) {
    const method = zip.readUInt16LE(offset + 8);
    const size = zip.readUInt32LE(offset + 18);
    const nameLength = zip.readUInt16LE(offset + 26);
    const extraLength = zip.readUInt16LE(offset + 28);
    const name = zip.toString("utf8", offset + 30, offset + 30 + nameLength);
    const start = offset + 30 + nameLength + extraLength;
    const data = zip.subarray(start, start + size);
    if (name === entry) {
      return (method === 8 ? inflateRawSync(data) : data).toString("utf8");
    }
    offset = start + size;
  }
  throw new Error(`в ${path} нет ${entry}`);
}

async function launch() {
  const session = await newSession(APP);
  await waitFor("запуск движка", async () => /GPU: /.test(await session.text()));
  // Клавиатурные события xdotool идут в активное окно — активируем окно приложения.
  const app = await waitFor("окно приложения", () => {
    try {
      return xdo("search", "--onlyvisible", "--name", "OpenPlanner").split("\n")[0];
    } catch {
      return null;
    }
  });
  xdo("windowactivate", "--sync", app);
  return session;
}

async function rename(session, title) {
  const field = await session.find(TITLE);
  await session.type(field, `${title}`);
  await waitFor(`название «${title}»`, async () => (await session.value(TITLE)) === title);
}

function assert(condition, message) {
  if (!condition) throw new Error(`проверка не прошла: ${message}`);
  console.log(`  ✓ ${message}`);
}

let session;

async function main() {
  // ── 1. Запуск, правка, сохранение через нативный диалог ─────────────────────
  step("запуск приложения");
  session = await launch();
  const backend = (await session.text()).match(/GPU: (\S+)/)?.[1];
  assert(backend, `движок запущен, бэкенд ${backend}`);
  shot("01-start");

  step("переименование и ⌘S → нативный диалог сохранения");
  await rename(session, "Комендантский пр.");
  assert((await session.text()).includes("Не сохранено"), "есть несохранённые изменения");
  // Фокус уходит с поля, иначе Ctrl+S ушёл бы в поле (оно коммитит по blur — это тоже проверяем).
  let before = windows();
  key("ctrl+s");
  const saveDialog = await waitNewWindow("диалог сохранения", before);
  await sleep(500);
  shot("02-save-dialog");
  assert(saveDialog, "открылся нативный диалог сохранения");
  // Поле имени уже содержит предложенное имя; вводим полный путь.
  key("ctrl+a");
  xdo("type", "--delay", "20", PROJECT.replace(/\.oplan$/, ""));
  await confirmDialog(saveDialog.id);

  await waitFor("файл на диске", () => existsSync(PROJECT));
  await waitFor("статус «Сохранено»", async () => /Сохранено \d/.test(await session.text()));
  const model = JSON.parse(readZipEntry(PROJECT, "model.json"));
  const manifest = JSON.parse(readZipEntry(PROJECT, "manifest.json"));
  assert(model.meta.title === "Комендантский пр.", "в файле правильное название");
  assert(
    manifest.formatVersion === 2,
    `манифест: ${manifest.generator}, формат ${manifest.formatVersion}`,
  );
  assert(existsSync(JOURNAL), "рядом с файлом создан журнал");
  assert(/kv/.test(await session.text()), "в шапке имя файла");
  shot("03-saved");

  // ── 2. Журнал на диске и аварийное завершение ───────────────────────────────
  step("правка после сохранения попадает в журнал");
  await rename(session, "Черновик");
  await waitFor("строка в журнале", () => readFileSync(JOURNAL, "utf8").includes("Черновик"));
  assert(true, "журнал дописан");

  step("аварийное завершение процесса (kill -9)");
  execFileSync("pkill", ["-9", "-f", "openplanner-desktop"]);
  await session.close();
  await sleep(1000);

  // ── 3. Открытие через нативный диалог и восстановление ──────────────────────
  step("повторный запуск и ⌘O → нативный диалог открытия");
  session = await launch();
  assert((await session.value(TITLE)) === "Новый проект", "после перезапуска — новый проект");
  before = windows();
  key("ctrl+o");
  const openDialog = await waitNewWindow("диалог открытия", before);
  await sleep(500);
  shot("04-open-dialog");
  assert(openDialog, "открылся нативный диалог открытия");
  before = windows();
  key("ctrl+l");
  xdo("type", "--delay", "20", PROJECT);
  await confirmDialog(openDialog.id);

  step("вопрос о восстановлении (нативный диалог)");
  const recovery = await waitNewWindow("вопрос о восстановлении", before);
  await sleep(300);
  shot("05-recovery-dialog");
  assert(recovery, "открылся нативный вопрос о восстановлении");
  key("Return");
  await waitFor(
    "восстановленное название",
    async () => (await session.value(TITLE)) === "Черновик",
  );
  assert(true, "изменение из журнала восстановлено");
  assert(
    (await session.text()).includes("Не сохранено"),
    "восстановленное считается несохранённым",
  );
  shot("06-recovered");

  step("⌘S сохраняет в тот же файл без диалога");
  key("ctrl+s");
  await waitFor(
    "файл обновлён",
    () => JSON.parse(readZipEntry(PROJECT, "model.json")).meta.title === "Черновик",
  );
  await waitFor(
    "журнал начат заново",
    () => readFileSync(JOURNAL, "utf8").trim().split("\n").length === 1,
  );
  assert(true, "файл перезаписан, журнал очищен");

  // ── 4. Обмер мышью в настоящем окне ─────────────────────────────────────────
  step("стены мышью (события X) и поиск помещений");
  const [left, top, width, height] = await session.execute(
    "const r = document.querySelector('[data-testid=plan-canvas]').getBoundingClientRect(); return [r.x, r.y, r.width, r.height];",
  );
  const app = xdo("search", "--onlyvisible", "--name", "OpenPlanner").split("\n")[0];
  // Камера по умолчанию: точка (4000, 3000) мм в центре холста, 0,1 px на мм.
  const clickAt = async (x, y) => {
    const sx = Math.round(left + width / 2 + (x - 4000) * 0.1);
    const sy = Math.round(top + height / 2 + (y - 3000) * 0.1);
    xdo("mousemove", "--window", app, String(sx), String(sy));
    await sleep(150);
    xdo("click", "1");
    await sleep(250);
  };
  key("w");
  for (const [x, y] of [
    [1000, 1000],
    [4000, 1000],
    [4000, 5000],
    [1000, 5000],
    [1000, 1000],
  ]) {
    await clickAt(x, y);
  }
  key("Escape");
  key("Escape");
  await waitFor("четыре стены в дереве", async () => /Стены\s*4/.test(await session.text()));
  assert(true, "стены нарисованы мышью");
  await session.click(await session.find("//button[contains(., 'Найти помещения')]"));
  await waitFor("площадь помещения", async () => (await session.text()).includes("11,17 м²"));
  assert(true, "помещение найдено: 11,17 м²");
  shot("07-survey");

  step("⌘S: обмер в файле");
  key("ctrl+s");
  await waitFor("стены и помещение в файле", () => {
    const saved = JSON.parse(readZipEntry(PROJECT, "model.json"));
    return saved.walls.length === 4 && saved.rooms.length === 1;
  });
  assert(true, "в model.json четыре стены и помещение");

  // ── 5. Импорт скана через нативный диалог ───────────────────────────────────
  step("новый проект и ⌘I → мастер импорта скана");
  key("ctrl+n");
  await waitFor("новый проект", async () => (await session.text()).includes("Пусто"));
  key("ctrl+i");
  await waitFor("мастер импорта", async () => (await session.text()).includes("Импорт скана"));
  before = windows();
  await session.click(await session.find("//button[contains(., 'Выбрать файл')]"));
  const scanDialog = await waitNewWindow("диалог выбора скана", before);
  await sleep(500);
  key("ctrl+l");
  xdo("type", "--delay", "20", SCAN);
  await confirmDialog(scanDialog.id);
  await waitFor(
    "распознавание",
    async () => (await session.text()).includes("Стен: 5, проёмов: 4"),
    90_000,
  );
  assert(true, "скан распознан в воркере: 5 стен, 4 проёма");
  shot("08-scan-review");
  await session.click(await session.find("//button[contains(., 'Далее')]"));
  await session.click(await session.find("//button[contains(., 'Импортировать')]"));
  await waitFor("помещения из скана", async () => {
    const text = await session.text();
    return text.includes("14,00 м²") && text.includes("13,52 м²");
  });
  assert(true, "помещения 14,00 и 13,52 м²");
  shot("09-scan-imported");

  await session.close();
  console.log("\nвсе проверки десктопа пройдены");
}

main().catch(async (error) => {
  console.error(`\n✗ ${error.message}`);
  try {
    shot("failure");
    console.error(`окно: ${JSON.stringify(await session?.text())}`);
  } catch {}
  process.exit(1);
});
