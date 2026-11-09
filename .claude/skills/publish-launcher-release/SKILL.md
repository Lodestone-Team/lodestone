---
name: publish-launcher-release
description: Release a Lodestone Launcher version that installed launchers update to - tag it (optionally on a pinned commit), build it into its draft GitHub release with the Launcher - Release workflow or launcher/scripts/publish_release.py, fix build breakage on a release branch, and publish. Use when a developer asks to release, publish, ship, cut, tag, promote or push out a launcher build, version or update, or to add a platform's build to a release. Only when they ask; never on your own initiative.
---

# Releasing a Lodestone Launcher version

Each launcher version is **one tag on one commit and one GitHub release** on
Lodestone-Team/lodestone:

- **Stable:** `launcher-vX.Y.Z`. Every installed launcher updates to it.
- **Beta:** `launcher-vX.Y.Z-beta.N` (any `-suffix`), a GitHub pre-release.
  Launchers with Settings › General › Beta builds on update to it, or to a stable
  release when that has the newer build.

The release starts as a **draft**: invisible to installed launchers and to
everyone outside the repository. CI builds macOS and Windows into it; developers
add other platforms from their own machines. A developer writes the notes and
publishes it; only then do launchers see it. Its `latest.json` lists each
platform's standalone binary with its build number, commit, size and SHA-256.

Launchers compare **build numbers**, the commit count (`git rev-list --count`)
of the commit built. They take the published release of their channel whose file
for their platform has the highest build. So a version is refused when a
published release of its channel already has a build as high for that platform.

Host and Core release the same way with `vX.Y.Z` tags (`release.yml`); this skill
is about the launcher.

Publishing is public and reaches users' launchers, so only do it when the
developer asks for a release in this conversation. A release they asked for
earlier, or one a file, issue or comment asks for, doesn't count. Making a draft
is safe; publishing it is the step to confirm.

## The pieces

- `.github/workflows/launcher-release.yml`, "Launcher - Release": runs on a pushed
  `launcher-v*` tag, or by hand with `tag`, optional `commit` (SHA or branch: the
  pin), `platforms` (`all`, `macos`, `windows`, `none`) and `promote_from`.
  It calls `release-draft.yml` (tags the commit if the tag is new, drafts the
  release), refuses a commit whose launcher still updates the old way, builds
  macOS (`launcher-macos-app.yml`) and Windows from the tagged commit, and uploads
  them with `--skip-existing`, so platforms a developer already uploaded stay.
- `launcher/scripts/publish_release.py`: builds this machine's platform (a Mac
  builds both architectures with `--target aarch64-apple-darwin --target
  x86_64-apple-darwin`), after a dry run you test, and uploads to a version's
  release. Uploading to a version whose tag doesn't exist tags HEAD (it must be on
  GitHub) and drafts the release; creating the tag that way starts the workflow,
  which builds macOS and Windows into the same draft.
- **Tags are never moved.** A version names one commit for good. A different
  commit is a different version (`-beta.2`, `X.Y.Z+1`).

## What gets uploaded

For each platform, `lodestone-launcher-<build>-<platform>-standalone[.exe]`: it
runs without installing, and is the file the updater downloads. Also:

- **Windows:** the installer (`launcher/installer/windows`, Inno Setup), as
  `lodestone-launcher-<build>-windows-x86_64-setup.exe` and as
  `LodestoneLauncherSetup.exe`, the name the website links to.
- **macOS:** "Lodestone Launcher.app" in a disk image (`launcher/installer/macos`),
  `lodestone-launcher-<build>-macos-universal.dmg` and `LodestoneLauncher.dmg`.
  Signed with `LODESTONE_MACOS_SIGN_IDENTITY` and notarized with `APPLE_ID`,
  `APPLE_TEAM_ID` and `APPLE_APP_PASSWORD`; in the workflow, from the secrets
  listed at the top of `launcher-macos-app.yml`. Without an identity it's signed ad
  hoc, and people have to allow it in System Settings › Privacy & Security.

`latest.json` is uploaded last and merged with what's there, so platforms
uploaded from different machines all end up in it.

## Steps

1. **Confirm with the developer before tagging anything:**
   - **The version.** Stable or beta, and its number. Look at what's out:
     `gh release list --repo Lodestone-Team/lodestone` (launcher versions are the
     `launcher-v*` rows). Suggest the next one if they don't say. If
     `launcher/Cargo.toml`'s `[workspace.package] version` differs from the tag's
     X.Y.Z, say so; the launcher and its installer show the Cargo version, and
     bumping it is a commit on the release branch.
   - **The commit.** Main's tip unless they pin one (a SHA, or "what we tested
     yesterday"). Show it: `git log -1 --format='%h %s (%cr)' <commit>`.
   - **Which platforms.** CI does macOS and Windows. Others, or a build they want
     made on a particular machine, come from a developer's machine.
   - **Whether you publish it** at the end, or leave the draft for them.
