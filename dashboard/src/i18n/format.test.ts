// Run with `npm test` (node --test, no test framework needed)
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
  formatBytes,
  formatBytesParts,
  formatCompact,
  formatFractionParts,
  type NumberPart,
} from './format.ts';

// French puts a (narrow) no-break space before the unit
const text = (parts: NumberPart[]) =>
  parts
    .map((p) => p.text)
    .join('')
    .replace(/\s/g, ' ');
const units = (parts: NumberPart[]) =>
  parts.filter((p) => p.unit).map((p) => p.text.trim());

describe('formatBytesParts', () => {
  test('reads the same as formatBytes and flags the unit', () => {
    for (const language of ['en', 'fr', 'zh-Hans'] as const) {
      const parts = formatBytesParts(language, 1.5 * 1024 * 1024);
      assert.equal(
        parts.map((p) => p.text).join(''),
        formatBytes(language, 1.5 * 1024 * 1024)
      );
      assert.equal(units(parts).length, 1, language);
    }
    const fr = formatBytesParts('fr', 1.5 * 1024 * 1024);
    assert.deepEqual(
      fr.filter((p) => !p.unit && p.text.trim()).map((p) => p.text),
      ['1', ',', '5']
    );
    assert.equal(units(fr)[0], 'Mo');
  });
  test('uses the unit of another size', () => {
    assert.equal(
      text(formatBytesParts('en', 512 * 1024, 1, 3 * 1024 ** 2)),
      '0.5 MB'
    );
  });
});

describe('formatFractionParts', () => {
  test('counts are figures around a slash', () => {
    assert.equal(text(formatFractionParts('en', 3, 20)), '3 / 20');
    assert.equal(text(formatFractionParts('en', null, 20)), '– / 20');
    assert.equal(text(formatFractionParts('en', 1234, null)), '1,234 / –');
  });
  test('sizes share the total unit, printed once', () => {
    const parts = formatFractionParts('en', 1.2 * 1024 ** 2, 3.4 * 1024 ** 2, {
      bytes: true,
    });
    assert.equal(text(parts), '1.2 / 3.4 MB');
    assert.deepEqual(units(parts), ['/', 'MB']);
    assert.equal(
      text(
        formatFractionParts('fr', 512 * 1024, 2 * 1024 ** 2, { bytes: true })
      ),
      '0,5 / 2 Mo'
    );
    assert.equal(
      text(formatFractionParts('en', null, 1024, { bytes: true })),
      '– / 1 kB'
    );
  });
});

describe('formatCompact', () => {
  test('compacts large counts', () => {
    assert.equal(formatCompact('en', 1234567), '1.2M');
    assert.equal(formatCompact('en', 999), '999');
    assert.equal(formatCompact('zh-Hans', 123456), '12.3万');
  });
});
