# Friends: joining a friend's server without a link

Status: proposal. Nothing here is built yet.

Today a friend joins a server through an invite link that the host sends them, one link
per shared instance (Lodestone Connect, `crates/lodestone-connect`). This plan adds a
**friend list**: you add a person once, and from then on the launcher shows which of
their servers are online and joins them with one click. No link changes hands after
that, and no Lodestone server learns who is friends with whom or can impersonate
anyone.

This file covers the design, what it protects against and what it doesn't, what
changes where, and the work in phases.

## Goals

- Add a friend once, then see and join the servers they share with you, without links.
- Keep the game traffic direct and end-to-end encrypted, as Connect does today.
- Make the introduction service (connect.lodestone.cc) something nobody has to trust:
  if it is compromised it can stop introductions, but it can't read them, impersonate a
  host or a friend, or join a server itself.
- Let the host remove one friend without breaking anyone else's access.
- Keep invite links working as they do now, for people who aren't friends.

## Not goals

- **Running with no service at all.** Two computers behind home routers need a third
  party to swap addresses before they can punch through. The service stays, but as a
  blind mailbox. A DHT fallback (phase 4) can cover the case where it is down.
- **Hiding IP addresses from friends.** A direct connection shows each side's address
  to the other. That's the price of having no relay.
- **Remote control of a friend's core.** Friends can see and join. Starting, stopping,
  the console and files stay with the owner (see "What friends can do").
- **Trusting a friend's mods.** Being friends doesn't mean the launcher installs
  whatever a friend's server sends (see "Mods").

## What Connect does today

See `crates/lodestone-connect/README.md` and `services/connect/README.md`. In short:

- Each shared instance has its own identity, in the core's
  `stores/connect/<uuid>.json`: an ECDSA P-256 certificate and key, a 16-byte token,
  and a stable code at the service.
- The link is `https://connect.lodestone.cc/j/<code>#<fingerprint>.<token>`. The part
  after `#` never reaches the service.
- The service swaps candidate addresses between the host and the friend. Both sides
  probe, then the friend opens QUIC, accepting only the certificate in the link. Every
  stream starts with the token.
- The launcher keeps the servers it joined in `<data>/friends.json`, with the link's
  secret part in the Keychain (`launcher_core::cores::friends`). Mod trust is kept per
  server and tied to the certificate (`trusted_as`).

So the cryptography for "the service can't impersonate the host" already exists. What's
missing is identity for **people**, a way for the host to recognise a friend without a
shared token, and a way to find a friend's host without a code.

## The design

### 1. Identities

- **A person** has a **launcher identity**: an Ed25519 key pair, made on first use and
  kept in the Keychain (`SecretStore`). The Launcher and the GPUI Host on the same
  computer share it, so a person is one friend whichever app they use. Its public key
  is the person's id. Shown to people, it's a short **safety code** (the first 40 bits
  of its SHA-256 as 8 Crockford base32 characters).
- **A core** gets a **core identity**: one certificate and key for the whole core
  (`stores/connect/core.json`), apart from the per-instance identities that invite links
  keep using. Friends pin this certificate.
- **A person's cores** are vouched for by that person: the owner's launcher reads the
  core's certificate fingerprint over the admin API and signs
  `("lodestone friend core", core fingerprint, core name)` with the launcher key. A
  friend accepts a core only with that signature from a key they already pin.

All of this needs only `ring` (Ed25519 signing, HKDF, HMAC, ChaCha20-Poly1305), which is
already in the launcher's lockfile. There's no static Diffie–Hellman (`ring` only offers
ephemeral key agreement), so the secret two friends share is made at pairing time and
stored, not derived from their keys.

### 2. Becoming friends

Pairing goes through the service as a blind, one-time mailbox, so the two people don't
have to be online at the same time and never connect directly to pair.

1. Ana clicks **Add friend** and gets a **friend link**:
   `https://connect.lodestone.cc/f/<code>#<one-time secret>`. She sends it to Ben any
   way she likes. It expires after 24 hours and can be used once.
2. Ana's launcher posts her **card** to the mailbox `<code>`, sealed with a key derived
   from the one-time secret (HKDF, then ChaCha20-Poly1305). The card holds her display
   name, launcher public key, her vouched cores, and a random 32-byte **pair secret**,
   all signed with her launcher key.
3. Ben opens the link. His launcher fetches and opens Ana's card, checks the signature,
   and shows "Add Ana (safety code `K7Q2-M9XD`)?". If he accepts, it posts his own
   signed card (name, key, vouched cores) to the same mailbox, sealed the same way.
4. The next time Ana's launcher is online it collects Ben's card and asks Ana to confirm
   "Ben (safety code `…`) accepted your friend link." Only then is the friendship saved
   on her side, and the mailbox is deleted.

Both launchers end up with the other's public key, vouched cores and the pair secret.
The pair secret goes in the Keychain. The rest goes in a new `<data>/friend_list.json`
(`launcher_core::friends`), apart from today's `friends.json`, which keeps the servers
joined.

