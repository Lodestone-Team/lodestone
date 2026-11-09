#!/usr/bin/env python3
"""Publishes Lodestone Launcher builds that installed launchers update to.

There are two channels, each a GitHub release with a `latest.json` that the
launcher's updater reads (launcher/crates/launcher-core/src/update.rs):

- **beta** (`launcher-beta`): new builds go here first. Launchers with "Beta
  builds" on read it, and the stable channel too.
- **stable** (`launcher-latest`): what every other launcher reads. A stable build
  is normally a beta build that beta users have run, promoted with `--promote`,
  which copies its files byte for byte.

Publishing has two halves. Staging builds the files into a folder: the standalone
binary for each platform (the file the updater downloads), the Windows installer,
and on macOS the app, in a disk image. Uploading puts a staged folder (or several)
on a channel and updates its `latest.json`. They run together on a developer's
machine, or apart in GitHub Actions (.github/workflows/launcher-release.yml):
each platform stages on a runner of its own, and one job uploads them all.

A build's number is the number of commits in its history (`git rev-list --count
HEAD`), so it grows along `main` and is the same on both channels. Only commits on
`main` on GitHub are published, and a channel refuses a build that isn't newer
than the one it has for that platform.

Usage, from anywhere in the repository:

    python3 launcher/scripts/publish_release.py --dry-run        # stage a build to test
    python3 launcher/scripts/publish_release.py --channel beta   # upload the dry run's build
    python3 launcher/scripts/publish_release.py --promote        # beta's builds to stable
    python3 launcher/scripts/publish_release.py --output DIR --build 2
                                                                 # local test, no GitHub
    # In GitHub Actions:
    python3 launcher/scripts/publish_release.py --stage DIR [--target TRIPLE ...]
    python3 launcher/scripts/publish_release.py --from DIR [--from DIR ...] --channel beta

`--target` builds for a Rust target other than this machine's (a Mac builds both
`aarch64-apple-darwin` and `x86_64-apple-darwin`, and the app holds both). Each
platform is otherwise staged on a machine of its own.

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
CHANNELS = {"stable": "launcher-latest", "beta": "launcher-beta"}
MANIFEST = "latest.json"
# Builds kept per platform: the newest and the one before it, so a launcher that
# started downloading just before a publish can finish.
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
# The newest installer and disk image under names that never change, so download
# links don't either.
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


def github_remote() -> str:
    """The git remote that points at REPO."""
    for line in run(["git", "remote", "-v"]).splitlines():
        parts = line.split()
        if len(parts) >= 2 and REPO.lower() in parts[1].lower().removesuffix(".git"):
            return parts[0]
    raise Failure(f"no git remote points at {REPO}")


def check_clean() -> None:
    if run(["git", "status", "--porcelain", "--untracked-files=no"]):
        raise Failure(
            "the checkout has uncommitted changes; commit or stash them, or pass --allow-dirty"
        )


def check_on_main(commit: str) -> None:
    remote = github_remote()
    run(["git", "fetch", "--quiet", remote, "main"])
    try:
        run(["git", "merge-base", "--is-ancestor", commit, f"{remote}/main"])
    except Failure:
        raise Failure(
            f"{commit[:7]} isn't on {remote}/main. Publish merged commits (check out main and "
            "pull), or pass --allow-unmerged"
        )


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


def updated_manifest(old: dict | None, key: str, entry: dict, version: str, published: str) -> dict:
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
    return {
        "build": newest["build"],
        "commit": newest["commit"],
        "version": version if newest is entry else (old or {}).get("version", version),
        "published": published,
        "assets": dict(sorted(assets.items())),
    }


def file_key(name: str) -> str | None:
    match = ASSET_NAME.match(name)
    return match.group(2) if match else None


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


def summary(manifest: dict, build: int) -> list[str]:
    lines = []
    for key, asset in manifest["assets"].items():
        line = f"  {key}: build {asset['build']} ({str(asset.get('commit') or '')[:7]})"
        if asset["build"] < build:
            line += " - older; publish it from that platform to bring it up to date"
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
# Where it goes: GitHub, or a folder for testing


class GitHub:
    def __init__(self, repo: str, tag: str):
        self.repo, self.tag = repo, tag

    def gh(self, *args: str) -> str:
        return run(["gh", *args])

    def check(self) -> None:
        try:
            self.gh("auth", "status")
        except Failure as err:
            raise Failure(f"the GitHub CLI isn't signed in (`gh auth login`):\n{err}")

    def exists(self) -> bool:
        try:
            self.gh("release", "view", self.tag, "--repo", self.repo, "--json", "tagName")
            return True
        except Failure:
            return False

    def names(self) -> list[str]:
        if not self.exists():
            return []
        out = self.gh("release", "view", self.tag, "--repo", self.repo, "--json", "assets")
        return [asset["name"] for asset in json.loads(out)["assets"]]

    def download(self, name: str, into: Path) -> Path:
        self.gh("release", "download", self.tag, "--repo", self.repo,
                "--pattern", name, "--dir", str(into), "--clobber")
        return into / name

    def manifest(self) -> dict | None:
        if MANIFEST not in self.names():
            return None
        with tempfile.TemporaryDirectory() as tmp:
            return json.loads(self.download(MANIFEST, Path(tmp)).read_text(encoding="utf-8"))

    def ensure_release(self, commit: str) -> None:
        if not self.exists():
            step(f"Creating the {self.tag} release")
            self.gh("release", "create", self.tag, "--repo", self.repo, "--target", commit,
                    "--prerelease", "--latest=false", "--title", release_title(self.tag),
                    "--notes", release_intro(self.tag))

    def upload(self, path: Path) -> None:
        self.gh("release", "upload", self.tag, str(path), "--repo", self.repo, "--clobber")

    def delete(self, name: str) -> None:
        self.gh("release", "delete-asset", self.tag, name, "--repo", self.repo, "--yes")

    def point_at(self, commit: str, manifest: dict) -> None:
        self.gh("api", "-X", "PATCH", f"repos/{self.repo}/git/refs/tags/{self.tag}",
                "-f", f"sha={commit}", "-F", "force=true")
        self.gh("release", "edit", self.tag, "--repo", self.repo,
                "--notes", release_notes(self.tag, manifest))

    def url(self) -> str:
        return f"https://github.com/{self.repo}/releases/tag/{self.tag}"


class Folder:
    """A local stand-in for a release, to test the updater against."""

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

    def ensure_release(self, commit: str) -> None:
        pass

    def upload(self, path: Path) -> None:
        self.path.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, self.path / path.name)

    def delete(self, name: str) -> None:
        (self.path / name).unlink()

    def point_at(self, commit: str, manifest: dict) -> None:
        pass

    def url(self) -> str:
        return str(self.path)


def release_title(tag: str) -> str:
    return "Lodestone Launcher (beta)" if tag == CHANNELS["beta"] else "Lodestone Launcher"


def release_intro(tag: str) -> str:
    if tag == CHANNELS["beta"]:
        return ("Lodestone Launcher beta builds: new features before everyone else, and "
                "sometimes new bugs. Launchers with Settings › General › Beta builds on "
                "update from here.")
    return "Lodestone Launcher builds. Installed launchers update from here."


def release_notes(tag: str, manifest: dict) -> str:
    notes = [release_intro(tag), ""]
    notes += [f"- {key}: build {asset['build']} ({str(asset.get('commit') or '')[:7]})"
              for key, asset in manifest["assets"].items()]
    keys = manifest["assets"]
    if any(key.startswith("macos-") for key in keys):
        notes += ["", f"**New on a Mac?** Download `{STABLE_DMG}` below, open it and drag "
                  "Lodestone Launcher into Applications. It keeps itself up to date from then on."]
    if any(has_installer(key) for key in keys):
        notes += ["", f"**New on Windows?** Download `{STABLE_INSTALLER}` below and run it. "
                  "It installs for your account without administrator rights, and the "
                  "launcher keeps itself up to date from then on."]
    notes += ["", "The `-standalone` files run without installing, from wherever you put "
              "them, and update themselves too."]
    notes += ["", "Published with `launcher/scripts/publish_release.py`."]
    return "\n".join(notes)


# ---------------------------------------------------------------------------
# Uploading


def check_newer(current: dict | None, assets: dict, force: bool) -> None:
    for key, entry in assets.items():
        published = (current or {}).get("assets", {}).get(key)
        if not published:
            continue
        have = published.get("build", (current or {}).get("build"))
        if have >= entry["build"] and not force:
            raise Failure(
                f"{key} is already at build {have} there, and this is build {entry['build']}. "
                "Publish a newer commit, or pass --force to replace it"
            )


def publish(target, stages: list[tuple[Path, dict]], force: bool = False) -> dict:
    """Uploads staged folders to `target` and merges their platforms into its manifest."""
    if not stages:
        raise Failure("nothing to publish")
    current = target.manifest()
    for _, record in stages:
        check_newer(current, record["assets"], force)
    newest = max(stages, key=lambda item: item[1]["build"])[1]
    target.ensure_release(newest["commit"])
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
        # Read the manifest again just before replacing it, so a platform published
        # meanwhile from another machine is kept.
        manifest = target.manifest()
        now = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        for _, record in stages:
            for key, entry in record["assets"].items():
                manifest = updated_manifest(manifest, key, entry, record["version"], now)
        manifest_file = Path(tmp) / MANIFEST
        manifest_file.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        step(f"Uploading {MANIFEST}")
        target.upload(manifest_file)
    if manifest["build"] == newest["build"]:
        target.point_at(newest["commit"], manifest)
    clean_up(target, manifest, [name for _, record in stages for name in record["files"]])
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


def promotion(beta: dict, names: list[str], platforms: list[str] | None) -> tuple[dict, list[str]]:
    """What promoting `beta` copies: its manifest entries for `platforms` (all by
    default), and every file of those builds: binaries, installers and disk images."""
    assets = {key: entry for key, entry in beta.get("assets", {}).items()
              if platforms is None or key in platforms}
    missing = set(platforms or []) - set(assets)
    if missing:
        raise Failure(f"beta has no build for {', '.join(sorted(missing))}")
    if not assets:
        raise Failure("beta has no builds to promote")
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
            raise Failure(f"beta's manifest lists {entry['name']} for {key}, but the file isn't there")
    return assets, files


def promote(beta, stable, platforms: list[str] | None, force: bool) -> dict:
    """Copies beta's builds to stable, byte for byte."""
    manifest = beta.manifest()
    if not manifest:
        raise Failure("there's no beta release to promote")
    assets, files = promotion(manifest, beta.names(), platforms)
    for key, entry in assets.items():
        print(f"    {key}: build {entry['build']} ({entry['commit'][:7]})")
    with tempfile.TemporaryDirectory() as tmp:
        folder = Path(tmp)
        for name in files:
            step(f"Downloading {name} from beta")
            beta.download(name, folder)
        for key, entry in assets.items():
            path = folder / entry["name"]
            if path.stat().st_size != entry["size"] or sha256(path) != entry["sha256"]:
                raise Failure(f"{entry['name']} doesn't match beta's manifest; not promoting it")
        builds = {entry["build"] for entry in assets.values()}
        record = {"build": max(builds),
                  "commit": max(assets.values(), key=lambda e: e["build"])["commit"],
                  "version": manifest.get("version", ""),
                  "assets": assets,
                  "files": {name: sha256(folder / name) for name in files}}
        return publish(stable, [(folder, record)], force)


