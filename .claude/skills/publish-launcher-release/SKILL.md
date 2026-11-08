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
3. Uploads `lodestone-launcher-<build>-<platform>[.exe]` to the release, creating
   the release the first time.
4. Rewrites `latest.json` with this platform's entry, keeping the other
   platforms' entries, and uploads it last.
5. Deletes this platform's binaries older than the two newest.

It refuses to publish a build that isn't newer than the one already published
for this platform.

## Steps

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
   write access to Lodestone-Team/lodestone, and `cargo` must be on PATH.
4. **Dry run first:**

   ```bash
   python3 launcher/scripts/publish_release.py --dry-run
   ```

   On Windows use `python`. This builds and prints the `latest.json` it would
   upload, and uploads nothing. Show the developer the platform, build number
   and commit. The release build takes a few minutes the first time, so run it
   in the background.
5. **Publish once they confirm:**

   ```bash
   python3 launcher/scripts/publish_release.py
   ```

6. **Report** the release URL the script prints, plus its per-platform summary.
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
