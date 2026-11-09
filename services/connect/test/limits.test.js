// The rate limits count only requests a web page elsewhere can't send from a visitor's
// browser. Otherwise any page could use up the visitor's limits (they're per address)
// and stop a host on the same network from sharing, or a friend from joining.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { useServer } from './server.js';

const s = useServer();

/// What a page elsewhere can send without asking first (no CORS preflight): a POST of
/// plain text, or a GET without an Authorization header.
const fromAPage = (path, method = 'POST') =>
  fetch(`${s.base}${path}`, {
    method,
    headers: method === 'POST' ? { 'content-type': 'text/plain' } : {},
    body: method === 'POST' ? JSON.stringify({ candidates: ['203.0.113.5:4000'] }) : undefined,
  });

test("a page elsewhere can't use up the limit on making invites", async () => {
  // the limit is 10 a minute
  for (let i = 0; i < 15; i++) {
    assert.equal((await fromAPage('/v1/invites')).status, 415);
  }
  await s.create();
});

test("a page elsewhere can't use up the limit on joining", async () => {
  const { code } = await s.create();
  // the limit is 1000 a minute here (30 in production)
  for (let i = 0; i < 1010; i++) {
    assert.equal((await fromAPage(`/v1/invites/${code}/joins`)).status, 415);
  }
  assert.equal((await s.join(code)).status, 200);
});

test("a page elsewhere can't use up a host's limit", async () => {
  const { code, secret } = await s.create();
  // the limit is 1000 a minute here (60 in production)
  for (let i = 0; i < 1010; i++) {
    assert.equal((await fromAPage(`/v1/invites/${code}/joins`, 'GET')).status, 401);
  }
  assert.equal((await s.get(`/v1/invites/${code}/joins`, secret)).status, 200);
});
