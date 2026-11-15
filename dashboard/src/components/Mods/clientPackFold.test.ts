// Run with `npm test`
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { packChips } from './clientPackFold.ts';

test('small client packs show every mod, with no control', () => {
  assert.deepEqual(packChips(0, false), { shown: 0, folds: false });
  assert.deepEqual(packChips(5, false), { shown: 5, folds: false });
  assert.deepEqual(packChips(16, false), { shown: 16, folds: false });
  assert.deepEqual(packChips(16, true), { shown: 16, folds: false });
});

test('a big client pack shows its first twelve until "Show all"', () => {
  assert.deepEqual(packChips(17, false), { shown: 12, folds: true });
  assert.deepEqual(packChips(158, false), { shown: 12, folds: true });
  assert.deepEqual(packChips(158, true), { shown: 158, folds: true });
});
