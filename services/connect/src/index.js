// connect.lodestone.cc: introduces a Lodestone host to the friends who open its invite,
// so they can connect to each other directly (UDP hole punching). It passes candidate
// addresses along and nothing else: no game traffic, and no keys. An invite link's
// certificate fingerprint and token come after its "#", which is never sent here.
//
// The protocol (JSON over HTTPS; README.md has the details):
//
//   POST   /v1/invites                    {"candidates": [...]}  -> 201 {"code", "secret", "expires_at"}
//   POST   /v1/invites/<code>/joins       {"candidates": [...]}  -> 200 {"host": [...]}
//   GET    /v1/invites/<code>/joins?after=<n>  (Authorization: Bearer <secret>)
//            as a WebSocket: {"joins": [...]} now, then one message per join as it comes
//            plain:          200 {"joins": [{"id", "candidates"}]}, at once
//   PUT    /v1/invites/<code>             (Authorization: Bearer <secret>) {"candidates": [...]}
//            -> 200 {"code", "expires_at", "kept_until"?}: renews the invite, or opens a kept one again
//   DELETE /v1/invites/<code>             (Authorization: Bearer <secret>) -> 200 {"ok": true}
//   GET    /v1/health                     -> 200 {"ok": true, "version"}
//   GET    /j/<code>                      -> the invite page people open (HTML)
//
// Errors are {"error": "<code>", "message": "<text>"} with a fitting status.
//
// Each invite is one Durable Object. Hosts wait for joins on a WebSocket, which the
// object holds while hibernating: an invite costs nothing while nobody joins it (a
// held-open request would keep the object awake, and billed, the whole time). The host
// sends "ping" every 30 s, answered by Cloudflare without waking the object. An invite
// ends when its host has gone quiet for two minutes, when the host deletes it, or after
// 24 hours, whichever comes first.
//
// Stable links: an invite made with {"keep": true} keeps its code for its host for 30
// days after it was last open. The host renews it (PUT) with its secret, before or after
// it ends, and gets the same code: a link shared once keeps working whenever the host
// is sharing. Anyone else gets "host_offline" meanwhile, and a new invite never takes a
// kept code.

import { DurableObject } from 'cloudflare:workers';

const VERSION = '1';
const CODE_ALPHABET = '0123456789ABCDEFGHJKMNPQRSTVWXYZ'; // Crockford base32: no I, L, O, U
const CODE_LEN = 10; // 50 bits
const SECRET_HEX_LEN = 32; // 128 bits
const MAX_JOINS = 16;
// A kept invite takes more joins a day: the same friends come back to it.
const MAX_JOINS_KEPT = 64;
const MAX_CANDIDATES = 8;
const MAX_BODY = 4 * 1024;
const MAX_SOCKETS = 4;
const IDLE_MS = 120_000;
const MAX_AGE_MS = 24 * 60 * 60 * 1000;
// How long a kept code stays its host's after the invite was last open.
const KEEP_MS = 30 * 24 * 60 * 60 * 1000;
// WebSocket close codes: "the invite has ended", and "the invite was renewed" (wait on
// it again from join 0).
const CLOSE_ENDED = 4000;
const CLOSE_RENEWED = 4001;

// ---------------------------------------------------------------------------
// Responses

function json(status, value) {
  return new Response(JSON.stringify(value), {
    status,
    headers: {
      'content-type': 'application/json',
      'cache-control': 'no-store',
      'x-content-type-options': 'nosniff',
    },
  });
}

function error(status, code, message) {
  return json(status, { error: code, message });
}

const notFound = () => error(404, 'not_found', 'No such invite. It may have ended.');
const OFFLINE_MESSAGE = "The host isn't sharing this server right now. Try again when they're online.";
const badCandidates = () => error(400, 'bad_candidates', 'Send {"candidates": [...]}: 1 to 8 reachable addresses.');
const unauthorized = () => error(401, 'unauthorized', 'Send the invite secret as "Authorization: Bearer <secret>".');

// ---------------------------------------------------------------------------
// Validation

const PORT = /^[1-9]\d{0,4}$/;
const OCTET = /^(0|[1-9]\d{0,2})$/;

function validPort(text) {
  return PORT.test(text) && Number(text) <= 65535;
}

