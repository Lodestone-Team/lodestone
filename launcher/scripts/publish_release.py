#!/usr/bin/env python3
"""Builds Lodestone Launcher versions and uploads them to their GitHub releases.

Each version is a `launcher-vX.Y.Z` tag on one commit (`launcher-vX.Y.Z-beta.N`
for a beta) and a GitHub release of its own. The release starts as a draft:
CI (.github/workflows/launcher-release.yml) builds macOS and Windows into it,
developers add other platforms from their own machines with this script, and a
developer publishes it by hand. Its `latest.json` lists each platform's
standalone binary with its build number, commit, size and SHA-256.

Installed launchers (launcher/crates/launcher-core/src/update.rs) list the
repository's releases, take the published `launcher-v*` one with the newest build
for their platform (pre-releases too, with "Beta builds" on), and download the
binary its `latest.json` lists. Drafts are invisible to them.

Staging builds the files into a folder: the standalone binary for each platform
(the file the updater downloads), the Windows installer, and on macOS the app, in
a disk image. Uploading puts staged folders on a version's release and merges
their platforms into its `latest.json`. On a developer's machine a dry run stages
and the upload follows once the build is tested; in GitHub Actions each platform
stages on a runner of its own and one job uploads them all.

A build's number is the number of commits in its history (`git rev-list --count
HEAD`). Launchers update to a higher number, so a version is refused when a
published release of its channel already has a build as high for that platform.
A version can come from any commit on GitHub: main's tip, an older commit, or a
fix on a release branch made from one.

Usage, from a checkout of the version's commit (`git checkout launcher-v0.2.0`):

    python3 launcher/scripts/publish_release.py --dry-run --tag launcher-v0.2.0
                                        # stage this machine's build to test
    python3 launcher/scripts/publish_release.py --tag launcher-v0.2.0
                                        # upload the tested build to the draft
    python3 launcher/scripts/publish_release.py --tag launcher-v0.2.0 --promote-from launcher-v0.2.0-beta.3
                                        # a stable version from a beta's files
    python3 launcher/scripts/publish_release.py --output DIR --build 2
                                        # local test, no GitHub
    # In GitHub Actions:
    python3 launcher/scripts/publish_release.py --stage DIR [--target TRIPLE ...]
    python3 launcher/scripts/publish_release.py --tag TAG --from DIR [--from DIR ...]

Uploading to a version whose tag doesn't exist yet tags this commit (it must be on
GitHub) and drafts the release; pushing a tag starts the release workflow, which
builds macOS and Windows into the same draft.

`--target` builds for a Rust target other than this machine's (a Mac builds both
`aarch64-apple-darwin` and `x86_64-apple-darwin`, and the app holds both).

On macOS the binaries, the app and the disk image are signed with the identity in
LODESTONE_MACOS_SIGN_IDENTITY (a "Developer ID Application" certificate), or
ad hoc without one. With APPLE_ID, APPLE_TEAM_ID and APPLE_APP_PASSWORD set too,
the disk image is notarized and stapled.

Needs Python 3.9+, the Rust toolchain, and the GitHub CLI (`gh`) signed in with
write access to the repository; on Windows also Inno Setup 6 (`winget install
--id JRSoftware.InnoSetup -e --scope user`). Uses only the standard library.
"""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = "Lodestone-Team/lodestone"
MANIFEST = "latest.json"
# A launcher version's tag: launcher-v0.2.0, launcher-v0.2.0-beta.1.
TAG_PREFIX = "launcher-v"
TAG = re.compile(r"^launcher-v(\d+\.\d+\.\d+)(-[0-9A-Za-z][0-9A-Za-z.-]*)?$")
# The release launchers read before they read versioned releases. Only `--bridge`
# writes to it, so launchers that old update to one that reads versioned releases.
LEGACY_TAG = "launcher-latest"
# Builds kept per platform where several share a release (the legacy one, and test
# folders): the newest and the one before it, so a launcher that started
# downloading just before an upload can finish.
KEEP = 2

LAUNCHER = Path(__file__).resolve().parent.parent
ROOT = LAUNCHER.parent
# Release files: the standalone binary (`-standalone`), the Windows installer
# (`-setup.exe`) and the macOS disk image (`.dmg`, `macos-universal` when it holds
# both architectures). Builds published before the standalone suffix have none.
ASSET_NAME = re.compile(
    r"^lodestone-launcher-(\d+)-([a-z0-9]+-[a-z0-9_]+)(-standalone|-setup)?(\.exe|\.dmg)?$"
)
INSTALLER_SCRIPT = LAUNCHER / "installer" / "windows" / "lodestone-launcher.iss"
MACOS = LAUNCHER / "installer" / "macos"
# Each release's installer and disk image under names that never change, so the
# website can link to them without knowing the build.
STABLE_INSTALLER = "LodestoneLauncherSetup.exe"
STABLE_DMG = "LodestoneLauncher.dmg"
APP_NAME = "Lodestone Launcher"
BUNDLE_ID = "cc.lodestone.launcher"
SIGN_IDENTITY_ENV = "LODESTONE_MACOS_SIGN_IDENTITY"
NOTARY_ENV = ("APPLE_ID", "APPLE_TEAM_ID", "APPLE_APP_PASSWORD")
# What a staged folder holds, beside its files.
STAGE_RECORD = "stage.json"


class Failure(Exception):
    pass


def run(cmd: list[str], cwd: Path = ROOT, env: dict | None = None, quiet: bool = True,
        shown: str | None = None) -> str:
    """Runs a command; returns its output, or raises Failure with its error.

    `shown` replaces the command line in errors, for commands that hold a secret."""
    try:
        result = subprocess.run(
            cmd,
            cwd=cwd,
            env=env,
            text=True,
            encoding="utf-8",
            errors="replace",
            stdout=subprocess.PIPE if quiet else None,
            stderr=subprocess.PIPE if quiet else None,
        )
    except FileNotFoundError:
        raise Failure(f"`{cmd[0]}` isn't installed or isn't on PATH")
    if result.returncode != 0:
        detail = (result.stderr or result.stdout or "").strip() if quiet else ""
        raise Failure(f"`{shown or ' '.join(cmd)}` failed" + (f":\n{detail}" if detail else ""))
    return (result.stdout or "").strip()


def step(message: str) -> None:
    print(f"==> {message}", flush=True)


# ---------------------------------------------------------------------------
# What is being published


