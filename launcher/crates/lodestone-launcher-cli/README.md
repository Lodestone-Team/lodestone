# lodestone-launcher-cli

Controls a running Lodestone Launcher from the command line: instances, the game and its
log, mods and modpacks from Modrinth, worlds and backups, accounts, settings, imports,
Lodestone cores and their servers, and friends' servers shared with an invite link. It
does what the launcher's window does, through the same code, and the window shows each
change as it happens.

```sh
cargo build -p lodestone-launcher-cli        # from launcher/
lodestone-launcher-cli status
lodestone-launcher-cli instances create --version 1.21.1 --loader fabric --name Survival
lodestone-launcher-cli content install Survival sodium
lodestone-launcher-cli play launch Survival
lodestone-launcher-cli --help                # every command has examples
lodestone-launcher-cli schema                # every command and flag, as JSON
```

## How it connects

The launcher serves a Unix domain socket while it runs, `control.sock` in its data
directory (`~/Library/Application Support/Lodestone Launcher/` on macOS,
`%APPDATA%\Lodestone Launcher\` on Windows, or `LODESTONE_LAUNCHER_DATA_DIR`). A data
directory too deep for a socket path uses `/tmp/lodestone-launcher-<uid>/<hash>.sock`
(on Windows, `%TEMP%\lodestone-launcher\<hash>.sock`). Pass `--socket` or set
`LODESTONE_LAUNCHER_SOCKET` to choose another.

The launcher refuses connections from other users, and the CLI talks only to a launcher
run by its own user: on macOS and Linux the socket has mode 0600 and each side checks
the other's uid; on Windows (10 version 1803 or later, which has Unix domain sockets)
the socket inherits the data folder's permissions and each side checks the SID of the
process on the other end. Nothing it sends holds a token, key or password.

## Output

On a terminal it prints text; piped, or with `--json`, it prints one JSON document on
stdout, progress as JSON lines on stderr, and errors as
`{"error": {"code", "message", "data"}}` on stderr. Exit codes are listed in `--help`.

Destructive commands (delete, remove, replace) need `--yes`; on a terminal without it,
the CLI asks. Long commands wait with progress; `--no-wait` returns a job to follow with
`jobs wait`.

## Debug traces

`trace snapshot`, `trace topics` and `trace watch [patterns…]` read the launcher's debug
traces: clicks on its controls, the actions keys ran, dialogs, navigation, saved
settings, HTTP requests (method, host, path, status, time), the game's launch, exit and
console, jobs, and logged warnings and errors. The launcher keeps the last 2000 events in
memory; busy topics (`game.log`, `download.file`, `perf.frame`, `log.debug`) are only
sent while someone watches. Nothing is written to disk or sent anywhere, typed text is
never traced, and payloads are scrubbed of anything that looks like a password, token
or key.

## Performance

`perf` (or `perf show`) reads the launcher's memory, CPU, threads, open files, frame
rate and frame times, a running game's memory and CPU, and what the launcher holds
(decoded images, the Modrinth cache, jobs, downloads, CLI clients, the trace bus).
`perf watch` prints one reading a second. Settings → Developer → Performance monitor
shows the same in a small window.

## Access

Settings → Developer → Command line decides what the CLI may do: whether the socket is
open at all, full or read-only access, debug traces, and game logs. These settings
change only in the launcher's window. `ping` reports them; refused commands fail with
`permission_denied` (exit 9).

## For AI agents

See [SKILL.md](SKILL.md): when to use it, the commands, the JSON shapes, the exit codes,
and the safety rules (confirmation, trust in cores, the EULA, secrets).

## The protocol

Newline-delimited JSON, one object per line, version 1 (`launcher-control`):

```json
{"v":1,"id":1,"method":"instances.list","params":{}}
{"v":1,"id":1,"result":{"instances":[...]}}
{"v":1,"id":2,"error":{"code":"not_found","message":"..."}}
{"v":1,"id":3,"progress":{"id":7,"state":"running","progress":{...}}}
{"v":1,"subscription":1,"event":"log","data":{...}}
```

`lodestone-launcher-cli methods` lists the methods; the app's handlers are in
`crates/lodestone-launcher/src/control/`.