/// An address another computer could send to: IPv4 or bracketed IPv6, with a port, each
/// written one way only (no leading zeros). Unspecified, multicast and broadcast
/// addresses and port 0 are refused; loopback only where `allowLoopback` (local
/// development).
export function validAddress(text, allowLoopback) {
  if (typeof text !== 'string' || text.length > 64) return false;
  if (text.startsWith('[')) {
    const close = text.indexOf(']:');
    if (close < 0) return false;
    return validIpv6(text.slice(1, close).toLowerCase(), allowLoopback) && validPort(text.slice(close + 2));
  }
  const colon = text.indexOf(':');
  if (colon < 0) return false;
  const octets = text.slice(0, colon).split('.');
  if (octets.length !== 4 || !octets.every((o) => OCTET.test(o) && Number(o) <= 255)) return false;
  const a = Number(octets[0]);
  if (a === 0 || a >= 224) return false; // unspecified, multicast, reserved, broadcast
  if (a === 127 && !allowLoopback) return false;
  return validPort(text.slice(colon + 1));
}

function validIpv6(ip, allowLoopback) {
  const halves = ip.split('::');
  if (halves.length > 2) return false;
  const groups = (part) => (part === '' ? [] : part.split(':'));
  const head = groups(halves[0]);
  const tail = halves.length === 2 ? groups(halves[1]) : [];
  const all = [...head, ...tail];
  if (!all.every((g) => /^[0-9a-f]{1,4}$/.test(g))) return false;
  if (halves.length === 2 ? all.length > 7 : all.length !== 8) return false;
  const filler = halves.length === 2 ? Array(8 - all.length).fill('0') : [];
  const value = [...head, ...filler, ...tail].map((g) => parseInt(g, 16));
  const zeros = (from, to) => value.slice(from, to).every((v) => v === 0);
  if (zeros(0, 8)) return false; // unspecified
  if (value[0] >= 0xff00) return false; // multicast
  if (zeros(0, 5) && value[5] === 0xffff) return false; // IPv4-mapped: send the IPv4 address
  if (zeros(0, 7) && value[7] === 1 && !allowLoopback) return false;
  return true;
}

/// The body as text, refusing more than MAX_BODY bytes without reading them all.
async function readBody(request) {
  if (Number(request.headers.get('content-length') || 0) > MAX_BODY) return null;
  if (!request.body) return '';
  const reader = request.body.getReader();
  const chunks = [];
  let size = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    size += value.byteLength;
    if (size > MAX_BODY) {
      await reader.cancel();
      return null;
    }
    chunks.push(value);
  }
  const bytes = new Uint8Array(size);
  let at = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, at);
    at += chunk.byteLength;
  }
  return new TextDecoder().decode(bytes);
}

/// The body's candidates (and whether it asks to keep the code), or null.
async function readCandidates(request, allowLoopback) {
  const text = await readBody(request);
  if (text === null) return null;
  let body;
  try {
    body = JSON.parse(text);
  } catch {
    return null;
  }
  const list = body?.candidates;
  if (!Array.isArray(list) || list.length < 1 || list.length > MAX_CANDIDATES) return null;
  if (!list.every((a) => validAddress(a, allowLoopback))) return null;
  return { candidates: [...new Set(list)], keep: body.keep === true };
}

/// Only JSON bodies, so a web page elsewhere can't send these from a visitor's browser
/// without a CORS preflight, which this service never answers.
function isJson(request) {
  const type = (request.headers.get('content-type') || '').split(';')[0].trim().toLowerCase();
  return type === 'application/json';
}

function random(alphabet, length) {
  // 256 is a multiple of both alphabets' lengths (32 and 16), so this is unbiased.
  const bytes = crypto.getRandomValues(new Uint8Array(length));
  return Array.from(bytes, (b) => alphabet[b % alphabet.length]).join('');
}

function validCode(code) {
  return typeof code === 'string' && code.length === CODE_LEN && [...code].every((c) => CODE_ALPHABET.includes(c));
}

function bearer(request) {
  const header = request.headers.get('authorization') || '';
  const match = /^Bearer ([0-9a-f]{32})$/.exec(header);
  return match ? match[1] : null;
}

function sameSecret(given, secret) {
  if (typeof given !== 'string' || typeof secret !== 'string' || given.length !== secret.length) return false;
  const encoder = new TextEncoder();
  return crypto.subtle.timingSafeEqual(encoder.encode(given), encoder.encode(secret));
}