def host_target() -> str:
    for line in run(["rustc", "-vV"], cwd=LAUNCHER).splitlines():
        if line.startswith("host:"):
            return line.split(":", 1)[1].strip()
    raise Failure("`rustc -vV` doesn't say which target this machine is")


def key_for_target(triple: str) -> str:
    """The manifest key for a Rust target: `windows-x86_64`, `macos-aarch64`.

    It matches `update::platform()` in the launcher, which is
    `std::env::consts::{OS, ARCH}` of the build.
    """
    arch = triple.split("-", 1)[0]
    if "apple-darwin" in triple:
        os_name = "macos"
    elif "windows" in triple:
        os_name = "windows"
    elif "linux" in triple:
        os_name = "linux"
    else:
        raise Failure(f"don't know the platform of the Rust target {triple!r}")
    if arch not in ("x86_64", "aarch64"):
        raise Failure(f"don't know the architecture of the Rust target {triple!r}")
    return f"{os_name}-{arch}"


def workspace_version() -> str:
    text = (LAUNCHER / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r'^\[workspace\.package\][^\[]*?^version\s*=\s*"([^"]+)"', text, re.M | re.S)
    return match.group(1) if match else ""


def check_clean() -> None:
    if run(["git", "status", "--porcelain", "--untracked-files=no"]):
        raise Failure(
            "the checkout has uncommitted changes; commit or stash them, or pass --allow-dirty"
        )


def check_tag(tag: str) -> None:
    if not TAG.match(tag):
        raise Failure(
            f"{tag!r} isn't a launcher version: use launcher-vX.Y.Z, or launcher-vX.Y.Z-beta.N "
            "(or another -suffix) for a beta"
        )


def is_prerelease_tag(tag: str) -> bool:
    """A version with a -suffix is a pre-release: launchers with Beta builds on take it."""
    match = TAG.match(tag)
    return bool(match and match.group(2))


def tag_version_note(tag: str, version: str) -> str | None:
    """A note when the tag's version isn't the workspace's (which the build shows)."""
    match = TAG.match(tag)
    if match and version and match.group(1) != version.split("-", 1)[0]:
        return (f"{tag} is version {match.group(1)}, but launcher/Cargo.toml says {version}, "
                "which the launcher and its installer show. Bump [workspace.package] version "
                "on the release branch to match, if it should.")
    return None


# ---------------------------------------------------------------------------
# The manifest


def asset_name(build: int, key: str) -> str:
    """The standalone binary: runs from anywhere, without installing, and updates itself.
    It's the file `latest.json` points the updater at, installed or not."""
    exe = ".exe" if key.startswith("windows-") else ""
    return f"lodestone-launcher-{build}-{key}-standalone{exe}"


def installer_name(build: int, key: str) -> str:
    return f"lodestone-launcher-{build}-{key}-setup.exe"


def dmg_name(build: int, keys: list[str]) -> str:
    """The disk image: `macos-universal` when the app holds more than one architecture."""
    key = keys[0] if len(keys) == 1 else "macos-universal"
    return f"lodestone-launcher-{build}-{key}.dmg"


def has_installer(key: str) -> bool:
    """Platforms with an installer. The installer script is for x64 Windows."""
    return key == "windows-x86_64"


def alias_for(name: str) -> str | None:
    """The never-changing name a file is also uploaded under, if any."""
    if name.endswith("-setup.exe"):
        return STABLE_INSTALLER
    if name.endswith(".dmg"):
        return STABLE_DMG
    return None


def updated_manifest(old: dict | None, key: str, entry: dict, version: str, published: str,
                     tag: str | None = None) -> dict:
    """`old` with this platform's entry replaced; the top-level build is the newest."""
    assets: dict = {}
    if old:
        for name, asset in old.get("assets", {}).items():
            asset = dict(asset)
            # Written before assets had their own build: they share the manifest's.
            asset.setdefault("build", old.get("build"))
            asset.setdefault("commit", old.get("commit"))
            assets[name] = asset
    assets[key] = entry
    newest = max(assets.values(), key=lambda asset: asset.get("build") or 0)
    manifest = {
        "build": newest["build"],
        "commit": newest["commit"],
        "version": version if newest is entry else (old or {}).get("version", version),
        "published": published,
        "assets": dict(sorted(assets.items())),
    }
    tag = tag or (old or {}).get("tag")
    if tag:
        manifest["tag"] = tag
    return manifest


def file_key(name: str) -> str | None:
    match = ASSET_NAME.match(name)
    return match.group(2) if match else None


def file_build(name: str, key: str) -> int | None:
    """The build of `name` if it's this platform's standalone binary."""
    match = ASSET_NAME.match(name)
    if match and match.group(2) == key and match.group(3) != "-setup" and match.group(4) != ".dmg":
        return int(match.group(1))
    return None


def stale_names(names: list[str], key: str, keep_builds: set[int]) -> list[str]:
    """This platform's files other than the newest KEEP builds and `keep_builds`."""
    mine = []
    for name in names:
        match = ASSET_NAME.match(name)
        if match and match.group(2) == key:
            mine.append((int(match.group(1)), name))
    # Count builds, not files: a Windows build has its binary and its installer.
    newest = set(sorted({build for build, _ in mine}, reverse=True)[:KEEP]) | keep_builds
    return [name for build, name in mine if build not in newest]


def newest_published(releases: list[dict], key: str, prerelease: bool,
                     exclude: str) -> tuple[int, str] | None:
    """The highest build for `key` in the published launcher releases that launchers
    reading a release like this one also read, and its tag.

    Stable launchers read stable releases only; beta launchers read both, so a beta
    competes with every release. `releases` are GitHub's release objects."""
    best = None
    for release in releases:
        tag = release.get("tag_name", "")
        if (not tag.startswith(TAG_PREFIX) or tag == exclude or release.get("draft")
                or (release.get("prerelease") and not prerelease)):
            continue
        for asset in release.get("assets", []):
            build = file_build(asset.get("name", ""), key)
            if build is not None and (best is None or build > best[0]):
                best = (build, tag)
    return best


def summary(manifest: dict, build: int) -> list[str]:
    lines = []
    for key, asset in manifest["assets"].items():
        line = f"  {key}: build {asset['build']} ({str(asset.get('commit') or '')[:7]})"
        if asset["build"] < build:
            line += " - older than the others"
        lines.append(line)
    return lines


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def describe(path: Path, build: int, commit: str) -> dict:
    return {
        "name": path.name,
        "size": path.stat().st_size,
        "sha256": sha256(path),
        "build": build,
        "commit": commit,
    }


