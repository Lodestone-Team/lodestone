// Run with `npm test` (node --test, no test framework needed)
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import type { ClientFile } from '../../bindings/ClientFile.ts';
import {
  compareNullable,
  compareValues,
  nextSort,
  sortFiles,
  type FileSort,
} from './sortFiles.ts';

function file(name: string, overrides: Partial<ClientFile> = {}): ClientFile {
  return {
    name,
    file_stem: name,
    extension: null,
    path: `/${name}`,
    size: 1,
    creation_time: null,
    modification_time: null,
    file_type: 'File',
    ...overrides,
  };
}

const dir = (name: string, overrides: Partial<ClientFile> = {}) =>
  file(name, { file_type: 'Directory', size: null, ...overrides });

const names = (files: ClientFile[]) => files.map((f) => f.name);
const by = (
  key: FileSort['key'],
  direction: FileSort['direction']
): FileSort => ({
  key,
  direction,
});

describe('compareValues', () => {
  test('equal values are 0, including 0 and the empty string', () => {
    assert.equal(compareValues(10, 10), 0);
    assert.equal(compareValues(0, 0), 0);
    assert.equal(compareValues('', ''), 0);
  });

  test('orders either way', () => {
    assert.equal(compareValues(1, 2), -1);
    assert.equal(compareValues(2, 1), 1);
    assert.equal(compareValues('a', 'b'), -1);
  });
});

describe('compareNullable', () => {
  test('a missing value ties with another missing value', () => {
    assert.equal(compareNullable(null, null, 'asc'), 0);
    assert.equal(compareNullable(null, undefined, 'desc'), 0);
  });

  test('a missing value is last in both directions', () => {
    for (const direction of ['asc', 'desc'] as const) {
      assert.equal(compareNullable(null, 5, direction), 1);
      assert.equal(compareNullable(5, null, direction), -1);
      assert.equal(compareNullable(null, 0, direction), 1);
    }
  });

  test('0 is a value, not a missing one', () => {
    assert.equal(compareNullable(0, 0, 'asc'), 0);
    assert.equal(compareNullable(0, 3, 'asc'), -1);
    assert.equal(compareNullable(0, 3, 'desc'), 1);
  });
});

