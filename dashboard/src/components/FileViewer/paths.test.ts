// Run with `npm test` (node --test, no test framework needed)
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
  coreIgnoresCase,
  getDirectorySeparator,
  isSameOrInside,
  nameTaken,
} from './paths.ts';

describe('getDirectorySeparator', () => {
  test('guesses from the instance path', () => {
    assert.equal(getDirectorySeparator('/home/mc/server'), '/');
    assert.equal(getDirectorySeparator('C:\\mc\\server'), '\\');
  });
});

describe('nameTaken', () => {
  const entries = [{ name: 'server.properties' }, { name: 'world' }];

  test('finds an existing file or folder', () => {
    assert.equal(nameTaken(entries, 'server.properties', false), true);
    assert.equal(nameTaken(entries, 'world', false), true);
    assert.equal(nameTaken(entries, 'notes.txt', false), false);
  });

  test('ignores case only where the core does', () => {
    assert.equal(nameTaken(entries, 'Server.Properties', true), true);
    assert.equal(nameTaken(entries, 'Server.Properties', false), false);
  });
});

describe('coreIgnoresCase', () => {
  test('Windows and macOS cores ignore case, Linux ones do not', () => {
    assert.equal(coreIgnoresCase('windows', '\\'), true);
    assert.equal(coreIgnoresCase('macos', '/'), true);
    assert.equal(coreIgnoresCase('linux', '/'), false);
  });

  test('guesses from the separator before the core info arrives', () => {
    assert.equal(coreIgnoresCase(undefined, '\\'), true);
    assert.equal(coreIgnoresCase(undefined, '/'), false);
  });
});

describe('isSameOrInside', () => {
  test('matches the path itself and what is inside a folder', () => {
    assert.equal(
      isSameOrInside('server.properties', 'server.properties', '/'),
      true
    );
    assert.equal(isSameOrInside('backup/a.txt', 'backup', '/'), true);
    assert.equal(isSameOrInside('backup\\a.txt', 'backup', '\\'), true);
  });

  test('a sibling whose name starts with the folder name is not inside it', () => {
    assert.equal(isSameOrInside('backup.sh', 'backup', '/'), false);
    assert.equal(isSameOrInside('config.yml', 'config', '\\'), false);
  });
});
