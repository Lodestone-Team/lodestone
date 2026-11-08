# lodestone-launcher-cli

Controls a running Lodestone Launcher from the command line: instances, the game and its
log, mods and modpacks from Modrinth, worlds and backups, accounts, settings, imports,
and Lodestone cores and their servers. It does what the launcher's window does, through
the same code, and the window shows each change as it happens.

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

The launcher serves a Unix socket while it runs, `control.sock` in its data directory
(`~/Library/Application Support/Lodestone Launcher/` on macOS, or
`LODESTONE_LAUNCHER_DATA_DIR`). A data directory too deep for a socket path uses
`/tmp/lodestone-launcher-<uid>/<hash>.sock`. Pass `--socket` or set
`LODESTONE_LAUNCHER_SOCKET` to choose another.

The socket is created with mode 0600 and the launcher refuses connections from other
users (it checks the peer's uid). Nothing it sends holds a token, key or password. The
socket is Unix-only for now; on Windows the CLI exits with code 3.

## Output

On a terminal it prints text; piped, or with `--json`, it prints one JSON document on
stdout, progress as JSON lines on stderr, and errors as
`{"error": {"code", "message", "data"}}` on stderr. Exit codes are listed in `--help`.

Destructive commands (delete, remove, replace) need `--yes`; on a terminal without it,
the CLI asks. Long commands wait with progress; `--no-wait` returns a job to follow with
`jobs wait`.

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
