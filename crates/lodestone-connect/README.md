# lodestone_connect

Lodestone Connect: a friend joins a Minecraft server shared from Lodestone Core with an
invite link. The game goes over a direct, encrypted tunnel between the two computers
(UDP hole punching, then QUIC). No relay carries it. The introduction service
(connect.lodestone.cc, `services/connect`) only swaps the two sides' addresses.

Two features:

- **`host`** (Lodestone Core): [`host::Host`] shares a server. It makes the
  certificate, keeps an invite open at the service, punches towards each friend who
  joins, and hands the streams friends open to a [`host::StreamHandler`].
- **`join`** (Lodestone Launcher): [`join::join`] connects to the host of an invite,
  and [`join::Joined`] opens streams or forwards a local TCP port into the tunnel.

The `join` feature only uses crates already in the launcher's lockfile (and Zed's).
The `host` feature adds rcgen and tokio-tungstenite.

Everything runs on Tokio (with the `net`, `time` and `io-util` drivers on).

## Joining (the launcher)

```rust
use lodestone_connect::{join::{join, JoinOptions}, Invite, StreamKind};

let invite = Invite::parse(text)?; // any of the three link forms
let joined = join(&invite, JoinOptions::default()).await?;
let server = joined.server_info().await?; // SharedServer
// Minecraft connects to this port:
let game = joined.forward(StreamKind::Game, "127.0.0.1:0".parse()?).await?;
// A core client pointed at http://<this address> reads the info routes:
let info = joined.forward(StreamKind::Info, "127.0.0.1:0".parse()?).await?;
joined.closed().await; // until the tunnel ends
```

- `join` takes about 2 to 10 seconds: it collects addresses (STUN, the router), asks
  the service for the host's, probes them, then opens QUIC.
- Errors to show people: `Error::Service { code: "not_found" | "host_offline", .. }`
  (the link ended / the host isn't sharing now), `Error::NoPath` (no way through
  these two networks), `Error::WrongHost` (the certificate doesn't match the link).
- Keep the `Joined` and the `Forward`s alive while playing; dropping them closes the
  tunnel or the port. `Joined::kind()` says how it is connected (same computer, local
  network, IPv6, internet).

## Links

| Form | Example |
|---|---|
| To share (clickable) | `https://connect.lodestone.cc/j/<code>#<fingerprint>.<token>` |
| Deep link (the invite page opens it) | `lodestone://join?invite=<the https link, percent-encoded>` |
| Prototype | `lhp2://connect.lodestone.cc/<code>#<fingerprint>.<token>` |

`<code>` is 10 characters of Crockford base32, `<fingerprint>` the SHA-256 of the host's
certificate (64 hex digits) and `<token>` 16 random bytes (32 hex digits). The part
after `#` never reaches the service. A test service on `ip:port` uses `http://`.
`Invite::parse` reads all three; `Invite::link`, `deep_link` and `lhp2` write them.

## The tunnel

1. Each side opens one UDP socket for IPv4 and IPv6 and collects candidates: its local
   network address, IPv6 address, the public address STUN servers see
   (stun.l.google.com, stun.cloudflare.com), and a port the router forwards (UPnP or
   NAT-PMP, renewed every 20 minutes, removed when sharing stops).
2. The friend posts its candidates to the service and gets the host's. The host hears
   of the join on its WebSocket.
3. Both send probes: 13 bytes, `LHP2`, an 8-byte tag (the first 8 bytes of
   SHA-256(`"lodestone-connect probe tag\0"` + token)) and a kind byte. The host sprays
   probes at the friend's candidates for a minute; the friend probes the host's and
   stops at the first packet from the host. That address is the path.
4. The friend opens QUIC (ALPN `lodestone-connect-1`, TLS 1.3) to the host on that
   path, accepting only the certificate whose SHA-256 is in the link. Keep-alives go
   every 5 seconds; a tunnel with no packets for 30 seconds closes.

## Streams

Every stream is a bidirectional QUIC stream opened by the friend. It starts with a
17-byte header:

| Bytes | Content |
|---|---|
| 0..16 | The invite's token |
| 16 | The kind: `0` game, `1` info, `2` hello |

The host closes the whole connection on a wrong token or an unknown kind, and closes a
connection that hasn't sent a stream with the right token within 20 seconds.

- **Game (0)**: the rest of the stream is Minecraft's TCP connection, passed to the
  server's game port.
- **Info (1)**: the rest of the stream is HTTP/1.1 to a few read-only routes of the
  host core, for the shared instance only:
  - `GET /api/v1/connect/info`: `SharedServer` as JSON (`core_name`, `core_uuid`,
    `instance_uuid`, `name`, `game_version`, `loader`, `loader_version`, `modpack`,
    `state`, `player_count`, `max_player_count`).
  - `GET /api/v1/public/instances`: a `PublicInstanceList` with just the shared
    instance (no address: the friend plays through the tunnel).
  - `GET /api/v1/public/instances/<uuid>/client-pack`: the instance's client modpack
    (`.mrpack`), with `ETag` and `If-None-Match`; `400` when it has none.

  Nothing else of the core's API answers. So the launcher's core client, pointed at a
  forwarded info port as `http://127.0.0.1:<port>`, reads these as it reads a core
  whose servers are listed publicly.
- **Hello (2)**: the host answers one byte, `0x4c`, and closes the stream. `join` sends
  one right after connecting, to prove it has the token and to check the host accepts
  it.

## Stable links

The host asks the service to keep its code (`"keep": true`) and saves its identity
(`HostIdentity::to_json`: certificate, key, token, code and secret). Started again with
the saved identity, it renews the same invite, so a shared link keeps working across
restarts. It renews the invite once a day and whenever the service ends it. Stopping
with `Host::stop(true)` ends the link for good. On a service without stable links the
host falls back to a new invite (and a new link) when one ends.

## Testing

```sh
cargo test -p lodestone_connect --all-features     # unit tests and tests/loopback.rs
```

`tests/loopback.rs` runs a host and a friend on this computer through an in-process
stand-in for the service, with a TCP echo server as the game port.

To try the real service, share a TCP port with the example and join it:

```sh
cargo run -p lodestone_connect --all-features --example connect -- host --game 127.0.0.1:25565
cargo run -p lodestone_connect --all-features --example connect -- join "<link>" --check hello
```

With `npm run dev` in `services/connect`, add `--via http://127.0.0.1:8787 --loopback`
to both.