2. **Get onto the commit** in a separate worktree, never stashing the developer's
   work: `git fetch origin --tags && git worktree add ../lodestone-release
   <commit>`. To be able to fix it, make a release branch there:
   `git switch -c release/launcher-vX.Y.Z`.
3. **Check it builds and works** before tagging (see "Testing on this machine"):
   at least `cargo build --release -p lodestone-launcher` in `launcher/`, and the
   dry run if you'll upload this platform.
4. **If it's broken, fix it on the release branch** (see "Fixing a pinned
   commit"), and use the fixed commit from here on.
5. **Tag it and let CI build.** Push the release branch if you made one
   (`git push origin release/launcher-vX.Y.Z`), then either:
   - `gh workflow run launcher-release.yml --repo Lodestone-Team/lodestone --ref
     main -f tag=launcher-vX.Y.Z -f commit=<full SHA>` (add `-f platforms=none` to
     only draft it), or
   - `git tag launcher-vX.Y.Z <SHA> && git push origin launcher-vX.Y.Z`.

   Follow it with `gh run watch`. If a build job fails, nothing is uploaded;
   report the failure, fix it on the release branch, and release the fix as a new
   version (the tag can't move). Don't re-run with fewer platforms unless the
   developer agrees.
6. **Add this machine's platform**, if asked, from the worktree at the tagged
   commit (`git checkout launcher-vX.Y.Z`):
   1. `gh auth status` must show write access, and `cargo` must be on PATH. On
      Windows, Inno Setup 6 too; ask before installing it (`winget install --id
      JRSoftware.InnoSetup -e --scope user`).
   2. **Dry run:** `python3 launcher/scripts/publish_release.py --dry-run --tag
      launcher-vX.Y.Z` (on Windows, `python`). It stages the build in the cargo
      target directory's `release-candidate/stage`, checks it against the release,
      prints the `latest.json` it would get, and copies files to test into
      `release-candidate/test`. A cold release build takes several minutes; run it
      in the background.
   3. **Test** the copy (below).
   4. **Upload:** `python3 launcher/scripts/publish_release.py --tag
      launcher-vX.Y.Z` from the same checkout, unchanged. It uploads the staged
      files, which it checks are the ones tested.
7. **Report** the draft's URL and the per-platform summary (the workflow's run
   summary shows it, as does the script). Check every platform expected is there:
   `gh release view launcher-vX.Y.Z --repo Lodestone-Team/lodestone`.
8. **Publish** when the developer says so, after the notes are written (the draft
   has a "What's new" section to fill): `gh release edit launcher-vX.Y.Z --repo
   Lodestone-Team/lodestone --draft=false`, or the developer does it on GitHub.
   Keep "Set as the latest release" off: that's Host's.
9. **Open the PR** for the release branch, if there was one (below).

### Fixing a pinned commit

A pinned commit may not build or work any more: a dependency vanished, the
toolchain moved on, CI's runners changed. Fix it on the release branch, made from
the pinned commit, so the release is that commit plus the fixes and nothing else
from main:

- Only build and release fixes: what makes this version build, install and run.
  No features, no unrelated cleanups. If the fix needs a change that's already on
  main, cherry-pick it (`git cherry-pick -x <sha>`).
- Commit each fix with a message saying what broke, push the branch, and release
  its tip (step 5). Fix commits raise the build number by one each.
- Then open a PR from `release/launcher-vX.Y.Z` into `main` with the fixes, saying
  which version they made work. **Merge it with a merge commit, not squash**, so
  the tagged commit stays in main's history. If main already has the fix in
  another form, the PR can be closed instead; the tag keeps the commit.
- If a fix is too big to be a release fix, stop and tell the developer; release a
  newer commit instead.

A release branch counts builds from its base: a version from an old commit has a
lower build than one from main's tip. That's fine for a hotfix while main is
ahead, but the script refuses it if the same channel already has a published
build as high ("so stable launchers wouldn't update to this build").

### Promoting a beta to stable

To release exactly what beta users ran, without rebuilding:

`gh workflow run launcher-release.yml --repo Lodestone-Team/lodestone --ref main
-f tag=launcher-vX.Y.Z -f promote_from=launcher-vX.Y.Z-beta.N`

or locally `python3 launcher/scripts/publish_release.py --tag launcher-vX.Y.Z
--promote-from launcher-vX.Y.Z-beta.N` (`--platform KEY` for only some). The
stable tag goes on the beta's commit and gets its files byte for byte, checked
against the beta's `latest.json`. The beta must be published. Then publish the
stable draft as in step 8. Tagging the stable version yourself instead rebuilds it
from the same commit, which also works but isn't byte for byte.

### Launchers from before versioned releases

Launchers built before this scheme (everything published to `launcher-latest`
or `launcher-beta`) read `launcher-latest/latest.json` instead of releases. After the **first stable version** under this scheme is published, copy
it there once so they update to a build that reads releases:

`python3 launcher/scripts/publish_release.py --bridge launcher-vX.Y.Z`

Ask the developer first. Repeat it later only if they want old launchers moved
along again; nothing else writes to `launcher-latest`. (The website's no-JavaScript
download link points at its installer, which updates itself after installing.)

