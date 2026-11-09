---
name: lodestone-launcher
description: Control a running Lodestone Launcher (the Minecraft launcher) with the `lodestone-launcher-cli` command line tool. Use it to list, create and configure Minecraft instances, install and update mods, resource packs, shaders and modpacks from Modrinth, launch and stop the game and read its log, back up and restore worlds, take and restore instance snapshots and share them as .lodesnap files, manage accounts and game settings, import instances from other launchers, and join, start, stop or host servers on Lodestone cores.
---

# Lodestone Launcher CLI

`lodestone-launcher-cli` talks to the **running** Lodestone Launcher app over a local
socket. Every command does what the launcher's window does, through the same code, so
the window shows each change at once. It can't do anything while the launcher is closed:
ask the user to open it.

Use it when the user wants something done in their Minecraft launcher: "make a Fabric
1.21.1 instance with Sodium", "update my mods", "back up my world", "join our server",
"why did Minecraft crash?". For a Lodestone *Core* (the server side, with its dashboard),
use `lodestone_cli` instead.

## Setup

The launcher serves the socket while it runs; nothing to configure. The CLI finds it:

1. `--socket <path>`, or `LODESTONE_LAUNCHER_SOCKET`;
2. else `<data dir>/control.sock`, where the data directory is
   `LODESTONE_LAUNCHER_DATA_DIR` (as for the launcher) or, on macOS,
   `~/Library/Application Support/Lodestone Launcher/`;
3. a data directory deeper than ~100 characters uses
   `/tmp/lodestone-launcher-<uid>/<hash>.sock` instead (Unix socket paths are short).

Check it with `lodestone-launcher-cli ping`. Exit code 3 (`not_running`) means the
launcher isn't open, or it uses another data directory: a development launcher started
with `LODESTONE_LAUNCHER_DATA_DIR=...` needs the same variable for the CLI.

The socket is Unix-only (macOS, Linux). On Windows every command fails with exit code 3,
`unsupported_platform`.

### What the user allows

The user decides in the launcher's **Settings → Developer → Command line** what you may
do. You can't change these settings: no command does, and `settings developer set`
refuses them. `ping` and `status` report them as
`"access": {"level": "full" | "read_only", "traces": bool, "game_logs": bool}`.

- **Allow command-line connections** off: the socket is closed, so every command fails
  with exit 3 (`not_running`), as if the launcher were closed. Turning it off while you
  are connected drops the connection (a stream ends; the next command fails).
- **Access: Read only**: commands that only look (list, show, status, logs, search,
  check, `jobs wait`, `events`, `trace`) work; anything that changes the launcher, its
  instances, settings, accounts, cores or servers, or writes a file (`instances export`,
  `snapshots export`, `worlds download`) fails with exit 9, `permission_denied`, and
  `"data": {"access": "read_only", "method": "..."}`. `meta.methods` marks each method's
  `read_only`. Tell the user what you wanted to do and that read-only access is on;
  don't retry.
- **Debug traces** off: `trace ...` fails with `permission_denied`, and nothing is
  recorded.
- **Game logs** off: `play logs` and `events --topics log` fail with
  `permission_denied`; other event streams leave the game's lines out.

The user also sees each connected program (this CLI says
`lodestone-launcher-cli <version>`) and can disconnect it.

## Output and errors

- **Output is JSON when stdout isn't a terminal** (so, for you, always). `--json` forces
  it, `--human` prints text. A command prints **one JSON document on stdout**.
- **Long commands** (marked *long* below) wait for their job and print progress to
  **stderr**, one `{"progress": {...}}` object per line, then the result on stdout. With
  `--no-wait` they print `{"job": {"id": 7, "state": "running", ...}}` at once; follow it
  with `jobs wait 7`. `--timeout SECS` stops waiting (exit 8); the job goes on.
- **Streams** (`play logs --follow`, `events`) print one JSON object per line on stdout.
- **Errors** go to stderr as one line:
  `{"error": {"code": "not_found", "message": "…", "data": {…}}}`. Match on `code`,
  show the user `message`. `data` holds details: the plan of a modpack update, the
  advisory of a core's mods, a certificate's fingerprint, ambiguous `matches`.
