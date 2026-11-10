// Invites end when their host goes quiet. With the idle time cut from two minutes to
// two seconds, so the tests are quick.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { useServer, Socket, sleep } from './server.js';

const s = useServer(['--var', 'IDLE_MS:2000']);

test('an invite nobody waits on ends', async () => {
  const { code } = await s.create();
  assert.equal((await s.join(code)).status, 200);
  await sleep(3500);
  assert.equal((await s.join(code)).status, 404);
});

test('a host that pings keeps its invite; one that stops loses it', async () => {
  const { code, secret } = await s.create();
  const host = s.connect(code, secret);
  await host.nextJson();
  for (let i = 0; i < 6; i++) {
    await sleep(700);
    host.ws.send('ping');
    assert.equal(await host.next(), 'pong');
  }
  // Four seconds in, past two idle times: still there.
  assert.equal((await s.join(code)).status, 200);
  assert.equal((await host.nextJson()).joins[0].id, 0);

  // Silent from here: the socket is dropped and the invite ends.
  const closed = await Promise.race([host.closed, sleep(8000).then(() => null)]);
  assert.ok(closed, 'the quiet socket is closed');
  assert.ok([1001, 4000].includes(closed.code), `closed with ${closed.code}`);
  await sleep(500);
  assert.equal((await s.join(code)).status, 404);
});

test('listing joins counts as the host being there', async () => {
  const { code, secret } = await s.create();
  for (let i = 0; i < 4; i++) {
    await sleep(900);
    assert.equal((await s.get(`/v1/invites/${code}/joins`, secret)).status, 200);
  }
  assert.equal((await s.join(code)).status, 200);
});

test('a kept invite whose host went quiet is offline until its host opens it again', async () => {
  const created = await s.post('/v1/invites', { candidates: ['203.0.113.5:4000'], keep: true });
  const { code, secret } = await created.json();
  await sleep(3500);
  const offline = await s.join(code);
  assert.equal(offline.status, 404);
  assert.equal((await offline.json()).error, 'host_offline');
  // its host's socket finds it ended, and renews it
  const late = s.connect(code, secret);
  await assert.rejects(late.opened);
  const reopened = await fetch(`${s.base}/v1/invites/${code}`, {
    method: 'PUT',
    headers: { 'content-type': 'application/json', authorization: `Bearer ${secret}` },
    body: JSON.stringify({ candidates: ['203.0.113.6:4000'] }),
  });
  assert.equal(reopened.status, 200);
  assert.equal((await reopened.json()).code, code);
  const joined = await s.join(code);
  assert.equal(joined.status, 200);
  assert.deepEqual(await joined.json(), { host: ['203.0.113.6:4000'] });
});

test('a meeting session stops publishing when its core goes quiet, and then ends', async () => {
  const lookup = 'ab'.repeat(16);
  const response = await fetch(`${s.base}/v2/meet`, {
    method: 'PUT',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ records: [{ lookup, sealed: 'cd'.repeat(20) }] }),
  });
  assert.equal(response.status, 201);
  const { session, secret } = await response.json();
  const core = new Socket(`${s.ws}/v2/meet/${session}/knocks`, secret);
  await core.opened;
  const presence = async () => (await fetch(`${s.base}/v2/lookups/${lookup}`, { method: 'HEAD', headers: { 'x-lodestone-client': '1' } })).status;
  for (let i = 0; i < 4; i++) {
    await sleep(700);
    core.ws.send('ping');
    assert.equal(await core.next(), 'pong');
    assert.equal(await presence(), 200);
  }
  // Silent from here: no longer live within the idle time, then closed and ended.
  await sleep(2100);
  assert.equal(await presence(), 404);
  const closed = await Promise.race([core.closed, sleep(8000).then(() => null)]);
  assert.ok(closed, 'the quiet socket is closed');
  assert.ok([1001, 4000].includes(closed.code), `closed with ${closed.code}`);
  await sleep(500);
  const late = new Socket(`${s.ws}/v2/meet/${session}/knocks`, secret);
  await assert.rejects(late.opened);
});

test('a session gets a lookup another session held once that one goes quiet', async () => {
  // as when a core restarts while its old session's socket hasn't been noticed gone
  const lookup = 'ef'.repeat(16);
  const open = async (sealed) => {
    const response = await fetch(`${s.base}/v2/meet`, {
      method: 'PUT',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ records: [{ lookup, sealed }] }),
    });
    assert.equal(response.status, 201);
    const { session, secret } = await response.json();
    const core = new Socket(`${s.ws}/v2/meet/${session}/knocks`, secret);
    await core.opened;
    return core;
  };
  const knock = async () => {
    const response = await fetch(`${s.base}/v2/lookups/${lookup}/knocks`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ sealed: '00'.repeat(16) }),
    });
    return response.status === 200 ? (await response.json()).sealed : null;
  };
  const old = await open('aa'.repeat(20));
  const fresh = await open('bb'.repeat(20));
  assert.equal(await knock(), 'aa'.repeat(20), 'the live holder keeps it');

  // The old session goes quiet; the new one keeps pinging and takes the lookup within
  // an idle time or two, without sending its records again.
  let got = null;
  for (let i = 0; i < 12 && got !== 'bb'.repeat(20); i++) {
    await sleep(700);
    fresh.ws.send('ping');
    while ((await fresh.next()) !== 'pong');
    got = await knock();
  }
  assert.equal(got, 'bb'.repeat(20));
  old.close();
  fresh.close();
});
