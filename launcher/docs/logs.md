# Logs

Lodestone Launcher and Lodestone Host each write a log file, so a failure can be looked
into after the fact: an app opened from Finder or the Start menu has no terminal, and
before these files a panic's message went to stderr and was lost. The code is
`launcher-core`'s `logging` module, a logger on the `log` facade both apps already use.

## Where

| App | Folder | Files |
| --- | --- | --- |
| Launcher | the default data folder's `logs/`: `~/Library/Application Support/Lodestone Launcher/logs` on macOS, `%APPDATA%\Lodestone Launcher\logs` on Windows | `launcher.log`, `launcher.log.1` … `.5`, `last-crash.txt` |
| Host | the Host's folder's `logs/` (`LODESTONE_HOST_DATA_DIR`, else `Lodestone Host` or `Lodestone Host Dev` in the app data folder) | `host.log`, `host.log.1` … `.5`, `last-crash.txt`, and the core's own `core.log` |

The Launcher's logs stay in the default data folder when the data folder is moved
(Settings › Storage), so they are found even while a moved folder's drive isn't
connected. `LODESTONE_LAUNCHER_DATA_DIR` replaces the default folder, as for everything
else. Settings › Developer › Diagnostics › **Open logs folder** (the Launcher) and
Settings › Updates › Logs › **Open the log folder** (the Host) show the folder.

Tests and the offscreen scenes never write to the real folders: the scenes log to their
own `LODESTONE_LAUNCHER_DATA_DIR`/`LODESTONE_HOST_DATA_DIR` when one is set, and
otherwise only to the terminal.

## What

One line per record, in UTC:

```
2026-11-16T08:30:12.345Z WARN  launcher_core::cores::live: the Events stream for "abc" ended (…); reconnecting in 2 s
```

A message's later lines are indented two spaces. What goes in:

- the start (version, build, pid, run id) and a clean quit;
- warnings and errors from the apps and from launcher-core where a failure is decided:
  event frames the client can't read (the serde error, with the values it quotes left
  out, never the frame), stream drops and reconnects, update checks, downloads and
  installs, downloads that failed for good (host and path), game launches that failed
  (`game.failed`), Connect tunnel failures, failed snapshot and world backup work,
  jobs that failed, the Host's core starting, exiting and restarting, and every error
  shown in a toast;
- failures on the debug trace bus (`http.error`, `core_http.error`, `game.failed`,
  `job.finished` that failed, `game.exit` that crashed, `images.*` warnings,
  `storage.move` errors, responses with a 5xx status), as `lodestone::trace` records,
  whether Debug traces are on or not;
- a panic's report (below).

A failure that repeats is written a few times a minute, then summed up ("and N more
like it"), so it doesn't rotate the useful lines away.

## Levels

The apps' own crates (`lodestone_*`, `launcher_*`) log at **info**, everything else
(GPUI, HTTP, TLS) at **warn**. **Log detail** (Settings › Developer in the Launcher,
Settings › Updates in the Host) switches the apps' own crates to **debug**, at once.
`LODESTONE_LOG`, written like `RUST_LOG` (`info,launcher_core::cores=debug,gpui=off`),
replaces both while the app runs with it; the setting then says so and is disabled.

## Rotation

At 5 MB a file becomes `.1`, `.1` becomes `.2`, and so on; `.5` is dropped. Lines are
buffered: warnings and errors are flushed at once, the rest within a second, and a
panic's report before the process carries on. A forced quit can lose up to a second of
info lines.

## Crashes

The panic hook writes a panic's message, place, thread and a backtrace to the log and to
`last-crash.txt` (a later panic of the same run, such as GPUI's "panic in a function
that cannot unwind", is added to it). The app marks a run in `logs/session.json` and
removes the mark when it quits cleanly; at the next start, a mark still there whose run
`last-crash.txt` names means a crash nobody saw end, and the app says so once: a notice
in the Launcher's top bar, a toast in the Host, both with **Open logs**. A panic the app
survived followed by a clean quit shows nothing, and neither does a forced quit.
Release builds strip symbols, so their backtraces are mostly addresses; the message,
place, thread and version are still there.

## Secrets

Every line goes through `lodestone_trace::scrub_line`: tokens (JWTs, `lsk_` keys,
Microsoft tokens, long mixed strings), `Bearer`/`Basic` credentials, secret query
parameters, `name=value`, `name: value` and JSON pairs named like secrets, the word
after `--accessToken` and other secret flags, and everything after `Authorization:`. The
callers leave secrets out in the first place: they log ids, names and errors, never
frames, bodies, headers, full URLs or the game's command line.

## Sending them

Open the logs folder and send `launcher.log` (and `launcher.log.1` if the problem was a
while ago) with `last-crash.txt` if there is one; for the Host, `host.log` and the
core's `core.log`. From a terminal, `lodestone-launcher-cli logs path` prints the folder
(no running launcher needed) and `lodestone-launcher-cli logs tail -n 200 --level warn`
the last lines; the control API has `logs.path` and `logs.tail` (refused while
Settings › Developer › Debug traces is off).
