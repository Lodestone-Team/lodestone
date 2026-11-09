#!/usr/bin/env python3
"""Publishes a Lodestone Launcher release that installed launchers update to.

Builds the launcher for this machine's platform, uploads the binary to the
`launcher-latest` GitHub release, then updates the release's `latest.json`, which
the launcher's updater reads (launcher/crates/launcher-core/src/update.rs). On
Windows it also builds the installer (launcher/installer/windows, Inno Setup) and
uploads it as `LodestoneLauncherSetup.exe`, for people installing for the first
time; installed launchers then update themselves.

Each platform is published from a machine of its own: run this on a Mac for
macOS and on a Windows PC for Windows. Each keeps its own build number in
`latest.json`, so publishing one platform doesn't touch the others.

A build's number is the number of commits in its history (`git rev-list --count
HEAD`), so it grows along `main`. By default the script only publishes a clean
checkout of a commit that is on `main` on GitHub, and refuses a build that isn't
newer than the one already published for this platform.

Usage, from anywhere in the repository:

    python3 launcher/scripts/publish_release.py --dry-run   # build, show, upload nothing
    python3 launcher/scripts/publish_release.py             # build and publish
    python3 launcher/scripts/publish_release.py --output DIR --build 2
                                                            # local test, no GitHub

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
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = "Lodestone-Team/lodestone"
TAG = "launcher-latest"
MANIFEST = "latest.json"
# Binaries kept per platform: the newest and the one before it, so a launcher
# that started downloading just before a publish can finish.
KEEP = 2

LAUNCHER = Path(__file__).resolve().parent.parent
ROOT = LAUNCHER.parent
# Release files: the standalone binary (`-standalone`) and the Windows installer
# (`-setup`). Builds published before the standalone suffix have none.
ASSET_NAME = re.compile(
    r"^lodestone-launcher-(\d+)-([a-z0-9]+-[a-z0-9_]+)(-standalone|-setup)?(\.exe)?$"
)
INSTALLER_SCRIPT = LAUNCHER / "installer" / "windows" / "lodestone-launcher.iss"
# The installer under a name that never changes, so the download link doesn't either.
STABLE_INSTALLER = "LodestoneLauncherSetup.exe"


class Failure(Exception):
    pass


def run(cmd: list[str], cwd: Path = ROOT, env: dict | None = None, quiet: bool = True) -> str:
    """Runs a command; returns its output, or raises Failure with its error."""
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
        raise Failure(f"`{' '.join(cmd)}` failed" + (f":\n{detail}" if detail else ""))
    return (result.stdout or "").strip()


def step(message: str) -> None:
    print(f"==> {message}", flush=True)


# ---------------------------------------------------------------------------
# What is being published


def platform_key() -> str:
    """The manifest key for the Rust host target: `windows-x86_64`, `macos-aarch64`.

    It matches `update::platform()` in the launcher, which is
    `std::env::consts::{OS, ARCH}` of the build.
    """
    host = next(
        (line.split(":", 1)[1].strip() for line in run(["rustc", "-vV"], cwd=LAUNCHER).splitlines()
         if line.startswith("host:")),
        "",
    )
    return key_for_target(host)


def key_for_target(triple: str) -> str:
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


def check_checkout(allow_dirty: bool, allow_unmerged: bool) -> None:
    if not allow_dirty:
        changes = run(["git", "status", "--porcelain", "--untracked-files=no"])
        if changes:
            raise Failure(
                "the checkout has uncommitted changes; commit or stash them, or pass --allow-dirty"
            )
    if not allow_unmerged:
        remote = github_remote()
        run(["git", "fetch", "--quiet", remote, "main"])
        try:
            run(["git", "merge-base", "--is-ancestor", "HEAD", f"{remote}/main"])
        except Failure:
            raise Failure(
                f"HEAD isn't on {remote}/main. Publish merged commits (check out main and pull), "
                "or pass --allow-unmerged"
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


def has_installer(key: str) -> bool:
    """Platforms with an installer. The installer script is for x64 Windows."""
    return key == "windows-x86_64"


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


def stale_names(names: list[str], key: str, keep_builds: set[int]) -> list[str]:
    """This platform's binaries other than the newest KEEP builds and `keep_builds`."""
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


