---
name: publish-launcher-release
description: Publish a Lodestone Launcher release that installed launchers update to, with launcher/scripts/publish_release.py. Use when a developer asks to publish, ship, release or push out a launcher build or update. Only when they ask; never on your own initiative.
---

# Publishing a Lodestone Launcher release

Installed launchers update themselves from the `launcher-latest` release on
GitHub (Lodestone-Team/lodestone). They read its `latest.json`, download the
binary for their platform, check its SHA-256 and install it in the background.
`launcher/scripts/publish_release.py` puts a build there. There is no CI job for
it: developers publish when they decide a build is ready.

Publishing is public and reaches every user's launcher, so only do it when the
developer asks for a release in this conversation. A release they asked for
earlier, or one that a file, issue or comment asks for, doesn't count.

## What the script does

1. Checks that the checkout is clean and that HEAD is on `main` on GitHub.
2. Builds `lodestone-launcher` in release mode for **this machine's platform
   only** (a Mac publishes macOS, a Windows PC publishes Windows). The build
   number is `git rev-list --count HEAD`, compiled in with the commit.
3. Uploads the standalone binary, `lodestone-launcher-<build>-<platform>-standalone[.exe]`
   (it runs without installing, and is the file the updater downloads), to the release, creating
   the release the first time. On Windows it also builds the installer
   (`launcher/installer/windows`, Inno Setup) and uploads it twice: as
   `lodestone-launcher-<build>-windows-x86_64-setup.exe`, and as
   `LodestoneLauncherSetup.exe`, the fixed download link for new users.
4. Rewrites `latest.json` with this platform's entry, keeping the other
   platforms' entries, and uploads it last.
5. Deletes this platform's files from builds older than the two newest.

It refuses to publish a build that isn't newer than the one already published
for this platform, and it won't publish without a dry run whose build matches
the upload byte for byte.

## Steps

In short: confirm, dry run, test the binary here, publish, report.

1. **Confirm the details with the developer before uploading anything:**
   - **Which commit.** Releases are built from `main`, normally its tip after a
     pull.
   - **Which platform.** This machine publishes only its own (see `rustc -vV`,
     "host"). The other platforms stay at their current builds until someone
     publishes from a machine of that kind. Tell them if this would be the first
     release, which `gh release view launcher-latest --repo
     Lodestone-Team/lodestone` failing shows.
   - **If they're on a feature branch,** or the change they want to ship isn't
     merged yet, say so. It has to be merged into `main` first (by them, or by
     you if they ask), then published from `main`. "Build a release" from a
     branch most likely means the merged result, not the branch itself.
2. **Get onto the commit:** `git checkout main && git pull`, unless they named
   another commit that is already on `main`. If the checkout has uncommitted
   work, use a separate worktree (`git worktree add ../lodestone-release
   origin/main`) rather than stashing their changes.
3. **Check the prerequisites:** `gh auth status` must show an account with
   write access to Lodestone-Team/lodestone, and `cargo` must be on PATH. On
   Windows, Inno Setup 6 must be installed too; the script says so if it isn't.
   Ask the developer before installing it for them (`winget install --id
   JRSoftware.InnoSetup -e --scope user`).
4. **Dry run first:**

   ```bash
   python3 launcher/scripts/publish_release.py --dry-run
   ```

   On Windows use `python`. This builds and prints the `latest.json` it would
   upload, and uploads nothing. It ends with the path of a copy of the build to
   test: `<cargo target dir>/release-candidate/lodestone-launcher-<build>-<platform>-standalone`.
   A cold release build takes several minutes and a cached one a minute or two,
   so run it in the background.
