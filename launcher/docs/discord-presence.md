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

## Setting up the Discord application

The status needs a Discord application: its name is what Discord shows ("Playing
Lodestone Launcher"), and it holds the images.

1. Create an application at <https://discord.com/developers/applications> and name it
   as the status should read.
2. Build with its Application ID in `LODESTONE_DISCORD_CLIENT_ID` (compiled in by
   `option_env!`, like `LODESTONE_CURSEFORGE_API_KEY`), or set the variable when
   running the launcher to try it. Without one, Settings says the build can't set the
   status, and the launcher never connects to Discord. The id isn't a secret.
3. Under Rich Presence › Art Assets, upload images with these keys (any image without
   its asset is left out):

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