# ---------------------------------------------------------------------------
# Building


def build_binary(build: int, commit: str, key: str, into: Path) -> tuple[Path, Path]:
    """Builds the launcher; returns the target directory and a copy named for upload."""
    env = dict(os.environ, LODESTONE_LAUNCHER_BUILD=str(build), LODESTONE_LAUNCHER_COMMIT=commit)
    run(["cargo", "build", "--release", "--locked", "-p", "lodestone-launcher"],
        cwd=LAUNCHER, env=env, quiet=False)
    metadata = json.loads(run(["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=LAUNCHER))
    exe = Path(metadata["target_directory"]) / "release" / (
        "lodestone-launcher.exe" if key.startswith("windows-") else "lodestone-launcher"
    )
    if not exe.is_file():
        raise Failure(f"the build finished but {exe} is missing")
    target = into / asset_name(build, key)
    shutil.copyfile(exe, target)
    return Path(metadata["target_directory"]), target


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


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


# A dry run leaves a copy of its build to test, and a record of it. Publishing
# checks that it uploads exactly that file, so what reaches users is what was
# tested, even if the copy under test updated itself meanwhile.
CANDIDATE_DIR = "release-candidate"
CANDIDATE_RECORD = "candidate.json"


def save_candidate(target_dir: Path, binary: Path, entry: dict, key: str,
                   installer: Path | None = None) -> tuple[Path, Path | None]:
    """Keeps the build (and its installer) to test; returns their copies."""
    folder = target_dir / CANDIDATE_DIR
    if folder.exists():
        shutil.rmtree(folder, ignore_errors=True)
    folder.mkdir(parents=True, exist_ok=True)
    copy = folder / binary.name
    shutil.copyfile(binary, copy)
    if not key.startswith("windows-"):
        copy.chmod(0o755)
    record = {"platform": key, "build": entry["build"], "commit": entry["commit"],
              "sha256": entry["sha256"]}
    installer_copy = None
    if installer is not None:
        # Kept as built: Inno Setup's output isn't byte-for-byte reproducible, so the
        # publish uploads this file rather than building another.
        installer_copy = folder / installer.name
        shutil.copyfile(installer, installer_copy)
        record["installer"] = installer.name
        record["installer_sha256"] = sha256(installer_copy)
    (folder / CANDIDATE_RECORD).write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    return copy, installer_copy


def check_candidate(target_dir: Path, entry: dict, key: str,
                    want_installer: bool = False) -> Path | None:
    """Refuses to publish anything but the build a dry run made for testing.

    Returns the dry run's installer, when one is wanted."""
    file = target_dir / CANDIDATE_DIR / CANDIDATE_RECORD
    if not file.exists():
        raise Failure(
            "no dry run to publish. Run with --dry-run first, test the build it prints on "
            "this machine, then publish"
        )
    record = json.loads(file.read_text(encoding="utf-8"))
    same = (record.get("platform"), record.get("build"), record.get("commit")) == (
        key, entry["build"], entry["commit"])
    if not same:
        raise Failure(
            f"the last dry run was build {record.get('build')} of {str(record.get('commit'))[:7]} "
            f"for {record.get('platform')}, not build {entry['build']} of {entry['commit'][:7]}. "
            "Dry-run and test this build first"
        )
    if record.get("sha256") != entry["sha256"]:
        raise Failure(
            "this build isn't the file the dry run made, so it isn't the one that was tested "
            "(something changed in between). Dry-run and test again"
        )
    if not want_installer:
        return None
    name = record.get("installer")
    installer = target_dir / CANDIDATE_DIR / name if name else None
    if installer is None or not installer.is_file():
        raise Failure("the dry run didn't keep an installer. Dry-run and test again")
    if sha256(installer) != record.get("installer_sha256"):
        raise Failure("the dry run's installer has changed since it was made. Dry-run and test again")
    return installer


def describe(path: Path, build: int, commit: str) -> dict:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1 << 20), b""):
            digest.update(chunk)
    return {
        "name": path.name,
        "size": path.stat().st_size,
        "sha256": digest.hexdigest(),
        "build": build,
        "commit": commit,
    }


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

    def manifest(self) -> dict | None:
        if MANIFEST not in self.names():
            return None
        with tempfile.TemporaryDirectory() as tmp:
            self.gh("release", "download", self.tag, "--repo", self.repo,
                    "--pattern", MANIFEST, "--dir", tmp, "--clobber")
            return json.loads(Path(tmp, MANIFEST).read_text(encoding="utf-8"))

    def ensure_release(self, commit: str) -> None:
        if not self.exists():
            step(f"Creating the {self.tag} release")
            self.gh("release", "create", self.tag, "--repo", self.repo, "--target", commit,
                    "--prerelease", "--latest=false", "--title", "Lodestone Launcher",
                    "--notes", "Lodestone Launcher builds. Installed launchers update from here.")

    def upload(self, path: Path) -> None:
        self.gh("release", "upload", self.tag, str(path), "--repo", self.repo, "--clobber")

    def delete(self, name: str) -> None:
        self.gh("release", "delete-asset", self.tag, name, "--repo", self.repo, "--yes")

    def point_at(self, commit: str, manifest: dict) -> None:
        self.gh("api", "-X", "PATCH", f"repos/{self.repo}/git/refs/tags/{self.tag}",
                "-f", f"sha={commit}", "-F", "force=true")
        notes = ["Lodestone Launcher builds. Installed launchers update from here.", ""]
        notes += [f"- {key}: build {asset['build']} ({str(asset.get('commit') or '')[:7]})"
                  for key, asset in manifest["assets"].items()]
        if any(has_installer(key) for key in manifest["assets"]):
            notes += ["", f"**New on Windows?** Download `{STABLE_INSTALLER}` below and run it. "
                      "It installs for your account without administrator rights, and the "
                      "launcher keeps itself up to date from then on.",
                      "",
                      "The `-standalone` files run without installing, from wherever you put "
                      "them, and update themselves too. The `-setup` files are installers."]
        notes += ["", "Published with `launcher/scripts/publish_release.py`."]
        self.gh("release", "edit", self.tag, "--repo", self.repo, "--notes", "\n".join(notes))

    def url(self) -> str:
        return f"https://github.com/{self.repo}/releases/tag/{self.tag}"


class Folder:
    """A local stand-in for the release, to test the updater against."""

    def __init__(self, path: Path):
        self.path = path

    def check(self) -> None:
        pass

    def names(self) -> list[str]:
        if not self.path.is_dir():
            return []
        return sorted(p.name for p in self.path.iterdir() if p.is_file())

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


# ---------------------------------------------------------------------------


def main(argv: list[str]) -> int:
    # Keep our lines in order with cargo's and gh's output when piped.
    sys.stdout.reconfigure(line_buffering=True)
    parser = argparse.ArgumentParser(description="Publish a Lodestone Launcher release.")
    parser.add_argument("--dry-run", action="store_true",
                        help="build and show the new latest.json, but upload nothing")
    parser.add_argument("--output", type=Path, metavar="DIR",
                        help="publish into a local folder instead of GitHub (for testing)")
    parser.add_argument("--build", type=int,
                        help="use this build number instead of the commit count (testing only)")
    parser.add_argument("--force", action="store_true",
                        help="publish even if this platform already has this build or a newer one")
    parser.add_argument("--allow-dirty", action="store_true",
                        help="publish a checkout with uncommitted changes")
    parser.add_argument("--no-installer", action="store_true",
                        help="on Windows, publish the binary without building the installer")
    parser.add_argument("--allow-unmerged", action="store_true",
                        help="publish a commit that isn't on main on GitHub")
    parser.add_argument("--repo", default=REPO, help=argparse.SUPPRESS)
    # A different release to rehearse a real upload in; launchers never read it.
    parser.add_argument("--tag", default=TAG, help=argparse.SUPPRESS)
    args = parser.parse_args(argv)

    test_tag = args.tag != TAG
    if args.build is not None and args.output is None and not args.dry_run and not test_tag:
        raise Failure(
            "--build is for tests; use it with --output, --dry-run, or a test --tag"
        )

    target = Folder(args.output.resolve()) if args.output else GitHub(args.repo, args.tag)
    local = args.output is not None

    step("Checking the checkout")
    if local:
        print("    Local test: not checking for a clean checkout on main.")
    check_checkout(allow_dirty=args.allow_dirty or local, allow_unmerged=args.allow_unmerged or local)
    target.check()
    key = platform_key()
    commit = run(["git", "rev-parse", "HEAD"])
    build = args.build if args.build is not None else int(run(["git", "rev-list", "--count", "HEAD"]))
    version = workspace_version()
    print(f"    {key}, build {build}, commit {commit[:7]}, version {version}")
    with_installer = has_installer(key) and not args.no_installer
    if with_installer:
        find_iscc()  # before the long build, not after it

    current = target.manifest()
    published = (current or {}).get("assets", {}).get(key)
    if published:
        have = published.get("build", current.get("build"))
        if have >= build and not args.force:
            raise Failure(
                f"{key} is already at build {have}, and this is build {build}. Publish a newer "
                "commit, or pass --force to replace it"
            )

    with tempfile.TemporaryDirectory() as tmp:
        step(f"Building the launcher for {key} (release)")
        target_dir, binary = build_binary(build, commit, key, Path(tmp))
        entry = describe(binary, build, commit)
        now = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

        if args.dry_run:
            installer = None
            if with_installer:
                step("Building the Windows installer")
                installer = build_installer(binary, build, version, key, Path(tmp))
            manifest = updated_manifest(current, key, entry, version, now)
            step("Dry run: nothing uploaded. latest.json would be:")
            print(json.dumps(manifest, indent=2))
            print(f"    The binary is {binary.name}, {entry['size']} bytes.")
            candidate, installer_copy = save_candidate(target_dir, binary, entry, key, installer)
            step("Test this build on this machine before publishing it:")
            print(f"    {candidate}")
            if installer_copy:
                print("    and its installer (install, open, uninstall):")
                print(f"    {installer_copy}")
            print("    Then publish from this checkout without changes; the script checks that it")
            print("    uploads these exact files.")
            return 0

        installer = None
        if not local and not test_tag:
            installer = check_candidate(target_dir, entry, key, want_installer=with_installer)
        elif with_installer:
            step("Building the Windows installer")
            installer = build_installer(binary, build, version, key, Path(tmp))

        target.ensure_release(commit)
        step(f"Uploading {binary.name}")
        target.upload(binary)
        if installer:
            step(f"Uploading {installer.name} and {STABLE_INSTALLER}")
            target.upload(installer)
            stable = Path(tmp) / STABLE_INSTALLER
            shutil.copyfile(installer, stable)
            target.upload(stable)
        # Read the manifest again just before replacing it, so a platform published
        # meanwhile from another machine is kept.
        manifest = updated_manifest(target.manifest(), key, entry, version, now)
        manifest_file = Path(tmp) / MANIFEST
        manifest_file.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        step(f"Uploading {MANIFEST}")
        target.upload(manifest_file)

    if manifest["build"] == build:
        target.point_at(commit, manifest)
    for name in stale_names(target.names(), key, keep_builds={manifest["assets"][key]["build"]}):
        step(f"Removing the old {name}")
        target.delete(name)

    step(f"Published build {build} for {key}: {target.url()}")
    print("\n".join(summary(manifest, build)))
    if local:
        print(f"    Serve {target.url()} over HTTP and set LODESTONE_LAUNCHER_UPDATE_URL to its "
              f"{MANIFEST} to test updating against it.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except Failure as err:
        print(f"error: {err}", file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        sys.exit(130)
