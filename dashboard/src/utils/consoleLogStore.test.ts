// Run with `npm test` (node --test, no test framework needed)
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import type { ClientEvent } from '../bindings/ClientEvent';
import type { ConsoleHistoryPage } from '../bindings/ConsoleHistoryPage';
import { MAX_CONSOLE_LINES, MAX_LOADED_LINES } from './consoleLog.ts';
import { ConsoleLogStore } from './consoleLogStore.ts';

// a promise settled by the test, to hold an answer back while something else happens
const deferred = <T>() => {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
};

// only the fields the store and the test's toConsoleEvents look at
const event = (uuid: string, snowflake: number): ClientEvent =>
  ({ snowflake: snowflake.toString(), uuid }) as unknown as ClientEvent;

const events = (uuid: string, from: number, count: number) =>
  Array.from({ length: count }, (_, i) => event(uuid, from + i));

type Pending<T> = { uuid: string; answer: ReturnType<typeof deferred<T>> };
type HistoryRequest = Pending<ConsoleHistoryPage> & { before: string | null };

/** A store whose requests wait until the test answers them. */
const setup = () => {
  const buffers: Pending<ClientEvent[]>[] = [];
  const pages: HistoryRequest[] = [];
  const store = new ConsoleLogStore({
    buffer: (uuid) => {
      const answer = deferred<ClientEvent[]>();
      buffers.push({ uuid, answer });
      return answer.promise;
    },
    history: (uuid, before) => {
      const answer = deferred<ConsoleHistoryPage>();
      pages.push({ uuid, before, answer });
      return answer.promise;
    },
    toConsoleEvents: (list) =>
      list.map((e) => ({
        timestamp: 0,
        snowflake: e.snowflake,
        detail: '',
        uuid: (e as unknown as { uuid: string }).uuid,
        name: '',
        message: `line ${e.snowflake}`,
      })),
  });
  const lines = () => store.getState().log.map((line) => line.uuid);
  return { store, buffers, pages, lines };
};

// lets the store's awaits run
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('ConsoleLogStore', () => {
  test('drops a buffer that arrives after another instance was selected', async () => {
    const { store, buffers, pages, lines } = setup();
    store.open('a');
    const fromA = store.fetchBuffer();
    // the user picks instance b while a's buffer is still on its way
    store.open('b');
    const fromB = store.fetchBuffer();
    buffers[1].answer.resolve(events('b', 500, 3));
    assert.equal(await fromB, 'fetched');
    // a's answer arrives late, with older snowflakes than b's lines
    buffers[0].answer.resolve(events('a', 100, 5));
    assert.equal(await fromA, 'stale');

    assert.equal(store.getState().uuid, 'b');
    assert.deepEqual(lines(), ['b', 'b', 'b'], "a's lines are not in b's log");
    // nor did they move b's history cursor
    store.loadOlder();
    assert.equal(pages[0].uuid, 'b');
    assert.equal(pages[0].before, '500');
  });

  test('drops a failed buffer of a previous instance without reporting it', async () => {
    const { store, buffers } = setup();
    store.open('a');
    const fromA = store.fetchBuffer();
    store.open('b');
    buffers[0].answer.reject(new Error('instance a was deleted'));
    assert.equal(await fromA, 'stale');
  });

  test('drops a history page that arrives after another instance was selected', async () => {
    const { store, buffers, pages, lines } = setup();
    store.open('a');
    store.loadOlder();
    assert.equal(store.getState().loadingOlder, true);
    store.open('b');
    // b can fetch its own history at once
    assert.equal(store.getState().loadingOlder, false);
    store.fetchBuffer();
    buffers[0].answer.resolve(events('b', 500, 2));
    await settle();
    store.loadOlder();
    assert.deepEqual(
      pages.map((page) => [page.uuid, page.before]),
      [
        ['a', null],
        ['b', '500'],
      ]
    );
    pages[0].answer.resolve({ events: events('a', 100, 4), has_more: false });
    await settle();
    assert.deepEqual(lines(), ['b', 'b']);
    assert.equal(store.getState().loadingOlder, true, "b's page is still due");
    assert.equal(store.getState().hasOlder, true, "a's has_more is ignored");
    pages[1].answer.resolve({ events: events('b', 400, 2), has_more: false });
    await settle();
    assert.deepEqual(lines(), ['b', 'b', 'b', 'b']);
    assert.equal(store.getState().loadingOlder, false);
    assert.equal(store.getState().hasOlder, false);
  });

  test('drops live lines of a closed session', () => {
    const { store, lines } = setup();
    const a = store.open('a');
    assert.equal(store.addLive(a, events('a', 1, 1)), true);
    const b = store.open('b');
    assert.equal(store.addLive(a, events('a', 2, 1)), false);
    store.addLive(b, events('b', 3, 1));
    assert.deepEqual(lines(), ['b']);
    store.close();
    assert.equal(store.addLive(b, events('b', 4, 1)), false);
  });

  test('keeps the lines when the same instance reconnects', async () => {
    const { store, buffers, lines } = setup();
    const first = store.open('a');
    store.addLive(first, events('a', 10, 2));
    const late = store.fetchBuffer();
    store.close();
    store.open('a');
    assert.deepEqual(lines(), ['a', 'a']);
    // the previous session's buffer is still dropped; the new one fetches its own
    buffers[0].answer.resolve(events('a', 1, 1));
    assert.equal(await late, 'stale');
    const own = store.fetchBuffer();
    buffers[1].answer.resolve(events('a', 1, 1));
    assert.equal(await own, 'fetched');
    assert.deepEqual(lines(), ['a', 'a', 'a']);
  });

  test('the next page starts before the oldest line still in a full log', async () => {
    const { store, buffers, pages } = setup();
    const session = store.open('a');
    store.fetchBuffer();
    buffers[0].answer.resolve(events('a', 1000, MAX_CONSOLE_LINES));
    await settle();
    // busy server: newer lines push the oldest ones out of the log
    store.addLive(session, events('a', 1000 + MAX_CONSOLE_LINES, 300));
    const log = store.getState().log;
    assert.equal(log.length, MAX_CONSOLE_LINES);
    assert.equal(log[0].snowflake, '1300');
    store.loadOlder();
    // not '1000', which would leave lines 1000..1299 out for good
    assert.equal(pages[0].before, '1300');
  });

  test('stops fetching history at the loaded-lines limit', async () => {
    const { store, buffers, pages } = setup();
    store.open('a');
    store.fetchBuffer();
    const start = 10 * MAX_LOADED_LINES;
    buffers[0].answer.resolve(events('a', start, MAX_CONSOLE_LINES));
    await settle();
    let next = start;
    for (let i = 0; store.getState().hasOlder; i++) {
      assert.ok(i < 1000, 'history never stopped');
      store.loadOlder();
      next -= 500;
      pages[i].answer.resolve({
        events: events('a', next, 500),
        has_more: true,
      });
      await settle();
    }
    const state = store.getState();
    assert.equal(state.log.length, MAX_LOADED_LINES);
    assert.equal(state.limitReached, true);
    assert.equal(state.historyFailed, false);
    // the newest lines are the ones kept
    assert.equal(
      state.log[state.log.length - 1].snowflake,
      (start + MAX_CONSOLE_LINES - 1).toString()
    );
  });
});