# ---------------------------------------------------------------------------


def main(argv: list[str]) -> int:
    # Keep our lines in order with cargo's and gh's output when piped.
    sys.stdout.reconfigure(line_buffering=True)
    parser = argparse.ArgumentParser(description="Publish Lodestone Launcher builds.")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--dry-run", action="store_true",
                      help="stage a build to test, and show the latest.json it would make")
    mode.add_argument("--output", type=Path, metavar="DIR",
                      help="build and publish into a local folder instead of GitHub (for testing)")
    mode.add_argument("--stage", type=Path, metavar="DIR",
                      help="only build, into DIR, for a later --from (GitHub Actions)")
    mode.add_argument("--from", dest="sources", type=Path, metavar="DIR", action="append",
                      help="upload a folder --stage made; repeat for several platforms")
    mode.add_argument("--promote", action="store_true",
                      help="copy beta's builds to stable, byte for byte")
    parser.add_argument("--channel", choices=sorted(CHANNELS),
                        help="where to publish: beta, or stable")
    parser.add_argument("--target", action="append", metavar="TRIPLE",
                        help="a Rust target to build for (repeat for several); this machine's by default")
    parser.add_argument("--platform", action="append", metavar="KEY",
                        help="with --promote, only this platform (repeat for several)")
    parser.add_argument("--build", type=int,
                        help="use this build number instead of the commit count (testing only)")
    parser.add_argument("--force", action="store_true",
                        help="publish even if the channel already has this build or a newer one")
    parser.add_argument("--allow-dirty", action="store_true",
                        help="publish a checkout with uncommitted changes")
    parser.add_argument("--no-installer", action="store_true",
                        help="on Windows, publish the binary without building the installer")
    parser.add_argument("--allow-unmerged", action="store_true",
                        help="publish a commit that isn't on main on GitHub")
    parser.add_argument("--repo", default=REPO, help=argparse.SUPPRESS)
    # Other releases to rehearse a real upload in; launchers never read them.
    parser.add_argument("--tag", help=argparse.SUPPRESS)
    parser.add_argument("--beta-tag", default=CHANNELS["beta"], help=argparse.SUPPRESS)
    args = parser.parse_args(argv)

    tag = args.tag or (CHANNELS[args.channel] if args.channel else None)
    test_tag = args.tag is not None and args.tag not in CHANNELS.values()

    if args.promote:
        if args.channel not in (None, "stable"):
            raise Failure("--promote copies beta to stable; leave out --channel")
        beta = GitHub(args.repo, args.beta_tag)
        stable = GitHub(args.repo, args.tag or CHANNELS["stable"])
        stable.check()
        step(f"Promoting {beta.tag} to {stable.tag}")
        manifest = promote(beta, stable, args.platform, args.force)
        step(f"Promoted: {stable.url()}")
        print("\n".join(summary(manifest, manifest["build"])))
        return 0

    if args.sources:
        if tag is None:
            raise Failure("say where to publish: --channel beta")
        if tag == CHANNELS["stable"]:
            raise Failure(
                "builds reach stable by --promote from beta, after beta users have run them"
            )
        target = GitHub(args.repo, tag)
        target.check()
        stages = [(folder, read_stage(folder)) for folder in args.sources]
        if not args.allow_unmerged:
            for _, record in stages:
                check_on_main(record["commit"])
        manifest = publish(target, stages, args.force)
        step(f"Published to {target.url()}")
        print("\n".join(summary(manifest, manifest["build"])))
        return 0

    publishing = not (args.dry_run or args.stage or args.output)
    if publishing and tag is None:
        raise Failure("say where to publish: --channel beta (or stable)")
    if args.build is not None and publishing and not test_tag:
        raise Failure("--build is for tests; use it with --output, --dry-run, --stage or a test --tag")

    step("Checking the checkout")
    local = args.output is not None
    if local:
        print("    Local test: not checking for a clean checkout on main.")
    commit = run(["git", "rev-parse", "HEAD"])
    if not (args.allow_dirty or local or args.stage):
        check_clean()
    if not (args.allow_unmerged or local or args.stage):
        check_on_main(commit)
    build = args.build if args.build is not None else int(run(["git", "rev-list", "--count", "HEAD"]))
    version = workspace_version()
    targets: list[str | None] = list(args.target or [None])
    keys = [key_for_target(target or host_target()) for target in targets]
    print(f"    {', '.join(keys)}, build {build}, commit {commit[:7]}, version {version}")
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
        if tag:
            current = GitHub(args.repo, tag).manifest()
            now = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
            manifest = current
            for key, entry in record["assets"].items():
                manifest = updated_manifest(manifest, key, entry, version, now)
            step(f"Dry run: nothing uploaded. {tag}'s latest.json would be:")
            print(json.dumps(manifest, indent=2))
        copies = save_test_copies(stage_dir, test_dir, record)
        step("Test this build on this machine before publishing it:")
        for copy in copies:
            print(f"    {copy}")
        print("    Then publish from this checkout without changes (--channel beta, or stable);")
        print("    the script uploads exactly these files.")
        return 0

    target = GitHub(args.repo, tag)
    target.check()
    if test_tag:
        # A rehearsal: build and upload in one go, to a release launchers don't read.
        with tempfile.TemporaryDirectory() as tmp:
            record = stage(targets, build, commit, version, Path(tmp), installer)
            manifest = publish(target, [(Path(tmp), record)], args.force)
    else:
        record = check_candidate(stage_dir, build, commit)
        if set(record["assets"]) != set(keys):
            raise Failure(
                f"the dry run staged {', '.join(sorted(record['assets']))}, not "
                f"{', '.join(sorted(keys))}. Dry-run with the same --target options"
            )
        manifest = publish(target, [(stage_dir, record)], args.force)
    step(f"Published build {build} for {', '.join(keys)}: {target.url()}")
    print("\n".join(summary(manifest, build)))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except Failure as err:
        print(f"error: {err}", file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        sys.exit(130)
