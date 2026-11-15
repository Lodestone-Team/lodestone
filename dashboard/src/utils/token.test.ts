// Run with `npm test` (node --test, no test framework needed)
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { tokenClaims } from './token.ts';

const b64 = (value: object) => Buffer.from(JSON.stringify(value)).toString('base64url');
const jwt = (claims: object) => `${b64({ typ: 'JWT', alg: 'HS512' })}.${b64(claims)}.sig`;

describe('tokenClaims', () => {
  test('reads a JWT', () => {
    assert.deepEqual(tokenClaims(jwt({ uid: 'USER_1', exp: 2000000000 })), {
      uid: 'USER_1',
      exp: 2000000000,
    });
  });

  test('an API key acts as the user the core named, and never expires', () => {
    assert.deepEqual(tokenClaims('lsk_abc_def', 'USER_2'), {
      uid: 'USER_2',
      exp: Infinity,
    });
  });

  test('an API key without its user is refused', () => {
    assert.equal(tokenClaims('lsk_abc_def'), 'invalidUid');
  });

  test('malformed tokens say why', () => {
    assert.equal(tokenClaims('not a token'), 'invalid');
    assert.equal(tokenClaims(jwt({ uid: 'USER_1' })), 'invalidExp');
    assert.equal(tokenClaims(jwt({ exp: 1 })), 'invalidUid');
  });
});
