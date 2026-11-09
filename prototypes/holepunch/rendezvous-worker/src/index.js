// The introduction service: passes candidate addresses between a host and the friends
// who open its invite, and nothing else. No game traffic, no keys: an invite's
// certificate fingerprint and token stay after the "#", which is never sent here.
//
// - POST   /v1/sessions                          {"candidates": [...]} -> {"code", "secret"}
// - POST   /v1/sessions/<code>/joins             {"candidates": [...]} -> {"host": [...]}
// - GET    /v1/sessions/<code>/joins?secret=&after=  -> {"joins": [{"id", "candidates"}]}
//          (waits up to 25 s for a join; polling keeps the invite alive)
// - DELETE /v1/sessions/<code>?secret=
//
// An invite no host has polled for two minutes is deleted (an alarm).

import { DurableObject } from 'cloudflare:workers';

const CODE_ALPHABET = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
const CODE_LEN = 10;
const MAX_JOINS = 16;
const MAX_CANDIDATES = 8;
const MAX_BODY = 16 * 1024;
const IDLE_MS = 120_000;
const LONG_POLL_MS = 25_000;
// An IPv4 address or a bracketed IPv6 one, and a port.
const ADDRESS = /^(\d{1,3}(\.\d{1,3}){3}|\[[0-9a-fA-F:.]{2,45}\]):\d{1,5}$/;

function json(status, value) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { 'content-type': 'application/json', 'cache-control': 'no-store' },
  });
}

const error = (status, message) => json(status, { error: message });

function random(alphabet, length) {
  const bytes = crypto.getRandomValues(new Uint8Array(length));
  return Array.from(bytes, (b) => alphabet[b % alphabet.length]).join('');
}

async function candidates(request) {
  const length = Number(request.headers.get('content-length') || 0);
  if (length > MAX_BODY) return null;
  let body;
  try {
    body = JSON.parse(await request.text());
  } catch {
    return null;
  }
  const list = body?.candidates;
  if (!Array.isArray(list) || list.length < 1 || list.length > MAX_CANDIDATES) return null;
  return list.every((a) => typeof a === 'string' && a.length <= 64 && ADDRESS.test(a)) ? list : null;
}

function sameSecret(given, secret) {
  if (typeof given !== 'string' || given.length !== secret.length) return false;
  const encoder = new TextEncoder();
  return crypto.subtle.timingSafeEqual(encoder.encode(given), encoder.encode(secret));
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const parts = url.pathname.split('/').filter(Boolean);
    if (parts[0] !== 'v1' || parts[1] !== 'sessions') return error(404, 'unknown request');

    if (parts.length === 2 && request.method === 'POST') {
      const host = await candidates(request);
      if (!host) return error(400, 'send {"candidates": [...]}, 1 to 8 addresses');
      const code = random(CODE_ALPHABET, CODE_LEN);
      const secret = random('0123456789abcdef', 32);
      await env.SESSIONS.get(env.SESSIONS.idFromName(code)).create(host, secret);
      return json(201, { code, secret });
    }

    const code = parts[2];
    if (!code || code.length !== CODE_LEN || ![...code].every((c) => CODE_ALPHABET.includes(c))) {
      return error(404, 'no such invite (it may have ended)');
    }
    const session = env.SESSIONS.get(env.SESSIONS.idFromName(code));
    const secret = url.searchParams.get('secret');

    if (parts.length === 4 && parts[3] === 'joins' && request.method === 'POST') {
      const join = await candidates(request);
      if (!join) return error(400, 'send {"candidates": [...]}, 1 to 8 addresses');
      const result = await session.join(join);
      return result.error ? error(result.status, result.error) : json(200, { host: result.host });
    }
    if (parts.length === 4 && parts[3] === 'joins' && request.method === 'GET') {
      const after = Math.max(0, Number(url.searchParams.get('after')) || 0);
      const result = await session.wait(secret, after);
      return result.error ? error(result.status, result.error) : json(200, { joins: result.joins });
    }
    if (parts.length === 3 && request.method === 'DELETE') {
      const result = await session.end(secret);
      return result.error ? error(result.status, result.error) : json(200, { ok: true });
    }
    return error(404, 'unknown request');
  },
};

export class Session extends DurableObject {
  constructor(ctx, env) {
    super(ctx, env);
    // Hosts waiting for a join; woken when one arrives.
    this.waiters = [];
  }

  async create(host, secret) {
    await this.ctx.storage.put('session', { host, secret, joins: [] });
    await this.ctx.storage.setAlarm(Date.now() + IDLE_MS);
  }

  async join(candidates) {
    const session = await this.ctx.storage.get('session');
    if (!session) return { status: 404, error: 'no such invite (it may have ended)' };
    if (session.joins.length >= MAX_JOINS) return { status: 429, error: 'this invite has had too many joins' };
    session.joins.push(candidates);
    await this.ctx.storage.put('session', session);
    const waiters = this.waiters;
    this.waiters = [];
    waiters.forEach((wake) => wake());
    return { host: session.host };
  }

  async wait(secret, after) {
    let session = await this.ctx.storage.get('session');
    if (!session) return { status: 404, error: 'no such session' };
    if (!sameSecret(secret, session.secret)) return { status: 403, error: 'wrong secret' };
    await this.ctx.storage.setAlarm(Date.now() + IDLE_MS);
    if (session.joins.length <= after) {
      await new Promise((resolve) => {
        this.waiters.push(resolve);
        setTimeout(resolve, LONG_POLL_MS);
      });
      session = (await this.ctx.storage.get('session')) ?? session;
    }
    const joins = session.joins.slice(after).map((candidates, i) => ({ id: after + i, candidates }));
    return { joins };
  }

  async end(secret) {
    const session = await this.ctx.storage.get('session');
    if (!session) return { status: 404, error: 'no such session' };
    if (!sameSecret(secret, session.secret)) return { status: 403, error: 'wrong secret' };
    await this.ctx.storage.deleteAll();
    return {};
  }

  async alarm() {
    await this.ctx.storage.deleteAll();
  }
}