# ---------------------------------------------------------------------------
# Building


def build_binary(target: str | None, build: int, commit: str, key: str, into: Path) -> Path:
    """Builds the launcher for `target` (None: this machine); returns a copy named for upload."""
    env = dict(os.environ, LODESTONE_LAUNCHER_BUILD=str(build), LODESTONE_LAUNCHER_COMMIT=commit)
    cmd = ["cargo", "build", "--release", "--locked", "-p", "lodestone-launcher"]
    if target:
        cmd += ["--target", target]
    run(cmd, cwd=LAUNCHER, env=env, quiet=False)
    metadata = json.loads(run(["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=LAUNCHER))
    out = Path(metadata["target_directory"])
    if target:
        out = out / target
    exe = out / "release" / (
        "lodestone-launcher.exe" if key.startswith("windows-") else "lodestone-launcher"
    )
    if not exe.is_file():
        raise Failure(f"the build finished but {exe} is missing")
    copy = into / asset_name(build, key)
    shutil.copyfile(exe, copy)
    if not key.startswith("windows-"):
        copy.chmod(0o755)
    return copy


def cargo_target_dir() -> Path:
    metadata = json.loads(run(["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=LAUNCHER))
    return Path(metadata["target_directory"])


def find_iscc() -> Path:
    """Inno Setup's compiler: $ISCC, on PATH, or where winget and its installer put it."""
    candidates = [os.environ.get("ISCC"), shutil.which("iscc")]
    for variable in ("LOCALAPPDATA", "ProgramFiles(x86)", "ProgramFiles"):
        base = os.environ.get(variable)
        if base:
            programs = Path(base) / "Programs" if variable == "LOCALAPPDATA" else Path(base)
            candidates.append(str(programs / "Inno Setup 6" / "ISCC.exe"))
    for candidate in candidates:
        if candidate and Path(candidate).is_file():
            return Path(candidate)
    raise Failure(
        "Inno Setup isn't installed, and Windows releases include an installer. Install it with "
        "`winget install --id JRSoftware.InnoSetup -e --scope user`, or pass --no-installer"
    )


def build_installer(exe: Path, build: int, version: str, key: str, into: Path) -> Path:
    """Wraps `exe` in the Windows installer; returns the installer."""
    name = installer_name(build, key)
    run([str(find_iscc()), "/Q", f"/DSourceExe={exe}", f"/DBuild={build}",
         f"/DVersion={version}", f"/DOutputDir={into}", f"/DOutputName={Path(name).stem}",
         str(INSTALLER_SCRIPT)], cwd=INSTALLER_SCRIPT.parent)
    installer = into / name
    if not installer.is_file():
        raise Failure(f"Inno Setup finished but {installer} is missing")
    return installer


# ---------------------------------------------------------------------------
# macOS: signing, the app and its disk image


def sign_identity() -> str:
    """The signing identity, or `-` to sign ad hoc."""
    return os.environ.get(SIGN_IDENTITY_ENV, "").strip() or "-"


def codesign(path: Path, identity: str, identifier: str | None = None) -> None:
    """Signs `path`. A real identity also gets the hardened runtime and a secure
    timestamp, which notarization requires."""
    cmd = ["codesign", "--force", "--sign", identity]
    if identifier:
        cmd += ["--identifier", identifier]
    if identity != "-":
        cmd += ["--options", "runtime", "--timestamp",
                "--entitlements", str(MACOS / "entitlements.plist")]
    run(cmd + [str(path)])


def info_plist(build: int, version: str) -> dict:
    return {
        "CFBundleDevelopmentRegion": "en",
        "CFBundleDisplayName": APP_NAME,
        "CFBundleExecutable": "lodestone-launcher",
        "CFBundleIconFile": "AppIcon",
        "CFBundleIdentifier": BUNDLE_ID,
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundleName": APP_NAME,
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": version or "0.0.0",
        "CFBundleVersion": str(build),
        "LSApplicationCategoryType": "public.app-category.games",
        # GPUI's oldest macOS, as Zed's.
        "LSMinimumSystemVersion": "10.15.7",
        "NSHighResolutionCapable": True,
        "NSSupportsAutomaticGraphicsSwitching": True,
        # The game runs as the launcher's child, so macOS asks on the launcher's behalf.
        "NSMicrophoneUsageDescription":
            "Minecraft and its mods can use your microphone, for example for voice chat.",
        "NSLocalNetworkUsageDescription":
            "Lodestone Launcher finds Minecraft worlds opened to LAN on your network.",
    }


def build_app(binaries: list[Path], build: int, version: str, identity: str, into: Path) -> Path:
    """`Lodestone Launcher.app` around the (already signed) binaries, one per architecture.

    With more than one, the app's executable is a universal binary of them all. An
    update later replaces it with the standalone binary for the Mac's own architecture.
    """
    app = into / f"{APP_NAME}.app"
    if app.exists():
        shutil.rmtree(app)
    (app / "Contents" / "MacOS").mkdir(parents=True)
    (app / "Contents" / "Resources").mkdir()
    exe = app / "Contents" / "MacOS" / "lodestone-launcher"
    if len(binaries) == 1:
        shutil.copyfile(binaries[0], exe)
    else:
        run(["lipo", "-create", *map(str, binaries), "-output", str(exe)])
    exe.chmod(0o755)
    shutil.copyfile(MACOS / "AppIcon.icns", app / "Contents" / "Resources" / "AppIcon.icns")
    with (app / "Contents" / "Info.plist").open("wb") as file:
        plistlib.dump(info_plist(build, version), file)
    (app / "Contents" / "PkgInfo").write_text("APPL????", encoding="ascii")
    codesign(app, identity)
    run(["codesign", "--verify", "--deep", "--strict", str(app)])
    return app


# The disk image's window: its size, and where Finder puts the app and the link to
# Applications, over the arrow in dmg-background.svg.
DMG_WINDOW = (640, 400)
DMG_ICONS = {"app": (170, 190), "Applications": (470, 190)}
DMG_TITLE_BAR = 28


def dmg_layout_script(volume: str, app_name: str) -> str:
    """AppleScript that has Finder lay out the mounted disk image's window."""
    width, height = DMG_WINDOW
    (ax, ay), (bx, by) = DMG_ICONS["app"], DMG_ICONS["Applications"]
    return f'''
tell application "Finder"
  tell disk "{volume}"
    open
    set current view of container window to icon view
    set toolbar visible of container window to false
    set statusbar visible of container window to false
    set pathbar visible of container window to false
    -- The bounds include the title bar, which is {DMG_TITLE_BAR} points tall.
    set the bounds of container window to {{200, 120, {200 + width}, {120 + height + DMG_TITLE_BAR}}}
    set options to the icon view options of container window
    set arrangement of options to not arranged
    set icon size of options to 128
    set text size of options to 13
    set background picture of options to file ".background:background.tiff"
    set position of item "{app_name}" of container window to {{{ax}, {ay}}}
    set position of item "Applications" of container window to {{{bx}, {by}}}
    update without registering applications
    delay 1
    close
  end tell
end tell
'''


def lay_out_dmg(volume_path: Path, app_name: str) -> None:
    """Gives the mounted image its background and icon positions, which Finder
    keeps in the volume's .DS_Store. Cosmetic: a failure only warns."""
    background = volume_path / ".background"
    background.mkdir()
    # One TIFF with both resolutions, so the background is sharp on Retina screens.
    run(["tiffutil", "-cathidpicheck", str(MACOS / "dmg-background.png"),
         str(MACOS / "dmg-background@2x.png"), "-out", str(background / "background.tiff")])
    try:
        run(["osascript", "-e", dmg_layout_script(volume_path.name, app_name)])
    except Failure as err:
        print(f"    warning: the disk image keeps Finder's plain window: {err}")
    # Finder writes .DS_Store a moment after the window closes.
    for _ in range(20):
        if (volume_path / ".DS_Store").exists():
            break
        subprocess.run(["sleep", "0.5"])


def build_dmg(app: Path, name: str, identity: str, into: Path) -> Path:
    """A disk image holding the app and a link to Applications, to drag it onto,
    in a window with a background that says so."""
    with tempfile.TemporaryDirectory() as tmp:
        folder = Path(tmp) / APP_NAME
        folder.mkdir()
        # `ditto` keeps the signature's extended attributes; copytree may not.
        run(["ditto", str(app), str(folder / app.name)])
        (folder / "Applications").symlink_to("/Applications")
        # Made writable first, so Finder can lay out its window, then compressed.
        writable = Path(tmp) / "writable.dmg"
        run(["hdiutil", "create", "-volname", APP_NAME, "-srcfolder", str(folder),
             "-fs", "HFS+", "-format", "UDRW", "-size", "400m", "-ov", str(writable)])
        attached = run(["hdiutil", "attach", "-readwrite", "-noverify", "-noautoopen", str(writable)])
        device, volume = None, None
        for line in attached.splitlines():
            parts = line.split("\t")
            if parts[0].strip().startswith("/dev/") and device is None:
                device = parts[0].strip()
            if len(parts) >= 3 and parts[-1].strip().startswith("/Volumes/"):
                volume = Path(parts[-1].strip())
        if device is None or volume is None:
            raise Failure(f"couldn't tell where hdiutil mounted the disk image:\n{attached}")
        try:
            lay_out_dmg(volume, app.name)
        finally:
            run(["sync"])
            run(["hdiutil", "detach", device])
        dmg = into / name
        run(["hdiutil", "convert", str(writable), "-format", "UDZO",
             "-imagekey", "zlib-level=9", "-ov", "-o", str(dmg)])
    if identity != "-":
        run(["codesign", "--force", "--sign", identity, "--timestamp", str(dmg)])
    return dmg


def notarize(dmg: Path) -> bool:
    """Notarizes and staples the disk image, when the Apple account is configured."""
    values = [os.environ.get(name, "").strip() for name in NOTARY_ENV]
    if not all(values):
        return False
    apple_id, team_id, password = values
    step(f"Notarizing {dmg.name} (this waits for Apple, often a few minutes)")
    run(["xcrun", "notarytool", "submit", str(dmg), "--apple-id", apple_id, "--team-id", team_id,
         "--password", password, "--wait"],
        shown=f"xcrun notarytool submit {dmg.name} --wait")
    run(["xcrun", "stapler", "staple", str(dmg)])
    return True


# ---------------------------------------------------------------------------
# Staging: a folder of finished files and a record of them


def stage(targets: list[str | None], build: int, commit: str, version: str, into: Path,
          installer: bool = True) -> dict:
    """Builds everything to publish into `into`, with STAGE_RECORD describing it.

    `None` in `targets` is this machine's own target."""
    into.mkdir(parents=True, exist_ok=True)
    host = host_target()
    record: dict = {"build": build, "commit": commit, "version": version,
                    "assets": {}, "files": {}}
    macs: list[tuple[str, Path]] = []
    for target in targets:
        key = key_for_target(target or host)
        step(f"Building the launcher for {key} (release)")
        binary = build_binary(target, build, commit, key, into)
        if key.startswith("macos-"):
            if not host.endswith("apple-darwin"):
                raise Failure("macOS builds are signed and bundled on a Mac")
            codesign(binary, sign_identity(), BUNDLE_ID)
            macs.append((key, binary))
        record["assets"][key] = describe(binary, build, commit)
        record["files"][binary.name] = sha256(binary)
        if has_installer(key) and installer:
            step("Building the Windows installer")
            setup = build_installer(binary, build, version, key, into)
            record["files"][setup.name] = sha256(setup)
    if macs:
        identity = sign_identity()
        keys = [key for key, _ in macs]
        step(f"Building {APP_NAME}.app ({', '.join(keys)}), "
             + ("signed ad hoc" if identity == "-" else f"signed by {identity}"))
        with tempfile.TemporaryDirectory() as tmp:
            app = build_app([binary for _, binary in macs], build, version, identity, Path(tmp))
            dmg = build_dmg(app, dmg_name(build, keys), identity, into)
        if not notarize(dmg) and identity != "-":
            print("    Not notarized: set " + ", ".join(NOTARY_ENV) + " to notarize it.")
        record["files"][dmg.name] = sha256(dmg)
    (into / STAGE_RECORD).write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    return record


def read_stage(folder: Path) -> dict:
    """A staged folder's record, after checking its files are the ones staged."""
    file = folder / STAGE_RECORD
    if not file.exists():
        raise Failure(f"{folder} has no {STAGE_RECORD}; stage a build into it first")
    record = json.loads(file.read_text(encoding="utf-8"))
    for name, digest in record.get("files", {}).items():
        path = folder / name
        if not path.is_file():
            raise Failure(f"{folder} is missing {name}")
        if sha256(path) != digest:
            raise Failure(f"{path} has changed since it was staged")
    for key, entry in record.get("assets", {}).items():
        if record["files"].get(entry["name"]) != entry["sha256"]:
            raise Failure(f"{folder}'s {key} binary doesn't match its record")
    return record


# A dry run stages into the cargo target directory and keeps a copy of the
# binary to test. Publishing uploads the staged files, which nothing touches, so
# what reaches users is what was tested, even if the copy under test updated itself.
CANDIDATE_DIR = "release-candidate"


def candidate_paths(target_dir: Path) -> tuple[Path, Path]:
    """The dry run's staged folder and the folder of copies to test."""
    folder = target_dir / CANDIDATE_DIR
    return folder / "stage", folder / "test"


def save_test_copies(stage_dir: Path, test_dir: Path, record: dict) -> list[Path]:
    """Copies of what a person runs to test: the binaries, the installer, the disk image."""
    if test_dir.exists():
        shutil.rmtree(test_dir, ignore_errors=True)
    test_dir.mkdir(parents=True)
    copies = []
    for name in record["files"]:
        copy = test_dir / name
        shutil.copyfile(stage_dir / name, copy)
        if not name.endswith((".exe", ".dmg")):
            copy.chmod(0o755)
        copies.append(copy)
    return copies


def check_candidate(stage_dir: Path, build: int, commit: str) -> dict:
    """Refuses to publish anything but the build a dry run made for testing."""
    if not (stage_dir / STAGE_RECORD).exists():
        raise Failure(
            "no dry run to publish. Run with --dry-run first, test the build it prints on "
            "this machine, then publish"
        )
    record = json.loads((stage_dir / STAGE_RECORD).read_text(encoding="utf-8"))
    if (record.get("build"), record.get("commit")) != (build, commit):
        raise Failure(
            f"the last dry run was build {record.get('build')} of "
            f"{str(record.get('commit'))[:7]}, not build {build} of {commit[:7]}. "
            "Dry-run and test this build first"
        )
    try:
        return read_stage(stage_dir)
    except Failure as err:
        raise Failure(f"{err}, so it isn't the build that was tested. Dry-run and test again")


# ---------------------------------------------------------------------------
# Where it goes: a version's GitHub release, the legacy release, or a test folder


def utc_now() -> str:
    return datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def restricted(record: dict, skip: set[str]) -> dict:
    """`record` without the platforms in `skip` and their files. The universal disk
    image goes only when every macOS platform in it does."""
    if not skip:
        return record
    assets = {key: entry for key, entry in record["assets"].items() if key not in skip}
    macs_left = any(key.startswith("macos-") for key in assets)
    files = {}
    for name, digest in record["files"].items():
        key = file_key(name)
        if key in skip or (key == "macos-universal" and not macs_left):
            continue
        files[name] = digest
    return dict(record, assets=assets, files=files)


class GitHubRelease:
    """The files of one GitHub release, through the GitHub CLI."""

    def __init__(self, repo: str, tag: str):
        self.repo, self.tag = repo, tag
        self._info: dict | None = None

    def gh(self, *args: str) -> str:
        return run(["gh", *args])

    def check(self) -> None:
        try:
            self.gh("auth", "status")
        except Failure as err:
            raise Failure(f"the GitHub CLI isn't signed in (`gh auth login`):\n{err}")

    def info(self, fresh: bool = False) -> dict | None:
        """The release (drafts too), or None if there isn't one."""
        if self._info is None or fresh:
            try:
                out = self.gh("release", "view", self.tag, "--repo", self.repo, "--json",
                              "tagName,isDraft,isPrerelease,url,assets")
            except Failure:
                return None
            self._info = json.loads(out)
        return self._info

    def names(self) -> list[str]:
        info = self.info(fresh=True)
        return [asset["name"] for asset in info["assets"]] if info else []

    def download(self, name: str, into: Path) -> Path:
        self.gh("release", "download", self.tag, "--repo", self.repo,
                "--pattern", name, "--dir", str(into), "--clobber")
        return into / name

    def manifest(self) -> dict | None:
        if MANIFEST not in self.names():
            return None
        with tempfile.TemporaryDirectory() as tmp:
            return json.loads(self.download(MANIFEST, Path(tmp)).read_text(encoding="utf-8"))

    def upload(self, path: Path) -> None:
        self.gh("release", "upload", self.tag, str(path), "--repo", self.repo, "--clobber")

    def delete(self, name: str) -> None:
        self.gh("release", "delete-asset", self.tag, name, "--repo", self.repo, "--yes")

    def tag_commit(self) -> str | None:
        """The commit the tag is on, or None if there's no such tag on GitHub."""
        try:
            return self.gh("api", f"repos/{self.repo}/commits/tags/{self.tag}", "--jq", ".sha")
        except Failure:
            return None

    def url(self) -> str:
        info = self.info()
        return info["url"] if info else f"https://github.com/{self.repo}/releases/tag/{self.tag}"


class VersionRelease(GitHubRelease):
    """A launcher version's own release: one tag, one commit, one build per platform.

    Uploading to one that doesn't exist yet tags the build's commit and drafts it.
    A platform it already has is refused (or skipped, or with --force replaced), and
    so is a build that launchers wouldn't update to because a published release of
    the same channel already has one as high."""

    keeps_history = False

    def prerelease(self) -> bool:
        info = self.info()
        return info["isPrerelease"] if info else is_prerelease_tag(self.tag)

    def published_releases(self) -> list[dict]:
        out = self.gh("api", "--paginate", f"repos/{self.repo}/releases?per_page=100", "--jq",
                      '.[] | select(.tag_name | startswith("launcher-v")) '
                      '| {tag_name, draft, prerelease, assets: [.assets[] | {name}]}')
        return [json.loads(line) for line in out.splitlines() if line.strip()]

    def create(self, commit: str) -> None:
        """Tags `commit` (unless the tag exists) and drafts the release."""
        tagged = self.tag_commit()
        if tagged is None:
            step(f"Tagging {commit[:7]} as {self.tag}")
            try:
                self.gh("api", "-X", "POST", f"repos/{self.repo}/git/refs",
                        "-f", f"ref=refs/tags/{self.tag}", "-f", f"sha={commit}")
            except Failure as err:
                raise Failure(f"couldn't tag {commit[:7]} on GitHub; push the commit first "
                              f"(a release branch: `git push origin HEAD`):\n{err}")
        elif tagged != commit:
            raise Failure(f"{self.tag} is on {tagged[:7]}, not {commit[:7]}")
        step(f"Drafting the {self.tag} release")
        args = ["release", "create", self.tag, "--repo", self.repo, "--draft", "--verify-tag",
                "--title", release_title(self.tag), "--notes", release_notes(self.tag, commit),
                "--latest=false"]
        if is_prerelease_tag(self.tag):
            args.append("--prerelease")
        self.gh(*args)
        self.info(fresh=True)

    def prepare(self, stages: list[tuple[Path, dict]], force: bool, skip_existing: bool,
                dry: bool = False) -> list[tuple[Path, dict]]:
        commits = {record["commit"] for _, record in stages}
        if len(commits) != 1:
            raise Failure("the builds are of different commits: "
                          + ", ".join(sorted(c[:7] for c in commits)))
        commit = commits.pop()
        tagged = self.tag_commit()
        if tagged is not None and tagged != commit:
            raise Failure(
                f"{self.tag} is on {tagged[:7]}, and this is a build of {commit[:7]}. Build the "
                f"tag's commit (`git checkout {self.tag}`), or release this commit as a new version"
            )
        info = self.info()
        if info is None:
            if dry:
                print(f"    Would tag {commit[:7]} as {self.tag} and draft its release.")
            else:
                self.create(commit)
                info = self.info()
        elif not info["isDraft"]:
            print(f"    {self.tag} is already published: launchers can see what's added to it.")
        current = (self.manifest() if info else None) or {"assets": {}}
        published = self.published_releases()
        kept = []
        for folder, record in stages:
            skip = set()
            for key, entry in record["assets"].items():
                have = current["assets"].get(key)
                if have:
                    if skip_existing:
                        print(f"    {self.tag} already has {key} (build {have.get('build')}); "
                              "leaving it.")
                        skip.add(key)
                        continue
                    if not force:
                        raise Failure(
                            f"{self.tag} already has {key} (build {have.get('build')}). Pass "
                            "--force to replace it" + ("" if info and info["isDraft"] else
                                                       ", though launchers may have it already")
                        )
                best = newest_published(published, key, self.prerelease(), self.tag)
                if best and best[0] >= entry["build"] and not force:
                    channel = "beta" if self.prerelease() else "stable"
                    raise Failure(
                        f"{best[1]} is published with {key} build {best[0]}, so {channel} launchers "
                        f"wouldn't update to this build {entry['build']}. Release a newer commit "
                        "(a fix on a release branch counts up from its base), or pass --force"
                    )
            record = restricted(record, skip)
            if record["assets"]:
                kept.append((folder, record))
        return kept

    def finished(self, manifest: dict) -> None:
        pass


class LegacyRelease(GitHubRelease):
    """`launcher-latest`, which launchers from before versioned releases read. Only
    --bridge writes to it, with a published version's files, so those launchers
    update to a build that reads versioned releases from then on."""

    keeps_history = True

    def prepare(self, stages, force, skip_existing, dry=False):
        if self.info() is None:
            raise Failure(f"there's no {self.tag} release; launchers that old don't exist")
        check_newer(self.manifest(), [record for _, record in stages], force)
        return stages

    def finished(self, manifest: dict) -> None:
        tag = manifest.get("tag")
        notes = ("Lodestone Launcher's old update channel. Launchers update from each version's "
                 "own release now (`launcher-v*`); this one only carries "
                 + (f"[{tag}](https://github.com/{self.repo}/releases/tag/{tag}) " if tag else
                    "a recent build ")
                 + "for launchers from before that.")
        self.gh("release", "edit", self.tag, "--repo", self.repo, "--notes", notes)


class Folder:
    """A local stand-in for a release, to test the updater against."""

    keeps_history = True

    def __init__(self, path: Path):
        self.path = path

    def check(self) -> None:
        pass

    def names(self) -> list[str]:
        if not self.path.is_dir():
            return []
        return sorted(p.name for p in self.path.iterdir() if p.is_file())

    def download(self, name: str, into: Path) -> Path:
        shutil.copyfile(self.path / name, into / name)
        return into / name

    def manifest(self) -> dict | None:
        file = self.path / MANIFEST
        return json.loads(file.read_text(encoding="utf-8")) if file.exists() else None

    def prepare(self, stages, force, skip_existing, dry=False):
        check_newer(self.manifest(), [record for _, record in stages], force)
        return stages

    def upload(self, path: Path) -> None:
        self.path.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, self.path / path.name)

    def delete(self, name: str) -> None:
        (self.path / name).unlink()

    def finished(self, manifest: dict) -> None:
        pass

    def url(self) -> str:
        return str(self.path)


def release_title(tag: str) -> str:
    match = TAG.match(tag)
    return f"Lodestone Launcher {tag[len('launcher-v'):] if match else tag}"


def release_notes(tag: str, commit: str) -> str:
    """A new draft's notes, for a developer to finish (the release workflow's
    release-draft.yml writes the same)."""
    channel = ("launchers with Settings › General › Beta builds on" if is_prerelease_tag(tag)
               else "every installed launcher")
    return "\n".join([
        "## What's new",
        "",
        "<!-- Write what changed before publishing this release. -->",
        "",
        "## Download",
        "",
        f"- **Windows:** `{STABLE_INSTALLER}`. It installs for your account, without "
        "administrator rights.",
        f"- **macOS:** `{STABLE_DMG}`. Open it and drag Lodestone Launcher into Applications.",
        "- The `-standalone` files run without installing, from wherever you put them.",
        "",
        f"Once this release is published, {channel} updates to it.",
        "",
        f"Built from {commit}.",
    ])


# ---------------------------------------------------------------------------
# Uploading


def check_newer(current: dict | None, records: list[dict], force: bool) -> None:
    """For releases that hold a run of builds: refuses one that isn't newer."""
    for record in records:
        for key, entry in record["assets"].items():
            published = (current or {}).get("assets", {}).get(key)
            if not published:
                continue
            have = published.get("build", (current or {}).get("build"))
            if have >= entry["build"] and not force:
                raise Failure(
                    f"{key} is already at build {have} there, and this is build {entry['build']}. "
                    "Publish a newer commit, or pass --force to replace it"
                )


def merged_manifest(current: dict | None, stages: list[tuple[Path, dict]],
                    tag: str | None) -> dict:
    manifest = current
    now = utc_now()
    for _, record in stages:
        for key, entry in record["assets"].items():
            manifest = updated_manifest(manifest, key, entry, record["version"], now, tag)
    return manifest


def publish(target, stages: list[tuple[Path, dict]], force: bool = False,
            skip_existing: bool = False, tag: str | None = None) -> dict | None:
    """Uploads staged folders to `target` and merges their platforms into its
    manifest. Returns the manifest, or None if there was nothing new to upload."""
    if not stages:
        raise Failure("nothing to publish")
    stages = target.prepare(stages, force, skip_existing)
    if not stages:
        step("Nothing new to upload")
        return None
    with tempfile.TemporaryDirectory() as tmp:
        for folder, record in stages:
            binaries = {entry["name"] for entry in record["assets"].values()}
            # Installers and disk images first, the binaries the updater fetches next,
            # and the manifest last, so a launcher never reads it before its file is up.
            for name in sorted(record["files"], key=lambda name: name in binaries):
                step(f"Uploading {name}")
                target.upload(folder / name)
                alias = alias_for(name)
                if alias:
                    step(f"Uploading it as {alias} too")
                    copy = Path(tmp) / alias
                    shutil.copyfile(folder / name, copy)
                    target.upload(copy)
        # Builds of other platforms can be uploaded at the same time, from CI or
        # another developer's machine: merge into the manifest as it is now, and
        # check it kept ours.
        ours = {key: entry["sha256"] for _, record in stages
                for key, entry in record["assets"].items()}
        for _ in range(3):
            manifest = merged_manifest(target.manifest(), stages, tag)
            manifest_file = Path(tmp) / MANIFEST
            manifest_file.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
            step(f"Uploading {MANIFEST}")
            target.upload(manifest_file)
            written = target.manifest() or {"assets": {}}
            if all(written["assets"].get(key, {}).get("sha256") == digest
                   for key, digest in ours.items()):
                manifest = written
                break
            print(f"    Another upload replaced {MANIFEST} meanwhile; merging again.")
        else:
            raise Failure(f"other uploads kept replacing {MANIFEST}; run this again")
    if target.keeps_history:
        clean_up(target, manifest, [name for _, record in stages for name in record["files"]])
    target.finished(manifest)
    return manifest


def clean_up(target, manifest: dict, uploaded: list[str]) -> None:
    """Removes files of old builds, for the platforms just uploaded."""
    keys = {file_key(name) for name in uploaded} - {None}
    names = target.names()
    for key in sorted(keys):
        entry = manifest["assets"].get(key)
        keep = {entry["build"]} if entry else set()
        if key == "macos-universal":
            keep = {asset["build"] for k, asset in manifest["assets"].items() if k.startswith("macos-")}
        for name in stale_names(names, key, keep):
            step(f"Removing the old {name}")
            target.delete(name)


def promotion(source: dict, names: list[str], platforms: list[str] | None) -> tuple[dict, list[str]]:
    """What copying `source`'s manifest takes: its entries for `platforms` (all by
    default), and every file of those builds: binaries, installers and disk images."""
    assets = {key: entry for key, entry in source.get("assets", {}).items()
              if platforms is None or key in platforms}
    missing = set(platforms or []) - set(assets)
    if missing:
        raise Failure(f"it has no build for {', '.join(sorted(missing))}")
    if not assets:
        raise Failure("it has no builds to copy")
    files = []
    for name in names:
        match = ASSET_NAME.match(name)
        if not match:
            continue
        build, key = int(match.group(1)), match.group(2)
        entry = assets.get(key)
        if key == "macos-universal":
            # The disk image holds every macOS architecture; it goes with them.
            macs = [entry for k, entry in assets.items() if k.startswith("macos-")]
            if macs and all(mac["build"] == build for mac in macs):
                files.append(name)
        elif entry and entry["build"] == build:
            files.append(name)
    for key, entry in assets.items():
        if entry["name"] not in files:
            raise Failure(f"its manifest lists {entry['name']} for {key}, but the file isn't there")
    return assets, files


def copy_release(source, dest, platforms: list[str] | None, force: bool,
                 tag: str | None = None) -> dict | None:
    """Copies `source`'s builds to `dest`, byte for byte, after checking them
    against `source`'s manifest."""
    manifest = source.manifest()
    if not manifest:
        raise Failure(f"{source.url()} has no {MANIFEST} to copy")
    try:
        assets, files = promotion(manifest, source.names(), platforms)
    except Failure as err:
        raise Failure(f"{source.url()}: {err}")
    for key, entry in assets.items():
        print(f"    {key}: build {entry['build']} ({entry['commit'][:7]})")
    with tempfile.TemporaryDirectory() as tmp:
        folder = Path(tmp)
        for name in files:
            step(f"Downloading {name}")
            source.download(name, folder)
        for key, entry in assets.items():
            path = folder / entry["name"]
            if path.stat().st_size != entry["size"] or sha256(path) != entry["sha256"]:
                raise Failure(f"{entry['name']} doesn't match its manifest; not copying it")
        newest = max(assets.values(), key=lambda e: e["build"])
        record = {"build": newest["build"],
                  "commit": newest["commit"],
                  "version": manifest.get("version", ""),
                  "assets": assets,
                  "files": {name: sha256(folder / name) for name in files}}
        return publish(dest, [(folder, record)], force, tag=tag)


def published_source(release: VersionRelease, beta: bool | None) -> None:
    """Refuses to copy from a release launchers haven't been offered."""
    info = release.info()
    if info is None:
        raise Failure(f"there's no {release.tag} release")
    if info["isDraft"]:
        raise Failure(f"{release.tag} is still a draft; copy builds that people have run")
    if beta is False and info["isPrerelease"]:
        raise Failure(f"{release.tag} is a pre-release; bridge a stable version")


# ---------------------------------------------------------------------------


def report(target, manifest: dict | None, build: int | None = None) -> None:
    step(f"Release: {target.url()}")
    if manifest:
        print("\n".join(summary(manifest, build or manifest["build"])))


def main(argv: list[str]) -> int:
    # Keep our lines in order with cargo's and gh's output when piped.
    sys.stdout.reconfigure(line_buffering=True)
    parser = argparse.ArgumentParser(description="Build and upload Lodestone Launcher versions.")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--dry-run", action="store_true",
                      help="stage this machine's build to test (with --tag: and check it against the release)")
    mode.add_argument("--output", type=Path, metavar="DIR",
                      help="build and publish into a local folder instead of GitHub (for testing)")
    mode.add_argument("--stage", type=Path, metavar="DIR",
                      help="only build, into DIR, for a later --from (GitHub Actions)")
    mode.add_argument("--from", dest="sources", type=Path, metavar="DIR", action="append",
                      help="upload a folder --stage made to --tag's release; repeat for several")
    mode.add_argument("--promote-from", metavar="TAG",
                      help="give --tag's release this published beta's files, byte for byte")
    mode.add_argument("--bridge", metavar="TAG",
                      help=f"copy this published version to {LEGACY_TAG}, for launchers too old "
                           "to read versioned releases")
    parser.add_argument("--tag", help="the version: launcher-vX.Y.Z, or launcher-vX.Y.Z-beta.N")
    parser.add_argument("--target", action="append", metavar="TRIPLE",
                        help="a Rust target to build for (repeat for several); this machine's by default")
    parser.add_argument("--platform", action="append", metavar="KEY",
                        help="with --promote-from or --bridge, only this platform (repeat for several)")
    parser.add_argument("--build", type=int,
                        help="use this build number instead of the commit count (--output, --dry-run, --stage)")
    parser.add_argument("--force", action="store_true",
                        help="replace a platform the release has, or upload a build launchers wouldn't take")
    parser.add_argument("--skip-existing", action="store_true",
                        help="with --from, leave platforms the release already has")
    parser.add_argument("--allow-dirty", action="store_true",
                        help="publish a checkout with uncommitted changes")
    parser.add_argument("--no-installer", action="store_true",
                        help="on Windows, publish the binary without building the installer")
    # Another repository (a fork) to rehearse in.
    parser.add_argument("--repo", default=REPO, help=argparse.SUPPRESS)
    args = parser.parse_args(argv)

    if args.tag:
        check_tag(args.tag)

    if args.bridge:
        check_tag(args.bridge)
        source = VersionRelease(args.repo, args.bridge)
        source.check()
        published_source(source, beta=False)
        legacy = LegacyRelease(args.repo, LEGACY_TAG)
        step(f"Copying {args.bridge} to {LEGACY_TAG}")
        manifest = copy_release(source, legacy, args.platform, args.force, tag=args.bridge)
        report(legacy, manifest)
        return 0

    if args.promote_from:
        if not args.tag:
            raise Failure("say which version to make: --tag launcher-vX.Y.Z")
        check_tag(args.promote_from)
        source = VersionRelease(args.repo, args.promote_from)
        source.check()
        published_source(source, beta=None)
        target = VersionRelease(args.repo, args.tag)
        step(f"Copying {args.promote_from}'s builds to {args.tag}")
        manifest = copy_release(source, target, args.platform, args.force, tag=args.tag)
        report(target, manifest)
        return 0

    if args.sources:
        if not args.tag:
            raise Failure("say which version these are: --tag launcher-vX.Y.Z")
        target = VersionRelease(args.repo, args.tag)
        target.check()
        stages = [(folder, read_stage(folder)) for folder in args.sources]
        manifest = publish(target, stages, args.force, args.skip_existing, tag=args.tag)
        report(target, manifest)
        return 0

    publishing = not (args.dry_run or args.stage or args.output)
    if publishing and not args.tag:
        raise Failure("say which version to upload to: --tag launcher-vX.Y.Z")
    if args.build is not None and publishing:
        raise Failure("--build is for tests; use it with --output, --dry-run or --stage")

    step("Checking the checkout")
    local = args.output is not None
    commit = run(["git", "rev-parse", "HEAD"])
    if not (args.allow_dirty or local or args.stage):
        check_clean()
    build = args.build if args.build is not None else int(run(["git", "rev-list", "--count", "HEAD"]))
    version = workspace_version()
    targets: list[str | None] = list(args.target or [None])
    keys = [key_for_target(target or host_target()) for target in targets]
    print(f"    {', '.join(keys)}, build {build}, commit {commit[:7]}, version {version}")
    if args.tag:
        note = tag_version_note(args.tag, version)
        if note:
            print(f"    Note: {note}")
    installer = not args.no_installer
    if installer and any(has_installer(key) for key in keys):
        find_iscc()  # before the long build, not after it

    if args.stage:
        record = stage(targets, build, commit, version, args.stage.resolve(), installer)
        step(f"Staged build {build} in {args.stage}:")
        for name in record["files"]:
            print(f"    {name}")
        return 0

    if local:
        with tempfile.TemporaryDirectory() as tmp:
            record = stage(targets, build, commit, version, Path(tmp), installer)
            folder = Folder(args.output.resolve())
            manifest = publish(folder, [(Path(tmp), record)], args.force)
        step(f"Published build {build} into {folder.url()}")
        print("\n".join(summary(manifest, build)))
        print(f"    Serve {folder.url()} over HTTP and set LODESTONE_LAUNCHER_UPDATE_URL to its "
              f"{MANIFEST} to test updating against it.")
        return 0

    stage_dir, test_dir = candidate_paths(cargo_target_dir())
    if args.dry_run:
        if stage_dir.exists():
            shutil.rmtree(stage_dir)
        record = stage(targets, build, commit, version, stage_dir, installer)
        if args.tag:
            target = VersionRelease(args.repo, args.tag)
            target.check()
            step(f"Dry run: nothing uploaded. Checking it against {args.tag}")
            kept = target.prepare([(stage_dir, record)], args.force, False, dry=True)
            current = target.manifest() if target.info() else None
            manifest = merged_manifest(current, kept, args.tag) if kept else current
            print(f"    {args.tag}'s {MANIFEST} would be:")
            print(json.dumps(manifest, indent=2))
        copies = save_test_copies(stage_dir, test_dir, record)
        step("Test this build on this machine before publishing it:")
        for copy in copies:
            print(f"    {copy}")
        print("    Then upload it from this checkout without changes (--tag launcher-vX.Y.Z);")
        print("    the script uploads exactly these files.")
        return 0

    target = VersionRelease(args.repo, args.tag)
    target.check()
    record = check_candidate(stage_dir, build, commit)
    if set(record["assets"]) != set(keys):
        raise Failure(
            f"the dry run staged {', '.join(sorted(record['assets']))}, not "
            f"{', '.join(sorted(keys))}. Dry-run with the same --target options"
        )
    manifest = publish(target, [(stage_dir, record)], args.force, tag=args.tag)
    report(target, manifest, build)
    if manifest and target.info(fresh=True)["isDraft"]:
        print("    It's a draft: write its notes and publish it on GitHub when every platform is in.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except Failure as err:
        print(f"error: {err}", file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        sys.exit(130)
