// v2, the friend list: pairing mailboxes, slots, meeting sessions and knocks, and the
// friend link page, against `wrangler dev --local`. Run with `npm test`.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash, randomBytes } from 'node:crypto';
import { useServer, Socket, sleep } from './server.js';

const s = useServer();

const hex = (digits) => randomBytes(digits / 2).toString('hex');
/// What the service keeps of an access or write proof: SHA-256 of its hex text.
const sha = (text) => createHash('sha256').update(text, 'utf8').digest('hex');
const CROCKFORD = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
const newCode = () => Array.from(randomBytes(10), (b) => CROCKFORD[b % 32]).join('');

function send(method, path, { body, bearer, type = 'application/json' } = {}) {
  // as Lodestone's own clients send it
  const headers = { 'x-lodestone-client': '1' };
  if (bearer) headers.authorization = `Bearer ${bearer}`;
  if (body !== undefined) headers['content-type'] = type;
  return fetch(`${s.base}${path}`, { method, headers, body: body === undefined ? undefined : typeof body === 'string' ? body : JSON.stringify(body) });
}

async function expectError(response, status, code) {
  assert.equal(response.status, status);
  if (code) assert.equal((await response.json()).error, code);
}

// ---------------------------------------------------------------------------
// Pairing mailboxes

async function newPair(card = hex(400)) {
  const code = newCode();
  const access = hex(64);
  const response = await send('PUT', `/v2/pairs/${code}`, { body: { access: sha(access), card } });
  assert.equal(response.status, 201);
  const { secret, expires_at } = await response.json();
  return { code, access, card, secret, expires_at };
}

test('a mailbox passes a card one way and up to 4 replies the other', async () => {
  const pair = await newPair();
  assert.match(pair.secret, /^[0-9a-f]{32}$/);
  const left = Date.parse(pair.expires_at) - Date.now();
  assert.ok(left > 23.9 * 3600_000 && left <= 24 * 3600_000, 'lasts 24 hours');

  const card = await send('GET', `/v2/pairs/${pair.code}`, { bearer: pair.access });
  assert.equal(card.status, 200);
  assert.equal(card.headers.get('cache-control'), 'no-store');
  assert.deepEqual(await card.json(), { card: pair.card });

  const replies = [hex(300), hex(310), hex(320), hex(330)];
  for (const [i, reply] of replies.entries()) {
    const posted = await send('POST', `/v2/pairs/${pair.code}/replies`, { bearer: pair.access, body: { reply } });
    assert.equal(posted.status, 200);
    assert.deepEqual(await posted.json(), { count: i + 1 });
  }
  const fifth = await send('POST', `/v2/pairs/${pair.code}/replies`, { bearer: pair.access, body: { reply: hex(300) } });
  await expectError(fifth, 429, 'full');

  const listed = await send('GET', `/v2/pairs/${pair.code}/replies`, { bearer: pair.secret });
  assert.equal(listed.status, 200);
  assert.deepEqual(await listed.json(), { replies });

  await expectError(await send('DELETE', `/v2/pairs/${pair.code}`, { bearer: pair.secret }), 200);
  await expectError(await send('GET', `/v2/pairs/${pair.code}`, { bearer: pair.access }), 404, 'not_found');
  await expectError(await send('GET', `/v2/pairs/${pair.code}/replies`, { bearer: pair.secret }), 404, 'not_found');
});

test('a taken code is refused, and the mailbox is unchanged', async () => {
  const pair = await newPair();
  const again = await send('PUT', `/v2/pairs/${pair.code}`, { body: { access: sha(hex(64)), card: hex(100) } });
  await expectError(again, 409, 'taken');
  const card = await send('GET', `/v2/pairs/${pair.code}`, { bearer: pair.access });
  assert.deepEqual(await card.json(), { card: pair.card });
});

