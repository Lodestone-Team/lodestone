// Stable links (invites made with "keep": renewed with PUT, kept after they end) and the
// invite page. A server of its own: these tests make invites, and the limit is small.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { useServer } from './server.js';

const s = useServer();

const put = (code, secret, candidates = ['203.0.113.9:4000'], headers = {}) =>
  fetch(`${s.base}/v1/invites/${code}`, {
    method: 'PUT',
    headers: { 'content-type': 'application/json', ...(secret ? { authorization: `Bearer ${secret}` } : {}), ...headers },
    body: JSON.stringify({ candidates }),
  });
const del = (code, secret) => fetch(`${s.base}/v1/invites/${code}`, { method: 'DELETE', headers: { authorization: `Bearer ${secret}` } });

async function keptInvite() {
  const response = await s.post('/v1/invites', { candidates: ['203.0.113.5:4000'], keep: true });
  assert.equal(response.status, 201);
  return response.json();
}

test('a kept invite says until when, a plain one does not', async () => {
  const kept = await keptInvite();
  assert.ok(Date.parse(kept.kept_until) > Date.now() + 29 * 24 * 3600 * 1000);
  const plain = await s.create();
  assert.equal(plain.kept_until, undefined);
});

test('renewing gives a new day, fresh candidates and no joins, and closes the sockets as renewed', async () => {
  const { code, secret } = await keptInvite();
  await s.join(code);
  const host = s.connect(code, secret);
  assert.equal((await host.nextJson()).joins.length, 1);

  const renewed = await put(code, secret, ['203.0.113.9:4000']);
  assert.equal(renewed.status, 200);
  const body = await renewed.json();
  assert.equal(body.code, code);
  assert.equal(body.secret, undefined, 'the secret is never sent back');
  assert.ok(Date.parse(body.expires_at) > Date.now());
  assert.ok(Date.parse(body.kept_until) > Date.now());
  assert.equal((await host.closed).code, 4001);

  const joined = await s.join(code);
  assert.deepEqual(await joined.json(), { host: ['203.0.113.9:4000'] });
  const again = s.connect(code, secret);
  assert.deepEqual(await again.nextJson(), { joins: [{ id: 0, candidates: ['198.51.100.7:5000'] }] });
  again.close();
});

test('renewing checks the secret and the body', async () => {
  const { code, secret } = await keptInvite();
  assert.equal((await put(code, null)).status, 401);
  assert.equal((await put(code, '0'.repeat(32))).status, 403);
  assert.equal((await put(code, secret, ['0.0.0.0:1'])).status, 400);
  assert.equal((await put(code, secret, ['203.0.113.9:4000'], { 'content-type': 'text/plain' })).status, 415);
  assert.equal((await put('ZZZZZZZZZZ', secret)).status, 404);
});

test('a plain invite can be renewed while it lasts, but not opened again once it ended', async () => {
  const { code, secret } = await s.create();
  assert.equal((await put(code, secret)).status, 200);
  assert.equal((await del(code, secret)).status, 200);
  assert.equal((await put(code, secret)).status, 404);
});

test('ending a kept invite for good lets its code go', async () => {
  const { code, secret } = await keptInvite();
  assert.equal((await del(code, secret)).status, 200);
  const join = await s.join(code);
  assert.equal(join.status, 404);
  assert.equal((await join.json()).error, 'not_found');
  assert.equal((await put(code, secret)).status, 404);
});

test('a kept invite takes up to 64 joins a day', async () => {
  const { code, secret } = await keptInvite();
  for (let i = 0; i < 64; i++) {
    assert.equal((await s.join(code, [`198.51.100.${i + 1}:5000`])).status, 200, `join ${i}`);
  }
  assert.equal((await s.join(code)).status, 429);
  const late = s.connect(code, secret, 63);
  assert.equal((await late.nextJson()).joins[0].id, 63);
  late.close();
  assert.equal((await put(code, secret)).status, 200);
  assert.equal((await s.join(code)).status, 200);
});

test('the invite page offers the launcher and keeps the key in the browser', async () => {
  const page = await fetch(`${s.base}/j/ABCDEFGH23`);
  assert.equal(page.status, 200);
  assert.match(page.headers.get('content-type'), /^text\/html/);
  const csp = page.headers.get('content-security-policy');
  assert.match(csp, /default-src 'none'/);
  assert.match(csp, /script-src 'sha256-[A-Za-z0-9+/=]+'/);
  assert.equal(page.headers.get('referrer-policy'), 'no-referrer');
  const html = await page.text();
  assert.match(html, /lodestone:\/\/join\?invite=/);
  assert.match(html, /releases\/download\/launcher-latest\/LodestoneLauncher\.dmg/);
  assert.match(html, /releases\/download\/launcher-latest\/LodestoneLauncherSetup\.exe/);
  // No outside resources.
  assert.doesNotMatch(html, /<(script|link|img)[^>]+(src|href)=/);

  // The script's hash is the one the policy allows.
  const script = html.match(/<script>([\s\S]*)<\/script>/)[1];
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(script));
  const hash = Buffer.from(digest).toString('base64');
  assert.ok(csp.includes(`'sha256-${hash}'`), 'the inline script runs');

  assert.equal((await fetch(`${s.base}/j/abcdefgh23`)).status, 200, 'codes in lower case too');
  assert.equal((await fetch(`${s.base}/j/IIIIIIIIII`)).status, 404);
  assert.equal((await fetch(`${s.base}/j/`)).status, 404);
});
