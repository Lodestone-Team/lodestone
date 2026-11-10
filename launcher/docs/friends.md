# Friends: joining a friend's server without a link

Status: built (protocol, service, core, launcher library; Launcher and Host UI in
progress). Revised after a security review of the design (see "Security review" at
the end).

Today a friend joins a server through an invite link that the host sends them, one link
per shared instance (Lodestone Connect, `crates/lodestone-connect`). The friend list
adds a person once; from then on the launcher shows which of their servers are online
and joins them with one click.

connect.lodestone.cc stays, as a mailbox and introducer that can't read what passes
through it, forge anyone's key, or join a server. It does still see which addresses look
each other up (see "What this doesn't protect against").

## Goals

- Add a friend once, then see and join the servers they share with you, without links.
- Keep game traffic direct and end-to-end encrypted, as Connect does today.
- Nothing the service stores lets it, or anyone who later gets a friend link, read
  addresses or impersonate anyone.
- Remove a friend, retire a core or retire your own key without breaking anyone else.
- Keep invite links working as they do now.

## Not goals

- **No service at all.** Two computers behind home routers need a third party to swap
  addresses. A DHT fallback is listed under "Later".
- **Hiding IP addresses from friends.** A direct connection shows them.
- **Remote control.** Friends can see and join servers shared with them, nothing else.
- **Trusting a friend's mods.** Friendship never trusts a server's mods.

## Pieces

| Piece | What it is | Where it lives |
|---|---|---|
| Person key | Ed25519 key pair; the person's id | Keychain (`SecretStore`), shared by Launcher and Host on one computer |
| Safety code | 80 bits of SHA-256(key), `XXXX-XXXX-XXXX-XXXX` | shown, compared out of band |
| Core certificate | One ECDSA certificate per core for friends (apart from invite identities) | core `stores/connect/friends/core.json` |
| Pair secret | 32 bytes two friends share, from an X25519 exchange at pairing | both launchers (encrypted friend file) |
| Core secret | `HKDF(pair, "core", owner key, friend key, core fp)`: one per core, per friend, per direction | the core, and the friend's launcher |
| State | The person's signed, versioned list of their cores | a slot at the service, for each friend |

A core only ever holds core secrets, never pair secrets: a compromised core exposes its
own lookups and nothing about the friendship's other cores.

## Becoming friends