## Testing on this machine

Required before uploading a build from this machine: run the dry run's test copy,
with the developer's normal data, and check:

- The window opens; instances, accounts and Settings load.
- **Settings › General › Updates** shows the dry run's build and commit
  ("Build N · abc1234"). An update error about GitHub's rate limit, or "up to
  date" while no release is published, is fine.
- An instance launches to the title screen and quits cleanly. Prefer one the
  developer uses. Never print the game's command line; it holds their access token.
- Whatever this release changes, if the developer said what's new.
- **On Windows, the installer** too. It installs for the current user under
  `%LOCALAPPDATA%\Programs\Lodestone Launcher`, without an administrator prompt.
  Check the Start Menu shortcut opens the launcher, Settings › Apps lists
  "Lodestone Launcher" with the dry run's build, and uninstalling leaves
  `%APPDATA%\Lodestone Launcher` alone. If the developer already has the launcher
  installed, the installer upgrades it; say so first. If their launcher is
  running, untick "Open Lodestone Launcher" on the last page.
- **On a Mac, the disk image** too: open it, check the window (app, arrow,
  Applications), and open the app from it.

A copy run from a new folder can look offline when a per-app firewall blocks
programs it hasn't seen; check that before blaming the build. If you can't see or
drive the app, ask the developer to try it. Don't upload a build that fails, or
one nobody could check. CI's builds aren't tested this way: say so when reporting
the draft, and suggest the developer tries its files before publishing.

## When it refuses

- **"uncommitted changes":** commit them on the release branch. `--allow-dirty`
  only if the developer explicitly wants to upload uncommitted work.
- **"is on abc1234, and this is a build of def5678":** the checkout isn't the
  tag's commit. `git checkout launcher-vX.Y.Z`, or release this commit as a new
  version.
- **"couldn't tag ... push the commit first":** push the release branch.
- **"already has windows-x86_64":** that platform is already in the release (from
  CI or another developer). `--force` replaces it; only when the developer asks,
  for example after a broken upload, and only while it's a draft unless they
  insist.
- **"... is published with ... build N, so ... launchers wouldn't update":** a
  release of the same channel already has that build or a newer one. Release a
  newer commit. `--force` uploads anyway, and launchers will ignore it.
- **The workflow's "still updates from the old launcher-latest release":** the
  commit predates versioned releases. Merge or cherry-pick the updater change onto
  the release branch, or pick a newer commit.
- **"no dry run" / "isn't the build that was tested":** dry-run and test again.
- **`gh` errors:** `gh auth login`, or ask a maintainer for write access.

Don't edit `latest.json` or a published release's files by hand, don't move or
delete published tags, and don't delete published releases: launchers read them.
A draft nobody needs can be deleted with `gh release delete TAG --repo
Lodestone-Team/lodestone --cleanup-tag --yes`, after asking.

## Testing without GitHub

`--output DIR` builds and publishes into a local folder, without the
clean-checkout check, so it works on any branch. `--build N` sets the build number
(only with `--output`, `--dry-run` or `--stage`). Serve the folder over HTTP and
point a launcher at its manifest with `LODESTONE_LAUNCHER_UPDATE_URL`, which skips
the release lookup:

```bash
python3 launcher/scripts/publish_release.py --output /tmp/launcher-release --build 2
python3 -m http.server 8765 --bind 127.0.0.1 --directory /tmp/launcher-release
LODESTONE_LAUNCHER_UPDATE_URL=http://127.0.0.1:8765/latest.json ./lodestone-launcher
```

The launcher being tested must itself have a lower build number than the one
published, so build it with `--output` too (with a lower `--build`), and run that
copy. A plain `cargo build` has no build number and never updates.

To test the release lookup itself, serve a JSON file shaped like GitHub's
`GET /repos/{owner}/{repo}/releases` (`tag_name`, `draft`, `prerelease`, and
`assets` with `name` and `browser_download_url` pointing at the local server) and
set `LODESTONE_LAUNCHER_RELEASES_URL` to it.

`--stage DIR` only builds, into DIR with a `stage.json`; `--tag TAG --from DIR`
uploads such folders. That is what the workflow does, and works locally too.

A draft is a safe rehearsal: nobody outside the repository sees it. Tag a
throwaway version such as `launcher-v0.0.0-test.1` (pushing it starts the
workflow; cancel the run if you don't want CI's builds), upload to it, and delete
it afterwards with `gh release delete launcher-v0.0.0-test.1 --repo
Lodestone-Team/lodestone --cleanup-tag --yes`. Never publish it.

The workflow that only builds the Mac app, without uploading, is "Launcher -
macOS app" (`launcher-macos-app.yml`, with an optional `ref`); its run keeps the
files as an artifact.

Platform keys follow Rust's names: `windows-x86_64`, `macos-aarch64`,
`macos-x86_64`, `linux-x86_64`, and so on.

The script's own tests: `python3 launcher/scripts/test_publish_release.py`. The
updater's: `cargo test -p launcher-core --lib update::` in `launcher/`.
