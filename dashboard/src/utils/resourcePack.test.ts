// Run with `npm test` (node --test, no test framework needed)
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { formatPackSize, isSha1, packUrlError } from './resourcePack.ts';

describe('packUrlError', () => {
  test('accepts http and https URLs and an empty field', () => {
    assert.equal(packUrlError(''), null);
    assert.equal(packUrlError('  https://example.com/pack.zip  '), null);
    assert.equal(packUrlError('http://192.168.1.5:8000/p.zip?v=2'), null);
  });
  test('refuses other schemes, spaces and non-URLs', () => {
    assert.notEqual(packUrlError('ftp://example.com/p.zip'), null);
    assert.notEqual(packUrlError('file:///etc/passwd'), null);
    assert.notEqual(packUrlError('https://example.com/a b.zip'), null);
    assert.notEqual(packUrlError('example.com/p.zip'), null);
  });
});

describe('isSha1', () => {
  test('is empty or 40 hex characters', () => {
    assert.ok(isSha1(''));
    assert.ok(isSha1('A'.repeat(40)));
    assert.ok(!isSha1('a'.repeat(39)));
    assert.ok(!isSha1('g'.repeat(40)));
  });
});

test('formatPackSize', () => {
  assert.equal(formatPackSize(512), '512 B');
  assert.equal(formatPackSize(1536), '1.5 KB');
  assert.equal(formatPackSize(5 * 1024 * 1024), '5.0 MB');
});