test("a mailbox's access and secret are checked", async () => {
  const pair = await newPair();
  const path = `/v2/pairs/${pair.code}`;
  // No access, the wrong one, or the stored hash itself instead of the access.
  await expectError(await send('GET', path), 401, 'unauthorized');
  await expectError(await send('GET', path, { bearer: hex(64) }), 403, 'forbidden');
  await expectError(await send('GET', path, { bearer: sha(pair.access) }), 403, 'forbidden');
  await expectError(await send('GET', path, { bearer: pair.access.toUpperCase() }), 401);
  await expectError(await send('GET', path, { bearer: pair.secret }), 401, 'unauthorized');
  await expectError(await send('POST', `${path}/replies`, { bearer: hex(64), body: { reply: hex(10) } }), 403, 'forbidden');
  // Only the owner's secret reads replies or ends the mailbox; the access doesn't.
  await expectError(await send('GET', `${path}/replies`, { bearer: pair.access }), 401);
  await expectError(await send('GET', `${path}/replies`, { bearer: hex(32) }), 403, 'forbidden');
  await expectError(await send('DELETE', path, { bearer: hex(32) }), 403, 'forbidden');
  await expectError(await send('DELETE', path, { bearer: pair.access }), 401);
  // Still there.
  assert.equal((await send('GET', path, { bearer: pair.access })).status, 200);
});

test('mailbox bodies and codes are checked', async () => {
  const code = newCode();
  const path = `/v2/pairs/${code}`;
  const access = sha(hex(64));
  const bad = [
    {},
    { access, card: '' },
    { access, card: 'abc' }, // odd length
    { access, card: 'ABCD' }, // upper case
    { access, card: 'zz00' },
    { access, card: 12 },
    { access, card: hex(4098) },
    { access: access.slice(1), card: hex(10) },
    { access: access.toUpperCase(), card: hex(10) },
  ];
  for (const body of bad) await expectError(await send('PUT', path, { body }), 400, 'bad_body');
  await expectError(await send('PUT', path, { body: 'not json' }), 400, 'bad_body');
  await expectError(await send('PUT', path, { body: { access, card: hex(10) }, type: 'text/plain' }), 415, 'not_json');
  await expectError(await send('PUT', path, { body: { access, card: hex(10000) } }), 413, 'too_large');
  // Malformed codes name no mailbox.
  for (const wrong of ['ABCDEFGHJ', 'IIIIIIIIII', 'abcdefghjk']) {
    await expectError(await send('PUT', `/v2/pairs/${wrong}`, { body: { access, card: hex(10) } }), 404);
  }
  // The largest card fits.
  assert.equal((await send('PUT', path, { body: { access, card: hex(4096) } })).status, 201);

  const pair = await newPair();
  const reply = (body) => send('POST', `/v2/pairs/${pair.code}/replies`, { bearer: pair.access, body });
  await expectError(await reply({ reply: hex(4098) }), 400, 'bad_body');
  await expectError(await reply({ reply: 'xyz0' }), 400, 'bad_body');
  await expectError(await reply({}), 400, 'bad_body');
  assert.equal((await reply({ reply: hex(4096) })).status, 200);
});

// ---------------------------------------------------------------------------
// Slots

test('a slot keeps the proof of its first write and checks it after', async () => {
  const id = hex(32);
  const path = `/v2/slots/${id}`;
  const proof = hex(64);
  const other = hex(64);
  await expectError(await send('GET', path), 404, 'not_found');

  const first = hex(1000);
  assert.equal((await send('PUT', path, { bearer: proof, body: { sealed: first } })).status, 200);
  const read = await send('GET', path);
  assert.equal(read.status, 200);
  assert.deepEqual(await read.json(), { sealed: first });

  // Another proof can't overwrite or delete it; nor can the proof's hash.
  await expectError(await send('PUT', path, { bearer: other, body: { sealed: hex(10) } }), 403, 'forbidden');
  await expectError(await send('PUT', path, { bearer: sha(proof), body: { sealed: hex(10) } }), 403, 'forbidden');
  await expectError(await send('DELETE', path, { bearer: other }), 403, 'forbidden');
  await expectError(await send('PUT', path, { body: { sealed: hex(10) } }), 401, 'unauthorized');
  await expectError(await send('DELETE', path), 401, 'unauthorized');
  assert.deepEqual(await (await send('GET', path)).json(), { sealed: first });

  const second = hex(8192);
  assert.equal((await send('PUT', path, { bearer: proof, body: { sealed: second } })).status, 200);
  assert.deepEqual(await (await send('GET', path)).json(), { sealed: second });

  assert.equal((await send('DELETE', path, { bearer: proof })).status, 200);
  await expectError(await send('GET', path), 404, 'not_found');
  await expectError(await send('DELETE', path, { bearer: proof }), 404, 'not_found');
});

