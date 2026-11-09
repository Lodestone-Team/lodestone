// The service end to end, against `wrangler dev --local`. Run with `npm test`.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { useServer, sleep } from './server.js';

const s = useServer();

test('health', async () => {
  const response = await s.get('/v1/health');
  assert.equal(response.status, 200);
  assert.deepEqual(await response.json(), { ok: true, version: '1' });
});

test('a waiting host hears about a join at once, and the friend gets the host', async () => {
  const { code, secret, expires_at } = await s.create(['203.0.113.5:4000', '[2001:db8::1]:4000']);
  assert.match(code, /^[0-9A-HJKMNP-TV-Z]{10}$/);
  assert.match(secret, /^[0-9a-f]{32}$/);
  assert.ok(Date.parse(expires_at) > Date.now());

  const host = s.connect(code, secret);
  await host.opened;
  assert.deepEqual(await host.nextJson(), { joins: [] });

  const joined = await s.join(code, ['198.51.100.7:5000', '10.0.0.8:5000']);
  assert.equal(joined.status, 200);
  assert.deepEqual(await joined.json(), { host: ['203.0.113.5:4000', '[2001:db8::1]:4000'] });
  assert.deepEqual(await host.nextJson(1000), { joins: [{ id: 0, candidates: ['198.51.100.7:5000', '10.0.0.8:5000'] }] });

  // The keep-alive.
  host.ws.send('ping');
  assert.equal(await host.next(), 'pong');
  host.close();

  // Reconnecting after the first join gets only what came after it; the plain GET lists.
  await s.join(code, ['198.51.100.8:5000']);
  const again = s.connect(code, secret, 1);
  assert.deepEqual(await again.nextJson(), { joins: [{ id: 1, candidates: ['198.51.100.8:5000'] }] });
  again.close();
  const listed = await (await s.get(`/v1/invites/${code}/joins?after=0`, secret)).json();
  assert.equal(listed.joins.length, 2);
});

test('the secret is checked, and only in the Authorization header', async () => {
  const { code, secret } = await s.create();
  assert.equal((await s.get(`/v1/invites/${code}/joins`)).status, 401);
  assert.equal((await s.get(`/v1/invites/${code}/joins?secret=${secret}`)).status, 401);
  const wrong = await s.get(`/v1/invites/${code}/joins`, '0'.repeat(32));
  assert.equal(wrong.status, 403);
  assert.equal((await wrong.json()).error, 'forbidden');

  // The WebSocket too: refused before it opens.
  for (const bad of [null, '0'.repeat(32)]) {
    const socket = s.connect(code, bad);
    await assert.rejects(socket.opened);
  }
  const ended = await fetch(`${s.base}/v1/invites/${code}`, { method: 'DELETE', headers: { authorization: `Bearer ${'0'.repeat(32)}` } });
  assert.equal(ended.status, 403);
});

test('unknown and malformed invites are not found', async () => {
  for (const code of ['ZZZZZZZZZZ', 'short', 'IIIIIIIIII', '../../x']) {
    const response = await s.join(encodeURIComponent(code));
    assert.equal(response.status, 404, code);
  }
});

test('candidates are checked', async () => {
  const bad = [
    {},
    { candidates: [] },
    { candidates: 'x' },
    { candidates: Array.from({ length: 9 }, (_, i) => `203.0.113.${i + 1}:4000`) },
    { candidates: ['0.0.0.0:4000'] },
    { candidates: ['224.0.0.1:4000'] },
    { candidates: ['255.255.255.255:4000'] },
    { candidates: ['203.0.113.5:0'] },
    { candidates: ['203.0.113.5:04000'] },
    { candidates: ['203.0.113.5:70000'] },
    { candidates: ['203.0.113.5'] },
    { candidates: ['300.0.0.1:4000'] },
    { candidates: ['010.0.0.1:4000'] },
    { candidates: ['1.2.3:4000'] },
    { candidates: ['[::]:4000'] },
    { candidates: ['[0:0:0:0:0:0:0:0]:4000'] },
    { candidates: ['[ff02::1]:4000'] },
    { candidates: ['[1::2::3]:4000'] },
    { candidates: ['[1:2:3:4:5:6:7::8]:4000'] },
    { candidates: ['[1:2:3:4:5:6:7]:4000'] },
    { candidates: ['[::ffff:7f00:1]:4000'] },
    { candidates: ['[2001:db8::1]'] },
    { candidates: ['[2001:db8::1%eth0]:4000'] },
    { candidates: ['example.com:4000'] },
    { candidates: [42] },
  ];
  // Joins rather than creates: the create limit is small.
  const { code } = await s.create();
  for (const body of bad) {
    const response = await s.post(`/v1/invites/${code}/joins`, body);
    assert.equal(response.status, 400, JSON.stringify(body));
    assert.equal((await response.json()).error, 'bad_candidates');
  }
  assert.equal((await s.post(`/v1/invites/${code}/joins`, 'not json')).status, 400);

  const good = ['10.0.0.8:5000', '[2001:db8::1]:65535', '[fe80::1]:4000', '[2001:db8:0:0:0:0:0:1]:1', '127.0.0.1:25565'];
  const response = await s.join(code, good);
  assert.equal(response.status, 200, 'valid addresses (and loopback, locally) are taken');
});

