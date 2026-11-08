// Run with `npm test`
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { nextZoomLevel, zoomShortcut } from './zoomLevels.ts';

const key = (k: string, code: string, mods: Partial<KeyboardEvent> = {}) => ({
  key: k,
  code,
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  ...mods,
});

test('zoom steps through the levels and stops at the ends', () => {
  assert.equal(nextZoomLevel(1, 1), 1.1);
  assert.equal(nextZoomLevel(1, -1), 0.9);
  assert.equal(nextZoomLevel(2, 1), 2);
  assert.equal(nextZoomLevel(0.5, -1), 0.5);
  // a level between steps goes to the next one
  assert.equal(nextZoomLevel(1.05, 1), 1.1);
});

test('Ctrl or ⌘ with +, − and 0 are the zoom shortcuts', () => {
  assert.equal(zoomShortcut(key('=', 'Equal', { metaKey: true })), 'in');
  assert.equal(
    zoomShortcut(key('+', 'Equal', { ctrlKey: true, shiftKey: true })),
    'in'
  );
  assert.equal(zoomShortcut(key('-', 'Minus', { ctrlKey: true })), 'out');
  assert.equal(zoomShortcut(key('0', 'Digit0', { metaKey: true })), 'reset');
  assert.equal(zoomShortcut(key('=', 'Equal')), null);
  assert.equal(
    zoomShortcut(key('-', 'Minus', { metaKey: true, altKey: true })),
    null
  );
  assert.equal(zoomShortcut(key('a', 'KeyA', { metaKey: true })), null);
});
