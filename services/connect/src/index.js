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
//
// v2 (the friend list: pairing mailboxes, slots and meeting sessions) is at the end of
// this file.

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

/// The body as text, refusing more than `limit` bytes without reading them all.
async function readBody(request, limit = MAX_BODY) {
  if (Number(request.headers.get('content-length') || 0) > limit) return null;
  if (!request.body) return '';
  const reader = request.body.getReader();
  const chunks = [];
  let size = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    size += value.byteLength;
    if (size > limit) {
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

/// The Authorization header's secret: `length` lower-case hex digits (32, or 64 for v2's
/// access and write proofs), or null.
function bearer(request, length = SECRET_HEX_LEN) {
  const header = request.headers.get('authorization') || '';
  const match = /^Bearer ([0-9a-f]+)$/.exec(header);
  return match && match[1].length === length ? match[1] : null;
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
/// setups, where everything passes. Ask it only after the checks a web page elsewhere
/// can't pass from a visitor's browser (JSON bodies, the Authorization header): else any
/// page could use up the visitor's limits, and stop them sharing or joining.
async function allowed(limiter, request) {
  if (!limiter) return true;
  const { success } = await limiter.limit({ key: limitKey(request.headers.get('cf-connecting-ip')) });
  return success;
}

/// What the rate limiters count by: an IPv4 address, or an IPv6 address's /64 (one
/// home or server usually gets a whole /64, so counting each address would let one
/// computer have billions of budgets).
export function limitKey(ip) {
  if (!ip) return 'local';
  if (!ip.includes(':')) return ip;
  const halves = ip.toLowerCase().split('::');
  const head = halves[0] ? halves[0].split(':') : [];
  const tail = halves.length > 1 && halves[1] ? halves[1].split(':') : [];
  const groups = [...head, ...Array(Math.max(0, 8 - head.length - tail.length)).fill('0'), ...tail];
  return groups.slice(0, 4).map((g) => g.replace(/^0+(?=.)/, '')).join(':') + '::/64';
}

/// Charges `changes` lookup registrations to the address's budget: one unit of
/// REGISTER_LIMIT per LOOKUPS_PER_UNIT changes. Each change costs Durable Object calls
/// and writes, so a session can't be made to churn its records for free.
async function chargeRegistrations(limiter, ip, changes) {
  if (!limiter || changes <= 0) return true;
  const key = limitKey(ip);
  for (let units = Math.ceil(changes / LOOKUPS_PER_UNIT); units > 0; units--) {
    const { success } = await limiter.limit({ key });
    if (!success) return false;
  }
  return true;
}

/// Unauthenticated reads (slot reads, presence checks) must carry this header. It isn't
/// one a web page may send to another site without a CORS preflight, which this service
/// never answers, so a page elsewhere can't use up a visitor's budget for them.
function fromClient(request) {
  return request.headers.get('x-lodestone-client') === '1';
}

const rateLimited = (what) => error(429, 'rate_limited', `Too many ${what} from this address. Wait a minute.`);

// ---------------------------------------------------------------------------
// The pages people open from a chat app:
//
//   invite:      https://connect.lodestone.cc/j/<code>#<fingerprint>.<token>
//   friend link: https://connect.lodestone.cc/f/<code>#<key hash>.<link secret>
//
// Each is the same for every code: its script reads the part after "#" in the browser
// (it never reaches this service) and offers to open the whole link in Lodestone
// Launcher, as lodestone://join?invite=<link> or lodestone://friend?link=<link>. They
// load nothing from anywhere, so the link goes nowhere else either.

const RELEASES = 'https://github.com/Lodestone-Team/lodestone/releases/download/launcher-latest';

/// The page's script: shows `#link` when the fragment matches `pattern` (a regular
/// expression literal's source, as the script writes it), and points "Open" at
/// `deepLink` + the whole link, percent-encoded.
function pageScript(pattern, deepLink) {
  return `
(function () {
  var fragment = location.hash.slice(1);
  var ok = /${pattern}/.test(fragment);
  var link = location.origin + location.pathname + '#' + fragment;
  document.getElementById(ok ? 'link' : 'broken').hidden = false;
  if (!ok) return;
  document.getElementById('open').href = '${deepLink}' + encodeURIComponent(link);
  var copy = document.getElementById('copy');
  copy.addEventListener('click', function () {
    navigator.clipboard.writeText(link).then(function () {
      copy.textContent = 'Copied';
    });
  });
})();
`;
}

function pageHtml({ title, ogTitle, ogDescription, body, script }) {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="referrer" content="no-referrer">
<meta name="robots" content="noindex">
<title>${title}</title>
<meta property="og:title" content="${ogTitle}">
<meta property="og:description" content="${ogDescription}">
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
${body}
</main>
<script>${script}</script>
</body>
</html>
`;
}

const DOWNLOADS = `<div class="downloads">
      <a class="button" href="${RELEASES}/LodestoneLauncher.dmg">macOS</a>
      <a class="button" href="${RELEASES}/LodestoneLauncherSetup.exe">Windows</a>
    </div>`;

const INVITE_SCRIPT = pageScript('^[0-9a-f]{64}\\.[0-9a-f]{32}$', 'lodestone://join?invite=');

const INVITE_PAGE = pageHtml({
  title: 'Join a Minecraft server with Lodestone',
  ogTitle: 'Join my Minecraft server',
  ogDescription: 'Open this link in Lodestone Launcher to play together.',
  script: INVITE_SCRIPT,
  body: `  <div id="link" hidden>
    <h1>You're invited to a Minecraft server</h1>
    <p>A friend is sharing their server with Lodestone. Open the invite in Lodestone Launcher to join them.</p>
    <a id="open" class="button primary" href="#">Open in Lodestone Launcher</a>
    <button id="copy" class="button" type="button">Copy invite link</button>
    <h2>Don't have Lodestone Launcher?</h2>
    <p>Download it, then come back here and open the invite.</p>
    ${DOWNLOADS}
    <small>Your game connects straight to your friend's computer. The key in the link stays in your browser: Lodestone's servers never see it.</small>
  </div>
  <div id="broken" hidden>
    <h1>This invite link is cut short</h1>
    <p>Ask your friend to send the whole link again, including the part after the #.</p>
  </div>
  <noscript><p>Copy this page's address and paste it into Lodestone Launcher to join.</p></noscript>`,
});

const FRIEND_SCRIPT = pageScript('^[0-9a-f]{32}\\.[0-9a-f]{64}$', 'lodestone://friend?link=');

const FRIEND_PAGE = pageHtml({
  title: 'Become friends on Lodestone',
  ogTitle: "Let's be friends on Lodestone",
  ogDescription: 'Open this link in Lodestone Launcher to add me as a friend.',
  script: FRIEND_SCRIPT,
  body: `  <div id="link" hidden>
    <h1>You've been invited to be friends on Lodestone</h1>
    <p>Open the link in Lodestone Launcher to see who sent it and add them. Friends see the servers you share with them, and join them without a link.</p>
    <a id="open" class="button primary" href="#">Open in Lodestone Launcher</a>
    <button id="copy" class="button" type="button">Copy link</button>
    <p>You can also paste the link into Lodestone Launcher, in Friends › Add friend.</p>
    <h2>Don't have Lodestone Launcher?</h2>
    <p>Download it, then come back here and open the link. It works for 24 hours.</p>
    ${DOWNLOADS}
    <small>Once you're friends, compare safety codes with your friend, by voice or in person. The secret in the link stays in your browser: Lodestone's servers never see it.</small>
  </div>
  <div id="broken" hidden>
    <h1>This friend link is cut short</h1>
    <p>Ask your friend to send the whole link again, including the part after the #.</p>
  </div>
  <noscript><p>Copy this page's address and paste it into Lodestone Launcher, in Friends › Add friend.</p></noscript>`,
});

const pageHashes = new Map();

/// One of the pages above, or 404 for a malformed code.
async function linkPage(validLinkCode, page, script) {
  if (!validLinkCode) {
    return new Response('No such link.', { status: 404, headers: { 'content-type': 'text/plain; charset=utf-8', 'x-content-type-options': 'nosniff' } });
  }
  let hash = pageHashes.get(script);
  if (!hash) {
    const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(script));
    hash = btoa(String.fromCharCode(...new Uint8Array(digest)));
    pageHashes.set(script, hash);
  }
  return new Response(page, {
    status: 200,
    headers: {
      'content-type': 'text/html; charset=utf-8',
      'cache-control': 'public, max-age=3600',
      'content-security-policy': `default-src 'none'; script-src 'sha256-${hash}'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`,
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
    return linkPage(validCode(parts[1].toUpperCase()), INVITE_PAGE, INVITE_SCRIPT);
  }
  if (parts[0] === 'f' && parts.length === 2 && (request.method === 'GET' || request.method === 'HEAD')) {
    return linkPage(validCode(parts[1].toUpperCase()), FRIEND_PAGE, FRIEND_SCRIPT);
  }
  if (parts[0] === 'v2') return routeV2(request, env, parts);
  if (parts[0] !== 'v1') return error(404, 'unknown', 'Unknown request.');

  if (parts.length === 2 && parts[1] === 'health' && request.method === 'GET') {
    return json(200, { ok: true, version: VERSION });
  }
  if (parts[1] !== 'invites') return error(404, 'unknown', 'Unknown request.');

  if (parts.length === 2) {
    if (request.method !== 'POST') return error(405, 'method', 'Use POST.');
    if (!isJson(request)) return error(415, 'not_json', 'Send JSON (Content-Type: application/json).');
    if (!(await allowed(env.CREATE_LIMIT, request))) return rateLimited('invites');
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
    if (!isJson(request)) return error(415, 'not_json', 'Send JSON (Content-Type: application/json).');
    if (!(await allowed(env.JOIN_LIMIT, request))) return rateLimited('joins');
    const join = await readCandidates(request, allowLoopback);
    if (!join) return badCandidates();
    const result = await invite.join(join.candidates);
    return 'error' in result ? error(result.status, result.error, result.message) : json(200, { host: result.host });
  }
  if (parts.length === 4 && parts[3] === 'joins' && request.method === 'GET') {
    const secret = bearer(request);
    if (!secret) return unauthorized();
    if (!(await allowed(env.HOST_LIMIT, request))) return rateLimited('requests');
    if ((request.headers.get('upgrade') || '').toLowerCase() === 'websocket') {
      // The object answers the upgrade itself, and keeps the socket.
      return invite.fetch(request);
    }
    const result = await invite.list(secret, afterParam(url));
    return 'error' in result ? error(result.status, result.error, result.message) : json(200, { joins: result.joins });
  }
  if (parts.length === 3 && request.method === 'PUT') {
    const secret = bearer(request);
    if (!secret) return unauthorized();
    if (!isJson(request)) return error(415, 'not_json', 'Send JSON (Content-Type: application/json).');
    if (!(await allowed(env.HOST_LIMIT, request))) return rateLimited('requests');
    const body = await readCandidates(request, allowLoopback);
    if (!body) return badCandidates();
    const result = await invite.renew(secret, body.candidates);
    return 'error' in result ? error(result.status, result.error, result.message) : json(200, { code, ...dates(result) });
  }
  if (parts.length === 3 && request.method === 'DELETE') {
    const secret = bearer(request);
    if (!secret) return unauthorized();
    if (!(await allowed(env.HOST_LIMIT, request))) return rateLimited('requests');
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

// ===========================================================================
// v2: friends (launcher/docs/friends.md, "Service protocol (v2)"; README.md has the
// details). Everything that passes through is sealed or hashed by the launchers and
// cores: the service can't read cards, replies, states, records or knocks.
//
//   PUT    /v2/pairs/<code>               {"access": SHA-256(access), "card"} -> 201 {"secret", "expires_at"}; 409 taken
//   GET    /v2/pairs/<code>               (Bearer <access>)  -> 200 {"card"}
//   POST   /v2/pairs/<code>/replies       (Bearer <access>)  {"reply"} -> 200 {"count"}; 429 full
//   GET    /v2/pairs/<code>/replies       (Bearer <secret>)  -> 200 {"replies": [...]}
//   DELETE /v2/pairs/<code>               (Bearer <secret>)  -> 200 {"ok": true}
//   PUT    /v2/slots/<id>                 (Bearer <write proof>) {"sealed"} -> 200 {"ok": true}
//   GET    /v2/slots/<id>                 -> 200 {"sealed"}
//   DELETE /v2/slots/<id>                 (Bearer <write proof>) -> 200 {"ok": true}
//   PUT    /v2/meet                       {"records": [{"lookup", "sealed"}]} -> 201 {"session", "secret", "expires_at"}
//   PUT    /v2/meet/<session>             (Bearer <secret>)  {"records": [...]} -> 200 {"expires_at"}
//   GET    /v2/meet/<session>/knocks      (Bearer <secret>)  WebSocket: {"knocks": [{"lookup", "sealed", "seen"}]}
//   DELETE /v2/meet/<session>             (Bearer <secret>)  -> 200 {"ok": true}
//   POST   /v2/lookups/<lookup>/knocks    {"sealed"} -> 200 {"sealed": <record>}; 404 not_found
//   HEAD   /v2/lookups/<lookup>           -> 200 publishing now, 404 not
//   GET    /f/<code>                      -> the friend link page (HTML)
//
// A mailbox's `access` and a slot's write proof are 64 hex digits, sent as the Bearer.
// The service keeps only SHA-256 of that hex text (UTF-8), written as 64 hex digits, and
// compares digests in constant time.
//
// Durable Objects: one Pair per mailbox code, one Slot per slot id, one MeetSession per
// core's session (holding its records and the core's hibernating WebSocket), and one
// Lookup per lookup id, remembering which session registered it last.

const HEX = '0123456789abcdef';
const HEX32 = /^[0-9a-f]{32}$/;
const HEX64 = /^[0-9a-f]{64}$/;
const PAIR_MS = 24 * 60 * 60 * 1000;
const MAX_REPLIES = 4;
const MAX_CARD_HEX = 4096; // 2 KiB
const SLOT_MS = 90 * 24 * 60 * 60 * 1000;
const MAX_STATE_HEX = 8192; // 4 KiB
const MAX_RECORDS = 256;
const MAX_RECORD_HEX = 2048; // 1 KiB, and the same for a knock
// Body caps, with room for the JSON around the hex.
const MAX_PAIR_BODY = 8 * 1024;
const MAX_SLOT_BODY = 16 * 1024;
const MAX_KNOCK_BODY = 4 * 1024;
const MAX_MEET_BODY = 576 * 1024; // 256 records of about 2.1 KB each
// Lookups registered or unregistered at once, of a session's up to 256 (each is a call
// to that lookup's Durable Object).
const LOOKUP_BATCH = 64;
// Lookup registrations (added or dropped) per unit of REGISTER_LIMIT.
const LOOKUPS_PER_UNIT = 16;

/// Non-empty lower-case hex, whole bytes, at most `max` digits.
function validHex(value, max) {
  return typeof value === 'string' && value.length > 0 && value.length <= max && value.length % 2 === 0 && /^[0-9a-f]+$/.test(value);
}

/// SHA-256 of `text` (UTF-8), as 64 lower-case hex digits.
async function sha256Hex(text) {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(text));
  return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, '0')).join('');
}

const TOO_LARGE = Symbol('too large');

/// The body as a JSON object, TOO_LARGE past `limit` bytes, or null.
async function readObject(request, limit) {
  const text = await readBody(request, limit);
  if (text === null) return TOO_LARGE;
  try {
    const value = JSON.parse(text);
    return value && typeof value === 'object' && !Array.isArray(value) ? value : null;
  } catch {
    return null;
  }
}

/// `{records: {lookup: sealed}}` from a body's `records`, or `{message}` saying what's wrong.
function readRecords(body) {
  const list = body?.records;
  if (!Array.isArray(list) || list.length > MAX_RECORDS) {
    return { message: `Send {"records": [{"lookup", "sealed"}]}: at most ${MAX_RECORDS}.` };
  }
  const records = {};
  for (const record of list) {
    if (!HEX32.test(record?.lookup) || !validHex(record?.sealed, MAX_RECORD_HEX)) {
      return { message: `Each record is {"lookup": 32 hex digits, "sealed": at most ${MAX_RECORD_HEX} hex digits}.` };
    }
    if (Object.hasOwn(records, record.lookup)) return { message: 'A lookup id appears twice.' };
    records[record.lookup] = record.sealed;
  }
  return { records };
}

const notJson = () => error(415, 'not_json', 'Send JSON (Content-Type: application/json).');
const tooLarge = () => error(413, 'too_large', 'The body is too large.');
const badBody = (message) => error(400, 'bad_body', message);
const unauthorizedAs = (what) => error(401, 'unauthorized', `Send the ${what} as "Authorization: Bearer <${what}>".`);
const unknown = () => error(404, 'unknown', 'Unknown request.');

/// An object's answer: its error, or `ok(result)`.
function answer(result, ok) {
  return 'error' in result ? error(result.status, result.error, result.message) : ok(result);
}

const iso = (ms) => new Date(ms).toISOString();

/// A body-less answer, for HEAD.
function bare(status) {
  return new Response(null, { status, headers: { 'cache-control': 'no-store', 'x-content-type-options': 'nosniff' } });
}

/// The v2 routes. Each checks what a page elsewhere can't send from a visitor's browser
/// (a JSON body, an Authorization header) before asking the rate limiter, as v1 does.
/// The unauthenticated reads (slot GET, presence HEAD) have nothing like that: they count
/// against their own limit, so a page can at worst use up a visitor's reads.
async function routeV2(request, env, parts) {
  const method = request.method;
  const kind = parts[1];

  if (kind === 'pairs' && (parts.length === 3 || (parts.length === 4 && parts[3] === 'replies'))) {
    if (!validCode(parts[2])) return error(404, 'not_found', 'No such mailbox. It may have expired.');
    const pair = env.PAIRS.get(env.PAIRS.idFromName(parts[2]));
    const replies = parts.length === 4;

    if (!replies && method === 'PUT') {
      if (!isJson(request)) return notJson();
      if (!(await allowed(env.FRIEND_CREATE_LIMIT, request))) return rateLimited('mailboxes or sessions');
      const body = await readObject(request, MAX_PAIR_BODY);
      if (body === TOO_LARGE) return tooLarge();
      if (!HEX64.test(body?.access) || !validHex(body?.card, MAX_CARD_HEX)) {
        return badBody(`Send {"access": SHA-256 of the access, 64 hex digits, "card": at most ${MAX_CARD_HEX} hex digits}.`);
      }
      const secret = random(HEX, SECRET_HEX_LEN);
      const created = await pair.create(body.access, body.card, secret);
      if (!created) return error(409, 'taken', 'That code is taken.');
      return json(201, { secret, expires_at: iso(created.expiresAt) });
    }
    if (!replies && method === 'GET') {
      const access = bearer(request, 64);
      if (!access) return unauthorizedAs('access');
      if (!(await allowed(env.FRIEND_LIMIT, request))) return rateLimited('requests');
      return answer(await pair.card(await sha256Hex(access)), (r) => json(200, { card: r.card }));
    }
    if (replies && method === 'POST') {
      const access = bearer(request, 64);
      if (!access) return unauthorizedAs('access');
      if (!isJson(request)) return notJson();
      if (!(await allowed(env.FRIEND_LIMIT, request))) return rateLimited('requests');
      const body = await readObject(request, MAX_PAIR_BODY);
      if (body === TOO_LARGE) return tooLarge();
      if (!validHex(body?.reply, MAX_CARD_HEX)) return badBody(`Send {"reply": at most ${MAX_CARD_HEX} hex digits}.`);
      return answer(await pair.reply(await sha256Hex(access), body.reply), (r) => json(200, { count: r.count }));
    }
    if (replies && method === 'GET') {
      const secret = bearer(request);
      if (!secret) return unauthorizedAs('secret');
      if (!(await allowed(env.FRIEND_LIMIT, request))) return rateLimited('requests');
      return answer(await pair.replies(secret), (r) => json(200, { replies: r.replies }));
    }
    if (!replies && method === 'DELETE') {
      const secret = bearer(request);
      if (!secret) return unauthorizedAs('secret');
      if (!(await allowed(env.FRIEND_LIMIT, request))) return rateLimited('requests');
      return answer(await pair.end(secret), () => json(200, { ok: true }));
    }
    return unknown();
  }

  if (kind === 'slots' && parts.length === 3) {
    if (!HEX32.test(parts[2])) return error(404, 'not_found', 'No such slot.');
    const slot = env.SLOTS.get(env.SLOTS.idFromName(parts[2]));

    if (method === 'PUT') {
      const proof = bearer(request, 64);
      if (!proof) return unauthorizedAs('write proof');
      if (!isJson(request)) return notJson();
      if (!(await allowed(env.FRIEND_LIMIT, request))) return rateLimited('requests');
      const body = await readObject(request, MAX_SLOT_BODY);
      if (body === TOO_LARGE) return tooLarge();
      if (!validHex(body?.sealed, MAX_STATE_HEX)) return badBody(`Send {"sealed": at most ${MAX_STATE_HEX} hex digits}.`);
      const proofHash = await sha256Hex(proof);
      let result = await slot.write(proofHash, body.sealed, false);
      if (result.create) {
        // A new slot counts as something made.
        if (!(await allowed(env.FRIEND_CREATE_LIMIT, request))) return rateLimited('new slots');
        result = await slot.write(proofHash, body.sealed, true);
      }
      return answer(result, () => json(200, { ok: true }));
    }
    if (method === 'GET') {
      if (!fromClient(request)) return error(400, 'not_client', 'Send "X-Lodestone-Client: 1".');
      if (!(await allowed(env.SLOT_READ_LIMIT, request))) return rateLimited('reads');
      return answer(await slot.read(), (r) => json(200, { sealed: r.sealed }));
    }
    if (method === 'DELETE') {
      const proof = bearer(request, 64);
      if (!proof) return unauthorizedAs('write proof');
      if (!(await allowed(env.FRIEND_LIMIT, request))) return rateLimited('requests');
      return answer(await slot.erase(await sha256Hex(proof)), () => json(200, { ok: true }));
    }
    return unknown();
  }

  if (kind === 'meet' && parts.length === 2) {
    if (method !== 'PUT') return error(405, 'method', 'Use PUT.');
    if (!isJson(request)) return notJson();
    if (!(await allowed(env.FRIEND_CREATE_LIMIT, request))) return rateLimited('mailboxes or sessions');
    const body = await readObject(request, MAX_MEET_BODY);
    if (body === TOO_LARGE) return tooLarge();
    const { records, message } = readRecords(body);
    if (!records) return badBody(message);
    const secret = random(HEX, SECRET_HEX_LEN);
    // 128 random bits never collide, but never hand out a live session twice.
    for (let attempt = 0; attempt < 3; attempt++) {
      const session = random(HEX, SECRET_HEX_LEN);
      const created = await env.MEET_SESSIONS.get(env.MEET_SESSIONS.idFromName(session)).create(session, secret, records, request.headers.get('cf-connecting-ip'));
      if (created?.rateLimited) return rateLimited('records');
      if (created) return json(201, { session, secret, expires_at: iso(created.expiresAt) });
    }
    throw new Error('no free session id');
  }

  if (kind === 'meet' && (parts.length === 3 || (parts.length === 4 && parts[3] === 'knocks'))) {
    if (!HEX32.test(parts[2])) return error(404, 'not_found', 'No such session. It may have ended.');
    const session = env.MEET_SESSIONS.get(env.MEET_SESSIONS.idFromName(parts[2]));
    const secret = bearer(request);

    if (parts.length === 4 && method === 'GET') {
      if (!secret) return unauthorizedAs('secret');
      if (!(await allowed(env.FRIEND_LIMIT, request))) return rateLimited('requests');
      if ((request.headers.get('upgrade') || '').toLowerCase() !== 'websocket') {
        return error(426, 'websocket', 'Wait for knocks on a WebSocket.');
      }
      // The object answers the upgrade itself, and keeps the socket.
      return session.fetch(request);
    }
    if (parts.length === 3 && method === 'PUT') {
      if (!secret) return unauthorizedAs('secret');
      if (!isJson(request)) return notJson();
      if (!(await allowed(env.FRIEND_LIMIT, request))) return rateLimited('requests');
      const body = await readObject(request, MAX_MEET_BODY);
      if (body === TOO_LARGE) return tooLarge();
      const { records, message } = readRecords(body);
      if (!records) return badBody(message);
      return answer(await session.replace(secret, records, request.headers.get('cf-connecting-ip')), (r) => json(200, { expires_at: iso(r.expiresAt) }));
    }
    if (parts.length === 3 && method === 'DELETE') {
      if (!secret) return unauthorizedAs('secret');
      if (!(await allowed(env.FRIEND_LIMIT, request))) return rateLimited('requests');
      return answer(await session.end(secret), () => json(200, { ok: true }));
    }
    return unknown();
  }

  if (kind === 'lookups' && (parts.length === 3 || (parts.length === 4 && parts[3] === 'knocks'))) {
    const lookup = parts[2];
    const valid = HEX32.test(lookup);

    if (parts.length === 3 && method === 'HEAD') {
      if (!valid) return bare(404);
      if (!fromClient(request)) return bare(400);
      if (!(await allowed(env.LOOKUP_LIMIT, request))) return bare(429);
      const present = await env.LOOKUPS.get(env.LOOKUPS.idFromName(lookup)).present(lookup);
      return bare(present ? 200 : 404);
    }
    if (parts.length === 4 && method === 'POST') {
      if (!valid) return error(404, 'not_found', 'Nobody is publishing that lookup id now.');
      if (!isJson(request)) return notJson();
      if (!(await allowed(env.KNOCK_LIMIT, request))) return rateLimited('knocks');
      const body = await readObject(request, MAX_KNOCK_BODY);
      if (body === TOO_LARGE) return tooLarge();
      if (!validHex(body?.sealed, MAX_RECORD_HEX)) return badBody(`Send {"sealed": at most ${MAX_RECORD_HEX} hex digits}.`);
      // Cloudflare sets it; `wrangler dev` may not.
      const seen = request.headers.get('cf-connecting-ip') || '127.0.0.1';
      const record = await env.LOOKUPS.get(env.LOOKUPS.idFromName(lookup)).knock(lookup, body.sealed, seen);
      if (!record) return error(404, 'not_found', 'Nobody is publishing that lookup id now.');
      return json(200, { sealed: record.sealed });
    }
    return unknown();
  }

  return unknown();
}

// ---------------------------------------------------------------------------
// One pairing mailbox: the card, the hash of the access, the owner's secret and up to 4
// replies, for 24 hours.

const goneMailbox = { status: 404, error: 'not_found', message: 'No such mailbox. It may have expired.' };
const wrongAccess = { status: 403, error: 'forbidden', message: 'Wrong access.' };

export class Pair extends DurableObject {
  async load() {
    const pair = await this.ctx.storage.get('pair');
    if (!pair) return null;
    if (Date.now() >= pair.expiresAt) {
      await this.clear();
      return null;
    }
    return pair;
  }

  async create(accessHash, card, secret) {
    if (await this.load()) return null;
    const expiresAt = Date.now() + PAIR_MS;
    await this.ctx.storage.put('pair', { accessHash, card, secret, replies: [], expiresAt });
    await this.ctx.storage.setAlarm(expiresAt);
    return { expiresAt };
  }

  async card(accessHash) {
    const pair = await this.load();
    if (!pair) return goneMailbox;
    if (!sameSecret(accessHash, pair.accessHash)) return wrongAccess;
    return { card: pair.card };
  }

  async reply(accessHash, reply) {
    const pair = await this.load();
    if (!pair) return goneMailbox;
    if (!sameSecret(accessHash, pair.accessHash)) return wrongAccess;
    if (pair.replies.length >= MAX_REPLIES) {
      return { status: 429, error: 'full', message: `This mailbox has had ${MAX_REPLIES} replies.` };
    }
    pair.replies.push(reply);
    await this.ctx.storage.put('pair', pair);
    return { count: pair.replies.length };
  }

  async replies(secret) {
    const pair = await this.load();
    if (!pair) return goneMailbox;
    if (!sameSecret(secret, pair.secret)) return forbidden;
    return { replies: pair.replies };
  }

  async end(secret) {
    const pair = await this.load();
    if (!pair) return goneMailbox;
    if (!sameSecret(secret, pair.secret)) return forbidden;
    await this.clear();
    return {};
  }

  async alarm() {
    await this.clear();
  }

  async clear() {
    await this.ctx.storage.deleteAlarm();
    await this.ctx.storage.deleteAll();
  }
}

// ---------------------------------------------------------------------------
// One slot: a person's latest sealed state for one friend, and the hash of the write
// proof its first write came with. Gone 90 days after its last write.

const goneSlot = { status: 404, error: 'not_found', message: 'No such slot.' };
const wrongProof = { status: 403, error: 'forbidden', message: 'Wrong write proof.' };

export class Slot extends DurableObject {
  async load() {
    const slot = await this.ctx.storage.get('slot');
    if (!slot) return null;
    if (Date.now() >= slot.writtenAt + SLOT_MS) {
      await this.clear();
      return null;
    }
    return slot;
  }

  /// Writes the state. An empty slot takes the proof it's given, but only with `mayCreate`;
  /// otherwise the answer is {create: true}, so the Worker can count it as something made.
  async write(proofHash, sealed, mayCreate) {
    const slot = await this.load();
    if (slot) {
      if (!sameSecret(proofHash, slot.proofHash)) return wrongProof;
    } else if (!mayCreate) {
      return { create: true };
    }
    const writtenAt = Date.now();
    await this.ctx.storage.put('slot', { proofHash: slot?.proofHash ?? proofHash, sealed, writtenAt });
    await this.ctx.storage.setAlarm(writtenAt + SLOT_MS);
    return {};
  }

  async read() {
    const slot = await this.load();
    return slot ? { sealed: slot.sealed } : goneSlot;
  }

  async erase(proofHash) {
    const slot = await this.load();
    if (!slot) return goneSlot;
    if (!sameSecret(proofHash, slot.proofHash)) return wrongProof;
    await this.clear();
    return {};
  }

  async alarm() {
    await this.clear();
  }

  async clear() {
    await this.ctx.storage.deleteAlarm();
    await this.ctx.storage.deleteAll();
  }
}

// ---------------------------------------------------------------------------
// One core's meeting session: its records ({lookup: sealed}) and its WebSocket, waiting
// for knocks. Like an invite, it ends when the core has been quiet for two minutes, when
// it deletes the session, or after 24 hours. Its lookups are live while a socket heard
// from within two minutes is open on it.

const goneSession = { status: 404, error: 'not_found', message: 'No such session. It may have ended.' };

export class MeetSession extends DurableObject {
  constructor(ctx, env) {
    super(ctx, env);
    // The core's keep-alive, answered without waking this object.
    ctx.setWebSocketAutoResponse(new WebSocketRequestResponsePair('ping', 'pong'));
    // Shorter only in the tests (wrangler dev --var IDLE_MS:...).
    this.idleMs = Number(env.IDLE_MS) || IDLE_MS;
  }

  async load() {
    const session = await this.ctx.storage.get('session');
    if (!session) return null;
    if (Date.now() >= session.expiresAt) {
      await this.finish(session);
      return null;
    }
    return session;
  }

  async create(name, secret, records, ip) {
    if (await this.ctx.storage.get('session')) return null;
    if (!(await chargeRegistrations(this.env.REGISTER_LIMIT, ip, Object.keys(records).length))) {
      return { rateLimited: true };
    }
    const now = Date.now();
    const session = { name, secret, records, seenAt: now, expiresAt: now + MAX_AGE_MS };
    await this.ctx.storage.put('session', session);
    await this.ctx.storage.setAlarm(now + this.idleMs);
    await this.register(name, Object.keys(records));
    return { expiresAt: session.expiresAt };
  }

  /// Replaces the records. Only the lookups added are registered to this session, and
  /// only the ones dropped are unregistered (where they still point here), each charged
  /// to the address's budget, so resending the same records costs next to nothing.
  /// Counts as the core being there.
  async replace(secret, records, ip) {
    const session = await this.load();
    if (!session) return goneSession;
    if (!sameSecret(secret, session.secret)) return forbidden;
    const added = Object.keys(records).filter((lookup) => !Object.hasOwn(session.records, lookup));
    const dropped = Object.keys(session.records).filter((lookup) => !Object.hasOwn(records, lookup));
    if (!(await chargeRegistrations(this.env.REGISTER_LIMIT, ip, added.length + dropped.length))) {
      return { status: 429, error: 'rate_limited', message: 'Too many record changes from this address. Wait a minute.' };
    }
    session.records = records;
    session.seenAt = Date.now();
    await this.ctx.storage.put('session', session);
    await this.register(session.name, added);
    await this.unregister(session.name, dropped);
    return { expiresAt: session.expiresAt };
  }

  /// A knock on one of this session's lookups: sent on the core's sockets, and answered
  /// with the lookup's record. Null if the session isn't live or doesn't have the lookup.
  async knock(lookup, sealed, seen) {
    const session = await this.load();
    if (!session || !Object.hasOwn(session.records, lookup)) return null;
    const sockets = this.liveSockets();
    if (!sockets.length) return null;
    const message = JSON.stringify({ knocks: [{ lookup, sealed, seen }] });
    for (const ws of sockets) {
      try {
        ws.send(message);
      } catch {}
    }
    return { sealed: session.records[lookup] };
  }

  async present(lookup) {
    const session = await this.load();
    return Boolean(session && Object.hasOwn(session.records, lookup) && this.liveSockets().length);
  }

  /// The core's WebSocket for waiting on knocks.
  async fetch(request) {
    const session = await this.load();
    if (!session) return error(goneSession.status, goneSession.error, goneSession.message);
    if (!sameSecret(bearer(request), session.secret)) return error(forbidden.status, forbidden.error, forbidden.message);
    // A core whose network changed may reconnect before its old socket is noticed gone:
    // make room by dropping the one heard from longest ago.
    const sockets = this.ctx.getWebSockets();
    if (sockets.length >= MAX_SOCKETS) {
      const stalest = sockets.reduce((a, b) => (this.lastHeard(a) <= this.lastHeard(b) ? a : b));
      closeQuietly(stalest, 1000, 'replaced');
    }
    const [client, server] = Object.values(new WebSocketPair());
    this.ctx.acceptWebSocket(server);
    server.serializeAttachment({ connectedAt: Date.now() });
    return new Response(null, { status: 101, webSocket: client });
  }

  async end(secret) {
    const session = await this.load();
    if (!session) return goneSession;
    if (!sameSecret(secret, session.secret)) return forbidden;
    await this.finish(session);
    return {};
  }

  /// Every idleMs (two minutes) or so: end the session if its core has gone quiet, or it's
  /// too old.
  async alarm() {
    const session = await this.ctx.storage.get('session');
    if (!session) return;
    const now = Date.now();
    if (now < session.expiresAt) {
      let seen = session.seenAt;
      for (const ws of this.ctx.getWebSockets()) {
        const heard = this.lastHeard(ws);
        if (now - heard >= this.idleMs) closeQuietly(ws, 1001, 'quiet');
        else seen = Math.max(seen, heard);
      }
      if (now - seen < this.idleMs) {
        await this.ctx.storage.setAlarm(Math.min(seen + this.idleMs, session.expiresAt));
        return;
      }
    }
    await this.finish(session);
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

  liveSockets() {
    const now = Date.now();
    return this.ctx.getWebSockets().filter((ws) => now - this.lastHeard(ws) < this.idleMs);
  }

  /// Ends the session: forgets it, closes its sockets with CLOSE_ENDED, and unregisters
  /// its lookups (a Lookup that misses this lets go after 24 hours anyway).
  async finish(session) {
    await this.ctx.storage.deleteAlarm();
    await this.ctx.storage.deleteAll();
    for (const ws of this.ctx.getWebSockets()) closeQuietly(ws, CLOSE_ENDED, 'ended');
    try {
      await this.unregister(session.name, Object.keys(session.records));
    } catch (err) {
      console.error('connect: unregistering lookups failed', err?.message || String(err));
    }
  }

  async register(name, lookups) {
    await eachLookup(this.env, lookups, (lookup) => lookup.register(name));
  }

  async unregister(name, lookups) {
    await eachLookup(this.env, lookups, (lookup) => lookup.unregister(name));
  }
}

async function eachLookup(env, lookups, call) {
  for (let i = 0; i < lookups.length; i += LOOKUP_BATCH) {
    await Promise.all(lookups.slice(i, i + LOOKUP_BATCH).map((id) => call(env.LOOKUPS.get(env.LOOKUPS.idFromName(id)))));
  }
}

// ---------------------------------------------------------------------------
// One lookup id: the session that registered it last, for 24 hours from then (no
// session lasts longer).

export class Lookup extends DurableObject {
  async register(session) {
    await this.ctx.storage.put('session', session);
    await this.ctx.storage.setAlarm(Date.now() + MAX_AGE_MS);
  }

  /// Lets go, unless another session registered this lookup since.
  async unregister(session) {
    if ((await this.ctx.storage.get('session')) !== session) return;
    await this.ctx.storage.deleteAlarm();
    await this.ctx.storage.deleteAll();
  }

  async session() {
    const name = await this.ctx.storage.get('session');
    return name ? this.env.MEET_SESSIONS.get(this.env.MEET_SESSIONS.idFromName(name)) : null;
  }

  /// {sealed: the record} once the knock reached its live session, else null.
  async knock(lookup, sealed, seen) {
    const session = await this.session();
    return session ? session.knock(lookup, sealed, seen) : null;
  }

  async present(lookup) {
    const session = await this.session();
    return session ? session.present(lookup) : false;
  }

  async alarm() {
    await this.ctx.storage.deleteAll();
  }
}
