# connect.lodestone.cc

Introduces a Lodestone host (the computer with the Minecraft server) to the friends
who open its invite, so the two can connect **directly**: UDP hole punching, then an
encrypted QUIC tunnel between them. This service only passes each side's candidate
addresses to the other. No game traffic flows through it, and it never holds a key.

It runs on Cloudflare Workers, one Durable Object per invite.

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

## Limits

- **Per client address, a minute:** 10 invites, 30 joins, 60 other host requests
  (WebSocket connections, listings, deletes). Only requests with a JSON body or an
  `Authorization` header count, so a web page elsewhere can't use up a visitor's limits.
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

## Privacy

The service stores, per invite and only while it lasts: the host's candidate
addresses, the friends' candidate addresses and the secret. For a kept code, the
secret stays (and nothing else) until the code runs out. It logs nothing about
invites or addresses: Workers' invocation logs (one per request) are off, and the
Worker logs only unexpected errors. The secret travels in a header, never the URL.

## Development

```sh
npm install          # wrangler (pinned), with its workerd runtime
npm run dev          # the Worker locally on http://localhost:8787 (wrangler.jsonc)
npm test             # the tests, against a local Worker
```

Locally, loopback candidates are allowed (`ALLOW_LOOPBACK`) and the join and host
limits are higher, so the tests fit in a minute. `test/idle.test.js` runs a second
local Worker with the idle time cut to 2 seconds (`--var IDLE_MS:2000`).

To point the apps at another service (this local Worker, or a test deployment such
as `https://connect-dev.lodestone.cc`), set "Connect service (relay)" in Lodestone
Launcher's Settings › Developer, or in Lodestone Host's Settings › Updates ›
Developer (shown in developer and debug builds). The Host passes it to its core as
`LODESTONE_CONNECT_SERVICE` and restarts the core. Setting that variable yourself wins
over both settings. The value must be `https://…`, or `http://` to this computer only
(`http://127.0.0.1:8787`), with no path.

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
