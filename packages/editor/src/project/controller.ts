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
    }));
  }

  /** Вызывается при запуске движка: восстанавливает несохранённый проект прошлого сеанса. */
  async start(): Promise<void> {
    await this.#exclusive(async () => {
      const key = await this.platform.journal.locate(null, UNTITLED_KEY_NAME);
      await this.#recoverOrReset(key, t("recovery.untitled"));
      this.store.setState({ path: null, name: null, journalKey: key, savedAt: null });
    });
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
      this.store.setState({ path: file.path, name: file.name, journalKey: key, savedAt: null });
    });
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
    const { path, busy } = this.store.getState();
    if (!path || busy || !this.engine.isDirty()) return;
    await this.save();
  }

  async #save(forceChoose: boolean): Promise<boolean> {
    const state = this.store.getState();
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

  /** Операции над проектом не пересекаются; журнал на это время приостановлен. */
  async #exclusive<T>(action: () => Promise<T>): Promise<T | false> {
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
