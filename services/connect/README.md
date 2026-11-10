# connect.lodestone.cc

Introduces a Lodestone host (the computer with the Minecraft server) to the friends
who open its invite, so the two can connect **directly**: UDP hole punching, then an
encrypted QUIC tunnel between them. This service only passes each side's candidate
addresses to the other. No game traffic flows through it, and it never holds a key.

It runs on Cloudflare Workers, one Durable Object per invite.

It also serves the friend list (v2, [below](#protocol-v2-friends)): a mailbox for
pairing two people's launchers, slots that carry each person's signed state to each
friend, and meeting sessions where a friend's launcher knocks on a core without a link.
Everything in v2 is sealed or hashed by the launchers and cores before it gets here.
The design is `launcher/docs/friends.md`.

## How an invite works

1. The host collects its candidate addresses (its local network address, IPv6, the
   public address STUN servers see, a router-forwarded port) and registers them:
   `POST /v1/invites`. It gets a short **code** and a **secret**.
2. The host shares an invite link:
   `https://connect.lodestone.cc/j/<code>#<certificate fingerprint>.<token>`.
   The part after `#` never reaches this service, so the service can neither pretend
   to be the host (the friend checks the fingerprint) nor join itself (the host
   checks the token). Opened in a browser, the link shows the [invite page](#the-invite-page).
   The prototype's form, `lhp2://connect.lodestone.cc/<code>#...`, names the same invite.
3. A friend opens the link and joins: `POST /v1/invites/<code>/joins` with its own
   candidates, and gets the host's back.
4. The host, waiting on a WebSocket (`GET /v1/invites/<code>/joins`), hears about
   the join at once and punches towards the friend's addresses; the friend punches
   towards the host's.

## Protocol (v1)

JSON over HTTPS. Bodies are sent as `Content-Type: application/json`. Errors are
`{"error": "<code>", "message": "<text>"}`.

| Request | Body / headers | Answer |
|---|---|---|
| `POST /v1/invites` | `{"candidates": [...], "keep"?: true}` | `201 {"code", "secret", "expires_at", "kept_until"?}` |
| `PUT /v1/invites/<code>` | `Authorization: Bearer <secret>`, `{"candidates": [...]}` | `200 {"code", "expires_at", "kept_until"?}` |
| `POST /v1/invites/<code>/joins` | `{"candidates": [...]}` | `200 {"host": [...]}` |
| `GET /v1/invites/<code>/joins?after=<n>` as a WebSocket | `Authorization: Bearer <secret>` | `101`, then `{"joins": [{"id", "candidates"}]}`: the joins after the `n`th at once, then each new join as it comes |
| `GET /v1/invites/<code>/joins?after=<n>` | `Authorization: Bearer <secret>` | `200 {"joins": [...]}` at once |
| `DELETE /v1/invites/<code>` | `Authorization: Bearer <secret>` | `200 {"ok": true}` |
| `GET /v1/health` | | `200 {"ok": true, "version": "1"}` |
| `GET /j/<code>` | | `200`, the invite page (HTML) |

On the WebSocket the host sends `ping` at least every 30 seconds and gets `pong`
(Cloudflare answers it without waking the invite's Durable Object, so a waiting invite
costs nothing). Anything else closes the socket. When the invite ends, the service
closes the socket with code 4000; when it is renewed, with code 4001 (connect again
with `after=0`).

### Stable links

An invite made with `"keep": true` keeps its code for its host after it ends, so a link
shared once keeps working whenever the host is sharing, across restarts:

- The answer has `kept_until`: the code stays the host's until then (30 days after the
  invite was last open). A new invite never gets a kept code.
- `PUT /v1/invites/<code>` with the secret renews the invite: new candidates, a new
  day, no joins yet (join ids start from 0 again), and the host's sockets close with
  4001. After the invite ended, it opens it again with the same code and secret.
  Without `keep`, an invite can be renewed only while it lasts.
- While a kept invite is closed, joins get `404 host_offline`.
- `DELETE` ends the invite and lets the code go.

The host renews about once a day, and after it opens again following a restart.

Candidates are 1 to 8 addresses: `a.b.c.d:port` or `[ipv6]:port`, written without
leading zeros. Unspecified, multicast, broadcast and IPv4-mapped IPv6 addresses and
port 0 are refused (`bad_candidates`).

Error codes: `bad_candidates` (400), `unauthorized` (401: no `Authorization`),
`forbidden` (403: wrong secret), `not_found` (404: no such invite, or it ended),
`host_offline` (404: a kept invite whose host isn't sharing right now),
`not_json` (415), `rate_limited` and `invite_full` (429), `internal` (500).

## Protocol (v2): friends

JSON over HTTPS, as v1: `POST`/`PUT` bodies are `application/json`, errors are
`{"error", "message"}`, and every answer is `cache-control: no-store`. Binary values are
lower-case hex, whole bytes; sealed blobs are `nonce | ciphertext | tag`, opaque here.

### Pairing mailboxes

One Durable Object (`Pair`) per code: 10 Crockford base32 characters, upper case, chosen
by the launcher that makes the friend link (`https://connect.lodestone.cc/f/<code>#...`).

| Request | Auth | Body | Answer |
|---|---|---|---|
| `PUT /v2/pairs/<code>` | | `{"access": SHA-256(access), "card"}` | `201 {"secret", "expires_at"}`; `409 taken` |
| `GET /v2/pairs/<code>` | `Bearer <access>` | | `200 {"card"}` |
| `POST /v2/pairs/<code>/replies` | `Bearer <access>` | `{"reply"}` | `200 {"count"}`; `429 full` after 4 |
| `GET /v2/pairs/<code>/replies` | `Bearer <secret>` | | `200 {"replies": [...]}`, in the order they came |
| `DELETE /v2/pairs/<code>` | `Bearer <secret>` | | `200 {"ok": true}` |

- `access` is 64 hex digits, known to whoever has the link. The mailbox keeps only
  `SHA-256(access)`: SHA-256 of the 64-digit hex text (UTF-8), sent at creation as 64
  hex digits. Later requests send the access itself; the service hashes it and compares
  the digests in constant time. The stored hash doesn't open the mailbox.
- `secret` (32 hex digits) is the owner's, for reading replies and deleting.
- A card or reply is at most 4096 hex digits (2 KiB). A mailbox takes 4 replies and is
  deleted 24 hours after it was made.

### Slots

One Durable Object (`Slot`) per slot id (32 hex digits).

| Request | Auth | Body | Answer |
|---|---|---|---|
| `PUT /v2/slots/<id>` | `Bearer <write proof>` | `{"sealed"}` | `200 {"ok": true}`; `403` wrong proof |
| `GET /v2/slots/<id>` | | | `200 {"sealed"}`; `404 not_found` |
| `DELETE /v2/slots/<id>` | `Bearer <write proof>` | | `200 {"ok": true}` |

The write proof is 64 hex digits. An empty slot keeps the SHA-256 of the first write's
proof (computed as for `access`), and later writes and deletes must bring the same proof
(trust on first write). A sealed state is at most 8192 hex digits (4 KiB). A slot is
deleted 90 days after its last write.

### Meeting

A core registers one **session** (a `MeetSession` Durable Object) holding all its
records, and waits for knocks on one WebSocket. Each lookup id has a `Lookup` Durable
Object remembering the session that registered it last.

| Request | Auth | Body | Answer |
|---|---|---|---|
| `PUT /v2/meet` | | `{"records": [{"lookup", "sealed"}]}` | `201 {"session", "secret", "expires_at"}` |
| `PUT /v2/meet/<session>` | `Bearer <secret>` | `{"records": [...]}` | `200 {"expires_at"}`: replaces the records |
| `GET /v2/meet/<session>/knocks` as a WebSocket | `Bearer <secret>` | | `101`, then `{"knocks": [{"lookup", "sealed", "seen"}]}` per knock |
| `DELETE /v2/meet/<session>` | `Bearer <secret>` | | `200 {"ok": true}` |
| `POST /v2/lookups/<lookup>/knocks` | | `{"sealed"}` | `200 {"sealed": <the lookup's record>}`; `404 not_found` |
| `HEAD /v2/lookups/<lookup>` | | | `200` publishing now, `404` not (no body) |

- `session`, `secret` and lookup ids are 32 hex digits. A session has at most 256
  records, each lookup once (a repeated one is refused, `400`); a sealed record or knock
  is at most 2048 hex digits (1 KiB).
- Making or replacing a session registers each of its lookups to it: a lookup belongs to
  the session that registered it last. Lookups dropped by a replacement, or left when a
  session ends, are unregistered unless another session has taken them since.
- A lookup is **live** while its session is: a WebSocket on it heard from (connected or
  pinged) within 2 minutes, and the session under 24 hours old. A knock on a live lookup
  goes to the session's sockets, with `seen`, the address the knock came from
  (`cf-connecting-ip`; an IP address without a port), and is answered with the record.
  A knock on anything else is `404 not_found` and goes nowhere: knocks aren't queued.
- The WebSocket works as an invite's: `ping` every 30 seconds gets `pong` without waking
  the session, anything else closes the socket, at most 4 sockets (a fifth replaces the
  one heard from longest ago). A session ends when its core has been quiet for 2 minutes
  (no ping, no replacement), when it's deleted, or after 24 hours; its sockets close with
  4000.

Error codes, beyond v1's: `bad_body` (400: a field missing or malformed), `forbidden`
(403: wrong access, secret or write proof), `taken` (409), `too_large` (413),
`websocket` (426: the knocks route is a WebSocket only), `full` (429).

## Limits

- **Per client address, a minute:** 10 invites, 30 joins, 60 other host requests
  (WebSocket connections, listings, deletes). Only requests with a JSON body or an
  `Authorization` header count, so a web page elsewhere can't use up a visitor's limits.
- **A client address** is an IPv4 address, or an IPv6 address's /64 (one home or server
  usually has a whole /64).
- **v2, per client address, a minute:** 10 mailboxes, sessions or new slots made
  (`FRIEND_CREATE_LIMIT`), 30 knocks (`KNOCK_LIMIT`), 120 presence checks
  (`LOOKUP_LIMIT`), 600 slot reads (`SLOT_READ_LIMIT`), 60 other requests
  (`FRIEND_LIMIT`: mailbox reads and replies, slot writes and deletes, session updates,
  WebSockets and deletes), and 40 units of lookup registrations (`REGISTER_LIMIT`, one
  per 16 lookups a session adds or drops). A first write to a slot counts against both
  the other-requests and the creation limits.
- **Sessions** register only the lookups added and unregister only those dropped, so
  resending the same records costs no Durable Object calls, and churning them is
  budgeted. A lookup another session took isn't taken back by resending it.
- **Presence checks and slot reads** must carry `X-Lodestone-Client: 1` (else 400). A web
  page elsewhere can't send that header without a CORS preflight, which this service
  never answers, so it can't use up a visitor's limits.
- **v2 bodies:** 8 KiB for mailboxes, 16 KiB for slots, 4 KiB for knocks, 576 KiB for a
  session's records.
- **An invite** ends when its host has been quiet (no ping, no listing) for 2 minutes,
  when the host deletes it, or 24 hours after it was made (or last renewed). It takes
  at most 16 joins, a kept invite 64.
  A host has at most 4 WebSockets on it; a fifth replaces the one heard from longest ago.
- **Bodies** are at most 4 KiB.

## The invite page

`GET /j/<code>` answers the same small page for every valid code. Its script reads the
part after `#` in the browser and offers:

- **Open in Lodestone Launcher**: `lodestone://join?invite=<the whole link,
  percent-encoded>`. The launcher registers the `lodestone://` scheme and parses the
  invite (the `lodestone_connect` crate reads all three forms).
- **Copy invite link**, to paste into the launcher.
- Downloads of the launcher for macOS and Windows, from the `launcher-latest` release.

It loads nothing from anywhere else (a strict Content-Security-Policy allows only its
own script), sends no referrer, and never sends the part after `#` anywhere.

## The friend link page

`GET /f/<code>` is the same kind of page for friend links
(`https://connect.lodestone.cc/f/<code>#<key hash>.<link secret>`, 32 and 64 hex
digits). It says "You've been invited to be friends on Lodestone" (it can't know who
from), and offers **Open in Lodestone Launcher** (`lodestone://friend?link=<the whole
link, percent-encoded>`), **Copy link**, the downloads, and a note that the link can be
pasted into Lodestone Launcher › Friends › Add friend. It has the same
Content-Security-Policy, with its own script's hash, and loads nothing from elsewhere.

## Privacy

The service stores, per invite and only while it lasts: the host's candidate
addresses, the friends' candidate addresses and the secret. For a kept code, the
secret stays (and nothing else) until the code runs out. It logs nothing about
invites or addresses: Workers' invocation logs (one per request) are off, and the
Worker logs only unexpected errors. The secret travels in a header, never the URL.

For v2 it stores, sealed or hashed so that it can't read or forge them: cards and
replies (for 24 hours), the SHA-256 of mailbox accesses and slot write proofs, sealed
states (90 days after their last write), and sessions' sealed records (while they last).
It can't read addresses, names or keys from any of these, and never sees a friend link's
part after `#`.

What it **can** see is who meets whom: which addresses register sessions holding which
lookup ids, and which addresses knock on or check the presence of which lookup ids. A
core's lookup ids sit in one session, so they're linked across periods by that session
and its address, and presence checks link a friend's address to a core. It also sees
which addresses read and write which slots and mailboxes. It keeps none of this beyond
what the tables above store (a knock's address goes to the core and is not stored), and
logs none of it; Cloudflare's platform still sees request metadata.

## Development

```sh
npm install          # wrangler (pinned), with its workerd runtime
npm run dev          # the Worker locally on http://localhost:8787 (wrangler.jsonc)
npm test             # the tests, against a local Worker
```

Locally, loopback candidates are allowed (`ALLOW_LOOPBACK`) and the join, host and v2
limits are higher, so the tests fit in a minute. `test/idle.test.js` runs a second
local Worker with the idle time cut to 2 seconds (`--var IDLE_MS:2000`), for invites
and meeting sessions. `test/friends.test.js` covers v2 and the friend link page.

## Deploying

Deploys go to Cloudflare under the account that owns lodestone.cc, with the `cf` CLI
signed in to it (`cf auth whoami` shows which account):

```sh
cf deploy --dry-run   # build and check, upload nothing
cf deploy
```

`cf deploy` reads `cloudflare.config.ts`; keep `wrangler.jsonc` (used by `npm run dev`
and the tests) in step with it. The Worker serves only `connect.lodestone.cc`: its
workers.dev address is off, since the account's workers.dev subdomain carries a
personal name.