1. **Ana creates a friend link.** Her launcher makes a 32-byte link secret `s`, an X25519
   key pair `eA` (kept, encrypted, until the link is used or expires), and a code
   `c = crockford(HKDF(s, "pair-code"))[..10]`. It posts her **card** to the mailbox `c`:
   sealed with `s` (role Card, context `c`), signed with her person key over
   `("card", key, eA pub, name, c, expires, SHA-256(s))`. Signing `SHA-256(s)` ties the
   card to this link, so it can't be replayed under another.

   The link is `https://connect.lodestone.cc/f/<c>#<h>.<s>`, where `h` is the first 16
   bytes of SHA-256(Ana's key). The part after `#` never reaches the service in a request.
   It expires after 24 hours.
2. **Ben opens it**, by pasting it into the launcher (Friends › Add friend) or through
   the page's "Open in Lodestone Launcher". His launcher fetches the card, proving it has
   the link with `HKDF(s, "pair-access", c)`. It checks the card's key against `h`, the
   signature, the code and the expiry, then shows Ana's name and safety code. If Ben
   accepts, it makes `eB`, posts a **reply** signed over
   `("reply", SHA-256(card), key, eB pub, name)`, and computes the pair secret:
   `pair = HKDF(X25519(eB, eA) ‖ s, "pair", SHA-256(card), SHA-256(reply))`.
   Ben's side of the friendship stays **pending**: nothing is shared, and no core of his
   accepts Ana, until Ana's first signed state arrives.
3. **Ana's launcher collects the replies.** The mailbox takes up to 4. More than one
   means someone else saw the link, and the launcher says so. For each reply it checks
   the signature and shows Ben's name and safety code. If Ana accepts, it computes the
   same pair secret, deletes `eA` and the mailbox, and writes her first **state** to her
   slot for Ben. That state is her signed confirmation.
4. Both launchers mark the friend **Not verified** until their people compare safety
   codes, by voice or in person, and tick "Codes match". That comparison is the only
   check against someone who saw the link first, including whoever served the link page.

Once `eA` and `eB` are deleted, the link secret and everything the service stored no
longer reveal the pair secret (forward secrecy).

**Names.** Display names are at most 32 characters, with control and bidirectional
formatting characters removed. The list shows a local nickname the owner can change, and
warns when a new friend's name matches an existing friend's.

**Nothing is shared by default.** A new friend sees no servers until the owner shares an
instance with them by name.

## States and slots

Each person keeps one **slot** at the service for each friend, holding their latest
signed state:

- slot id `= HKDF(pair, "slot", sender key, receiver key)[..16]`;
- write proof `= HKDF(pair, "slot-write", sender key, receiver key)`; the service keeps
  only its SHA-256, from the first write;
- the state is sealed with `pair` (role State, context the slot id) and signed over
  `("state", receiver key, seq, issued, expires, name, cores, retired)`.

`cores` lists `(core fingerprint, core name)` for each core the person shares from.
Receivers keep the highest `seq` seen and refuse lower ones. A state expires 90 days after
it was issued; launchers issue a new one weekly while they run. A friend's cores in an
expired state aren't joined.

States are how a person:

- **adds or retires a core.** A retired core is dropped, and trust in its servers' mods
  is cleared;
- **confirms a new friendship** (the first state);
- **retires their key** (`retired: true`, signed by the old key). Friends' launchers mark
  the friend retired and remove them from their own cores.

## Finding a friend's core

Each core shares with friends through one more `lodestone_connect` host, with the core
certificate, a separate QUIC protocol (ALPN `lodestone-friend-1`) and its own socket.
For each friend with at least one instance shared with them, it publishes a record:

- **period** `= (now + offset) / 86400`, where `offset = HKDF(core secret, "offset") mod
  86400`: each pair's ids change at a different time of day;
- **lookup id** `= HKDF(core secret, "lookup", period)[..16]`;
- **record**: `{issued, candidates}`, sealed with the core secret (role Record, context
  the lookup id).

Within an hour of a period's end, the core publishes the next period's ids as well. The
friend tries the current period's id, then the previous and next ones (clock skew).

To join, the friend's launcher **knocks** on the lookup id with
`{id: 16 random bytes, issued, candidates}`, sealed with the core secret (role Knock).
The service passes the knock to the core with the address it saw it from, and hands back
the record. The core refuses a knock issued more than 10 minutes away from its own clock,
or whose id it has seen in the last 20 minutes.

Presence: `HEAD` on a lookup id says whether that core is publishing for you now. The
launcher checks every 5 minutes while the friend list is open, and never in the
background. Owners can hide from chosen friends, which stops their records.

## Joining

1. The core probes towards the knock's addresses: at most 8, one spray per friend at a
   time, and at most 8 sprays at once. The probe tag is derived per knock:
   `HKDF(core secret, "probe", knock id)[..8]`.
2. The core answers QUIC only from IP addresses in a knock it accepted in the last 2
   minutes (its candidates; the address the service says it saw isn't used, since the
   service could name any), after the usual QUIC retry. Each friend's latest knock
   replaces their addresses, and knock ids are remembered per friend, so one friend can't
   crowd out another; each friend may hold at most 4 connections. A
   removed or hidden friend who remembers the core's address gets no answer.
3. The friend opens QUIC, accepting only the core certificate in Ana's current state.
4. **Friend hello** must be the first stream: kind `3`, the friend's public key, and a
   signature over `("friend hello", exporter, core fingerprint, friend key)`, where
   `exporter` is 32 bytes of the TLS exporter (label `lodestone-friend hello`). It's tied
   to this connection. The core checks the key is a friend's and answers `0x4c`. Any
   other first stream, a bad signature or an unknown key closes the connection. Auth
   state lives on the connection: there's no token. No 0-RTT.
5. Later streams start with one byte: `4` friend info (HTTP), or `5` game, followed by a
   length byte and the instance uuid. Kinds `0`–`2` (invite streams) are refused here,
   and the invite hosts refuse `3`–`5`.
6. The core checks on every stream that the friend is still on its list and the instance
   is still shared with them. Removing a friend or unsharing an instance with them
   closes their connection.

Friend info routes, for the instances shared with this friend only:

- `GET /api/v1/friend/servers`: their `SharedServer`s;
- `GET /api/v1/public/instances`: those instances, as the public list shows them;
- `GET /api/v1/public/instances/{uuid}/client-pack`: a shared instance's client pack.

In the launcher, a server joined this way is a `FriendServer` with the id
`friend:<core fingerprint>/<instance uuid>`.

## The core's friend list

The owner's launcher keeps each of the owner's cores up to date:
`PUT /connect/friends` with `{version, friends: [{key, name, secret, shared}]}`, where
`secret` is that friend's core secret for this core. The core refuses a version lower
than the one it has. These routes:

- need the **owner**, not just an instance's settings permission;
- are refused unless the request arrived over the core's HTTPS or from loopback, since
  they carry secrets. The launcher sends them only to a core whose certificate it has
  pinned, or over loopback.

Lodestone Host keeps its core in step: it syncs at start, whenever the friend file
changes and after sharing changes, and shows whether the last sync worked. A removal
reaches a core only through that sync, so a core whose Host isn't running keeps its old
list until it next starts (and syncs) or is reached.

A core shares with at most 120 friends: each gets one or two records, and the service
takes at most 256 from a core.

The core keeps its list in `stores/connect/friends/list.json` (0600 on Unix; on Windows, the
user's profile folder already limits it to that user).

## Mods

- Trust stays per server, tied to its certificate (`trusted_as`). A friend's server
  starts untrusted.
- For servers joined from the friend list, trust is also tied to the client pack: when
  the pack changes, the launcher asks again before installing it, listing the mods it
  adds or changes. Whoever changed the pack (the owner or another user of their core),
  nothing new runs without asking.
- Retiring a core clears trust in its servers.

## Launcher storage

- Person key: Keychain.
- Friend file `<data>/friend_list.json`: sealed with a key kept in the Keychain. It holds
  friends (key, names, pair secret, verified, pending, last state), pending links (`s`,
  `eA`, the mailbox secret) and own state numbers. The friend graph isn't readable from
  the disk alone.
- Keychain items can be read by any program running as the same user (on Windows, DPAPI
  protects them with the user's sign-in). That's the boundary.

## Service protocol (v2)

JSON over HTTPS. Binary values are lower-case hex. Sealed blobs are `nonce | ciphertext
| tag` (ChaCha20-Poly1305). Errors are `{"error", "message"}` as in v1. All `POST`/`PUT`
bodies are `application/json`. Rate limits are per client address.

### Pairing mailboxes (one Durable Object per code)

| Request | Auth | Body | Answer |
|---|---|---|---|
| `PUT /v2/pairs/<code>` | | `{"access": SHA-256(access), "card"}` | `201 {"secret", "expires_at"}`; `409 taken` |
| `GET /v2/pairs/<code>` | `Bearer <access>` | | `200 {"card"}` |
| `POST /v2/pairs/<code>/replies` | `Bearer <access>` | `{"reply"}` | `200 {"count"}`; `429 full` after 4 |
| `GET /v2/pairs/<code>/replies` | `Bearer <secret>` | | `200 {"replies": [...]}` |
| `DELETE /v2/pairs/<code>` | `Bearer <secret>` | | `200 {"ok": true}` |

`access` is 64 hex digits, `secret` 32. A mailbox lasts 24 hours. A card or reply is at
most 2 KiB (4096 hex digits).

### Slots (one Durable Object per slot id)

| Request | Auth | Body | Answer |
|---|---|---|---|
| `PUT /v2/slots/<id>` | `Bearer <write proof>` | `{"sealed"}` | `200 {"ok": true}`; `403` wrong proof |
| `GET /v2/slots/<id>` | | | `200 {"sealed"}`; `404` |
| `DELETE /v2/slots/<id>` | `Bearer <write proof>` | | `200 {"ok": true}` |

A slot keeps the SHA-256 of the first write's proof and lasts 90 days after its last
write. A sealed state is at most 4 KiB (8192 hex digits).

### Meeting

A core registers one **session** with all its records, and waits on one WebSocket.

| Request | Auth | Body | Answer |
|---|---|---|---|
| `PUT /v2/meet` | | `{"records": [{"lookup", "sealed"}]}` | `201 {"session", "secret", "expires_at"}` |
| `PUT /v2/meet/<session>` | `Bearer <secret>` | `{"records": [...]}` | `200 {"expires_at"}`: replaces the records |
| `GET /v2/meet/<session>/knocks` (WebSocket) | `Bearer <secret>` | | `{"knocks": [{"lookup", "sealed", "seen"}]}` per knock |
| `DELETE /v2/meet/<session>` | `Bearer <secret>` | | `200 {"ok": true}` |
| `POST /v2/lookups/<lookup>/knocks` | | `{"sealed"}` | `200 {"sealed": <record>}`; `404 not_found` |
| `HEAD /v2/lookups/<lookup>` | | | `200` publishing now, `404` otherwise |

- A session has at most 256 records; a sealed record or knock is at most 1 KiB.
- `seen` is the address the knock came from (`cf-connecting-ip`).
- A lookup id is held by one session; another session takes it only while the holder
  isn't live, and a session that couldn't take one tries again on its next update and
  every 2 minutes while it's live (a restarted core gets its ids back once its old
  session has gone quiet). A record is live while its session is (pinged within 2
  minutes, at most 24 hours old).
- Sessions use `ping`/`pong` and close codes 4000 (ended) as invites do.
- Session updates register only the lookups added and unregister only those dropped;
  each 16 changes cost one unit of a per-address budget (40 units a minute), so records
  can't be churned to burn Durable Object calls.
- Slot reads and presence checks need `X-Lodestone-Client: 1`, a header a web page
  elsewhere can't send without a CORS preflight (never answered), so pages can't spend a
  visitor's budget.
- Rate limits count an IPv4 address, or an IPv6 address's /64.
- Per address, a minute: 10 mailboxes or sessions made, 30 knocks, 120 presence checks,
  600 slot reads and 60 other requests.

## What this protects against

| Attack | What happens |
|---|---|
| The service pretends to be a friend's core | Refused: the friend accepts only the certificate in the friend's signed state. |
| The service pretends to be a friend joining | Refused: the hello is signed with the friend's key and tied to the TLS session. |
| The service, or a later leak of a link, reveals pair secrets | The pair secret comes from an X25519 exchange whose keys are deleted after pairing. |
| Someone uses the link before Ben | The mailbox takes up to 4 replies and Ana sees them all; comparing safety codes catches the rest. Whoever serves the link page could also see the link. |
| A card or state is replayed elsewhere | Cards sign the link's hash; states sign the receiver's key; seals bind role and context. |
| A knock is replayed | Refused when more than 10 minutes from the core's clock, or once seen (ids are remembered for 25 minutes, longer than the window, but only while the core runs: just after a restart a replay can open the knocker's addresses for 2 minutes, and the hello still needs the friend's key). Records carry their issue time. |
| A stolen core | Its owner retires it in a new state; friends drop it and clear mod trust. It never held pair secrets. |
| A stolen launcher key | A final retired state, signed with it, tells friends to drop it. |
| A removed friend keeps trying | Their records stop, cores that got the new list refuse their key and forget their knocks, and cores don't answer QUIC without an accepted knock. Someone sharing an address with a friend who knocked does get a handshake, but not past the hello. |
| A friend floods a core | One spray, one set of allowed addresses, 256 remembered knocks and 4 connections per friend; the service rate-limits per address. |
| The Launcher and the Host change the friend file at once | Every change reloads the file under a lock first; state numbers never go back. |
| Name tricks | Names are cleaned and shown beside local nicknames; duplicates are flagged. |

## What this doesn't protect against

- **IP addresses.** Friends see each other's addresses, as with invite links.
- **Who meets whom.** The service sees which addresses register and knock on which
  lookup ids. A core's ids all sit in one session, so they're linked across periods by
  that session and its address, and presence checks link a friend's address to a core.
  Our Worker logs none of this (invocation logs are off), but Cloudflare's platform sees
  request metadata.
- **Who served the page.** A friend link opened in a browser runs the service's page,
  which could read it. Compare safety codes, or paste links straight into the launcher.
- **Clocks.** A core or launcher whose clock is more than 10 minutes off can't meet its
  friends; it shows as a failed join, not as a clock problem yet.
- **Local reverse proxies and tunnels.** The friend list routes treat loopback as safe.
  A request that a reverse proxy on the same computer passes on with a header that says
  so (`Forwarded`, `X-Forwarded-For`, `X-Forwarded-Host`, `X-Forwarded-Proto`,
  `X-Real-IP`, `X-Client-IP`, `Cf-Connecting-Ip`, `True-Client-IP`, `Via` and a few
  more) isn't counted as loopback. A proxy that sends none of them, or a raw TCP tunnel
  (`ssh -L`, a TCP tunnel service), still looks like loopback. Don't expose the core's
  port to others through such a tunnel without HTTPS.
- **Windows file permissions.** The core's friend certificate and list are only as
  private as its data folder: fine in the user's profile, not in a folder others can read
  (as for the core's other secrets).
- **Offline cores.** A core that hasn't received a removal yet still accepts that
  friend, but only after an accepted knock, and the launcher shows it's pending.
- **A friend who turns bad** can join what you shared with them and sees your address.
- **Networks that can't punch through** still fail (`NoPath`).

## Code

- `crates/lodestone-connect/src/friend/`: `crypto` (keys, derivation, sealing, lookups,
  X25519), `pair` (links, cards, replies), `state` (states and slots), `meet` (records,
  knocks), `host` (the core's friend host) and `join` (joining a friend's core). The
  service client's v2 calls are in `service.rs`.
- `services/connect`: the v2 routes.
- `core/src/connect/friends.rs`: the core certificate, the friend list, the friend host
  and the friend info routes; `core/src/handlers/connect.rs`: the owner-only routes.
- `launcher/crates/launcher-core/src/friend_list/`: the person key, the sealed friend
  file, pairing, states, presence and joining (`ops.rs`), and `core_sync.rs`, which
  puts the friend list on a core for the app with an owner session to it.
- Launcher: the Friends section of the Servers page, the friend deep link, and CLI
  commands. Host: sharing an instance with chosen friends, and syncing its core.

The Launcher and the Host share one friend list: the file in the Launcher's data folder
and the Keychain items `friend-identity` and `friend-list-key` (account "Lodestone
Launcher"). The Host syncs its core whenever the file changes. When the data folder
isn't the default one (`LODESTONE_LAUNCHER_DATA_DIR`, tests, dev bundles), the items
are `friend-identity@<8 hex digits>` and so on, so development never makes or uses a
person's real identity.

Tests: `crates/lodestone-connect/tests/friends.rs` pairs, publishes and joins through an
in-process service (or the real Worker with `CONNECT_TEST_SERVICE`);
`services/connect/test/friends.test.js` checks the v2 routes; the core's
`connect::tests` check the owner-only routes and the friend info routes; and
launcher-core's `friend_list::tests` check the sealed file and, with
`CONNECT_TEST_SERVICE`, the whole flow between two people.

## Later

- A DHT fallback (BEP 44) for when the service is down.
- A relay through a mutual friend's core for networks that can't punch through.
- Friends asking to start a stopped server, as a permission per friend.

## Security review

The design was reviewed before building, and the code again after. The second review
found: one address could make the service churn Durable Object calls (now diffed and
budgeted, with IPv6 counted per /64); one friend could crowd others out of a core's
allowed addresses, replay memory or connections (now per friend); the Launcher and Host
could overwrite each other's changes to the friend file (now locked reload-modify-write);
the service-supplied `seen` address was trusted (no longer used); plus a replay-window
edge, missing invisible characters and look-alikes in names, secrets in `Debug` output,
friend links to any host over plain HTTP (now only the configured service), a record cap
mismatch, cross-site use of unauthenticated reads, forwarded requests counted as
loopback, and a missing "retire my identity". All are fixed or described above.

What the first review changed:

1. A malicious page server could stand in for either side at pairing → the link carries
   a hash of the sender's key, comparing safety codes is part of the flow, and friends
   stay "Not verified" until they do.
2. The pair secret had no forward secrecy and was copied to every core → X25519 at
   pairing (`aws-lc-rs`, already in both lockfiles); cores get only per-direction core
   secrets; friend routes need the owner and HTTPS or loopback.
3. Cores couldn't be retired, and a trusted pack updated silently → signed, versioned
   states with expiry; mod trust is tied to the pack for friends' servers.
4. Metadata → described plainly above; ids rotate at a different time for each pair.
5. A removed friend could still reach the core → QUIC only after an accepted knock.
6. Cards could be replayed → cards sign the link's hash; the pair secret comes from DH.
7. Sealing gaps → roles and contexts in keys and associated data, per-direction core
   secrets, random nonces, knock ids and times.
8. The accepting side switched on early; names could be spoofed → pending until the
   first state; names cleaned; nothing shared by default; several replies shown.
9. Removal didn't reach offline cores; keys couldn't be retired → versioned lists with
   retries and "pending"; retired states.
10. One friend could crowd out others → a spray per friend.
11. A 40-bit safety code could be ground → 80 bits.
12. Stream separation → its own ALPN and socket, hello first, auth on the connection,
    checks on every stream, no token, no 0-RTT.
13. Probe tags per knock, clock skew, Windows file permissions, and the friend file
    sealed on disk.
