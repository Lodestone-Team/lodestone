// Slots: the budget for new slots per address. With the hourly budget
// cut from 20 to 3, so the test is quick.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { randomBytes } from 'node:crypto';
import { useServer } from './server.js';

const s = useServer(['--var', 'SLOTS_PER_HOUR:3']);

const hex = (digits) => randomBytes(digits / 2).toString('hex');

function send(method, path, { body, bearer } = {}) {
  const headers = { 'x-lodestone-client': '1' };
  if (bearer) headers.authorization = `Bearer ${bearer}`;
  if (body !== undefined) headers['content-type'] = 'application/json';
  return fetch(`${s.base}${path}`, { method, headers, body: body === undefined ? undefined : JSON.stringify(body) });
}

const read = async (id) => {
  const response = await send('GET', `/v2/slots/${id}`);
  return response.status === 200 ? (await response.json()).sealed : response.status;
};

test('new slots are budgeted per address; rewriting a slot is not', async () => {
  const slots = [0, 1, 2].map(() => ({ id: hex(32), proof: hex(64) }));
  for (const { id, proof } of slots) {
    assert.equal((await send('PUT', `/v2/slots/${id}`, { bearer: proof, body: { sealed: hex(100) } })).status, 200);
  }
  const fourth = await send('PUT', `/v2/slots/${hex(32)}`, { bearer: hex(64), body: { sealed: hex(100) } });
  assert.equal(fourth.status, 429);
  assert.equal((await fourth.json()).error, 'rate_limited');

  // The slots made can still be rewritten, as often as a launcher needs.
  for (let i = 0; i < 5; i++) {
    const sealed = hex(100);
    assert.equal((await send('PUT', `/v2/slots/${slots[0].id}`, { bearer: slots[0].proof, body: { sealed } })).status, 200);
    assert.equal(await read(slots[0].id), sealed);
  }
  // A wrong proof on an existing slot is refused as before, not charged.
  assert.equal((await send('PUT', `/v2/slots/${slots[1].id}`, { bearer: hex(64), body: { sealed: hex(10) } })).status, 403);
});