function afterParam(url) {
  return Math.min(MAX_JOINS_KEPT, Math.max(0, Math.floor(Number(url.searchParams.get('after')) || 0)));
}

/// Cloudflare's rate limiter, keyed by the client's address. Missing in some local
/// setups, where everything passes.
async function allowed(limiter, request) {
  if (!limiter) return true;
  const key = request.headers.get('cf-connecting-ip') || 'local';
  const { success } = await limiter.limit({ key });
  return success;
}

const rateLimited = (what) => error(429, 'rate_limited', `Too many ${what} from this address. Wait a minute.`);

// ---------------------------------------------------------------------------
// The invite page: https://connect.lodestone.cc/j/<code>#<fingerprint>.<token>
//
// What people open from a chat app. It is the same for every code: the script reads the
// part after "#" in the browser (it never reaches this service) and offers to open the
// whole link in Lodestone Launcher, as lodestone://join?invite=<link>. It loads nothing
// from anywhere, so the link goes nowhere else either.

const RELEASES = 'https://github.com/Lodestone-Team/lodestone/releases/download/launcher-latest';

const PAGE_SCRIPT = `
(function () {
  var fragment = location.hash.slice(1);
  var ok = /^[0-9a-f]{64}\\.[0-9a-f]{32}$/.test(fragment);
  var link = location.origin + location.pathname + '#' + fragment;
  document.getElementById(ok ? 'invite' : 'broken').hidden = false;
  if (!ok) return;
  document.getElementById('open').href = 'lodestone://join?invite=' + encodeURIComponent(link);
  var copy = document.getElementById('copy');
  copy.addEventListener('click', function () {
    navigator.clipboard.writeText(link).then(function () {
      copy.textContent = 'Copied';
    });
  });
})();
`;