test('slot bodies and ids are checked', async () => {
  const path = `/v2/slots/${hex(32)}`;
  const proof = hex(64);
  for (const sealed of ['', 'abc', 'ABCD', 'not hex!', hex(8194), 5]) {
    await expectError(await send('PUT', path, { bearer: proof, body: { sealed } }), 400, 'bad_body');
  }
  await expectError(await send('PUT', path, { bearer: proof, body: { sealed: hex(10) }, type: 'text/plain' }), 415);
  await expectError(await send('PUT', path, { bearer: proof, body: { sealed: hex(20000) } }), 413);
  await expectError(await send('PUT', path, { bearer: proof.slice(2), body: { sealed: hex(10) } }), 401);
  for (const id of [hex(30), hex(32).toUpperCase(), 'x'.repeat(32)]) {
    await expectError(await send('GET', `/v2/slots/${id}`), 404);
  }
  // Nothing was written by the refused requests: the first good write sets the proof.
  await expectError(await send('GET', path), 404);
  assert.equal((await send('PUT', path, { bearer: proof, body: { sealed: hex(10) } })).status, 200);
});

// ---------------------------------------------------------------------------
// Meeting

const records = (n) => Array.from({ length: n }, () => ({ lookup: hex(32), sealed: hex(200) }));

async function newSession(list) {
  const response = await send('PUT', '/v2/meet', { body: { records: list } });
  assert.equal(response.status, 201);
  return response.json();
}

const waitOn = (session, secret) => new Socket(`${s.ws}/v2/meet/${session}/knocks`, secret);
const knock = (lookup, sealed = hex(160)) => send('POST', `/v2/lookups/${lookup}/knocks`, { body: { sealed } });
const presence = async (lookup) =>
  (await fetch(`${s.base}/v2/lookups/${lookup}`, { method: 'HEAD', headers: { 'x-lodestone-client': '1' } })).status;
const endSession = (session, secret) => send('DELETE', `/v2/meet/${session}`, { bearer: secret });

test('a knock reaches the core with the address it came from, and gets the record', async () => {
  const list = records(3);
  const { session, secret, expires_at } = await newSession(list);
  assert.match(session, /^[0-9a-f]{32}$/);
  assert.match(secret, /^[0-9a-f]{32}$/);
  assert.ok(Date.parse(expires_at) > Date.now() + 23.9 * 3600_000);

  // Not publishing until the core waits on its socket.
  assert.equal(await presence(list[0].lookup), 404);
  await expectError(await knock(list[0].lookup), 404, 'not_found');

  const core = waitOn(session, secret);
  await core.opened;
  assert.equal(await presence(list[0].lookup), 200);
  assert.equal(await presence(list[2].lookup), 200);
  assert.equal(await presence(hex(32)), 404);

  const sealed = hex(160);
  const knocked = await knock(list[1].lookup, sealed);
  assert.equal(knocked.status, 200);
  assert.deepEqual(await knocked.json(), { sealed: list[1].sealed });
  const message = await core.nextJson();
  assert.equal(message.knocks.length, 1);
  const [heard] = message.knocks;
  assert.equal(heard.lookup, list[1].lookup);
  assert.equal(heard.sealed, sealed);
  assert.equal(typeof heard.seen, 'string');
  assert.match(heard.seen, /^[0-9a-f.:]+$/i, 'an IP address');

  core.ws.send('ping');
  assert.equal(await core.next(), 'pong');

  await expectError(await knock(hex(32)), 404, 'not_found');

  // Ending the session closes the socket and its lookups stop.
  await expectError(await endSession(session, hex(32)), 403, 'forbidden');
  assert.equal((await endSession(session, secret)).status, 200);
  assert.equal((await core.closed).code, 4000);
  assert.equal(await presence(list[0].lookup), 404);
  await expectError(await knock(list[0].lookup), 404, 'not_found');
  await expectError(await endSession(session, secret), 404, 'not_found');
});

