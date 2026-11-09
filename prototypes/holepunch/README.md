# Hole-punching prototype

Can two computers behind home routers connect **directly**, so a Minecraft server can
be shared with a link and no relay carries the game? This program tries it: UDP hole
punching, then an encrypted QUIC tunnel carrying Minecraft's TCP connection.

It's an experiment: the tickets are swapped by hand. If it works on real networks, the
next step is a tiny introduction service and doing this inside Lodestone.

## Build

Needs Rust (<https://rustup.rs>). From this folder:

```sh
cargo build --release --offline   # the binary is target/release/holepunch(.exe)
```

## Try it

On the computer with the Minecraft server (the host):

```sh
holepunch host --target 127.0.0.1:25565
```

It prints a ticket starting `lhp1|host|`. Send it to your friend, who runs:

```sh
holepunch join "lhp1|host|..."        # keep the quotes
```

That prints a ticket starting `lhp1|join|`: send it back, and paste it into the host's
prompt. Both sides then punch for up to 90 seconds. When the friend's side says
**Connected directly**, the friend adds the server `127.0.0.1:25566` in Minecraft and
joins.

No friend handy? Use a phone hotspot for one computer, so the two are on different
networks. Running both on one network may fail even though it would work across the
internet, because many routers don't route a packet back in to themselves.

## What it prints

- `... sees us as <ip>:<port>`: your public address, from two STUN servers.
- **"the STUN servers see different ports"**: this network's router is "symmetric".
  Punching usually fails through it; the connection would need a relay.
- **"couldn't get through"**: punching failed for this pair of networks.

Please note both sides' STUN lines and whether it connected: that's the data this
prototype is for.

## Security

The host's ticket holds its public address, the SHA-256 of a certificate it made for
this run, and a random token. The friend only accepts that certificate, and the host
only forwards streams that start with that token, so nobody who sees the tickets'
addresses can join or pretend to be the host. Minecraft's own login still applies.
`--local` skips STUN and uses 127.0.0.1, to try the tunnel on one computer.