const PAGE = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="referrer" content="no-referrer">
<meta name="robots" content="noindex">
<title>Join a Minecraft server with Lodestone</title>
<meta property="og:title" content="Join my Minecraft server">
<meta property="og:description" content="Open this link in Lodestone Launcher to play together.">
<style>
  :root { color-scheme: light dark; --bg: #f6f6f4; --card: #ffffff; --text: #1b1b1a; --muted: #5d5d58; --accent: #2f6f3e; --line: #deded8; }
  @media (prefers-color-scheme: dark) { :root { --bg: #151514; --card: #1f1f1d; --text: #ececea; --muted: #a3a39c; --accent: #4fae66; --line: #34342f; } }
  * { box-sizing: border-box; }
  body { margin: 0; min-height: 100vh; display: grid; place-items: center; padding: 24px 16px; background: var(--bg); color: var(--text); font: 16px/1.5 system-ui, -apple-system, "Segoe UI", sans-serif; }
  main { width: 100%; max-width: 440px; background: var(--card); border: 1px solid var(--line); border-radius: 16px; padding: 28px 24px; }
  h1 { font-size: 22px; line-height: 1.25; margin: 0 0 8px; }
  p { margin: 0 0 16px; color: var(--muted); }
  .button { display: block; width: 100%; text-align: center; padding: 12px 16px; border-radius: 10px; font-weight: 600; text-decoration: none; border: 1px solid var(--line); background: transparent; color: var(--text); font-size: 15px; cursor: pointer; }
  .primary { background: var(--accent); border-color: var(--accent); color: #fff; margin-bottom: 10px; }
  .downloads { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-bottom: 16px; }
  h2 { font-size: 15px; margin: 24px 0 8px; }
  small { color: var(--muted); }
</style>
</head>
<body>
<main>
  <div id="invite" hidden>
    <h1>You're invited to a Minecraft server</h1>
    <p>A friend is sharing their server with Lodestone. Open the invite in Lodestone Launcher to join them.</p>
    <a id="open" class="button primary" href="#">Open in Lodestone Launcher</a>
    <button id="copy" class="button" type="button">Copy invite link</button>
    <h2>Don't have Lodestone Launcher?</h2>
    <p>Download it, then come back here and open the invite.</p>
    <div class="downloads">
      <a class="button" href="${RELEASES}/LodestoneLauncher.dmg">macOS</a>
      <a class="button" href="${RELEASES}/LodestoneLauncherSetup.exe">Windows</a>
    </div>
    <small>Your game connects straight to your friend's computer. The key in the link stays in your browser: Lodestone's servers never see it.</small>
  </div>
  <div id="broken" hidden>
    <h1>This invite link is cut short</h1>
    <p>Ask your friend to send the whole link again, including the part after the #.</p>
  </div>
  <noscript><p>Copy this page's address and paste it into Lodestone Launcher to join.</p></noscript>
</main>
<script>${PAGE_SCRIPT}</script>
</body>
</html>
`;

let pageHash;

async function invitePage(validInviteCode) {
  if (!validInviteCode) {
    return new Response('No such invite.', { status: 404, headers: { 'content-type': 'text/plain; charset=utf-8', 'x-content-type-options': 'nosniff' } });
  }
  if (!pageHash) {
    const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(PAGE_SCRIPT));
    pageHash = btoa(String.fromCharCode(...new Uint8Array(digest)));
  }
  return new Response(PAGE, {
    status: 200,
    headers: {
      'content-type': 'text/html; charset=utf-8',
      'cache-control': 'public, max-age=3600',
      'content-security-policy': `default-src 'none'; script-src 'sha256-${pageHash}'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`,
      'referrer-policy': 'no-referrer',
      'x-content-type-options': 'nosniff',
    },
  });
}

// ---------------------------------------------------------------------------
// The Worker

export default {
  async fetch(request, env) {
    try {
      return await route(request, env);
    } catch (err) {
      console.error('connect: unexpected error', err?.stack || String(err));
      return error(500, 'internal', 'Something went wrong. Try again.');
    }
  },
};

async function route(request, env) {
  const url = new URL(request.url);
  const parts = url.pathname.split('/').filter(Boolean);
  const allowLoopback = env.ALLOW_LOOPBACK === '1';

  if (parts[0] === 'j' && parts.length === 2 && (request.method === 'GET' || request.method === 'HEAD')) {
    return invitePage(validCode(parts[1].toUpperCase()));
  }
  if (parts[0] !== 'v1') return error(404, 'unknown', 'Unknown request.');

  if (parts.length === 2 && parts[1] === 'health' && request.method === 'GET') {
    return json(200, { ok: true, version: VERSION });
  }
  if (parts[1] !== 'invites') return error(404, 'unknown', 'Unknown request.');

  if (parts.length === 2) {
    if (request.method !== 'POST') return error(405, 'method', 'Use POST.');
    if (!(await allowed(env.CREATE_LIMIT, request))) return rateLimited('invites');
    if (!isJson(request)) return error(415, 'not_json', 'Send JSON (Content-Type: application/json).');
    const body = await readCandidates(request, allowLoopback);
    if (!body) return badCandidates();
    const secret = random('0123456789abcdef', SECRET_HEX_LEN);
    // A 50-bit code is very unlikely to be taken, but never hand out a live or kept one twice.
    for (let attempt = 0; attempt < 3; attempt++) {
      const code = random(CODE_ALPHABET, CODE_LEN);
      const created = await env.INVITES.get(env.INVITES.idFromName(code)).create(body.candidates, secret, body.keep);
      if (created) return json(201, { code, secret, ...dates(created) });
    }
    throw new Error('no free invite code');
  }

  const code = parts[2];
  if (!validCode(code)) return notFound();
  const invite = env.INVITES.get(env.INVITES.idFromName(code));

  if (parts.length === 4 && parts[3] === 'joins' && request.method === 'POST') {
    if (!(await allowed(env.JOIN_LIMIT, request))) return rateLimited('joins');
    if (!isJson(request)) return error(415, 'not_json', 'Send JSON (Content-Type: application/json).');
    const join = await readCandidates(request, allowLoopback);
    if (!join) return badCandidates();
    const result = await invite.join(join.candidates);
    return 'error' in result ? error(result.status, result.error, result.message) : json(200, { host: result.host });
  }
  if (parts.length === 4 && parts[3] === 'joins' && request.method === 'GET') {
    if (!(await allowed(env.HOST_LIMIT, request))) return rateLimited('requests');
    const secret = bearer(request);
    if (!secret) return unauthorized();
    if ((request.headers.get('upgrade') || '').toLowerCase() === 'websocket') {
      // The object answers the upgrade itself, and keeps the socket.
      return invite.fetch(request);
    }
    const result = await invite.list(secret, afterParam(url));
    return 'error' in result ? error(result.status, result.error, result.message) : json(200, { joins: result.joins });
  }
  if (parts.length === 3 && request.method === 'PUT') {
    if (!(await allowed(env.HOST_LIMIT, request))) return rateLimited('requests');
    const secret = bearer(request);
    if (!secret) return unauthorized();
    if (!isJson(request)) return error(415, 'not_json', 'Send JSON (Content-Type: application/json).');
    const body = await readCandidates(request, allowLoopback);
    if (!body) return badCandidates();
    const result = await invite.renew(secret, body.candidates);
    return 'error' in result ? error(result.status, result.error, result.message) : json(200, { code, ...dates(result) });
  }
  if (parts.length === 3 && request.method === 'DELETE') {
    if (!(await allowed(env.HOST_LIMIT, request))) return rateLimited('requests');
    const secret = bearer(request);
    if (!secret) return unauthorized();
    const result = await invite.end(secret);
    return 'error' in result ? error(result.status, result.error, result.message) : json(200, { ok: true });
  }
  return error(404, 'unknown', 'Unknown request.');
}

function dates({ expiresAt, keptUntil }) {
  const out = { expires_at: new Date(expiresAt).toISOString() };
  if (keptUntil) out.kept_until = new Date(keptUntil).toISOString();
  return out;
}

// ---------------------------------------------------------------------------
// One invite

const gone = { status: 404, error: 'not_found', message: 'No such invite. It may have ended.' };
const forbidden = { status: 403, error: 'forbidden', message: 'Wrong secret.' };
const offline = { status: 404, error: 'host_offline', message: OFFLINE_MESSAGE };

export class Invite extends DurableObject {
  constructor(ctx, env) {
    super(ctx, env);
    // The host's keep-alive, answered without waking this object.
    ctx.setWebSocketAutoResponse(new WebSocketRequestResponsePair('ping', 'pong'));
    // Shorter only in the tests (wrangler dev --var IDLE_MS:...).
    this.idleMs = Number(env.IDLE_MS) || IDLE_MS;
  }

  /// The invite, or null once it has ended (and then it's cleared away, but for a kept code).
  async load() {
    const invite = await this.ctx.storage.get('invite');
    if (!invite) return null;
    if (Date.now() >= invite.expiresAt) {
      await this.finish();
      return null;
    }
    return invite;
  }

  /// The kept code's record ({secret, until}), or null.
  async loadKeep() {
    const keep = await this.ctx.storage.get('keep');
    return keep && Date.now() < keep.until ? keep : null;
  }

  async create(host, secret, keep) {
    if ((await this.ctx.storage.get('invite')) || (await this.loadKeep())) return null;
    return this.open(host, secret, keep);
  }

  /// Opens the invite (anew), and keeps the code for its host if `keep`.
  async open(host, secret, keep) {
    const now = Date.now();
    const invite = { host, secret, joins: [], kept: keep, createdAt: now, seenAt: now, expiresAt: now + MAX_AGE_MS };
    await this.ctx.storage.put('invite', invite);
    let keptUntil;
    if (keep) {
      keptUntil = now + KEEP_MS;
      await this.ctx.storage.put('keep', { secret, until: keptUntil });
    }
    await this.ctx.storage.setAlarm(now + this.idleMs);
    return { expiresAt: invite.expiresAt, keptUntil };
  }

  /// A new day for the invite with fresh candidates and no joins, or the kept code open
  /// again after the invite ended. The host's sockets close with CLOSE_RENEWED.
  async renew(secret, host) {
    const invite = await this.load();
    const keep = await this.loadKeep();
    if (invite) {
      if (!sameSecret(secret, invite.secret)) return forbidden;
    } else if (keep) {
      if (!sameSecret(secret, keep.secret)) return forbidden;
    } else {
      return gone;
    }
    const opened = await this.open(host, secret, Boolean(invite?.kept || keep));
    for (const ws of this.ctx.getWebSockets()) closeQuietly(ws, CLOSE_RENEWED, 'renewed');
    return opened;
  }

  async join(candidates) {
    const invite = await this.load();
    if (!invite) return (await this.loadKeep()) ? offline : gone;
    if (invite.joins.length >= (invite.kept ? MAX_JOINS_KEPT : MAX_JOINS)) {
      return { status: 429, error: 'invite_full', message: 'This invite has had too many joins.' };
    }
    const id = invite.joins.length;
    invite.joins.push(candidates);
    await this.ctx.storage.put('invite', invite);
    const message = JSON.stringify({ joins: [{ id, candidates }] });
    for (const ws of this.ctx.getWebSockets()) {
      try {
        ws.send(message);
      } catch {}
    }
    return { host: invite.host };
  }

  /// The joins after the `after`th, at once (no waiting). Counts as the host being there.
  async list(secret, after) {
    const invite = await this.load();
    if (!invite) return gone;
    if (!sameSecret(secret, invite.secret)) return forbidden;
    invite.seenAt = Date.now();
    await this.ctx.storage.put('invite', invite);
    return { joins: joinsAfter(invite, after) };
  }

  /// The host's WebSocket for waiting on joins.
  async fetch(request) {
    const invite = await this.load();
    if (!invite) return error(gone.status, gone.error, gone.message);
    if (!sameSecret(bearer(request), invite.secret)) return error(forbidden.status, forbidden.error, forbidden.message);
    // A host whose network changed may reconnect before its old socket is noticed gone:
    // make room by dropping the one heard from longest ago.
    const sockets = this.ctx.getWebSockets();
    if (sockets.length >= MAX_SOCKETS) {
      const stalest = sockets.reduce((a, b) => (this.lastHeard(a) <= this.lastHeard(b) ? a : b));
      closeQuietly(stalest, 1000, 'replaced');
    }
    const [client, server] = Object.values(new WebSocketPair());
    this.ctx.acceptWebSocket(server);
    server.serializeAttachment({ connectedAt: Date.now() });
    server.send(JSON.stringify({ joins: joinsAfter(invite, afterParam(new URL(request.url))) }));
    return new Response(null, { status: 101, webSocket: client });
  }

  /// Ends the invite for good: a kept code is let go too.
  async end(secret) {
    const invite = await this.load();
    const owner = invite?.secret ?? (await this.loadKeep())?.secret;
    if (!owner) return gone;
    if (!sameSecret(secret, owner)) return forbidden;
    await this.finish(true);
    return {};
  }

  /// Every idleMs (two minutes) or so: end the invite if its host has gone quiet, or it's
  /// too old. Once it has ended, when the kept code runs out: let it go.
  async alarm() {
    const invite = await this.ctx.storage.get('invite');
    const now = Date.now();
    if (invite && now < invite.expiresAt) {
      let seen = invite.seenAt;
      for (const ws of this.ctx.getWebSockets()) {
        const heard = this.lastHeard(ws);
        if (now - heard >= this.idleMs) closeQuietly(ws, 1001, 'quiet');
        else seen = Math.max(seen, heard);
      }
      if (now - seen < this.idleMs) {
        await this.ctx.storage.setAlarm(Math.min(seen + this.idleMs, invite.expiresAt));
        return;
      }
    }
    await this.finish();
  }

  async webSocketMessage(ws) {
    // Only "ping" is expected, and that's answered without coming here.
    closeQuietly(ws, 1003, 'only "ping" is expected');
  }

  async webSocketClose(ws, code) {
    closeQuietly(ws, code === 1005 || code === 1006 ? 1000 : code, '');
  }

  async webSocketError(ws) {
    closeQuietly(ws, 1011, '');
  }

  /// When `ws` was last heard from: its last ping, or when it connected.
  lastHeard(ws) {
    const ping = this.ctx.getWebSocketAutoResponseTimestamp(ws)?.getTime() ?? 0;
    return Math.max(ping, ws.deserializeAttachment()?.connectedAt ?? 0);
  }

  /// Ends the invite. A kept code stays its host's until it runs out, unless `release`.
  async finish(release = false) {
    const keep = release ? null : await this.loadKeep();
    if (keep) {
      await this.ctx.storage.delete('invite');
      await this.ctx.storage.setAlarm(keep.until);
    } else {
      await this.ctx.storage.deleteAlarm();
      await this.ctx.storage.deleteAll();
    }
    for (const ws of this.ctx.getWebSockets()) closeQuietly(ws, CLOSE_ENDED, 'ended');
  }
}

function joinsAfter(invite, after) {
  return invite.joins.slice(after).map((candidates, i) => ({ id: after + i, candidates }));
}

function closeQuietly(ws, code, reason) {
  try {
    ws.close(code, reason);
  } catch {}
}