test("a session's secret is checked", async () => {
  const list = records(1);
  const { session, secret } = await newSession(list);
  const path = `/v2/meet/${session}`;
  await expectError(await send('PUT', path, { body: { records: list } }), 401, 'unauthorized');
  await expectError(await send('PUT', path, { bearer: hex(32), body: { records: list } }), 403, 'forbidden');
  await expectError(await send('DELETE', path), 401, 'unauthorized');
  for (const bad of [null, hex(32)]) {
    const socket = waitOn(session, bad);
    await assert.rejects(socket.opened);
  }
  await expectError(await send('GET', `${path}/knocks`, { bearer: secret }), 426, 'websocket');
  await expectError(await send('PUT', `/v2/meet/${hex(32)}`, { bearer: secret, body: { records: list } }), 404, 'not_found');
  assert.equal((await endSession(session, secret)).status, 200);
});

test('replacing the records registers new lookups and unregisters dropped ones', async () => {
  const [a, b, c] = records(3);
  const { session, secret } = await newSession([a, b]);
  const core = waitOn(session, secret);
  await core.opened;
  assert.equal(await presence(a.lookup), 200);
  assert.equal(await presence(c.lookup), 404);

  const newB = { lookup: b.lookup, sealed: hex(200) };
  const replaced = await send('PUT', `/v2/meet/${session}`, { bearer: secret, body: { records: [newB, c] } });
  assert.equal(replaced.status, 200);
  assert.ok(Date.parse((await replaced.json()).expires_at) > Date.now());

  assert.equal(await presence(a.lookup), 404);
  await expectError(await knock(a.lookup), 404, 'not_found');
  assert.equal(await presence(c.lookup), 200);
  assert.deepEqual(await (await knock(c.lookup)).json(), { sealed: c.sealed });
  assert.equal((await core.nextJson()).knocks[0].lookup, c.lookup);
  assert.deepEqual(await (await knock(b.lookup)).json(), { sealed: newB.sealed });
  assert.equal((await core.nextJson()).knocks[0].lookup, b.lookup);

  // Empty records: nothing published, the session lives on.
  assert.equal((await send('PUT', `/v2/meet/${session}`, { bearer: secret, body: { records: [] } })).status, 200);
  assert.equal(await presence(b.lookup), 404);
  assert.equal(await presence(c.lookup), 404);
  assert.equal((await endSession(session, secret)).status, 200);
});

