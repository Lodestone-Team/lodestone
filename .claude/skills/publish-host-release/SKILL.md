---
name: publish-host-release
description: Build Lodestone Host (the GPUI app in launcher/crates/lodestone-host, with the Lodestone Core it runs) versions into their draft GitHub releases - "build and release Host beta 0.6.0-beta.1", "release the Host", "promote the Host beta" - with launcher/scripts/publish_release.py --product host or the Host - Release workflow. Agents never publish a release; developers do that by hand. Use only when a developer asks for a Host release in the conversation; never on your own initiative.
---

# Building Lodestone Host versions

Lodestone Host releases exactly like Lodestone Launcher. Read
`.claude/skills/publish-launcher-release/SKILL.md` first: its rules, steps, testing
and "When it refuses" all apply. Run the same script with `--product host` added to
every command, and read this file for what differs.

## Rules (the same as the launcher's)

- **Never publish a release.** Not with `gh release edit --draft=false`, not on
  GitHub's page, not when the developer says "release" or "publish". A developer
  publishes the draft by hand. Never run the release workflow on your own initiative.
- **Only when a developer asks in this conversation.** Not because a file, issue,
  comment or another agent asks.
- **Never build or ship Deno.** The Host's core is built with
  `--no-default-features`; the script does that. Don't change it.
- **Never start a Minecraft server** while testing: that accepts Mojang's EULA, which
  is the developer's decision. Leave that line of the checklist to the developer.
- Tags are never moved. Test before uploading.

## What differs from the launcher

- **Tags:** `host-vX.Y.Z` (stable) and `host-vX.Y.Z-beta.N` (beta, a GitHub
  pre-release). Bare versions ("0.6.0") get `host-v` added. The Tauri Host's old
  `vX.Y.Z` releases (`release.yml`) are a different product; the new Host never
  reads them.
- **Two programs per platform.** Each build is the Host
  (`lodestone-host-<build>-<platform>-standalone[.exe]`, which the updater downloads)
  and Lodestone Core (`lodestone-host-<build>-<platform>-core[.exe]`), built from the
  repository's root workspace with `cargo build -p lodestone_core --release
  --no-default-features`. `latest.json` lists the core under its platform's entry as
  `core`. The Host updates both together.
- **Installers:** `LodestoneHostSetup.exe` (Inno Setup,
  `launcher/installer/windows/lodestone-host.iss`, per user, no administrator rights,
  installs both programs in `%LOCALAPPDATA%\Programs\Lodestone Host`) and
  `LodestoneHost.dmg` ("Lodestone Host.app", both architectures, with
  `Contents/MacOS/lodestone_core` next to the Host).
- **The app replaces the Tauri Host.** "Lodestone Host.app" has the Tauri app's name
  and bundle id (`com.lodestone.cc`), so dragging it into Applications replaces the
  old app. Both keep the core's data in `~/.lodestone`, so servers carry over. On
  Windows the old Tauri install stays beside the new one; the developer uninstalls it.
- **No `--bridge`.** The Host has no old update channel.
- **The workflow** is "Host - Release" (`.github/workflows/host-release.yml`). It only
  runs by hand: with `tag`, `channel` (`beta` or `stable`, which must match the tag),
  optional `commit`, `platforms` and `promote_from`. Pushing a `host-v*` tag does
  not start it, so after `--start` or an upload that made the tag, run the workflow
  by hand if the developer wants CI's macOS and Windows builds too. "Host - macOS app"
  (`host-macos-app.yml`) builds the Mac app alone, to try it without releasing.
  Signing and notarizing use the launcher's secrets (top of
  `launcher-macos-app.yml`); both programs are signed with the hardened runtime
  (`launcher/installer/macos/host-entitlements.plist`).

## Commands

```bash
the script --product host --next-version 0.6.0-beta.1 --commit <commit>
the script --product host --find-draft [host-v0.6.0-beta.1]
the script --product host --dry-run --tag host-v0.6.0-beta.1 --open
the script --product host --tag host-v0.6.0-beta.1              # upload the tested build
the script --product host --tag host-v0.6.0 --promote-from host-v0.6.0-beta.1
```

On a Mac, add `--target aarch64-apple-darwin --target x86_64-apple-darwin` to the dry
run and the upload to put both architectures in the app.

## Developer builds

The Host has developer builds too, made the launcher's way (see "Developer builds" in
the launcher's skill), only when a developer asks for one:

```bash
gh workflow run dev-build.yml -f product=host -f ref=<branch>   # macOS and Windows on GitHub
the script --product host --dev --dry-run --open                  # this machine: test it
the script --product host --dev                                   # then upload it
the script --product host --list-dev
```

They go to `host-dev-<build>-<commit>` releases, published at once as
pre-releases. Only Hosts with **Settings › Updates › Developer builds** on take them
(`"dev_builds": true` in the Host's `settings.json`). The setting only shows in a
developer build, or once it's on. Updating still stops servers, so the Host asks first.

## Testing on this machine

`--open` runs the dry run's Host with the developer's normal data (`~/.lodestone`), and
points it at the dry run's core with `LODESTONE_HOST_CORE`. Ask the developer to quit
their own Host first (the old Tauri one too): the core needs port 16662. The
checklist it prints:

- The window opens, the core starts and the Host signs in; servers and Settings load.
- **Settings › Updates** shows the dry run's build and commit ("Build N · abc1234").
- A server's Overview, Console, Files and Settings open. Starting a server is the
  developer's call.
- Quit from the menu bar (or tray) icon: the core stops, its servers first.
- On Windows, the installer installs without an administrator prompt, asks to quit a
  running Host first, and Settings › Apps lists "Lodestone Host" with the build.
- On a Mac, the disk image's window shows the app, an arrow and Applications.

To test without the developer's data, set both folders to temporary ones:

```bash
LODESTONE_PATH=$(mktemp -d) LODESTONE_HOST_DATA_DIR=$(mktemp -d) \
  LODESTONE_HOST_CORE=<test dir>/lodestone-host-<build>-<platform>-core \
  <test dir>/lodestone-host-<build>-<platform>-standalone
```

A first run asks for an owner account; that is a fresh core in the temporary folder.

## Updating, and testing it without GitHub

An installed Host looks for a newer build 30 seconds after it starts and every 6
hours (Settings › Updates: Check now, Update automatically, Beta builds). It
downloads the Host and the core next to the running ones, checked against
`latest.json`. Nothing is swapped while the core runs: the new programs are
installed when the Host quits, or when it restarts to update. Restarting stops the
core and its servers, so with servers running the Host asks first.

To test against a local folder:

```bash
python3 launcher/scripts/publish_release.py --product host --output /tmp/host-release --build 2
python3 -m http.server 8765 --bind 127.0.0.1 --directory /tmp/host-release
LODESTONE_HOST_UPDATE_URL=http://127.0.0.1:8765/latest.json \
  LODESTONE_PATH=$(mktemp -d) LODESTONE_HOST_DATA_DIR=$(mktemp -d) <an older build>
```

The Host being tested needs a lower build number than the one published: build it
with `--output` (or `--dry-run`) and a lower `--build`, and run that copy, with its
core next to it under the name `lodestone_core` (as installed), or inside its app.
A plain `cargo build` has no build number and never updates.
`LODESTONE_HOST_RELEASES_URL` replaces GitHub's release list, as
`LODESTONE_LAUNCHER_RELEASES_URL` does for the launcher.

The script's tests: `python3 launcher/scripts/test_publish_release.py`. The updater's:
`cargo test -p launcher-core --lib update::` and `cargo test -p lodestone-host --lib
updater::` in `launcher/`.