The one-time secret is the only thing standing between the link and a stranger, so:
the link works once and expires, Ana confirms the person who answered, and both sides
see a safety code they can compare out of band if they care to.

Each launcher then tells **its own** cores about the new friend (admin API, owner
only): the friend's public key and the pair secret. A core keeps them in
`stores/connect/friends.json`, readable only by the core's user, like the other files in
`stores/`.

### 3. Finding a friend's core

A core that shares with friends publishes one blind record at the service for each
friend:

- **Lookup id**: `HMAC-SHA256(pair secret, "lookup" | core fingerprint | UTC day)`,
  first 16 bytes. It changes every day, so the service can't link one day's records to
  the next or to a person. Around midnight both sides also try the previous day's id.
- **Record**: the core's candidate addresses, sealed with
  `HKDF(pair secret, "seal" | lookup id)` and the lookup id as associated data, so a
  record can't be moved to another id.

To join, the friend's launcher computes the lookup id for each of the friend's vouched
cores and **knocks**: it posts its own candidates, sealed the same way, and gets the
core's sealed record back. Then both sides probe exactly as today. Only the two friends
can compute the id or open either side's addresses. The service sees random ids and
opaque bytes.

**Presence** uses the same records: whether a lookup id is live tells the friend whether
that core is sharing with them now. The launcher checks while the Servers page is open
(every 2 minutes, and when the page opens), not in the background. A core shares with
friends only when its owner turned it on, and the owner can hide from chosen friends,
which simply stops their records.

New service routes (v2), next to the v1 invite routes:

| Request | Body / headers | Answer |
|---|---|---|
| `PUT /v2/meet` | `{"records": [{"lookup", "sealed"}]}` (up to 128) | `200 {"secret", "expires_at"}` |
| `GET /v2/meet/knocks` as a WebSocket | `Authorization: Bearer <secret>` | knocks for any of the host's lookups as they come |
| `POST /v2/meet/<lookup>/knocks` | `{"sealed"}` | `200 {"sealed"}`, or `404 not_found` (offline or no such friend, which look the same) |
| `HEAD /v2/meet/<lookup>` | | `200` live, `404` otherwise |
| `POST /v2/pair` | `{"sealed"}` | `201 {"code", "secret"}` |
| `GET /v2/pair/<code>` / `POST /v2/pair/<code>/reply` / `GET /v2/pair/<code>/reply` | `Authorization` for the owner's calls | the sealed cards |
| `DELETE /v2/pair/<code>` | `Authorization: Bearer <secret>` | `200` |

A core registers all its friends' records in one call and waits on one WebSocket, so
100 friends cost about what one invite costs today. Records expire like invites (2
minutes without a ping, 24 hours at most). Sealed blobs are at most 1 KiB. The rate
limits are per client address, as in v1.

### 4. Joining

Once probing finds a path, the friend opens QUIC to the core, accepting only the
**core certificate** they pinned from the vouched card. Then it proves who it is:

- A new stream kind, **friend hello** (`3`), carries the friend's launcher public key
  and an Ed25519 signature over 32 bytes of the TLS exporter
  (`export_keying_material`, label `"lodestone-connect friend"`). The signature is tied
  to this connection, so it can't be replayed on another one, and the launcher needs no
  certificate of its own.
- The core checks the key against its friends and answers with a 16-byte session token
  for this connection. Later streams start with that token, so the 17-byte stream header
  stays as it is.
- Two more kinds work only on a friend connection: **friend info** (`4`), HTTP for the
  instances shared with this friend (a list, plus each one's info and client pack, as
  `info_router` serves for one), and **friend game** (`5`), followed by the 16-byte
  instance uuid, passed to that instance's game port if it is shared with this friend.

The core closes a connection whose hello is unknown or wrong, as it does for a wrong
token today, and keeps the same limits (32 friends connected, 16 unauthenticated
connections, the same buffer caps).

In the launcher a server joined this way is a `FriendServer` with the id
`friend:<core fingerprint>/<instance uuid>`, so instances link to it as they link to a
server joined by invite (`cores::link`).

### 5. Removing a friend

- **The host removes Ben**: Ana's launcher tells her cores to drop his key and pair
  secret. They stop his records and close his connections. To Ben, Ana's servers just
  look offline. He isn't told he was removed, and nobody else is affected.
- **Ben loses his computer**: his launcher key is in the Keychain, so a stolen disk
  alone doesn't give it away. If he thinks the key is out, he resets his identity and
  sends new friend links; his friends remove the old entry. A friend whose key changes
  is a new friend: the launcher never accepts a changed key for an existing one.

### 6. What friends can do

Version one: see a friend's online servers that are shared with them (name, version,
loader, players), and join. Sharing with friends is chosen per instance, for all friends
or chosen ones, in the Launcher's hosting screen and the GPUI Host's instance Settings.