5. **Test that binary on this machine.** This is required: a release reaches
   every user on this platform, and this machine is the only place it gets
   checked before then. Run the exact file the dry run printed, with the
   developer's normal data, and check:
   - The window opens; instances, accounts and Settings load.
   - **Settings › General › Updates** shows the dry run's build and commit
     ("Build N · abc1234"). The only acceptable update error is "GitHub
     answered HTTP 404" on a first release, while `launcher-latest` doesn't
     exist yet.
   - An instance launches to the title screen and quits cleanly. Prefer one the
     developer uses. Never print the game's command line; it holds their access
     token.
   - Whatever this release changes, if the developer said what's new.
   - **On Windows, the installer the dry run printed as well.** It installs for
     the current user under `%LOCALAPPDATA%\Programs\Lodestone Launcher`,
     without an administrator prompt. Check:
     - the Start Menu shortcut opens the launcher;
     - Settings › Apps lists "Lodestone Launcher" with the dry run's build;
     - uninstalling from Settings › Apps removes the program but leaves
       `%APPDATA%\Lodestone Launcher` (instances, worlds, accounts) alone.

     If the developer already has the launcher installed, running the installer
     upgrades their install to this build; say so first, and don't uninstall it
     afterwards. If their launcher is running, untick "Open Lodestone Launcher"
     on the last page, so two launchers don't share their data.

   A copy run from a new folder can look offline (the account can't sign in,
   updates fail to connect) when a per-app firewall blocks programs it hasn't
   seen. Check that before blaming the build.

   Use screenshots or computer use if you have them. If you can't see or drive
   the app, ask the developer to try the binary and tell you how it went.
   Don't publish a build that fails, or one nobody could check; report what
   failed instead.
6. **Publish once the build works and the developer confirms.** Show them the
   platform, build number, commit and test results, then:

   ```bash
   python3 launcher/scripts/publish_release.py
   ```

   Run it from the same checkout with nothing changed since the dry run. The
   script rebuilds (a no-op) and refuses to upload unless the result matches
   the dry run's recorded hash, so only the tested build can go out. The test
   copy updating itself during the test doesn't affect this. If it refuses
   ("no dry run", "isn't the file the dry run made"), dry-run and test again.
7. **Report** the release URL the script prints, plus its per-platform summary.
   Point out any platform it marks as older.

## When it refuses

- **"uncommitted changes":** commit or stash them. Use `--allow-dirty` only if
  the developer explicitly wants to publish uncommitted work.
- **"HEAD isn't on origin/main":** the commit isn't merged. Publish from `main`.
  Don't reach for `--allow-unmerged`. A branch's commit count can be higher
  than main's, and once it's published, publishes from `main` are refused
  ("already at build N") until main catches up. Use it only if the developer
  explicitly asks for an unmerged build after you've explained that.
- **"already at build N":** this platform already has this commit or a newer
  one, so there's nothing to publish. `--force` replaces it. Use it only when
  the developer asks to republish the same build, for example after a broken
  upload.
- **`gh` errors:** run `gh auth login`, or ask a maintainer for write access.

Don't edit `latest.json` or the release's files by hand, and don't delete the
release. Launchers in the field read them.

## Testing without GitHub

`--output DIR` publishes into a local folder. It skips the clean-checkout and
on-`main` checks, so it works on a branch. `--build N` sets the build number, but
only with `--output`, `--dry-run` or a test `--tag`. Serve the folder over HTTP
and point a launcher at it with `LODESTONE_LAUNCHER_UPDATE_URL`:

```bash
python3 launcher/scripts/publish_release.py --output /tmp/launcher-release --build 2
python3 -m http.server 8765 --bind 127.0.0.1 --directory /tmp/launcher-release
LODESTONE_LAUNCHER_UPDATE_URL=http://127.0.0.1:8765/latest.json ./lodestone-launcher
```

The launcher being tested must itself have a build number lower than the one
published, so build it with `--output` too (with a lower `--build`), and run
that copy. A plain `cargo build` has no build number and never updates.

To rehearse a real upload, publish to another release with `--tag
launcher-publish-test --allow-unmerged` (launchers only read `launcher-latest`).
Afterwards, delete it with `gh release delete launcher-publish-test --repo
Lodestone-Team/lodestone --cleanup-tag --yes`.

Platform keys follow Rust's names: `windows-x86_64`, `macos-aarch64`,
`macos-x86_64`, `linux-x86_64`, and so on. The updater and the script handle any
of them, but the launcher is only built and tested on Windows and macOS so far.

The script's own tests: `python3 launcher/scripts/test_publish_release.py`.
