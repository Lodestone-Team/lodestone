# Logs

Lodestone Launcher and Lodestone Host each write a log file, so a failure can be looked
into after the fact: an app opened from Finder or the Start menu has no terminal, and
before these files a panic's message went to stderr and was lost. The code is
`launcher-core`'s `logging` module, a logger on the `log` facade both apps already use.

## Where

| App | Folder | Files |
| --- | --- | --- |
| Launcher | the default data folder's `logs/`: `~/Library/Application Support/Lodestone Launcher/logs` on macOS, `%APPDATA%\Lodestone Launcher\logs` on Windows | `launcher.log`, `launcher.log.1` … `.5`, `last-crash.txt`, `previous-crash.txt` |
| Host | the Host's folder's `logs/` (`LODESTONE_HOST_DATA_DIR`, else `Lodestone Host` or `Lodestone Host Dev` in the app data folder) | `host.log`, `host.log.1` … `.5`, `last-crash.txt`, `previous-crash.txt`, and the core's own `core.log` |

The Launcher's logs stay in the default data folder when the data folder is moved
(Settings › Storage), so they are found even while a moved folder's drive isn't
connected. Moving the default folder on the same drive is still one rename: the log is
closed for it (Windows won't rename a folder with an open file in it), its lines wait in
memory, and it reopens in the default folder. `LODESTONE_LAUNCHER_DATA_DIR` replaces the default folder, as for everything
else. Settings › Developer › Diagnostics › **Open logs folder** (the Launcher) and
Settings › Updates › Logs › Log files › **Open logs folder** (the Host) show the folder.
On macOS and Linux the folder is the user's alone (0700) and its files too (0600),
wherever it is, when the drive allows it (a network drive that refuses to change
permissions still gets the log). Moving the log or its folder away (to the Trash, to
start afresh) is noticed within a few seconds, and a new file is opened in its place. Two Launchers on one data folder (a dev build beside the installed app)
share the files: their lines interleave and the files grow past 5 MB, but each removes
only its own run's mark.

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
  jobs that failed, the Host's core starting, exiting and restarting, every error
  shown in a toast, and the Launcher's failure dialogs (a modpack import or update
  that failed, files a game still has open, a copy that couldn't be made);
- core replies the client can't read (the request, the type and serde's error, never
  the body), failed sign-ins and skin changes, the Host's instance list failing (once
  per change);
- failures on the debug trace bus (`http.error`, `core_http.error`, `game.failed`,
  `job.finished` that failed, with the job's kind and instance, `game.exit` that
  crashed, `images.*` warnings, `storage.move` errors, responses with a 5xx status), as
  `lodestone::trace` records, whether Debug traces are on or not; with traces off only
  topics that can report a failure are looked at (`trace::may_fail`), so clicks and
  navigation cost nothing;
- a panic's report (below).

A failure that repeats is written a few times a minute, so it doesn't rotate the useful
lines away; the next one after the minute says how many were left out ("and N more like
it before"). A request that couldn't reach its host is written once per outage ("more
failed requests to … aren't logged until it answers", then "… answers again"): a saved
core that is off is polled every few seconds, and that is normal. A host that fails and
answers by turns (flaky Wi-Fi) gets one such pair a minute at most. Game file downloads
are logged by the downloader: single attempts at debug, a file it gave up on at warn.

Failure dialogs and toasts are logged as they are shown, so their text is in the app's
language; the errors under them, logged where they are decided, are in English.

## Levels

The apps' own crates (`lodestone_*`, `launcher_*`) log at **info**, everything else
(GPUI, HTTP, TLS) at **warn**. **Log detail** (Settings › Developer in the Launcher,
Settings › Updates in the Host) switches the apps' own crates to **debug**, at once.
`LODESTONE_LOG`, written like `RUST_LOG` (`info,launcher_core::cores=debug,gpui=off`),
replaces both while the app runs with it; the setting then says so and is disabled.
Even then, the network crates that print whole requests at trace (tungstenite, hyper,
h2, reqwest, rustls) stop at debug, so a log made with `LODESTONE_LOG=trace` can still
be sent.

## Rotation

At 5 MB a file becomes `.1`, `.1` becomes `.2`, and so on; `.5` is dropped. The file is
moved aside (`.rotating`) before the older ones move up, so when another program holds
it (Windows), the history is kept and the rotation is tried again after another 1 MB. Lines are
buffered: warnings and errors are flushed at once, the rest within a second, and a
panic's report before the process carries on. A forced quit can lose up to a second of
info lines. A message is cut at 16 KB (the line says how much was cut).

## Crashes

The panic hook writes a panic's message, place, thread and a backtrace to the log and to
`last-crash.txt` (a later panic of the same run, such as GPUI's "panic in a function
that cannot unwind", is added to it). A run's first panic moves the report it replaces
to `previous-crash.txt`, so a crash the start just reported isn't lost to a panic the
new run survived. A message is cut at 16 KB, and only a run's first three panics carry
a backtrace (a panic someone catches, around a library a LAN device can upset, is
reported too). The app marks a run in `logs/session.json` and
removes the mark when it quits cleanly; at the next start, a mark still there whose run
`last-crash.txt` names means a crash nobody saw end, and the app says so once: a notice
in the Launcher's top bar (beside the update notice when both fit, in its place when
they don't, and down to its dot and **Open logs** in the narrowest window: it is never
held back, since the run's mark is already replaced), a toast in the Host (once its window is first shown, so a
Host started hidden at sign-in still says it), both with **Open logs**. A panic the app
survived followed by a clean quit shows nothing, and neither does a forced quit without
a panic; a forced quit after a panic the app survived is reported as a crash.
Release builds strip symbols, so their backtraces are mostly addresses; the message,
place, thread and version are still there.

## Secrets

Every line goes through `lodestone_trace::scrub_line`, once: tokens (JWTs, `lsk_` keys,
Microsoft tokens, long mixed strings, base64 runs with `+` and `/` like Xbox tokens)
wherever they are in a line, `Bearer`/`Basic` credentials, secret query parameters
(also when percent-encoded JSON holds one), a URL's password, a core's download key
(`/file/…`), invite and friend links, `name=value`, `name: value` and JSON pairs named
like secrets (quoted values whole, numbers too; names in any case, `privateKey`,
`pass`, `pwd`, `passphrase`), every cookie after `Cookie:`, the value after
`--accessToken` and other secret flags, and everything after `Authorization:`.
Scrubbing a scrubbed line changes nothing. A download that fails leaves its key out of
the error, and a core reply the client can't read leaves the value serde quotes out of
the error a toast shows, not only out of the log. Control characters are written as escapes (`\u{1b}`), so a
core's error message can't drive a terminal that shows the log. The same scrubbed text
goes to the trace bus's `log.*` events. The scrubber is a heuristic, a last line of
defence: the callers leave secrets out in the first place, logging ids, names and
errors, never frames, bodies, headers, full URLs or the game's command line.

## Sending them

The logs leave out tokens and passwords, but they hold Minecraft account names, server
and core names and addresses, friends' server names and folder paths (with the
computer's user name): send them privately, or look them over before posting them in a
public issue.

Open the logs folder and send `launcher.log` (and `launcher.log.1` if the problem was a
while ago) with `last-crash.txt` if there is one; for the Host, `host.log` and the
core's `core.log`. From a terminal, `lodestone-launcher-cli logs path` prints the folder
and its log files (no running launcher needed) and
`lodestone-launcher-cli logs tail -n 200 --level warn` the last lines, from the running
launcher or, when none answers (it crashed), from the file itself; the control API has
`logs.path` and `logs.tail`, open to any client command-line access lets in.
