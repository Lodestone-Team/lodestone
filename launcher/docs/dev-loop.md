# The edit, build and run loop

How to see a change to Lodestone Launcher or Lodestone Host quickly, what each step
costs, and what was tried to make it faster.

## Quick start

`launcher/scripts/dev_loop.py` rebuilds the app when you save a file and runs it again.
It needs only Python 3 and the Rust toolchain.

```sh
python3 launcher/scripts/dev_loop.py launcher                   # the launcher, in a window
python3 launcher/scripts/dev_loop.py launcher --scene settings  # open on a prepared page
python3 launcher/scripts/dev_loop.py launcher --scene settings --png /tmp/settings.png
python3 launcher/scripts/dev_loop.py host                       # the Host, with its own core
python3 launcher/scripts/dev_loop.py host --png /tmp/host.png -- --settings general
python3 launcher/scripts/dev_loop.py launcher --once            # build, run, and stop
```

- It watches `launcher/crates`, `launcher/assets` and the shared `crates/` folder (for
  the Host, `core/` too). After a save it runs an incremental `cargo build`. If the
  build works, it stops the app and starts the new one. If the build fails, the old app
  keeps running and you see the errors.
- `--scene NAME` builds with the `screenshot` feature and opens a real window on one of
  the offscreen scenes, with made-up accounts and instances. The window opens on the
  page you are changing every time, so you don't have to click back to it.
  `crates/lodestone-launcher/src/preview.rs` lists the scenes.
- `--png FILE` renders the window offscreen to FILE after each build, and opens the
  picture once. Most image viewers then show each new version. Coding agents can add
  `--no-open` and read the file.
- Arguments after `--` go to the app: `--locale fr`, `--zoom 1.5`, or the Host's
  screenshot options (`--settings general`, `--tab console`, `--demo`, `--wait 6`).
- The apps never touch your real data. The launcher uses
  `launcher/target/dev-loop/launcher` (or `--data DIR`). The Host uses folders under
  `launcher/target/dev-loop/host` for the core's data, its own data and the launcher
  folder it reads friends from. Its core runs on port 16762 (`--core-port`), so it
  never takes 16662 from an installed Host. The core is built once at the start,
  and again when a file in `core/` changes.
- Ctrl+C stops the loop and the app.

## Where the time goes

Measured on an Apple M4 Pro (14 cores), macOS 15, Rust 1.99.1, with the default `dev`
profile. Other builds were running on the machine at the same time, so treat the
numbers as about ±20%. Each incremental number is a real edit (a number inside a
function body), then `cargo build`, three times.

