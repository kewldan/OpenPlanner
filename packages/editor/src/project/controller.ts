import type {
  EngineCall,
  EngineCallResult,
  FromEngine,
  ToEngine,
} from "@openplanner/engine-client";
import { createStore, type StoreApi } from "zustand/vanilla";
import { t } from "../i18n";
import { fileNameFor, type Platform, type SavedFile } from "../platform/types";
import { GENERATOR } from "../version";

/** Состояние открытого проекта (файл, журнал, сохранение). */
export interface ProjectState {
  /** Путь файла; `null` — проект ещё не сохранялся (или платформа не знает путей). */
  path: string | null;
  /** Имя файла без расширения; `null` — проект без имени. */
  name: string | null;
  /** Ключ журнала автосохранения текущего проекта. */
  journalKey: string | null;
  /** Время последнего сохранения, мс с эпохи. */
  savedAt: number | null;
  /** Идёт сохранение или открытие. */
  busy: boolean;
  /** Гостевой просмотр по ссылке: только чтение, без журнала и сохранения. */
  guest: boolean;
}

/** Что контроллеру нужно от движка. */
export interface EngineApi {
  send(message: ToEngine): void;
  call<C extends EngineCall>(call: C): Promise<EngineCallResult[C["type"]]>;
  /** Есть ли несохранённые изменения (по последнему событию движка). */
  isDirty(): boolean;
}

/** Интервал автосохранения проекта, у которого есть файл. */
export const AUTOSAVE_INTERVAL_MS = 30_000;
const UNTITLED_KEY_NAME = "untitled";