describe('sortFiles', () => {
  test('does not change the input array or its order', () => {
    const input = [file('b'), file('c'), file('a')];
    const snapshot = [...input];
    const sorted = sortFiles(input, by('name', 'asc'));
    assert.deepEqual(names(input), ['b', 'c', 'a']);
    input.forEach((f, i) => assert.equal(f, snapshot[i]));
    assert.notEqual(sorted, input);
  });

  test('no sort returns the list as it is', () => {
    const input = [file('b'), file('a')];
    assert.equal(sortFiles(input, null), input);
  });

  test('sorts by name both ways, naturally and ignoring case', () => {
    const input = [file('file10'), file('File2'), file('file1'), file('Zed')];
    assert.deepEqual(names(sortFiles(input, by('name', 'asc'))), [
      'file1',
      'File2',
      'file10',
      'Zed',
    ]);
    assert.deepEqual(names(sortFiles(input, by('name', 'desc'))), [
      'Zed',
      'file10',
      'File2',
      'file1',
    ]);
  });

  test('folders stay first in either direction', () => {
    const input = [file('b'), dir('z'), file('a'), dir('y')];
    for (const key of ['name', 'date_modified', 'size'] as const) {
      for (const direction of ['asc', 'desc'] as const) {
        const sorted = sortFiles(input, by(key, direction));
        assert.equal(sorted[0].file_type, 'Directory', `${key} ${direction}`);
        assert.equal(sorted[1].file_type, 'Directory', `${key} ${direction}`);
      }
    }
    assert.deepEqual(names(sortFiles(input, by('name', 'asc'))), [
      'y',
      'z',
      'a',
      'b',
    ]);
    assert.deepEqual(names(sortFiles(input, by('name', 'desc'))), [
      'z',
      'y',
      'b',
      'a',
    ]);
  });

  test('folders are ordered by name when sorting by size', () => {
    const input = [dir('b', { size: 99 }), dir('a', { size: 5 })];
    assert.deepEqual(names(sortFiles(input, by('size', 'desc'))), ['a', 'b']);
  });

  test('sorts by size, with 0 bytes as the smallest', () => {
    const input = [
      file('big', { size: 2048 }),
      file('empty', { size: 0 }),
      file('small', { size: 10 }),
    ];
    assert.deepEqual(names(sortFiles(input, by('size', 'asc'))), [
      'empty',
      'small',
      'big',
    ]);
    assert.deepEqual(names(sortFiles(input, by('size', 'desc'))), [
      'big',
      'small',
      'empty',
    ]);
  });

  test('files without a size come last in both directions', () => {
    const input = [
      file('none', { size: null }),
      file('empty', { size: 0 }),
      file('some', { size: 7 }),
    ];
    assert.deepEqual(names(sortFiles(input, by('size', 'asc'))), [
      'empty',
      'some',
      'none',
    ]);
    assert.deepEqual(names(sortFiles(input, by('size', 'desc'))), [
      'some',
      'empty',
      'none',
    ]);
  });

  test('ties keep a stable order by name, whichever the direction', () => {
    const input = [
      file('c', { size: 5 }),
      file('a', { size: 5 }),
      file('b', { size: 5 }),
      file('d', { size: 1 }),
    ];
    assert.deepEqual(names(sortFiles(input, by('size', 'asc'))), [
      'd',
      'a',
      'b',
      'c',
    ]);
    assert.deepEqual(names(sortFiles(input, by('size', 'desc'))), [
      'a',
      'b',
      'c',
      'd',
    ]);
    // the same result from any starting order
    assert.deepEqual(
      names(sortFiles([...input].reverse(), by('size', 'desc'))),
      ['a', 'b', 'c', 'd']
    );
  });

  test('names that differ only by case keep a fixed order', () => {
    const input = [file('a'), file('A')];
    const forward = names(sortFiles(input, by('name', 'asc')));
    const backward = names(sortFiles([...input].reverse(), by('name', 'asc')));
    assert.deepEqual(forward, backward);
  });

  test('sorts by modification time, falling back to creation time', () => {
    const input = [
      file('created', { modification_time: null, creation_time: 150 }),
      file('old', { modification_time: 100, creation_time: 999 }),
      file('new', { modification_time: 200 }),
    ];
    assert.deepEqual(names(sortFiles(input, by('date_modified', 'asc'))), [
      'old',
      'created',
      'new',
    ]);
    assert.deepEqual(names(sortFiles(input, by('date_modified', 'desc'))), [
      'new',
      'created',
      'old',
    ]);
  });

  test('files with no timestamp come last in both directions, tied by name', () => {
    const input = [
      file('z-none'),
      file('dated', { modification_time: 50 }),
      file('a-none'),
      file('epoch', { modification_time: 0 }),
    ];
    assert.deepEqual(names(sortFiles(input, by('date_modified', 'asc'))), [
      'epoch',
      'dated',
      'a-none',
      'z-none',
    ]);
    assert.deepEqual(names(sortFiles(input, by('date_modified', 'desc'))), [
      'dated',
      'epoch',
      'a-none',
      'z-none',
    ]);
  });

  test('an empty list sorts to an empty list', () => {
    assert.deepEqual(sortFiles([], by('size', 'asc')), []);
  });
});

describe('nextSort', () => {
  test('a new column starts ascending', () => {
    assert.deepEqual(nextSort(null, 'size'), by('size', 'asc'));
    assert.deepEqual(nextSort(by('size', 'desc'), 'name'), by('name', 'asc'));
  });

  test('the same column flips', () => {
    assert.deepEqual(nextSort(by('name', 'asc'), 'name'), by('name', 'desc'));
    assert.deepEqual(nextSort(by('name', 'desc'), 'name'), by('name', 'asc'));
  });
});
