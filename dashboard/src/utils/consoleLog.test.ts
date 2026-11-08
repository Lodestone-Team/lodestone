// Run with `npm test` (node --test, no test framework needed)
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
  type ConsoleEvent,
  MAX_CONSOLE_LINES,
  mergeConsoleEvents,
  olderSnowflake,
} from './consoleLog.ts';

const line = (snowflake: number | bigint): ConsoleEvent => ({
  timestamp: 0,
  snowflake: snowflake.toString(),
  detail: '',
  uuid: 'a',
  name: 'a',
  message: `line ${snowflake}`,
});

const ids = (log: ConsoleEvent[]) => log.map((event) => event.snowflake);

describe('mergeConsoleEvents', () => {
  test('appends newer lines', () => {
    const log = mergeConsoleEvents([line(1), line(2)], [line(3), line(4)]);
    assert.deepEqual(ids(log), ['1', '2', '3', '4']);
  });

  test('puts older history in front without repeating lines already received', () => {
    // live lines arrived first; the older page overlaps them by two lines
    const live = [line(5), line(6), line(7)];
    const page = [line(3), line(4), line(5), line(6)];
    const log = mergeConsoleEvents(live, page);
    assert.deepEqual(ids(log), ['3', '4', '5', '6', '7']);
  });

  test('is unchanged by a page it already has', () => {
    const log = [line(3), line(4), line(5)];
    assert.deepEqual(ids(mergeConsoleEvents(log, [line(3), line(4)])), [
      '3',
      '4',
      '5',
    ]);
  });

  test('orders 64-bit snowflakes exactly', () => {
    const big = BigInt(1) << BigInt(62);
    const log = mergeConsoleEvents(
      [line(big + BigInt(2))],
      [line(big + BigInt(1))]
    );
    assert.deepEqual(ids(log), [
      (big + BigInt(1)).toString(),
      (big + BigInt(2)).toString(),
    ]);
  });

  test('keeps the newest maxLines', () => {
    const log = mergeConsoleEvents(
      [line(1), line(2), line(3)],
      [line(4), line(5)],
      3
    );
    assert.deepEqual(ids(log), ['3', '4', '5']);
  });

  test('keeps loaded history when the limit is raised', () => {
    const live = Array.from({ length: MAX_CONSOLE_LINES }, (_, i) =>
      line(10000 + i)
    );
    const older = Array.from({ length: 100 }, (_, i) => line(i));
    assert.equal(
      mergeConsoleEvents(live, older)[0].snowflake,
      '10000',
      'the default limit drops the history again'
    );
    const log = mergeConsoleEvents(live, older, MAX_CONSOLE_LINES + 100);
    assert.equal(log.length, MAX_CONSOLE_LINES + 100);
    assert.equal(log[0].snowflake, '0');
  });
});

describe('olderSnowflake', () => {
  test('picks the older one, ignoring none', () => {
    assert.equal(olderSnowflake(null, null), null);
    assert.equal(olderSnowflake(null, '5'), '5');
    assert.equal(olderSnowflake('7', null), '7');
    assert.equal(olderSnowflake('7', '5'), '5');
    assert.equal(olderSnowflake('5', '7'), '5');
  });
});
