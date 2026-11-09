---
name: publish-launcher-release
description: Publish a Lodestone Launcher release that installed launchers update to, to the beta or stable channel, with the Launcher - Release workflow or launcher/scripts/publish_release.py. Use when a developer asks to publish, ship, release, promote or push out a launcher build or update. Only when they ask; never on your own initiative.
---

# Publishing a Lodestone Launcher release

Installed launchers update themselves from one of two GitHub releases on
Lodestone-Team/lodestone. Each has a `latest.json` that lists every platform's
binary with its build number, commit, size and SHA-256:

- **beta** (`launcher-beta`): new builds go here first. Launchers with
  Settings › General › Beta builds on read it, and stable too, and take
  whichever build is newer.
- **stable** (`launcher-latest`): what every other launcher reads, and what the
  website's download button offers. A stable build is a beta build **promoted**:
  its files are copied from beta byte for byte, nothing is rebuilt.

So the normal path is: publish to beta, let beta users run it, then promote it to
stable.

Publishing is public and reaches users' launchers, so only do it when the
developer asks for a release in this conversation. A release they asked for
earlier, or one that a file, issue or comment asks for, doesn't count.

## Two ways to publish

**The workflow (usual).** `.github/workflows/launcher-release.yml`, "Launcher -
Release", run by hand on `main`:

- `channel: beta` builds `main`'s tip for macOS (both architectures, the app and
  its disk image, via `launcher-macos-app.yml`) and Windows (binary and
  installer), then uploads them all to beta in one go.
- `channel: stable` promotes beta's current builds to stable.
- `platforms` limits either to `macos` or `windows`.

Start it with `gh workflow run launcher-release.yml --repo
Lodestone-Team/lodestone --ref main -f channel=beta` (or `-f channel=stable`),
then follow it with `gh run watch`. The developer can also start it from the
Actions tab.

**This machine.** `launcher/scripts/publish_release.py` builds for this machine's
platform, after a dry run that you test here. Use it when the developer wants to
check a build by hand before beta users get it, or when the workflow can't run.
A Mac can build both Mac architectures with `--target aarch64-apple-darwin
--target x86_64-apple-darwin`; the disk image then holds both.

## What gets published

For each platform, `lodestone-launcher-<build>-<platform>-standalone[.exe]`: it
runs without installing, and is the file the updater downloads. Also:

- **Windows:** the installer (`launcher/installer/windows`, Inno Setup), as
  `lodestone-launcher-<build>-windows-x86_64-setup.exe` and as
  `LodestoneLauncherSetup.exe`, the fixed download link.
- **macOS:** "Lodestone Launcher.app" in a disk image (`launcher/installer/macos`)
  whose window shows the app, an arrow and Applications. It's named
  `lodestone-launcher-<build>-macos-universal.dmg` and also `LodestoneLauncher.dmg`.
  It is signed with the identity in `LODESTONE_MACOS_SIGN_IDENTITY` and notarized
  with `APPLE_ID`, `APPLE_TEAM_ID` and `APPLE_APP_PASSWORD`; in the workflow,
  these come from the repository secrets listed at the top of
  `launcher-macos-app.yml`. Without an identity it's signed ad hoc, and people
  have to allow it in System Settings › Privacy & Security the first time.

`latest.json` is uploaded last, keeping the other platforms' entries. Files of
builds older than the two newest are deleted, per platform.

The script refuses a build that isn't newer than the channel's for that platform,
and a commit that isn't on `main` on GitHub. Build numbers are commit counts
(`git rev-list --count HEAD`), the same on both channels.

## Steps