test('bodies are JSON and small', async () => {
  const { code } = await s.create();
  const plain = await s.post(`/v1/invites/${code}/joins`, { candidates: ['198.51.100.7:5000'] }, { 'content-type': 'text/plain' });
  assert.equal(plain.status, 415);
  assert.equal((await s.post(`/v1/invites/${code}/joins`, 'x'.repeat(5000))).status, 400);

  // Without a Content-Length, the body is cut off rather than read whole.
  let sent = 0;
  const body = new ReadableStream({
    pull(controller) {
      if (sent >= 64) return controller.close();
      sent++;
      controller.enqueue(new Uint8Array(1024).fill(32));
    },
  });
  const streamed = await fetch(`${s.base}/v1/invites/${code}/joins`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body,
    duplex: 'half',
  });
  assert.equal(streamed.status, 400);
});

test('an invite takes at most 16 joins', async () => {
  const { code } = await s.create();
  for (let i = 0; i < 16; i++) {
    const response = await s.join(code, [`198.51.100.${i + 1}:5000`]);
    assert.equal(response.status, 200, `join ${i}`);
  }
  const full = await s.join(code, ['198.51.100.99:5000']);
  assert.equal(full.status, 429);
  assert.equal((await full.json()).error, 'invite_full');
});

test('the host can end an invite, which closes its sockets', async () => {
  const { code, secret } = await s.create();
  const host = s.connect(code, secret);
  await host.nextJson();
  const ended = await fetch(`${s.base}/v1/invites/${code}`, { method: 'DELETE', headers: { authorization: `Bearer ${secret}` } });
  assert.equal(ended.status, 200);
  assert.equal((await host.closed).code, 4000);
  assert.equal((await s.join(code)).status, 404);
  assert.equal((await s.get(`/v1/invites/${code}/joins`, secret)).status, 404);
  const late = s.connect(code, secret);
  await assert.rejects(late.opened);
});

test('a host has at most 4 sockets on an invite: a fifth replaces the stalest', async () => {
  const { code, secret } = await s.create();
  const sockets = [];
  for (let i = 0; i < 4; i++) {
    const socket = s.connect(code, secret);
    await socket.nextJson();
    sockets.push(socket);
    await sleep(20);
  }
  const fifth = s.connect(code, secret);
  await fifth.nextJson();
  assert.equal((await sockets[0].closed).reason, 'replaced');
  await s.join(code);
  for (const socket of [...sockets.slice(1), fifth]) assert.equal((await socket.nextJson()).joins[0].id, 0);
  for (const socket of [...sockets.slice(1), fifth]) socket.close();
});

test('a socket that sends anything but "ping" is closed', async () => {
  const { code, secret } = await s.create();
  const host = s.connect(code, secret);
  await host.nextJson();
  host.ws.send('hello');
  assert.equal((await host.closed).code, 1003);
});

test('making invites is rate limited per address', async () => {
  // 10 a minute; the tests above made 8.
  let limited = false;
  for (let i = 0; i < 12 && !limited; i++) {
    const response = await s.post('/v1/invites', { candidates: ['203.0.113.5:4000'] });
    if (response.status === 429) {
      limited = true;
      assert.equal((await response.json()).error, 'rate_limited');
    } else {
      assert.equal(response.status, 201);
    }
  }
  assert.ok(limited, 'the 11th invite in a minute is refused');
});
