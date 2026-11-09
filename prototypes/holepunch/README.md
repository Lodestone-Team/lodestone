# Hole-punching prototype

Can two computers behind home routers connect **directly**, so a Minecraft server can
be shared with a link and no relay carries the game? This program tries it: UDP hole
punching, then an encrypted QUIC tunnel carrying Minecraft's TCP connection.

Version 2 tries several ways in, as Tailscale does: local network and IPv6 addresses,
the public address STUN servers see, and a port the router forwards on request (UPnP or
NAT-PMP, removed again on exit). Both sides probe every address of the other, and use
the first one that answers.

It's an experiment: the tickets are swapped by hand. If it works on real networks, the
next step is a tiny introduction service and doing this inside Lodestone.

## Build

Needs Rust (<https://rustup.rs>). From this folder:

```sh
cargo build --release --locked    # the binary is target/release/holepunch(.exe)
```

On a cloud server (for example AWS EC2) as the other side: keep its firewall's inbound
rules closed apart from SSH. Opening UDP would connect without punching, and the test
would prove nothing. A cloud server's NAT is much friendlier than a home router's, so a
success there doesn't promise one between two homes.

No Minecraft on that side? Check the tunnel with `nc`: `nc -l 25565` on the host, then
`nc 127.0.0.1 25566` on the joining side once it says Connected, and type a line.

## With the introduction service (version 3)

Instead of swapping tickets, the host registers with the introduction service, which
only passes addresses along (never game traffic), and gets an invite link to send to any
number of friends. It's `services/connect` in this repository (protocol v1, on Cloudflare
Workers at connect.lodestone.cc), over HTTPS:

```sh
holepunch host --target 127.0.0.1:25565 --via https://connect.lodestone.cc
holepunch join "lhp2://connect.lodestone.cc/<code>#<fingerprint>.<token>"
```

`holepunch rendezvous --listen 0.0.0.0:8080` runs the same protocol locally, over plain
HTTP (`--via <ip>:8080`), for tests; so does `npm run dev` in `services/connect`
(`--via 127.0.0.1:8787`, loopback addresses allowed). The host waits for joins on a
WebSocket, and ends its invite when you press Ctrl-C.

The part after `#` (the host's certificate fingerprint and the token) is never sent to
the service, so the service can't pretend to be the host or join itself.

## Try it (tickets by hand)

On the computer with the Minecraft server (the host):

```sh
holepunch host --target 127.0.0.1:25565
```

It prints a ticket starting `lhp2|host|`. Send it to your friend, who runs:

```sh
holepunch join "lhp2|host|..."        # keep the quotes
```

That prints a ticket starting `lhp2|join|`: send it back, and paste it into the host's
prompt. Both sides then probe for up to two minutes. When the friend's side says
**Connected directly**, the friend adds the server `127.0.0.1:25566` in Minecraft and
joins.

No friend handy? Use a phone hotspot for one computer, so the two are on different
networks. Running both on one network may fail even though it would work across the
internet, because many routers don't route a packet back in to themselves.

## What it prints

- `... sees us as <ip>:<port>`: your public address, from two STUN servers.
- **"the STUN servers see different ports"**: this network's router is "symmetric".
  Punching usually fails through it; the connection would need a relay.
- **"heard a probe from …" / "… answered our probe"**: packets get through that way.
- **"their probes arrived, but ours never reached them"**: only one direction works.
- **"nothing arrived from the other side"**: no path between these two networks; they'd
  need a relay.
- **"router forwarding: not available"**: UPnP and NAT-PMP are off on the router (common).
  Turning UPnP on in the router's settings often makes this work.

Both sides must run the same version: version 2 tickets start with `lhp2`.

Please note both sides' STUN lines and whether it connected: that's the data this
prototype is for.

## Security

The host's ticket holds its public address, the SHA-256 of a certificate it made for
this run, and a random token. The friend only accepts that certificate, and the host
only forwards streams that start with that token, so nobody who sees the tickets'
addresses can join or pretend to be the host. Minecraft's own login still applies.
`--local` skips STUN and uses 127.0.0.1, to try the tunnel on one computer.