/** Копия своих правок при конфликте: «Кв (конфликт 2026-11-02 14-05).oplan» рядом с файлом. */
export function conflictCopy(path: string, name: string, now: Date): SavedFile {
  const pad = (n: number) => String(n).padStart(2, "0");
  const stamp = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())} ${pad(now.getHours())}-${pad(now.getMinutes())}`;
  const copy = `${name} (конфликт ${stamp})`;
  const slash = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return { path: `${path.slice(0, slash + 1)}${copy}.oplan`, name: copy };
}

function hasEntries(journal: string | null): journal is string {
  if (!journal) return false;
  return journal.split("\n").filter((line) => line.trim() !== "").length > 1;
}

/**
 * Жизненный цикл проекта: новый, открыть, сохранить, автосохранение и
 * восстановление из журнала после сбоя.
 *
 * Журнал — строки операций от движка. Он пишется непрерывно, после каждого
 * сохранения начинается заново, а при открытии проекта (или запуске для
 * несохранённого) воспроизводится, если в нём остались операции.
 */
export class ProjectController {
  readonly store: StoreApi<ProjectState>;
  #pendingLines = "";
  #flushTimer: ReturnType<typeof setTimeout> | null = null;
  #writes: Promise<void> = Promise.resolve();
  /** Пока идёт смена проекта или сохранение, строки журнала копятся. */
  #paused = 0;
  /** Запуск (проверка журнала восстановления). Команды пользователя ждут его окончания. */
  #starting: Promise<unknown> = Promise.resolve();
  /** Отметка файла на момент открытия или сохранения (облачная папка). */
  #stamp: string | null = null;

  constructor(
    private readonly platform: Platform,
    private readonly engine: EngineApi,
  ) {
    this.store = createStore<ProjectState>()(() => ({
      path: null,
      name: null,
      journalKey: null,
      savedAt: null,
      busy: false,
      guest: false,
    }));
  }

  /** Вызывается при запуске движка: восстанавливает несохранённый проект прошлого сеанса. */
  async start(): Promise<void> {
    this.#starting = this.#exclusive(async () => {
      const key = await this.platform.journal.locate(null, UNTITLED_KEY_NAME);
      await this.#recoverOrReset(key, t("recovery.untitled"));
      this.store.setState({ path: null, name: null, journalKey: key, savedAt: null });
    }, false);
    await this.#starting;
  }

  /** События движка: строки журнала уходят в журнал текущего проекта. */
  onEngineEvents(events: FromEngine[]) {
    for (const event of events) {
      if (event.type === "journal") this.#pendingLines += event.line;
    }
    if (this.#pendingLines && !this.#flushTimer && this.#paused === 0) {
      this.#flushTimer = setTimeout(() => void this.flushJournal(), 50);
    }
  }

  /** Дописывает накопленные строки в журнал. Записи идут строго по очереди. */
  flushJournal(): Promise<void> {
    if (this.#flushTimer) clearTimeout(this.#flushTimer);
    this.#flushTimer = null;
    const key = this.store.getState().journalKey;
    const text = this.#pendingLines;
    if (!key || !text || this.#paused > 0) return this.#writes;
    this.#pendingLines = "";
    this.#writes = this.#writes
      .then(() => this.platform.journal.append(key, text))
      .catch((error) => this.#report(error));
    return this.#writes;
  }

  /** Новый пустой проект. */
  async newProject(): Promise<void> {
    await this.#exclusive(async () => {
      if (!(await this.#leaveCurrent())) return;
      this.engine.send({ type: "newDocument" });
      const key = await this.platform.journal.locate(null, UNTITLED_KEY_NAME);
      await this.#resetJournal(key);
      this.store.setState({ path: null, name: null, journalKey: key, savedAt: null });
    });
  }

  /** Открыть проект с диска. */
  async openProject(): Promise<void> {
    await this.#exclusive(async () => {
      if (!(await this.#leaveCurrent())) return;
      const file = await this.platform.openProject();
      if (!file) return;
      await this.engine.call({ type: "importFile", bytes: file.bytes });
      this.#pendingLines = "";
      const key = await this.platform.journal.locate(file.path, file.name);
      await this.#recoverOrReset(key, t("recovery.file", { name: file.name }));
      this.#stamp = file.path ? await this.platform.fileStamp(file.path) : null;
      this.store.setState({ path: file.path, name: file.name, journalKey: key, savedAt: null });
    });
  }

  /** Гостевой просмотр: проект из ссылки только для чтения, без журнала. */
  async openGuest(bytes: Uint8Array): Promise<void> {
    this.#starting = this.#exclusive(async () => {
      await this.engine.call({ type: "importFile", bytes });
      this.engine.send({ type: "setReadOnly", readOnly: true });
      this.#pendingLines = "";
      this.store.setState({
        path: null,
        name: t("guest.name"),
        journalKey: null,
        savedAt: null,
        guest: true,
      });
    }, false);
    await this.#starting;
  }

  /**
   * Файл проекта изменили на другом устройстве (облачная папка): предлагает
   * открыть новую версию; несохранённые правки при этом сохраняются копией рядом.
   */
  async checkExternal(): Promise<void> {
    // Без файла или без изменений — ничего не блокируем: команды пользователя идут дальше.
    const before = this.store.getState();
    if (!before.path || before.busy || this.#stamp === null) return;
    const changed = await this.platform.fileStamp(before.path);
    if (changed === null || changed === this.#stamp) return;
    await this.#exclusive(async () => {
      const { path, name } = this.store.getState();
      if (!path || this.#stamp === null) return;
      const stamp = await this.platform.fileStamp(path);
      if (stamp === null || stamp === this.#stamp) return;
      const dirty = this.engine.isDirty();
      const open = await this.platform.confirm(
        t(dirty ? "sync.changedDirty" : "sync.changed", { name: name ?? "" }),
        {
          title: t("sync.title"),
          okLabel: t("sync.open"),
          cancelLabel: t("sync.keep"),
        },
      );
      if (!open) {
        // Своя версия остаётся и при сохранении заменит файл; больше не спрашиваем.
        this.#stamp = stamp;
        return;
      }
      if (dirty) {
        const mine = await this.engine.call({ type: "exportFile", generator: GENERATOR });
        await this.platform.writeProject(mine, conflictCopy(path, name ?? "", new Date()));
      }
      const bytes = await this.platform.readProject(path);
      await this.engine.call({ type: "importFile", bytes });
      this.#pendingLines = "";
      const key = this.store.getState().journalKey;
      if (key) await this.#resetJournal(key);
      this.#stamp = stamp;
    }, false);
  }

  /** Сохранить; для проекта без файла — спросить, куда. */
  async save(): Promise<boolean> {
    return (await this.#exclusive(() => this.#save(false))) === true;
  }

  /** Сохранить в новый файл. */
  async saveAs(): Promise<boolean> {
    return (await this.#exclusive(() => this.#save(true))) === true;
  }

  /** Автосохранение: только для проекта с файлом и только если есть изменения. */
  async autosave(): Promise<void> {
    const { path, busy, guest } = this.store.getState();
    if (!path || busy || guest || !this.engine.isDirty()) return;
    await this.save();
  }

  async #save(forceChoose: boolean): Promise<boolean> {
    const state = this.store.getState();
    if (state.guest) return false;
    // Файл изменили на другом устройстве после открытия — перезаписать только с согласия.
    if (state.path && !forceChoose && this.#stamp !== null) {
      const stamp = await this.platform.fileStamp(state.path);
      if (stamp !== null && stamp !== this.#stamp) {
        const overwrite = await this.platform.confirm(
          t("sync.overwrite", { name: state.name ?? "" }),
          {
            title: t("sync.title"),
            okLabel: t("sync.overwriteOk"),
            cancelLabel: t("confirm.cancel"),
          },
        );
        if (!overwrite) return false;
      }
    }
    let target: SavedFile | null =
      state.path && !forceChoose ? { path: state.path, name: state.name ?? "" } : null;
    if (!target) {
      const title = await this.engine.call({ type: "documentTitle" });
      target = await this.platform.chooseSaveTarget(fileNameFor(title));
      if (!target) return false;
    }
    // Всё, что пришло в журнал до выгрузки, войдёт в файл: эти строки больше не нужны.
    // Строки, пришедшие после, попадут в новый журнал.
    const bytes = await this.engine.call({ type: "exportFile", generator: GENERATOR });
    this.#pendingLines = "";
    await this.platform.writeProject(bytes, target);
    this.engine.send({ type: "markSaved" });
    this.#stamp = target.path ? await this.platform.fileStamp(target.path) : null;

    const key = await this.platform.journal.locate(target.path, target.name);
    if (state.journalKey && state.journalKey !== key) {
      await this.platform.journal.remove(state.journalKey);
    }
    await this.#resetJournal(key);
    this.store.setState({
      path: target.path,
      name: target.name,
      journalKey: key,
      savedAt: Date.now(),
    });
    return true;
  }

  /**
   * Перед сменой проекта: сохраняет изменения проекта с файлом, для
   * несохранённого — спрашивает, можно ли их потерять. `false` — остаться.
   */
  async #leaveCurrent(): Promise<boolean> {
    const { path, journalKey } = this.store.getState();
    if (this.engine.isDirty()) {
      if (path) {
        if (!(await this.#save(false))) return false;
      } else {
        const discard = await this.platform.confirm(t("confirm.discard"), {
          title: t("confirm.discardTitle"),
          okLabel: t("confirm.discardOk"),
          cancelLabel: t("confirm.cancel"),
        });
        if (!discard) return false;
      }
    }
    this.#pendingLines = "";
    const key = this.store.getState().journalKey ?? journalKey;
    if (key) await this.platform.journal.remove(key);
    this.store.setState({ journalKey: null });
    return true;
  }

  async #recoverOrReset(key: string, question: string) {
    const journal = await this.platform.journal.read(key);
    if (hasEntries(journal)) {
      const restore = await this.platform.confirm(question, {
        title: t("recovery.title"),
        okLabel: t("recovery.restore"),
        cancelLabel: t("recovery.discard"),
      });
      if (restore) {
        await this.engine.call({ type: "replayJournal", text: journal });
        return; // журнал продолжает дописываться
      }
    }
    await this.#resetJournal(key);
  }

  async #resetJournal(key: string) {
    const header = await this.engine.call({ type: "journalHeader" });
    await this.platform.journal.reset(key, header);
  }

  /**
   * Операции над проектом не пересекаются; журнал на это время приостановлен.
   * Команды пользователя сначала дожидаются запуска (`userAction`), а во время
   * другой своей операции — отклоняются, чтобы двойное ⌘O не открыло два диалога.
   */
  async #exclusive<T>(action: () => Promise<T>, userAction = true): Promise<T | false> {
    if (userAction) await this.#starting;
    if (this.store.getState().busy) return false;
    this.store.setState({ busy: true });
    await this.flushJournal();
    this.#paused++;
    try {
      return await action();
    } catch (error) {
      this.#report(error);
      return false;
    } finally {
      this.#paused--;
      this.store.setState({ busy: false });
      void this.flushJournal();
    }
  }

  #report(error: unknown) {
    const message = error instanceof Error ? error.message : String(error);
    void this.platform.alert(message, t("error.title"));
  }
}