- `lodestone-launcher-cli schema` prints every command with its arguments, method,
  whether it is long-running or needs `--yes`, and examples, as JSON (no launcher needed).
- Instances, accounts, worlds, cores and servers are named by id or by name (any case).
  An ambiguous name fails with `ambiguous` and lists `data.matches`: use the id.

| Exit | Codes | Meaning |
|---|---|---|
| 0 | | Success |
| 1 | `failed`, `internal` | The operation ran and failed; read `message` |
| 2 | `usage`, `invalid_params`, `method_not_found` | Wrong command line |
| 3 | `not_running`, `unsupported_platform` | Launcher not running / not reachable |
| 4 | `not_found`, `ambiguous` | No such thing, or the name matches several |
| 5 | `confirmation_required` | Destructive: ask the user, then add `--yes` |
| 6 | `untrusted_core`, `untrusted_certificate`, `mods_changed`, `eula_required` | Needs the user's decision |
| 7 | `busy`, `conflict` | The game or another job runs; or it conflicts |
| 8 | `timeout` | Stopped waiting; the job goes on (`jobs wait`) |
| 9 | `unavailable`, `not_signed_in`, `permission_denied`, `protocol_version` | Not available, sign in first, refused |
| 10 | `cancelled` | Cancelled |

## Safety rules

- **Destructive commands need `--yes`, and you need the user's OK first.** Without it they
  fail with exit 5 and change nothing; the message says what would happen. These are:
  `instances delete`, `worlds delete`, `worlds delete-backup`, `worlds restore --replace`,
  `content remove`, `accounts remove`, `cores remove`, `cores trust`, `shared clear`,
  `play clear-logs`, `storage clean`, `snapshots revert`, `snapshots delete`, applying `modpacks update`, a `content switch-loader` that leaves mods
  behind, and turning on `settings developer set --sign-in-as-minecraft-launcher true`.
  Deleting moves to the Trash. Never add `--yes` on your own.
- **Trust in a core is the user's decision.** Mods from a Lodestone core run as programs
  with the user's access. When a core isn't trusted for mods, installing its server's
  mods fails with `untrusted_core` and `data.advisory` (the mods, uploaded jars, scripts).
  Show the advisory to the user. Only if they agree, rerun with `--accept-advisory`
  (this once) or run `cores trust <core> --yes` (from now on).
- **Snapshot files from other people.** `snapshots import` of a `.lodesnap` with mods
  fails with `confirmation_required` and lists the mods: programs that run when the game
  starts. Run `snapshots import <file> --preview` and show the user what it holds; only if
  they trust who sent it, rerun with `--yes`.
