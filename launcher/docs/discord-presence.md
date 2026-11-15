# Discord status (Rich Presence)

While a game runs, the launcher sets the player's Discord status: the instance or
modpack, its Minecraft version and loader, the time played, and where the player is
(the menus, a single-player world with its dimension, coordinates and stats, or a
server). Settings › General › Discord turns it off or leaves parts out; coordinates
are off by default.

## Code

- `launcher-core/src/discord/`: the local connection to the Discord app (`ipc.rs`),
  where the player is from the game's log (`activity.rs`), and the open world's save
  files (`world.rs`).
- `lodestone-launcher/src/discord.rs`: follows the running games and hands them to
  one worker thread, which reads the world's files, formats the status and talks to
  Discord.

## Performance

- The window's thread only checks the start of each log line (most are passed over on
  their first byte) and compares a few fields when the instances change.
- The worker thread sleeps on its channel while no game runs or the setting is off. It
  connects to Discord only while a game runs, and retries every 5 s to 60 s while
  Discord isn't open.
- In a single-player world it checks `level.dat`'s modification time every 15 s (one
  `stat`), and two seconds after the game says it saved (as it does when paused). The
  player's data, stats and `level.dat` are read only when that changed. In
  multiplayer, nothing is read.
- The status is sent only when it changed, at most every 5 s. In a world the second
  line takes turns between place, location and stats every 20 s.

## The Discord application

The status uses Lodestone's Discord application, `1571639485601685634`
(`discord::LODESTONE_CLIENT_ID`): its name is what Discord shows ("Playing
Lodestone"), and it holds the images. The id isn't a secret. To try another
application, set `LODESTONE_DISCORD_CLIENT_ID` when building or running the launcher.

Its Rich Presence art assets have these keys (an image whose asset is missing is left
out):

| Key | Shown |
| --- | --- |
| `lodestone` | The large image, unless the instance is a Modrinth modpack (its icon shows) |
| `overworld`, `the_nether`, `the_end` | The small image in a world, in that dimension |
| `dimension` | The small image in a mod's dimension |
| `fabric`, `quilt`, `forge`, `neoforge` | The small image outside a world |

## Limits

- Single-player locations come from the save files, so they are as fresh as the last
  save: pausing saves at once; otherwise the game saves every few minutes.
- On a server the launcher knows only the address typed: no dimension, coordinates or
  stats. IP addresses and local names (`localhost`, `*.local`, Lodestone Connect
  tunnels) are never shown.
- When several games run, the status shows the one started last.

## Game invites for shared servers

While a server of this computer's core runs and is shared with a Lodestone Connect
invite, Lodestone Host can show it in the owner's Discord status, so Discord offers
its game invites: "Ask to Join" on their profile, and an "Invite to join" card they
can send in a chat. A friend who presses Join gets the invite in their Lodestone
Launcher, which opens the join dialog as a `lodestone://join` link does (joining
still needs their click on Play). Settings › Profile › Discord in the Host turns it
on; it's off by default, because the invite, key included, goes through Discord to
everyone the owner invites or lets in there.

- `launcher-core/src/discord/invite.rs`: the join secret, the Host's and the
  launcher's loops, and the command Discord starts the launcher with.
- `lodestone-host/src/discord.rs`: which server to show (the first running one
  that's shared), the status, and the "asks to join" toast with "Let them join".
- `lodestone-launcher/src/discord_join.rs`: hands the invites Discord sends to the
  join dialog, and registers the launch command.

Join only works for friends whose launcher takes Discord's invites: an older one
never connects or registers the launch command, and people without Lodestone have
nothing to open. Discord refuses buttons on a status with a join secret, so the
status can't carry the link. Instead the "asks to join" toast and Settings ›
Profile › Discord (while a server shows) have **Copy invite link**, which copies the
whole `https://connect.lodestone.cc/j/…` link to send them.

What Discord wants (checked against the Discord app in November 2026):

- `party.id` (2 to 128 characters), `party.size` (`[current, max]`, 0 people
  allowed) and `secrets.join` (at most 128 characters). Discord then marks the
  status joinable (`flags: 2`).
- No `party.privacy`. A public party sets the PARTY_PRIVACY_FRIENDS or
  PARTY_PRIVACY_VOICE_CHANNEL flag, and then Discord gives the join secret to any
  friend who asks for it, without the host letting them in. With JOIN only, viewers
  get "Ask to Join", and the secret only on an accepted request or an invite card.
- What friends receive with the status (checked from a friend's account): details,
  state, party id and size, `flags` and assets, never `secrets`. The party id is
  seen, so the Host's is a hash of the instance id (`invite::party_id`).
- Only `join` in `secrets`: another field, even null, makes invites arrive blank
  (Lachee/discord-rpc-csharp#286). Buttons together with secrets are refused
  (error 5005), so the Host's status has none.
- `SUBSCRIBE` to `ACTIVITY_JOIN` and `ACTIVITY_JOIN_REQUEST` works for the
  application without OAuth. Answers can come back out of order.
- Legcord's and Vesktop's arRPC serve the same pipes but never answer `SUBSCRIBE`
  and never send these events. The connection for events takes the app that
  confirms within a second (`Connection::open_subscribed`), and the launcher lets an
  unconfirmed one go and tries again later.
- The invite link is about 140 characters, longer than a secret may be, so the
  secret writes the same invite more tightly: `1/<service>/<code>/<fingerprint and
  token in base64url>`, about 100 characters.
- To start the launcher for a join, Discord runs the command registered for
  `discord-<application id>`: on Windows, `HKCU\Software\Classes\discord-<id>`
  (the launcher with `lodestone://discord`); on macOS,
  `~/Library/Application Support/discord/games/<id>.json` (`{"command":
  "lodestone://discord"}`, which opens the launcher through its URL scheme). Release
  builds write it as they start; development builds don't, so they leave the
  installed launcher's alone. Then the launcher connects, and Discord hands over the
  invite.

Performance: the Host does nothing while the setting is off. While it's on, it reads
the share status of running servers from the local core when they start, when the
Overview reads or changes it, and once a minute. One thread talks to Discord, only
while there's a status to show, sends it only when it changed (at most every 5 s),
and looks for "Ask to Join" requests once a second meanwhile. The launcher's thread
waits on Discord's connection without waking up, and retries every 5 s to 60 s
while Discord is closed.

Testing needs two Discord accounts: nobody sees their own invites.
`cargo test -p launcher-core live_discord -- --ignored` checks that the Discord app
on this computer confirms the subscriptions (and shows nothing).