Later, as an explicit permission per friend that the owner grants: **ask to start** a
stopped server. The core shows the owner the request, or starts it at once if the owner
allowed that friend to start it. A friend never gets the console, files, settings or
the rest of the admin API: friend streams reach only the routes listed above, as invite
streams do today.

### 7. Mods

Joining installs the server's client pack, and that's the biggest risk here: a friend
whose computer or core is compromised could hand out a malicious jar. So nothing about
mod trust changes:

- Trust stays per server and tied to the certificate (`trusted_as`): until the user
  picks "Trust and install", pulling code from a server shows the advisory
  (`cores::trust::needs_advisory`). Adding a friend doesn't trust any of their servers,
  and a server joined through the friend list starts untrusted like any other.
- Worth adding alongside (not required by this plan): today a trusted server's new pack
  installs without asking again. Listing the mods a pack update adds or changes before
  installing it would limit the damage a compromised friend can do.

## What this protects against

| Attack | What happens |
|---|---|
| The service, or someone who controls it, pretends to be a friend's core | Refused: the friend accepts only the core certificate vouched for by a key they pinned when pairing. |
| The service pretends to be a friend joining | Refused: the core needs a signature from a friend's key, tied to the TLS session. |
| The service reads addresses or learns who's friends with whom | It sees daily-changing random ids and sealed bytes. It can still see two addresses meet (see below). |
| Someone squats a lookup id or replays an old record | They can't compute the id without the pair secret. A replayed record carries yesterday's addresses under yesterday's id, so it gets no answer. |
| Someone grabs a friend link before the friend opens it | They get one try in 24 hours, and the sender sees their name and safety code before confirming. |
| Someone floods a core with knocks | The service rate-limits per address, and the core probes towards at most 8 knocks at once, as it does for invites. |
| A friend of a friend tries to join | No access is passed on: a core accepts only keys its owner added. |
| A removed friend keeps trying | Their records are gone and the core refuses their key. |

## What this doesn't protect against

- **IP addresses.** Friends see each other's addresses, as with invite links.
- **The meeting itself.** When Ben knocks on Ana's lookup, the service sees Ben's
  address talk to Ana's. Nothing can hide that from the party introducing them. The
  service keeps no logs of it, as v1 doesn't.
- **Presence.** Friends can tell when your core is sharing. That's opt-in, and you can
  hide from chosen friends.
- **A friend who turns bad.** They can join what you share with them and see your
  address. Remove them.
- **Networks that can't punch through.** `NoPath` still fails, as it does today (see
  phase 4).

## What changes where

- `crates/lodestone-connect`: pairing (cards, sealing, the friend link), lookup ids,
  the v2 service client, the friend stream kinds, and friend authentication in `host`
  and `join`.
- `services/connect`: the v2 `meet` and `pair` routes. One Durable Object per core
  session holding its lookups, plus an index from lookup to session that expires with
  the records.
- `core/src/connect.rs` and `core/src/handlers/connect.rs`: the core identity, the
  friends file, publishing records, friend connections, and owner-only routes to add or
  remove friends and to choose which instances are shared with which friends.
- `launcher/crates/launcher-core`: the launcher identity, `friends` (the friend list and
  pairing), and joining by friend (next to `cores::connect`).
- `launcher/crates/lodestone-launcher`: a Friends section on the Servers page (friends,
  their online servers, Join, Add friend, Remove), the friend link in `deep_link.rs`,
  and the CLI commands in `lodestone-launcher-cli` that tests use.
- `launcher/crates/lodestone-host`: sharing an instance with friends, and the list of
  friends the core accepts.

## Phases

1. **Identity and pairing.** Launcher identity, friend links, the `pair` routes, the
   friend list file, and the Add friend / Remove flows. Testable alone: two launchers
   become friends against a local test service.
2. **Finding and joining.** Core identity, the core's friends file and owner-only
   routes, the `meet` routes, friend streams, and Join from the friend list. Invite
   links stay as they are.
3. **The interface.** The Friends section in the Launcher, sharing with friends in the
   Host, presence, and hiding from chosen friends.
4. **Later.**
   - **A DHT fallback** for when connect.lodestone.cc is down: publish the same sealed
     records as BEP 44 mutable items in the BitTorrent DHT, under a signing key derived
     from the lookup id. Off by default: lookups take seconds and it needs another
     crate outside Zed's lockfile.
   - **A relay through a mutual friend** for networks that can't punch through: a
     friend's core that both sides already trust forwards the QUIC packets, which it
     can't read.
   - **Ask to start**, as a permission per friend.
   - **Friend request from an invite**: after joining by link, offer to send the host a
     friend request through the tunnel, for the owner to accept.

## Open questions

- Should the friend list sync between a person's computers? With a Launcher on two
  machines, either both share one identity (an export and import of the key) or each is
  its own friend. Version one keeps one identity per computer.
- How many friends? 100 per person keeps one `PUT /v2/meet` call per core and the
  presence checks cheap.
- Does a friend see servers that are stopped but shared with them, so they can ask to
  start them? That depends on "ask to start".