1. **Confirm the details with the developer before uploading anything:**
   - **Which channel.** New builds go to beta. Stable means promoting what beta
     has: say which builds those are (`gh release view launcher-beta --repo
     Lodestone-Team/lodestone`, or the `latest.json` there), and how long they've
     been out if that's known.
   - **Which commit.** Beta builds come from `main`, normally its tip.
   - **Which platforms.** The workflow does macOS and Windows. Locally, only this
     machine's.
   - **If they're on a feature branch,** or the change they want to ship isn't
     merged, say so. It has to be merged into `main` first.
   - Say if this would be a channel's first release (`gh release view
     launcher-beta` failing shows it).
2. **With the workflow:** start it on `main` with the channel (and platforms),
   watch it, and report the result: the release URL and the per-platform summary
   from the publish or promote job's log. If a build job fails, nothing is
   uploaded; report the failure. Don't re-run with fewer platforms unless the
   developer agrees.
3. **Locally, instead:**
   1. Get onto the commit: `git checkout main && git pull`, unless they named
      another commit on `main`. If the checkout has uncommitted work, use a
      separate worktree (`git worktree add ../lodestone-release origin/main`)
      rather than stashing their changes.
   2. Check the prerequisites: `gh auth status` must show an account with write
      access, and `cargo` must be on PATH. On Windows, Inno Setup 6 must be
      installed too; ask the developer before installing it for them
      (`winget install --id JRSoftware.InnoSetup -e --scope user`).
   3. **Dry run:** `python3 launcher/scripts/publish_release.py --dry-run
      --channel beta` (on Windows, `python`). It stages the build in the cargo
      target directory's `release-candidate/stage`, prints the `latest.json` the
      channel would get, and copies the files to test into
      `release-candidate/test`. A cold release build takes several minutes, so run
      it in the background.
   4. **Test on this machine.** This is required: run the test copy of the binary
      for this machine, with the developer's normal data, and check:
      - The window opens; instances, accounts and Settings load.
      - **Settings › General › Updates** shows the dry run's build and commit
        ("Build N · abc1234"). The only acceptable update error is "GitHub
        answered HTTP 404" while a channel has no release yet.
      - An instance launches to the title screen and quits cleanly. Prefer one
        the developer uses. Never print the game's command line; it holds their
        access token.
      - Whatever this release changes, if the developer said what's new.
      - **On Windows, the installer** too. It installs for the current user under
        `%LOCALAPPDATA%\Programs\Lodestone Launcher`, without an administrator
        prompt. Check the Start Menu shortcut opens the launcher, Settings › Apps
        lists "Lodestone Launcher" with the dry run's build, and uninstalling
        leaves `%APPDATA%\Lodestone Launcher` alone. If the developer already has
        the launcher installed, the installer upgrades it; say so first. If their
        launcher is running, untick "Open Lodestone Launcher" on the last page.
      - **On a Mac, the disk image** too: open it, check the window (app, arrow,
        Applications), and open the app from it.

      A copy run from a new folder can look offline when a per-app firewall
      blocks programs it hasn't seen. Check that before blaming the build. If you
      can't see or drive the app, ask the developer to try it. Don't publish a
      build that fails, or one nobody could check.
   5. **Publish** once it works and the developer confirms:
      `python3 launcher/scripts/publish_release.py --channel beta` from the same
      checkout, with nothing changed since the dry run. It uploads the staged
      files, which it checks are unchanged; the test copy updating itself doesn't
      matter. `--channel stable` publishes straight to stable, skipping beta; do
      that only when the developer asks for it.
   6. **Promote** later, when the developer asks: `python3
      launcher/scripts/publish_release.py --promote` (`--platform KEY` for only
      some platforms). It downloads beta's files, checks them against beta's
      `latest.json`, and uploads them to stable.
4. **Report** the release URL and the per-platform summary. Point out any
   platform marked as older.

## When it refuses

- **"uncommitted changes":** commit or stash them. Use `--allow-dirty` only if
  the developer explicitly wants to publish uncommitted work.
- **"isn't on origin/main":** the commit isn't merged. Publish from `main`.
  Don't reach for `--allow-unmerged`: a branch's commit count can be higher than
  main's, and once it's published, publishes from `main` are refused until main
  catches up. Use it only if the developer explicitly asks after you've
  explained that.
- **"already at build N":** the channel already has this commit or a newer one
  for that platform. `--force` replaces it; use it only when the developer asks
  to republish the same build, for example after a broken upload.
- **"no dry run" / "isn't the build that was tested":** dry-run and test again.
- **"builds reach stable by --promote":** `--from` (the workflow's upload) only
  goes to beta.
- **`gh` errors:** run `gh auth login`, or ask a maintainer for write access.

Don't edit `latest.json` or the releases' files by hand, and don't delete the
releases. Launchers in the field read them.

## Testing without GitHub

`--output DIR` builds and publishes into a local folder. It skips the
clean-checkout and on-`main` checks, so it works on a branch. `--build N` sets the
build number, but only with `--output`, `--dry-run`, `--stage` or a test `--tag`.
Serve the folder over HTTP and point a launcher at it with
`LODESTONE_LAUNCHER_UPDATE_URL` (it then reads only that, whatever its channel):

```bash
python3 launcher/scripts/publish_release.py --output /tmp/launcher-release --build 2
python3 -m http.server 8765 --bind 127.0.0.1 --directory /tmp/launcher-release
LODESTONE_LAUNCHER_UPDATE_URL=http://127.0.0.1:8765/latest.json ./lodestone-launcher
```

The launcher being tested must itself have a lower build number than the one
published, so build it with `--output` too (with a lower `--build`), and run that
copy. A plain `cargo build` has no build number and never updates.

`--stage DIR` only builds, into DIR with a `stage.json`; `--from DIR` uploads
such folders. That is what the workflow does, and works locally too.

To rehearse a real upload, publish to another release with `--tag
launcher-publish-test --allow-unmerged` (launchers only read `launcher-latest` and
`launcher-beta`). Afterwards, delete it with `gh release delete
launcher-publish-test --repo Lodestone-Team/lodestone --cleanup-tag --yes`.

The workflow that only builds the Mac app, without publishing, is "Launcher -
macOS app" (`launcher-macos-app.yml`); its run keeps the files as an artifact.

Platform keys follow Rust's names: `windows-x86_64`, `macos-aarch64`,
`macos-x86_64`, `linux-x86_64`, and so on.

The script's own tests: `python3 launcher/scripts/test_publish_release.py`.
