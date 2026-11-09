import type { ClientEvent } from '../bindings/ClientEvent';
import type { ConsoleHistoryPage } from '../bindings/ConsoleHistoryPage';
import {
  type ConsoleEvent,
  MAX_CONSOLE_LINES,
  MAX_LOADED_LINES,
  mergeConsoleEvents,
  olderSnowflake,
} from './consoleLog.ts';

/** Lines asked for in one request for older history (the core caps it as well). */
export const HISTORY_PAGE_SIZE = 200;

export type ConsoleLogState = {
  /** The instance the lines belong to (`null` before one is opened). */
  uuid: string | null;
  log: ConsoleEvent[];
  /** Whether older lines may be fetched from the core's history. */
  hasOlder: boolean;
  loadingOlder: boolean;
  historyFailed: boolean;
  /** No more history is fetched because the log holds `MAX_LOADED_LINES`. */
  limitReached: boolean;
};

/** Where the lines come from: the core's API, or a fake one in tests. */
export type ConsoleLogSource = {
  /** The instance's in-memory console buffer (its newest lines). */
  buffer: (uuid: string) => Promise<ClientEvent[]>;
  /** A page of the instance's stored lines older than `before`. */
  history: (
    uuid: string,
    before: string | null,
    limit: number
  ) => Promise<ConsoleHistoryPage>;
  /** The console output among `events`, as lines. */
  toConsoleEvents: (events: ClientEvent[]) => ConsoleEvent[];
};

export type BufferResult = 'fetched' | 'failed' | 'stale';

const emptyState = (uuid: string | null): ConsoleLogState => ({
  uuid,
  log: [],
  hasOlder: true,
  loadingOlder: false,
  historyFailed: false,
  limitReached: false,
});

/**
 * The console lines of one instance: its buffer, its live stream and the older
 * history fetched page by page, merged by snowflake.
 *
 * Every request belongs to the session that sent it. A session lasts while one
 * instance is shown over one connection: switching instance, reconnecting with
 * another token or leaving the console ends it. An answer that arrives after
 * its session ended is dropped before it touches the log or the history
 * cursor, so a slow answer for the previous instance never shows up in the
 * console of the one selected since.
 */
export class ConsoleLogStore {
  private state: ConsoleLogState = emptyState(null);
  private readonly listeners = new Set<() => void>();
  private readonly source: ConsoleLogSource;
  private session = 0;
  // the cursor for older history: the oldest line received (from the buffer,
  // the live stream or an earlier page), so a page never repeats a line
  private oldest: string | null = null;
  private maxLines = MAX_CONSOLE_LINES;

  constructor(source: ConsoleLogSource) {
    this.source = source;
  }

  getState = (): ConsoleLogState => this.state;

  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  private set(patch: Partial<ConsoleLogState>) {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) listener();
  }

  /**
   * Starts a session showing `uuid`'s lines and returns it; answers to the
   * requests of earlier sessions are dropped from now on. Another instance
   * starts from an empty log, the same one keeps the lines it has.
   */
  open(uuid: string): number {
    this.session += 1;
    if (uuid !== this.state.uuid) {
      this.oldest = null;
      this.maxLines = MAX_CONSOLE_LINES;
      this.set(emptyState(uuid));
    } else if (this.state.loadingOlder) {
      // the page being fetched is dropped, so it can be asked for again
      this.set({ loadingOlder: false });
    }
    return this.session;
  }

  /** Ends the current session; answers still on their way are dropped. */
  close() {
    this.session += 1;
    if (this.state.loadingOlder) this.set({ loadingOlder: false });
  }

  isCurrent(session: number) {
    return session === this.session;
  }

  /** Whether no line has been received yet (the core may have restarted). */
  isEmpty() {
    return this.oldest === null;
  }

  /** Adds lines from the live stream of `session`, unless it has ended. */
  addLive(session: number, events: ClientEvent[]) {
    if (!this.isCurrent(session)) return false;
    this.merge(events, this.maxLines);
    return true;
  }

  private merge(events: ClientEvent[], maxLines: number) {
    // lines that are not console output still move the cursor
    for (const event of events) {
      this.oldest = olderSnowflake(this.oldest, event.snowflake);
    }
    const log = mergeConsoleEvents(
      this.state.log,
      this.source.toConsoleEvents(events),
      maxLines
    );
    // A full log drops its oldest lines as new ones come in. The next page
    // must start right before the oldest line still in the log, or the lines
    // dropped meanwhile would be missing from the console.
    if (log.length >= maxLines && log.length > 0)
      this.oldest = log[0].snowflake;
    if (log !== this.state.log) this.set({ log });
  }

  /** Fetches the instance's console buffer and merges it into the log. */
  async fetchBuffer(): Promise<BufferResult> {
    const session = this.session;
    const uuid = this.state.uuid;
    if (uuid === null) return 'stale';
    let events: ClientEvent[];
    try {
      events = await this.source.buffer(uuid);
    } catch (e) {
      if (!this.isCurrent(session)) return 'stale';
      console.error(e);
      return 'failed';
    }
    // checked before anything is merged, see the class comment
    if (!this.isCurrent(session)) return 'stale';
    this.merge(events, this.maxLines);
    return 'fetched';
  }

  /** Fetches the page of lines before the oldest one received. */
  loadOlder = async (): Promise<void> => {
    const { uuid, loadingOlder, hasOlder } = this.state;
    if (uuid === null || loadingOlder || !hasOlder) return;
    const session = this.session;
    this.set({ loadingOlder: true, historyFailed: false });
    let page: ConsoleHistoryPage;
    try {
      page = await this.source.history(uuid, this.oldest, HISTORY_PAGE_SIZE);
    } catch (e) {
      if (!this.isCurrent(session)) return;
      console.error(e);
      this.set({ loadingOlder: false, historyFailed: true });
      return;
    }
    if (!this.isCurrent(session)) return;
    // the loaded history is kept, up to a limit
    const total = this.state.log.length + page.events.length;
    this.maxLines = Math.min(MAX_LOADED_LINES, Math.max(this.maxLines, total));
    this.merge(page.events, this.maxLines);
    const limitReached = page.has_more && total >= MAX_LOADED_LINES;
    this.set({
      loadingOlder: false,
      hasOlder: page.has_more && !limitReached,
      limitReached,
    });
  };
}