test('a live session keeps its lookups; another session takes them once it has gone', async () => {
  const shared = hex(32);
  const old = await newSession([{ lookup: shared, sealed: 'aa'.repeat(50) }]);
  const oldCore = waitOn(old.session, old.secret);
  await oldCore.opened;

  // Registering the same lookup id elsewhere doesn't take it from a live session.
  const fresh = await newSession([{ lookup: shared, sealed: 'bb'.repeat(50) }]);
  const freshCore = waitOn(fresh.session, fresh.secret);
  await freshCore.opened;
  assert.deepEqual(await (await knock(shared)).json(), { sealed: 'aa'.repeat(50) });
  assert.equal((await oldCore.nextJson()).knocks[0].lookup, shared);
  await assert.rejects(freshCore.next(500), 'the other session hears nothing');
  // Nor does resending the records, while the holder is live.
  const resend = () => send('PUT', `/v2/meet/${fresh.session}`, { bearer: fresh.secret, body: { records: [{ lookup: shared, sealed: 'bb'.repeat(50) }] } });
  assert.equal((await resend()).status, 200);
  assert.deepEqual(await (await knock(shared)).json(), { sealed: 'aa'.repeat(50) });
  await oldCore.nextJson();

  // Once the holder ends, the other session takes it on its next update.
  assert.equal((await endSession(old.session, old.secret)).status, 200);
  assert.equal(await presence(shared), 404);
  assert.equal((await resend()).status, 200);
  assert.equal(await presence(shared), 200);
  assert.deepEqual(await (await knock(shared)).json(), { sealed: 'bb'.repeat(50) });
  assert.equal((await freshCore.nextJson()).knocks[0].lookup, shared);

  // A session without a live socket doesn't keep its lookups: a new one takes them at once.
  const other = hex(32);
  const idle = await newSession([{ lookup: other, sealed: 'cc'.repeat(50) }]);
  const taker = await newSession([{ lookup: other, sealed: 'dd'.repeat(50) }]);
  const takerCore = waitOn(taker.session, taker.secret);
  await takerCore.opened;
  assert.deepEqual(await (await knock(other)).json(), { sealed: 'dd'.repeat(50) });
  await takerCore.nextJson();
  // The one it was taken from dropping it, or ending, leaves it with the new one.
  assert.equal((await send('PUT', `/v2/meet/${idle.session}`, { bearer: idle.secret, body: { records: [] } })).status, 200);
  assert.equal((await endSession(idle.session, idle.secret)).status, 200);
  assert.equal(await presence(other), 200);

  // Once the holders end, nobody publishes them.
  assert.equal((await endSession(fresh.session, fresh.secret)).status, 200);
  assert.equal((await endSession(taker.session, taker.secret)).status, 200);
  assert.equal(await presence(shared), 404);
  assert.equal(await presence(other), 404);
  await expectError(await knock(shared), 404, 'not_found');
});

test('records and knocks are checked', async () => {
  const one = records(1)[0];
  const bad = [
    {},
    { records: 'x' },
    { records: [one, { ...one }] }, // the same lookup twice
    { records: [{ lookup: one.lookup.toUpperCase(), sealed: one.sealed }] },
    { records: [{ lookup: hex(30), sealed: one.sealed }] },
    { records: [{ lookup: one.lookup, sealed: hex(2050) }] },
    { records: [{ lookup: one.lookup, sealed: 'abc' }] },
    { records: [{ lookup: one.lookup }] },
    { records: Array.from({ length: 257 }, () => ({ lookup: hex(32), sealed: '00' })) },
  ];
  for (const body of bad) await expectError(await send('PUT', '/v2/meet', { body }), 400, 'bad_body');
  await expectError(await send('PUT', '/v2/meet', { body: { records: [one] }, type: 'text/plain' }), 415);

  // The most a session holds: 256 records of 1 KiB.
  const most = Array.from({ length: 256 }, () => ({ lookup: hex(32), sealed: hex(2048) }));
  const { session, secret } = await newSession(most);
  const core = waitOn(session, secret);
  await core.opened;
  assert.equal(await presence(most[255].lookup), 200);
  assert.deepEqual(await (await knock(most[200].lookup)).json(), { sealed: most[200].sealed });
  await core.nextJson();
  // Duplicates are refused on replacing too, and leave the records as they were.
  await expectError(await send('PUT', `/v2/meet/${session}`, { bearer: secret, body: { records: [one, one] } }), 400, 'bad_body');
  assert.equal(await presence(most[0].lookup), 200);

  for (const sealed of ['', 'abc', 'ZZ', hex(2050), 7]) {
    await expectError(await send('POST', `/v2/lookups/${most[0].lookup}/knocks`, { body: { sealed } }), 400, 'bad_body');
  }
  await expectError(await send('POST', `/v2/lookups/${most[0].lookup}/knocks`, { body: { sealed: hex(10) }, type: 'text/plain' }), 415);
  await expectError(await send('POST', `/v2/lookups/${most[0].lookup}/knocks`, { body: { sealed: hex(6000) } }), 413);
  await expectError(await knock('x'.repeat(32)), 404);
  assert.equal(await presence(most[0].lookup.toUpperCase()), 404);
  assert.equal((await knock(most[0].lookup, hex(2048))).status, 200, 'the largest knock fits');
  assert.equal((await endSession(session, secret)).status, 200);
});

