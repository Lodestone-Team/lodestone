// Run with `npm test` (node --test, no test framework needed)
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { basicAuthHeader } from './basicAuth.ts';

const utf8Base64 = (text: string) =>
  Buffer.from(text, 'utf8').toString('base64');

describe('basicAuthHeader', () => {
  test('encodes plain ASCII as before', () => {
    assert.equal(
      basicAuthHeader('owner', 'hunter2'),
      'Basic b3duZXI6aHVudGVyMg=='
    );
  });

  test('sends accented and CJK usernames as UTF-8', () => {
    assert.equal(
      basicAuthHeader('José', 'pässword'),
      `Basic ${utf8Base64('José:pässword')}`
    );
    assert.equal(
      basicAuthHeader('用户', '密码'),
      `Basic ${utf8Base64('用户:密码')}`
    );
  });
});
