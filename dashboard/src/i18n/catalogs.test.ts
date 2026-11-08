// Run with `npm test` (node --test, no test framework needed)
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import {
  detectLanguage,
  formatMessage,
  interpolate,
  type MessageValue,
  matchLanguage,
  placeholders,
} from './core.ts';
import { en } from './en.ts';
import { fr } from './fr.ts';
import { zhHans } from './zh-Hans.ts';
import { zhHant } from './zh-Hant.ts';

const source: Record<string, MessageValue> = en;
const locales: [string, Record<string, MessageValue>][] = [
  ['fr', fr],
  ['zh-Hans', zhHans],
  ['zh-Hant', zhHant],
];

const forms = (message: MessageValue): string[] =>
  typeof message === 'string' ? [message] : Object.values(message);

const placeholderSet = (message: MessageValue) =>
  new Set(forms(message).flatMap(placeholders));

const sorted = (set: Iterable<string>) => [...set].sort();

describe('catalogs', () => {
  test('English has messages', () => {
    assert.ok(Object.keys(source).length > 0);
    for (const [key, message] of Object.entries(source)) {
      for (const form of forms(message))
        assert.notEqual(form.trim(), '', `en: ${key} is empty`);
      if (typeof message !== 'string') {
        assert.ok('other' in message, `en: ${key} has no "other" form`);
        assert.ok(
          placeholderSet(message).has('count'),
          `en: plural ${key} doesn't use {count}`
        );
      }
    }
  });

  for (const [name, catalog] of locales) {
    describe(name, () => {
      test('has exactly the keys of en', () => {
        const missing = Object.keys(source).filter((key) => !(key in catalog));
        const extra = Object.keys(catalog).filter((key) => !(key in source));
        assert.deepEqual(missing, [], `${name} is missing keys`);
        assert.deepEqual(extra, [], `${name} has keys en doesn't have`);
      });

      test('messages are plural where en is, and not empty', () => {
        for (const [key, message] of Object.entries(catalog)) {
          if (!(key in source)) continue;
          const plural = typeof source[key] !== 'string';
          assert.equal(
            typeof message !== 'string',
            plural,
            `${name}: ${key} should ${plural ? '' : 'not '}have plural forms`
          );
          if (typeof message !== 'string')
            assert.ok('other' in message, `${name}: ${key} has no "other"`);
          for (const form of forms(message))
            assert.notEqual(form.trim(), '', `${name}: ${key} is empty`);
        }
      });

      test('placeholders match en', () => {
        for (const [key, message] of Object.entries(catalog)) {
          if (!(key in source)) continue;
          const expected = placeholderSet(source[key]);
          const actual = placeholderSet(message);
          // a plural form may say "one file" without the number itself
          if (expected.has('count')) actual.add('count');
          assert.deepEqual(
            sorted(actual),
            sorted(expected),
            `${name}: ${key} has placeholders ${sorted(actual)}, en has ${sorted(expected)}`
          );
          for (const form of forms(message)) {
            const unknown = placeholders(form).filter((p) => !expected.has(p));
            assert.deepEqual(unknown, [], `${name}: ${key}: "${form}"`);
          }
        }
      });
    });
  }
});

describe('language detection', () => {
  test('matches supported languages and regions', () => {
    assert.equal(matchLanguage('en-GB'), 'en');
    assert.equal(matchLanguage('fr-CA'), 'fr');
    assert.equal(matchLanguage('zh'), 'zh-Hans');
    assert.equal(matchLanguage('zh-CN'), 'zh-Hans');
    assert.equal(matchLanguage('zh-SG'), 'zh-Hans');
    assert.equal(matchLanguage('zh-TW'), 'zh-Hant');
    assert.equal(matchLanguage('zh-HK'), 'zh-Hant');
    assert.equal(matchLanguage('zh-Hant-CN'), 'zh-Hant');
    assert.equal(matchLanguage('zh-Hans-HK'), 'zh-Hans');
    assert.equal(matchLanguage('de-DE'), null);
  });

  test('takes the first supported browser language, else English', () => {
    assert.equal(detectLanguage(['de-DE', 'fr-FR', 'en-US']), 'fr');
    assert.equal(detectLanguage(['ja', 'de']), 'en');
    assert.equal(detectLanguage([]), 'en');
  });
});

describe('formatting', () => {
  test('fills placeholders and leaves unknown ones', () => {
    assert.equal(
      interpolate('en', 'Delete {name}? {other}', { name: 'world' }),
      'Delete world? {other}'
    );
    assert.equal(
      interpolate('en', 'Port {port}', { port: 25565 }),
      'Port 25565'
    );
  });

  test('picks plural forms and formats the count', () => {
    const files = { one: '{count} file', other: '{count} files' };
    assert.equal(formatMessage('en', files, { count: 1 }), '1 file');
    assert.equal(formatMessage('en', files, { count: 0 }), '0 files');
    assert.equal(formatMessage('en', files, { count: 1234 }), '1,234 files');
    const fichiers = { one: '{count} fichier', other: '{count} fichiers' };
    // French: 0 and 1 are singular
    assert.equal(formatMessage('fr', fichiers, { count: 0 }), '0 fichier');
    assert.equal(formatMessage('fr', fichiers, { count: 2 }), '2 fichiers');
    // a form the catalog doesn't have falls back to "other"
    assert.equal(
      formatMessage('fr', fichiers, { count: 1_000_000 }),
      '1 000 000 fichiers'
    );
    assert.equal(
      formatMessage('zh-Hans', { other: '{count} 个文件' }, { count: 1 }),
      '1 个文件'
    );
  });
});