| Step | Time |
| --- | --- |
| Clean debug build, launcher and Host | 74 s (60 s to 105 s, depending on load) |
| Edit in `lodestone-launcher` (`shell/storage_settings.rs`), build the launcher | 3.6 to 4.5 s |
| Edit in `lodestone-ui` (`components/checkbox.rs`), build the launcher | 4.3 to 4.5 s |
| The same edit, build the Host | 2.7 s |
| Edit in `launcher-core` (`disk.rs`), build the launcher | 7.5 to 7.9 s |
| Edit in `lodestone-host` (`pages/users.rs`), build the Host | 2.1 to 2.2 s |
| `cargo build` with nothing changed | 0.4 s |
| Link (Apple's linker), launcher or Host | 0.4 to 0.6 s |
| Launcher start to first frame, binary already started once | 0.3 s |
| Launcher start to first frame, binary just rebuilt | 1.6 s (1.2 to 1.3 s of it before `main`) |
| Save, to the launcher window open again on a scene | about 6 s |
| Save, to a new scene PNG | 10 to 11 s (build 4 to 7 s, render 4.9 s) |

What this shows:

- **The front end and code generation of the edited crates are most of it.** For an
  edit in `lodestone-launcher`, `cargo check` alone takes 1.7 s, code generation about
  1.5 s, and the link 0.5 s. An edit in `launcher-core` costs more because
  `launcher-core` (70,000 lines) and `lodestone-launcher` (86,000 lines) both build
  again.
- **Linking is already fast on macOS.** Apple's linker links the 170 MB debug launcher
  in about half a second. Debug builds on macOS already use
  `split-debuginfo = "unpacked"`, so `dsymutil` never runs.
- **A clean build** waits mostly on GPUI (22 s, after `syn` and GPUI's macros),
  `launcher-core` (19 s) and `lodestone-launcher` (19 s). `aws-lc-sys`'s build script
  (30 s) runs at the same time and is not on the critical path.
- **macOS checks every new executable once** before it starts it: the first start
  after a rebuild takes 1.2 to 1.3 s longer than the next one. A plain copy of the
  binary costs the same, and a smaller binary (no debug info for dependencies) did not
  change it. macOS skips this check for programs started from an app listed in System
  Settings › Privacy & Security › Developer Tools. Adding your terminal or editor there
  should remove that second; we didn't change this setting to measure it.
- **A scene PNG** waits 1.5 s on purpose before it renders (images and the skin
  settle), on top of the start.

## Faster data changes, no rebuild

- **Skins** are data. Settings › Skin › **Pop out editor** edits the skin in use
  while the launcher runs, and `lodestone-launcher-cli skins use <id>` switches
  skins. See [skins.md](skins.md).
- **Translations** are Rust code (`src/i18n/<language>/*.rs`), compiled into the apps.
  Loading them from files at run time would allow live changes, but it means a second
  format and a loader, for strings that change less often than layout. Not done.

## What was tried

### Profile settings: no change

Each was measured with the same edits as the table, then put back.

| Setting | Result | Why not kept |
| --- | --- | --- |
| `debug = "line-tables-only"` for all crates | launcher edit 3.4 to 3.6 s (was 3.6 to 4.5), `launcher-core` edit 6.3 to 7.1 s (was 7.5 to 7.9), Host edits no change | About 10% faster, but the debugger can't show local variables. Try it yourself with `CARGO_PROFILE_DEV_DEBUG=line-tables-only`. |
| `debug = false` for dependencies only | no change beyond noise; binary 172 MB to 167 MB; first start after a rebuild still 1.3 s | Nothing to gain. |
| `split-debuginfo = "unpacked"` | already the default on macOS | |
| Another linker on macOS | the link is 0.5 s | Nothing to gain. |

Changing the profile, the linker or the features rebuilds every crate (about 75 s),
so try settings in a separate target folder (`CARGO_TARGET_DIR`). Release builds,
`[profile.release]` and the `dev-build` profile used for developer builds are
unchanged.

### Splitting `lodestone-launcher`: not now

An edit in the launcher costs about 3 s of compiler time, spread over the crate's
front end and code generation. Moving rarely changed modules into their own crate
would help only edits to the rest, and generic code that the moved modules use would be
built in both crates. For the numbers above, it isn't worth the churn. Look at it again
if a launcher edit takes more than about 8 s.

### Hot patching with GPUI and Dioxus's `dx`: works, not adopted

GPUI at our Zed revision has a `hot-reload` feature (`gpui/src/hot_reload.rs`). With it,
a debug build connects to the development server of Dioxus's CLI, `dx serve
--hot-patch`, which uses `subsecond` to patch changed functions into the running app.
GPUI then draws every window again, and the app keeps its state: the page, the open
dialog, what you typed. The crates it adds (`subsecond`, `dioxus-cli-config`,
`dioxus-devtools-types` and their dependencies, all 0.7.10) are in Zed v1.27.0's
Cargo.lock, at Zed's versions once `const_format` (0.2.35), `const_format_proc_macros`
(0.2.34) and `memfd` (0.6.5) are held back. `dx` itself, dioxus-cli 0.7.10 (July 2026),
has no OSV advisory (checked 2026-11-10).

It was tried on the launcher, with `--scene settings`:

| Change | Hot patch: save to new code running | Rebuild and start again |
| --- | --- | --- |
| A launcher render function | 4 to 5 s (dx reports 0.9 to 1.5 s for the patch) | about 5 s, plus clicking back to the page |
| A `lodestone-ui` render function | about 9 s | about 6 s |

The first build under `dx` takes 15 to 100 s, because `dx` builds for an explicit
target into its own folder.

Why it isn't in the repository yet:

1. **It only patches the launcher with a workaround.** `dx` patches the crate that has
   `main` and the workspace crates it depends on. The launcher's `main.rs` and
   `lib.rs` are one package with one crate name, so `dx` treats edits in the library
   as edits to `main.rs` and leaves the library's code unchanged. It worked with a
   small extra binary crate whose `main.rs` is a copy of the launcher's, and which
   depends on `lodestone-launcher`.
2. **Edits in shared crates break the patch** without a second workaround. `dx` 0.7.10
   rebuilds every workspace crate that depends on the edited one, including
   `lodestone-host`, which isn't part of the launcher's build, and stops with "Missing
   rustc args for replay: 'lodestone_host'". The extra crate had to depend on
   `lodestone-host` too. `dx` also watches only the extra crate's direct dependencies.
3. **It isn't much faster here.** Most of the time is still the compiler on the edited
   crates, the same as an incremental build. The gain is keeping the app's state.
4. **It needs `dx`**, a large tool each developer builds or downloads (`cargo install
   dioxus-cli@0.7.10 --locked`, about 4 minutes on this Mac). Patches crash the app when
   a struct's fields change, and GPUI draws only windows on screen, so a window behind
   others shows the change only when it comes forward.

If we want it later, the cost is: a `hot-reload` feature on the launcher (and Host)
that turns on `gpui/hot-reload`, about 12 more crates in `launcher/Cargo.lock` (built
only with the feature), a small dev-only binary crate per app that depends on every
workspace crate, and a note on installing `dx`. Then:

```sh
cd launcher
LODESTONE_LAUNCHER_DATA_DIR=/tmp/launcher-dev dx serve --hot-patch -p <dev crate> \
  --features "hot-reload screenshot" "--args=--scene settings" --platform macos
```

A newer `dx` that limits that rebuild to the crates in the app's build would remove the
second workaround.

### Other ideas, not measured

- **Windows linking.** Microsoft's `link.exe` is much slower than Apple's linker on a
  large debug binary. `rust-lld` ships with the Rust toolchain and is usually several
  times faster. To try it in your shell (one full rebuild):
  `$env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = "rust-lld.exe"`. It isn't set in
  `.cargo/config.toml` because that would change release builds too (Cargo can't set a
  linker for one profile on stable Rust), and nobody has measured it on our Windows
  builds yet.
- **Windows Defender** scans every file a build writes. A Dev Drive, or excluding the
  repository's `target` folders, usually speeds up builds a lot.
- **sccache** caches whole dependency builds between target folders, which helps clean
  builds in new worktrees. It can't cache incremental builds of workspace crates, so
  it doesn't help the edit loop, and it is another tool to install.
- **One shared target folder for all worktrees** (`CARGO_TARGET_DIR`) saves the clean
  build too, but Cargo locks a target folder during a build, so builds in different
  worktrees wait for each other and rebuild for each other's changes. Not recommended
  while several people or agents build at once.