- **Certificates:** a core whose HTTPS certificate isn't trusted fails with
  `untrusted_certificate` and `data.fingerprint`. Ask the user to compare it with the
  fingerprint the core shows (dashboard: Settings, Core Settings; or the core's log).
  **Never confirm a fingerprint yourself.** Then pass the one they confirmed with
  `--fingerprint`.
- **EULA:** launching Minecraft and hosting a server accept Mojang's EULA
  (https://aka.ms/MinecraftEULA). Ask before the first `play launch`, and `servers host`
  needs `--accept-eula`, which only the user can give.
- **No secrets.** The CLI never prints Microsoft tokens, core keys or passwords, and
  never takes a password as an argument: core passwords come on stdin
  (`--password-stdin`) or in `LODESTONE_CORE_PASSWORD`. Don't echo, log or store them.
  Never ask for the user's Microsoft password: `accounts sign-in` gives a code for the
  user to enter on Microsoft's site.
- **Only your own launcher.** The CLI refuses a socket served by another user
  (`permission_denied`) and sends nothing to it. Don't work around this with `--socket`.
- **Don't fight `busy`.** While a game runs, its instance's content, worlds and loader
  can't change. Ask the user before `play stop`: it ends their game.

## Commands

Instances:

```sh
lodestone-launcher-cli instances list                     # with effective_group (auto groups)
lodestone-launcher-cli instances show Survival
lodestone-launcher-cli instances versions --limit 10      # Minecraft releases
lodestone-launcher-cli instances loader-versions fabric 1.21.1
lodestone-launcher-cli instances create --version 1.21.1 --loader fabric --name Survival --group Friends
lodestone-launcher-cli instances rename Survival "Survival 2"
lodestone-launcher-cli instances group Survival Friends    # or --none
lodestone-launcher-cli instances prerelease-updates Survival on   # betas/alphas as updates
lodestone-launcher-cli instances icon Survival --library grass   # --image FILE, --emoji ⛏ --tile 3A6EA5, --clear
lodestone-launcher-cli instances background Survival --image ./castle.jpg
lodestone-launcher-cli instances background Survival --slideshow all       # instance | all | custom | off
lodestone-launcher-cli instances background Survival --slideshow custom --add ./a.png --add ./b.jpg
lodestone-launcher-cli instances duplicate Survival
lodestone-launcher-cli instances select Survival          # shows it in the window
lodestone-launcher-cli instances export Survival ./survival.mrpack          # long
lodestone-launcher-cli instances verify Survival                            # long; downloads missing game files
lodestone-launcher-cli instances share-code Survival --json | jq -r .code   # an LDS1:/LDS2: share code
lodestone-launcher-cli instances import-code "$CODE" --name "Friend's pack" # long; a new instance
lodestone-launcher-cli instances delete Survival --yes
```

A share code holds the Minecraft version, the loader and its version, and exact Modrinth
versions. There are two kinds:

- `LDS1:…` (a few KB for hundreds of mods): every Modrinth mod, resource pack (in
  order) and shader pack, with which are on.
- `LDS2:…` (a few hundred characters): for an instance made from a Modrinth modpack.
  It names the pack's project and version (`pack` in the JSON), then only what differs
  from it: Modrinth content added on top, the pack's files removed or turned off/on
  (`changes`), and the resource packs and shader in use. The pack's own files,
  including those bundled in it, come with the pack.

Only content that will arrive as it is goes in. `share-code` lists the rest in
`left_out`, each with a `reason_code`: `added_by_hand`, `not_on_modrinth`,
`changed_since_pack` (the recipient gets the pack's original), `curseforge` or
`unchecked` (Modrinth couldn't be asked). Tell the user about them before they share
the code. `mods`, `resource_packs` and `shader_packs` count what arrives.

`import-code` makes the instance, then installs each version; `failed` lists any that
are gone from Modrinth. An `LDS2` code is imported like the modpack (the pack is
downloaded first) and then its changes are applied; it waits like `modpacks install`.

`instances list` gives `instances` (each with `effective_group`: its own group, or with
automatic groups on, "Fabric (modded)", "Vanilla", "Imported" …), `groups` in sidebar
order, `auto_groups` and `last_played`.

Play:

```sh
lodestone-launcher-cli play launch Survival              # long: until the game runs
lodestone-launcher-cli play launch Survival --join mc.example.com:25565
lodestone-launcher-cli play status
lodestone-launcher-cli play logs Survival --lines 200
lodestone-launcher-cli play logs Survival --follow --until "Sound engine started" --timeout 300
lodestone-launcher-cli play logs Survival --follow --until-exit
lodestone-launcher-cli play stop Survival
```

`play launch` needs an account (`accounts list`). An instance linked to a server joins
it, like the Play button (see `servers join`); `--no-join` plays without joining. A crash
shows up as a `notice` event of kind `crashed` with the crash summary, and in the log.

Content (mods, resource packs, shader packs; Modrinth):

```sh
lodestone-launcher-cli content list Survival                       # --kind mod|resourcepack|shader
lodestone-launcher-cli content search sodium --instance Survival   # only what fits; --all for everything
lodestone-launcher-cli content versions sodium --instance Survival --limit 5
lodestone-launcher-cli content install Survival fabric-api          # long; with required mods
lodestone-launcher-cli content install Survival https://modrinth.com/mod/lithium
lodestone-launcher-cli content install Survival complementary-reimagined --kind shader   # adds Iris if needed
lodestone-launcher-cli content add Survival ./my-mod.jar
lodestone-launcher-cli content disable Survival sodium
lodestone-launcher-cli content enable Survival sodium
lodestone-launcher-cli content updates Survival
lodestone-launcher-cli content update Survival --all                # long
lodestone-launcher-cli content remove Survival sodium --yes
lodestone-launcher-cli content switch-loader Vanilla121             # vanilla → Fabric (or Forge before 1.14)
lodestone-launcher-cli content switch-loader Survival quilt --disable-mismatched
```

Items are named by file name, slug, title or mod id. `install` picks the newest version
that fits the instance's Minecraft version and loader; `--version` picks one, `--force`
installs one that doesn't fit.

Modpacks:

```sh
lodestone-launcher-cli modpacks search "fabulously optimized"
lodestone-launcher-cli modpacks install fabulously-optimized --name FO     # long; makes an instance
lodestone-launcher-cli modpacks install --file ./pack.mrpack
lodestone-launcher-cli modpacks check FO
lodestone-launcher-cli modpacks update FO           # exit 5 with data.plan: show it to the user
lodestone-launcher-cli modpacks update FO --yes     # long; backs up worlds first (--no-backup-worlds)
```

Worlds:

```sh
lodestone-launcher-cli worlds list Survival
lodestone-launcher-cli worlds backup Survival "New World"                   # long
lodestone-launcher-cli worlds backups Survival "New World"
lodestone-launcher-cli worlds restore Survival "New World"                  # a copy beside it
lodestone-launcher-cli worlds restore Survival "New World" --backup 2026-11-08_14-30-00.zip --replace --yes
lodestone-launcher-cli worlds delete Survival "Old World" --yes
printf '%s\n' "$PASSWORD" | lodestone-launcher-cli worlds download Survival --server Smp --username alice --password-stdin
```

Snapshots (a whole instance, or the parts you choose; files shared between snapshots are
stored once). Parts: mods, resource_packs, shader_packs, configs, worlds, screenshots,
settings, other. Logs and caches are never included.

```sh
lodestone-launcher-cli snapshots list Survival
lodestone-launcher-cli snapshots create Survival --name "Before 1.21"          # long; all but screenshots/other
lodestone-launcher-cli snapshots create Survival --include worlds --world "New World"
lodestone-launcher-cli snapshots revert Survival "Before 1.21" --yes            # long; snapshots what it replaces first
lodestone-launcher-cli snapshots revert Survival latest --include mods --include configs --yes
lodestone-launcher-cli snapshots export Survival latest ~/Desktop               # writes a .lodesnap to share
lodestone-launcher-cli snapshots import ~/Downloads/Survival.lodesnap --preview # what it holds, its mods
lodestone-launcher-cli snapshots import ~/Downloads/Survival.lodesnap --yes     # a new instance (needs --yes with mods)
lodestone-launcher-cli snapshots delete Survival "Before 1.21" --yes
lodestone-launcher-cli storage clean snapshots --yes   # automatic ones, deleted instances' ones, share files
```

Settings:

```sh
lodestone-launcher-cli settings instance get Survival
lodestone-launcher-cli settings instance set Survival --max-memory 6G --resolution 1920x1080
lodestone-launcher-cli settings instance reset Survival --key memory      # follow the default again
lodestone-launcher-cli settings defaults get
lodestone-launcher-cli settings defaults set --max-memory 4G --jvm-args "-XX:+UseG1GC"
lodestone-launcher-cli settings general set --auto-groups false
lodestone-launcher-cli settings general set --transparency glass --opacity 70   # or skin, off, clear, frosted
lodestone-launcher-cli settings developer get
lodestone-launcher-cli shared get                                 # shared game settings
lodestone-launcher-cli shared set guiScale 3
lodestone-launcher-cli shared category video --off
lodestone-launcher-cli shared sync-from Survival                  # or --none
lodestone-launcher-cli shared copy-from Survival
lodestone-launcher-cli shared instance set Survival --override guiScale --value guiScale=2
```

Settings keys: `memory`, `java`, `jvm_args`, `game_args`, `window`, `env`. Memory is
typed as in the window (`4096`, `4G`); the environment as `NAME=value NAME2="a b"`.

Accounts:

```sh
lodestone-launcher-cli accounts list            # never tokens
lodestone-launcher-cli accounts switch Steve
lodestone-launcher-cli accounts sign-in         # long; prints a code and URL for the USER
lodestone-launcher-cli accounts add-offline Tester   # development builds or LODESTONE_LAUNCHER_OFFLINE=1
lodestone-launcher-cli accounts remove Steve --yes
```

`accounts sign-in` first prints a progress line with `user_code`, `verification_uri` and
`message`: give these to the user, who signs in on Microsoft's page. It then waits (up to
the code's expiry) and prints the new account. `--no-wait` returns the code and a `job`.

Import from other launchers (Minecraft Launcher, Prism, MultiMC, Modrinth App,
ATLauncher, CurseForge, GDLauncher):

```sh
lodestone-launcher-cli import detect            # each instance has a "key"
lodestone-launcher-cli import run prism:Fabulous --without-worlds     # long
```

Cores and servers:

```sh
lodestone-launcher-cli cores list
printf '%s\n' "$PASSWORD" | lodestone-launcher-cli cores add 192.168.1.20 --username alice --password-stdin
lodestone-launcher-cli cores trust "Home core" --yes        # only when the user decides
lodestone-launcher-cli cores trust-certificate "Home core" --fingerprint AB:CD:…
lodestone-launcher-cli cores sign-out "Home core"
lodestone-launcher-cli servers list
lodestone-launcher-cli servers start Smp                     # needs this device signed in
lodestone-launcher-cli servers stop Smp
lodestone-launcher-cli servers create-instance Smp           # long; matching version, loader, mods
lodestone-launcher-cli servers join Smp                      # long; starts it first if stopped
lodestone-launcher-cli servers link Survival Smp
lodestone-launcher-cli servers unlink Survival
lodestone-launcher-cli servers resource-pack Smp --instance Survival
printf '%s\n' "$PASSWORD" | lodestone-launcher-cli servers host Survival --core "Home core" --username alice --password-stdin --accept-eula
lodestone-launcher-cli prompts list                          # questions the window is asking
lodestone-launcher-cli prompts answer update play
```

Disk space (Settings > Storage). Sizes are bytes; `totals` sums them:

```sh
lodestone-launcher-cli storage show              # instances, game files, runtimes, cache, logs
lodestone-launcher-cli storage clean game-files --yes   # versions/libraries/assets no instance uses
lodestone-launcher-cli storage clean runtimes --yes     # Java runtimes no version in use needs
lodestone-launcher-cli storage clean cache --yes
lodestone-launcher-cli storage clean leftovers --yes    # unfinished modpack downloads/imports
lodestone-launcher-cli storage clean old-logs --yes     # older than 30 days; `logs` for all
```

Game files and runtimes download again when needed; logs are deleted for good. Removing
game files, runtimes or the cache is refused (`busy`) while a game runs. To free an
instance's space, `instances delete` moves it to the Trash.

Skins (Settings > Skin), the launcher's look. A skin is a pack: `skin.json`, its
images and fonts (see `launcher/docs/skins.md`). Built-in: `default`, `grassblock`,
`glass`, `neon`.

```sh
lodestone-launcher-cli skins list                     # ids, names, which is in use
lodestone-launcher-cli skins use neon                 # `default` for the default look
lodestone-launcher-cli skins import ./Cozy.zip --use  # a .zip or a folder; checked first
lodestone-launcher-cli skins export ./mine.zip        # the one in use, or --id
lodestone-launcher-cli skins check --path ./cozy      # validate a pack and check its text, not kept
lodestone-launcher-cli skins check                    # the one in use, or an id
lodestone-launcher-cli skins fix cozy                 # apply the suggested fixes (not built-ins)
```

To make a skin, see `launcher/skills/create-skin/SKILL.md`. Readability is the
skin's job: the launcher doesn't adjust a skin's colours. `skins
check` lists each surface's text with its contrast (`ratio`), the `target` (4.5 for
text, 3 for labels and hints) and a suggested `fix` such as
`surfaces.sidebar.muted = #D9CDBE`; `passes` is true when nothing falls short. An
invalid pack gives `valid: false` and the reason.

An instance's own Play button colour and accent are in `instances show` (`play_color`,
`accent`). Start the launcher with `--no-skin` (or `LODESTONE_LAUNCHER_NO_SKIN=1`) if a
skin makes the window unusable.

Jobs and events:

```sh
lodestone-launcher-cli status                    # instances, games, jobs, accounts, cores
lodestone-launcher-cli jobs list --running
lodestone-launcher-cli jobs wait 7 --timeout 900
lodestone-launcher-cli jobs cancel 7
lodestone-launcher-cli events --topics state,notices,jobs --timeout 60
```

Debugging (what the user did and what the launcher did, without screenshots):

```sh
lodestone-launcher-cli trace snapshot                        # page, tab, instance, dialog, size, zoom; games; jobs
lodestone-launcher-cli trace topics                          # every topic, and how many events so far
lodestone-launcher-cli trace watch --backlog 100 --count 100 # the last 100 events, then live
lodestone-launcher-cli trace watch ui nav settings.change    # clicks, keys, dialogs, navigation, settings
lodestone-launcher-cli trace watch '*.error' log.warn --backlog 200
lodestone-launcher-cli trace watch game.log --backlog 0 --timeout 60   # console lines, live only
```

Performance ("the launcher is slow", "it uses a lot of memory", "the game stutters"):

```sh
lodestone-launcher-cli perf                 # one reading, taken over a second
lodestone-launcher-cli perf watch --count 30   # a reading every second, as JSON lines
```

`perf` reports the launcher's `memory_bytes` (on macOS the physical footprint, the
number Activity Monitor shows), `cpu_percent` (of **one** core, so it can pass 100 on
several cores; `cores` says how many there are), `threads` and `open_files`; `frames`
(`fps`, `avg_ms`/`p95_ms`/`max_ms` between frames while it draws, `idle` when nothing
redrew for 2 s); each running game's process (`games[].process`); decoded `images`,
the Modrinth `content_cache`, `jobs`, `downloads` (`bytes_per_sec`), `cli_clients` and
the `trace` bus (`null` while traces are off). The user can open the same numbers in
Settings → Developer → Performance monitor. Both only read, so they work with
read-only access.

## JSON shapes

An instance (`instances list`, `instances show`, `instances create`):

```json
{"id": "survival", "name": "Survival", "game_version": "1.21.1", "loader": "fabric",
 "loader_version": "0.16.14", "loader_label": "Fabric 0.16.14", "group": "Friends",
 "effective_group": "Friends", "state": "idle", "selected": true, "last_played": 1794143678,
 "play_time": 3600, "memory": {"min": 512, "max": 6144}, "server": null, "modpack": null,
 "dir": "/Users/me/Library/Application Support/Lodestone Launcher/instances/survival"}
```

`state` is `idle`, `preparing` (installing or verifying) or `running`.

A job (`--no-wait`, `jobs get`, progress lines):

```json
{"id": 3, "kind": "content", "label": "Installing sodium in Survival",
 "instance": {"id": "survival", "name": "Survival"}, "state": "running",
 "stage": null, "progress": {"label": "Downloading", "files_done": 1, "files_total": 3,
 "bytes_done": 524288, "bytes_total": 1574609, "fraction": 0.333},
 "started": 1794143683, "finished": null, "result": null, "error": null}
```

Job `state`: `running`, `succeeded`, `failed`, `cancelled`. A waited command prints the
job's `result` with `"job": <id>` added, for example `content install`:
`{"installed": [{"title": "Sodium", "file": "sodium-fabric-0.6.0.jar"}], "removed": [], "skipped": [], "job": 3}`.

A log row (`play logs`, `--follow`, `log` events):
`{"instance": "survival", "level": "INFO", "time": "14:02:11", "thread": "Render thread", "stream": "stdout", "text": "Sound engine started"}`.

Events (`events`): `{"event": "state", "data": {"instance": "survival", "state": "running", "previous": "preparing"}}`;
`event` is `log`, `state`, `progress`, `job`, `job_progress`, `notice` (`kind`:
`crashed`, `launch_failed`, `notice`, `server_failed`, …) or `lagged`.

A `perf watch` line: the same object as `perf`, with `at` (Unix seconds).

Trace events (`trace watch`): `{"seq": 412, "at_ms": 1762611727120, "topic": "ui.click", "data": {"control": "button", "id": "create-instance", "label": "Create instance"}}`.
Topics: `ui.click`/`ui.toggle`/`ui.select` (shared controls: id, label or value),
`ui.action` (a key ran an action), `ui.pointer` (where a click landed), `ui.modal`,
`ui.scope`, `ui.settings`, `ui.window`, `ui.zoom`; `nav.page`, `nav.tab`,
`nav.instance`, `nav.account`; `settings.change` (`scope`, `path`, `from`, `to`);
`game.prepare`/`launch`/`exit`/`failed`/`state`/`log`; `job.started`/`finished`;
`http.response`/`error` (method, host, path, status, ms), `core_http.*`,
`download.*`; `log.error`/`warn`/`info`; `perf.frame`. In a terminal `trace watch`
prints `14:02:07.120  ui.click  {...}` (UTC); with `--json` or a pipe, JSON lines.

## Workflows

Create a Fabric instance, add mods, launch (ask about the EULA first):

```sh
lodestone-launcher-cli instances create --version 1.21.1 --loader fabric --name Survival
lodestone-launcher-cli content install Survival fabric-api
lodestone-launcher-cli content install Survival sodium
lodestone-launcher-cli content install Survival https://modrinth.com/mod/lithium
lodestone-launcher-cli accounts list            # an active account? else ask the user to sign in
lodestone-launcher-cli play launch Survival     # waits until the game runs
lodestone-launcher-cli play logs Survival --follow --until "Sound engine started" --timeout 600
```

If `play launch` fails, read `message`, then `play logs Survival --lines 100`. A crash
after launch is a `crashed` notice: `events --topics notices --timeout 600`.

Debug something the user saw ("the button did nothing", "it froze", "the download
failed"):

```sh
lodestone-launcher-cli trace snapshot                       # where the window is now
lodestone-launcher-cli trace watch --backlog 200 --count 200 --timeout 5   # what just happened
lodestone-launcher-cli trace watch ui nav http '*.error' --backlog 0 --timeout 120   # ask the user to try again
```

Read the clicks and actions in order, then the `http.*`, `game.*` and `*.error`
events around them. Traces never hold typed text, passwords, tokens or keys, and
they stay on this machine (the socket only answers this user).

Join a server on a Lodestone core:

```sh
lodestone-launcher-cli servers list                         # find it; is an instance linked?
lodestone-launcher-cli servers create-instance Smp          # if not; may fail with untrusted_core
# untrusted_core: show data.advisory to the user; if they agree:
lodestone-launcher-cli servers create-instance Smp --accept-advisory
lodestone-launcher-cli servers join Smp                     # may fail with mods_changed
# mods_changed: ask the user, then --update-mods or --skip-update
lodestone-launcher-cli servers join Smp --update-mods
```

A plain address works too: `play launch Survival --join mc.example.com:25565`.

Back up a world (quit the game first; ask the user before stopping it):

```sh
lodestone-launcher-cli play status Survival                 # must be idle
lodestone-launcher-cli worlds list Survival
lodestone-launcher-cli worlds backup Survival "New World"
lodestone-launcher-cli worlds backups Survival "New World"  # the zip's path is in "path"
```

Update everything:

```sh
lodestone-launcher-cli content updates Survival
lodestone-launcher-cli content update Survival --all
lodestone-launcher-cli modpacks check FO && lodestone-launcher-cli modpacks update FO   # shows the plan (exit 5)
```