test('a core has at most 4 sockets on a session, and knocks reach each', async () => {
  const [record] = records(1);
  const { session, secret } = await newSession([record]);
  const sockets = [];
  for (let i = 0; i < 5; i++) {
    const socket = waitOn(session, secret);
    await socket.opened;
    sockets.push(socket);
    await sleep(20);
  }
  assert.equal((await sockets[0].closed).reason, 'replaced');
  assert.equal((await knock(record.lookup)).status, 200);
  for (const socket of sockets.slice(1)) assert.equal((await socket.nextJson()).knocks[0].lookup, record.lookup);
  assert.equal((await endSession(session, secret)).status, 200);
});

// ---------------------------------------------------------------------------
// The friend link page

test('the friend link page offers the launcher and keeps the secret in the browser', async () => {
  const page = await fetch(`${s.base}/f/ABCDEFGH23`);
  assert.equal(page.status, 200);
  assert.match(page.headers.get('content-type'), /^text\/html/);
  const csp = page.headers.get('content-security-policy');
  assert.match(csp, /default-src 'none'/);
  assert.equal(page.headers.get('referrer-policy'), 'no-referrer');
  const html = await page.text();
  assert.match(html, /You've been invited to be friends on Lodestone/);
  assert.match(html, /lodestone:\/\/friend\?link=/);
  assert.match(html, /Friends › Add friend/);
  assert.match(html, /releases\/download\/launcher-latest\/LodestoneLauncherSetup\.exe/);
  assert.doesNotMatch(html, /<(script|link|img)[^>]+(src|href)=/);

  const script = html.match(/<script>([\s\S]*)<\/script>/)[1];
  assert.ok(script.includes(String.raw`/^[0-9a-f]{32}\.[0-9a-f]{64}$/`), 'checks the fragment');
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(script));
  assert.ok(csp.includes(`'sha256-${Buffer.from(digest).toString('base64')}'`), 'the inline script runs');

  // The script's check accepts a friend link's fragment and nothing else.
  const pattern = new RegExp(script.match(/var ok = \/(.*)\/\.test/)[1]);
  assert.ok(pattern.test(`${hex(32)}.${hex(64)}`));
  assert.ok(!pattern.test(`${hex(64)}.${hex(32)}`));
  assert.ok(!pattern.test(`${hex(32)}.${hex(64)}x`));

  assert.equal((await fetch(`${s.base}/f/abcdefgh23`)).status, 200, 'codes in lower case too');
  assert.equal((await fetch(`${s.base}/f/IIIIIIIIII`)).status, 404);
  // The invite page is still its own.
  assert.match(await (await fetch(`${s.base}/j/ABCDEFGH23`)).text(), /lodestone:\/\/join\?invite=/);
});

test('unauthenticated reads need the client header, so other sites cannot spend them', async () => {
  const id = hex(32);
  assert.equal((await fetch(`${s.base}/v2/slots/${id}`)).status, 400);
  assert.equal((await fetch(`${s.base}/v2/lookups/${hex(32)}`, { method: 'HEAD' })).status, 400);
  assert.equal((await send('GET', `/v2/slots/${id}`)).status, 404, 'with it: just not found');
});
