// Invites end when their host goes quiet. With the idle time cut from two minutes to
// two seconds, so the tests are quick.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { useServer, sleep } from './server.js';

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
