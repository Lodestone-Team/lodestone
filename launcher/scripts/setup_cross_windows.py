#!/usr/bin/env python3
"""Sets up a Mac to build Lodestone Launcher's and Lodestone Host's Windows releases.

`publish_release.py --target x86_64-pc-windows-msvc` builds the Windows programs and
installer on a Mac with what this installs (launcher/docs/cross-windows.md):

- Homebrew's llvm (clang-cl, llvm-lib), lld (lld-link) and msitools (msiextract).
- cargo-xwin (pinned), which gives cargo the environment to build for Windows
  with those, and the Rust toolchain's x86_64-pc-windows-msvc target.
- Microsoft's C runtime and Windows SDK libraries, which cargo-xwin downloads
  into its cache, only with --accept-microsoft-license.
- Wine (Gcenx's macOS build, pinned by SHA-256; it runs under Rosetta), and in
  its own Wine prefix Inno Setup 6 (pinned by SHA-256), which builds the installer.
- fxc.exe, Microsoft's HLSL compiler from the Windows SDK, which GPUI's shaders
  compile with, located through the Visual Studio manifest (itself checked against
  the SHA-256 the channel lists) and checked against the SHA-256 the manifest
  lists; also only with --accept-microsoft-license, as it's under the Windows
  SDK's license terms.
- `bin/fxc` and `bin/iscc`, which run fxc and Inno Setup's compiler under Wine and
  turn the Unix paths they're given into Wine's (Z:\\...).

Everything goes into one folder, ~/Library/Application Support/Lodestone/cross-windows
(or $LODESTONE_CROSS_WINDOWS_DIR, or --dir), where publish_release.py finds it, so
nothing needs setting. Each download goes to a folder of its own under
`downloads/` and is checked before it's used. Running it again only does what's
missing; --check reports what's there without downloading or installing anything
(and runs nothing under Wine unless everything Wine needs is there).

Usage:

    python3 launcher/scripts/setup_cross_windows.py --check
    python3 launcher/scripts/setup_cross_windows.py
    python3 launcher/scripts/setup_cross_windows.py --accept-microsoft-license

Uses only the standard library. It never changes security settings: nothing it
downloads is quarantined (it downloads them itself), and Rosetta, if it's missing,
is for the developer to install.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shlex
import shutil
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))
import publish_release as pr  # noqa: E402  (the folder layout and checks it reads)

ROOT = SCRIPTS.parent.parent
TARGET = pr.CROSS_WINDOWS_TARGET
CARGO_XWIN_VERSION = pr.CARGO_XWIN_VERSION
BREW_FORMULAS = ("llvm", "lld", "msitools")


@dataclass(frozen=True)
class Download:
    url: str
    sha256: str
    name: str


# Gcenx's Wine builds for macOS (github.com/Gcenx/macOS_Wine_builds). Homebrew's wine
# casks are disabled, as they fail Gatekeeper.
WINE = Download(
    "https://github.com/Gcenx/macOS_Wine_builds/releases/download/11.0_1/"
    "wine-stable-11.0_1-osx64.tar.xz",
    "b50dc50ec7f41d58b115a6b685d4d1315ba3c797bd3aa0f49213f2703cb82388",
    "wine-stable-11.0_1-osx64.tar.xz",
)
WINE_VERSION = "11.0_1"
INNO_SETUP = Download(
    "https://github.com/jrsoftware/issrc/releases/download/is-6_7_4/innosetup-6.7.4.exe",
    "a549929319f9120716677c685535ca4671e4b937b5fdc87e2585f32d8eb30e8c",
    "innosetup-6.7.4.exe",
)
INNO_SETUP_VERSION = "6.7.4"
# Visual Studio's release channel lists the manifest of everything it installs, the
# Windows SDK's files among them, each with its URL and SHA-256.
VS_CHANNEL = "https://aka.ms/vs/17/release/channel"
VS_MANIFEST_ID = "Microsoft.VisualStudio.Manifests.VisualStudio"
# The Windows SDK fxc.exe comes from (the newest Windows 11 SDK if the manifest no
# longer has it), and its installer that holds fxc.exe and D3DCOMPILER_47.dll.
SDK_PACKAGE = "Win11SDK_10.0.26100"
SDK_TOOLS_MSI = "Windows SDK for Windows Store Apps Tools-x86_en-us.msi"
# Quiet Wine: no debug output, no offer to install Mono or Gecko, quiet MoltenVK.
WINE_ENV = {"WINEDEBUG": "-all", "WINEDLLOVERRIDES": "mscoree,mshtml=",
            "MVK_CONFIG_LOG_LEVEL": "0"}
INNO_DIR = r"C:\InnoSetup6"
# The Windows SDK's license terms, which fxc.exe and D3DCOMPILER_47.dll come under
# (the SDK's download page shows them; the SDK's installers carry them too).
WINDOWS_SDK_LICENSE = "https://developer.microsoft.com/windows/downloads/windows-sdk/"
LICENSE_FLAG = "--accept-microsoft-license"


class Failure(Exception):
    pass


class Layout:
    """The setup folder."""

    def __init__(self, root: Path):
        self.root = root
        self.downloads = root / "downloads"
        self.wine_dir = root / "wine"
        wine_bin = self.wine_dir / "Wine Stable.app" / "Contents" / "Resources" / "wine" / "bin"
        self.wine = wine_bin / "wine"
        self.wineserver = wine_bin / "wineserver"
        self.wine_record = self.wine_dir / "archive.sha256"
        self.sdk_dir = root / "windows-sdk"
        self.sdk_record = self.sdk_dir / "fxc.json"
        self.prefix = root / "wine-prefix"
        self.iscc_exe = self.prefix / "drive_c" / "InnoSetup6" / "ISCC.exe"
        # The SHA-256 of the Inno Setup installer that finished installing there.
        self.inno_record = self.prefix / "inno-setup.sha256"
        self.bin = root / "bin"
        self.fxc_wrapper = self.bin / "fxc"
        self.iscc_wrapper = self.bin / "iscc"

    def fxc_exe(self) -> Path | None:
        try:
            record = json.loads(self.sdk_record.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            return None
        fxc = self.sdk_dir / record.get("fxc", "")
        return fxc if fxc.is_file() and record.get("fxc") else None


# ---------------------------------------------------------------------------
# Helpers


def say(message: str) -> None:
    print(f"==> {message}", flush=True)


def note(message: str) -> None:
    print(f"    {message}", flush=True)


def run(cmd: list[str], cwd: Path | None = None, env: dict | None = None,
        quiet: bool = True) -> str:
    try:
        result = subprocess.run(cmd, cwd=cwd, env=env, text=True, encoding="utf-8",
                                errors="replace", stdin=subprocess.DEVNULL,
                                stdout=subprocess.PIPE if quiet else None,
                                stderr=subprocess.STDOUT if quiet else None)
    except FileNotFoundError:
        raise Failure(f"`{cmd[0]}` isn't installed or isn't on PATH")
    if result.returncode != 0:
        detail = (result.stdout or "").strip() if quiet else ""
        raise Failure(f"`{' '.join(cmd)}` failed" + (f":\n{detail}" if detail else ""))
    return (result.stdout or "").strip()


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


class HttpsOnlyRedirects(urllib.request.HTTPRedirectHandler):
    """Follows redirects only to https URLs."""

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        if urllib.parse.urlsplit(newurl).scheme != "https":
            raise Failure(f"{req.full_url} redirects to {newurl}, which isn't https")
        return super().redirect_request(req, fp, code, msg, headers, newurl)


OPENER = urllib.request.build_opener(HttpsOnlyRedirects)


def check_https(url: object) -> str:
    if not isinstance(url, str) or urllib.parse.urlsplit(url).scheme != "https":
        raise Failure(f"refusing to download {url!r}: it isn't an https URL")
    return url


def open_url(url: str, timeout: int):
    """`url` (https only, redirects too) opened for reading."""
    return OPENER.open(check_https(url), timeout=timeout)


def fetch(url: str) -> bytes:
    try:
        with open_url(url, 120) as response:
            return response.read()
    except (urllib.error.URLError, OSError, ValueError) as err:
        raise Failure(f"couldn't download {url}: {err}")


def fetch_json(url: str) -> dict:
    try:
        data = json.loads(fetch(url))
    except ValueError as err:
        raise Failure(f"{url} isn't the JSON expected: {err}")
    if not isinstance(data, dict):
        raise Failure(f"{url} isn't the JSON object expected")
    return data


def have_download(download: Download, folder: Path) -> Path | None:
    path = folder / download.name
    if path.is_file() and sha256(path) == download.sha256.lower():
        return path
    return None


def ensure_download(download: Download, folder: Path) -> tuple[Path, bool]:
    """`download` in `folder`, checked against its SHA-256; downloads it if it isn't
    there (or doesn't match). Returns it, and whether it was downloaded now."""
    path = have_download(download, folder)
    if path:
        return path, False
    folder.mkdir(parents=True, exist_ok=True)
    path = folder / download.name
    partial = folder / (download.name + ".partial")
    digest = hashlib.sha256()
    size = 0
    try:
        with open_url(download.url, 300) as response, partial.open("wb") as file:
            for chunk in iter(lambda: response.read(1 << 20), b""):
                digest.update(chunk)
                file.write(chunk)
                size += len(chunk)
    except (urllib.error.URLError, OSError, ValueError) as err:
        partial.unlink(missing_ok=True)
        raise Failure(f"couldn't download {download.url}: {err}")
    except Failure:
        partial.unlink(missing_ok=True)
        raise
    if digest.hexdigest() != download.sha256.lower():
        partial.unlink()
        raise Failure(f"{download.url} isn't the file expected: its SHA-256 is "
                      f"{digest.hexdigest()}, not {download.sha256.lower()}")
    partial.replace(path)
    note(f"downloaded {download.name} ({size / 1e6:.1f} MB), SHA-256 checked")
    return path, True


def find_program(name: str) -> str | None:
    found = shutil.which(name)
    if found:
        return found
    for prefix in ("/opt/homebrew", "/usr/local"):
        candidate = Path(prefix) / "bin" / name
        if candidate.is_file():
            return str(candidate)
    return None


def wine_env(layout: Layout) -> dict:
    return dict(os.environ, WINEPREFIX=str(layout.prefix), **WINE_ENV)


def toolchain_channel() -> str:
    text = (ROOT / "rust-toolchain.toml").read_text(encoding="utf-8")
    match = re.search(r'^\s*channel\s*=\s*"([^"]+)"', text, re.M)
    if not match:
        raise Failure("rust-toolchain.toml doesn't name a channel")
    return match.group(1)


# ---------------------------------------------------------------------------
# What it sets up. Each step returns (state, detail): "ok" (it was there), "done"
# (set up now), or "missing" (not there, with what to do).


def step_rosetta(check: bool) -> tuple[str, str]:
    if platform.machine() != "arm64":
        return "ok", "not needed on an Intel Mac"
    try:
        run(["arch", "-x86_64", "/usr/bin/true"])
    except Failure:
        return "missing", ("Wine needs Rosetta 2, which isn't installed. Install it yourself: "
                           "`softwareupdate --install-rosetta`")
    return "ok", "Rosetta 2 runs x86_64 programs"


def step_brew(check: bool) -> tuple[str, str]:
    brew = find_program("brew")
    if not brew:
        return "missing", "Homebrew isn't installed (https://brew.sh)"
    have, missing = [], []
    for formula in BREW_FORMULAS:
        try:
            have.append(run([brew, "list", "--formula", "--versions", formula]))
        except Failure:
            missing.append(formula)
    if not missing:
        return "ok", ", ".join(have)
    if check:
        return "missing", f"not installed: {', '.join(missing)} (`brew install {' '.join(missing)}`)"
    run([brew, "install", *missing], quiet=False)
    return "done", f"installed {', '.join(missing)}"


def installed_cargo_xwin() -> str | None:
    try:
        out = run(["cargo", "install", "--list"], cwd=ROOT)
    except Failure:
        return None
    match = re.search(r"^cargo-xwin v(\S+):", out, re.M)
    return match.group(1) if match else None


def step_cargo_xwin(check: bool) -> tuple[str, str]:
    version = installed_cargo_xwin()
    if version == CARGO_XWIN_VERSION and pr.find_cargo_xwin():
        return "ok", f"cargo-xwin {version}"
    if check:
        return "missing", (f"cargo-xwin {CARGO_XWIN_VERSION} isn't installed"
                           + (f" ({version} is)" if version else ""))
    run(["cargo", "install", "--locked", "cargo-xwin", "--version", CARGO_XWIN_VERSION],
        cwd=ROOT, quiet=False)
    return "done", f"installed cargo-xwin {CARGO_XWIN_VERSION}"


def step_rust_target(check: bool) -> tuple[str, str]:
    channel = toolchain_channel()
    if not find_program("rustup") and not shutil.which("rustup"):
        return "missing", "rustup isn't installed (https://rustup.rs)"
    installed = run(["rustup", "target", "list", "--installed", "--toolchain", channel], cwd=ROOT)
    if TARGET in installed.split():
        return "ok", f"{TARGET} for Rust {channel}"
    if check:
        return "missing", f"`rustup target add {TARGET} --toolchain {channel}`"
    run(["rustup", "target", "add", TARGET, "--toolchain", channel], cwd=ROOT, quiet=False)
    return "done", f"added {TARGET} to Rust {channel}"


def step_wine(layout: Layout, check: bool) -> tuple[str, str]:
    unpacked = (layout.wine.is_file() and layout.wine_record.is_file()
                and layout.wine_record.read_text().strip() == WINE.sha256)
    if unpacked:
        return "ok", f"Wine {WINE_VERSION} in {layout.wine_dir}"
    if check:
        return "missing", f"Wine {WINE_VERSION} isn't unpacked in {layout.wine_dir}"
    archive, _ = ensure_download(WINE, layout.downloads / "wine")
    partial = layout.root / "wine.partial"
    if partial.exists():
        shutil.rmtree(partial)
    partial.mkdir(parents=True)
    # bsdtar refuses absolute paths and `..` unless asked to keep them.
    run(["tar", "-xf", str(archive), "-C", str(partial)])
    if not (partial / layout.wine.relative_to(layout.wine_dir)).is_file():
        raise Failure(f"{archive.name} has no {layout.wine.relative_to(layout.wine_dir)}")
    (partial / layout.wine_record.name).write_text(WINE.sha256 + "\n")
    if layout.wine_dir.exists():
        shutil.rmtree(layout.wine_dir)
    partial.rename(layout.wine_dir)
    return "done", f"unpacked Wine {WINE_VERSION} into {layout.wine_dir}"


def sdk_package(manifest: dict) -> tuple[dict, str | None]:
    """The Windows SDK to take fxc.exe from: SDK_PACKAGE, or the newest Windows 11 SDK
    (with a note) if the manifest no longer lists it."""
    packages = manifest.get("packages")
    if not isinstance(packages, list):
        raise Failure("Visual Studio's manifest has no package list")
    sdks = [p for p in packages
            if isinstance(p, dict) and str(p.get("id", "")).startswith("Win11SDK_10.0.")]
    for package in sdks:
        if package["id"] == SDK_PACKAGE:
            return package, None
    if not sdks:
        raise Failure("Visual Studio's manifest lists no Windows 11 SDK")
    newest = max(sdks, key=lambda p: [int(n) for n in re.findall(r"\d+", p["id"])])
    return newest, f"the manifest no longer has {SDK_PACKAGE}; using {newest['id']}"


def sdk_payloads(package: dict) -> dict[str, dict]:
    """The SDK's files by name (`Installers/x.msi` -> `x.msi`)."""
    payloads = package.get("payloads")
    if not isinstance(payloads, list):
        raise Failure(f"{package.get('id')} lists no files in Visual Studio's manifest")
    return {str(p["fileName"]).replace("\\", "/").rsplit("/", 1)[-1]: p
            for p in payloads if isinstance(p, dict) and p.get("fileName")}


def media_cabinets(media_table: str) -> list[str]:
    """The cabinet files an MSI's Media table (`msiinfo export X.msi Media`) names."""
    cabs = []
    for line in media_table.splitlines():
        for column in line.split("\t"):
            column = column.strip()
            if column.lower().endswith(".cab") and column not in cabs:
                cabs.append(column)
    return cabs


def find_fxc(folder: Path) -> Path | None:
    """The x64 fxc.exe in an unpacked SDK, with D3DCOMPILER_47.dll next to it."""
    found = sorted(folder.glob("**/bin/*/x64/fxc.exe"))
    for fxc in reversed(found):
        if any(f.name.lower() == "d3dcompiler_47.dll" for f in fxc.parent.iterdir()):
            return fxc
    return None


def manifest_download(channel: dict) -> Download:
    """Visual Studio's manifest, as the channel lists it (https URL and SHA-256)."""
    items = channel.get("channelItems")
    item = next((i for i in items if isinstance(i, dict) and i.get("type") == "Manifest"
                 and i.get("id") == VS_MANIFEST_ID), None) if isinstance(items, list) else None
    if not item:
        raise Failure(f"Visual Studio's channel has no {VS_MANIFEST_ID}")
    payloads = item.get("payloads")
    payload = payloads[0] if isinstance(payloads, list) and payloads else None
    if not isinstance(payload, dict) or not payload.get("url") or not payload.get("sha256"):
        raise Failure(f"Visual Studio's channel lists {VS_MANIFEST_ID} without a URL and SHA-256")
    return Download(check_https(payload["url"]), str(payload["sha256"]), "VisualStudio.vsman")


def payload_download(package: dict, payloads: dict[str, dict], name: str) -> Download:
    """One of the SDK's files (https URL and SHA-256, from the manifest)."""
    payload = payloads.get(name)
    if not payload:
        raise Failure(f"{package.get('id')} has no {name}")
    if not payload.get("url") or not payload.get("sha256"):
        raise Failure(f"Visual Studio's manifest lists {name} without a URL and SHA-256")
    return Download(check_https(payload["url"]), str(payload["sha256"]), name)


def sdk_license_missing() -> str:
    return (
        "fxc.exe and D3DCOMPILER_47.dll come from Microsoft's Windows SDK, under the Windows "
        f"SDK's license terms ({WINDOWS_SDK_LICENSE}). Once you've read and accepted them, run "
        f"this again with {LICENSE_FLAG} (it covers these and the C runtime below)"
    )


def step_fxc(layout: Layout, check: bool, accept: bool) -> tuple[str, str]:
    fxc = layout.fxc_exe()
    if fxc:
        record = json.loads(layout.sdk_record.read_text(encoding="utf-8"))
        return "ok", f"fxc.exe from {record.get('package')} {record.get('version')}"
    if not accept:
        return "missing", sdk_license_missing()
    if check:
        return "missing", f"fxc.exe isn't unpacked in {layout.sdk_dir}"
    msiinfo, msiextract = find_program("msiinfo"), find_program("msiextract")
    if not (msiinfo and msiextract):
        raise Failure("msitools (msiinfo, msiextract) isn't installed: `brew install msitools`")
    note(f"reading Visual Studio's manifest ({VS_CHANNEL})")
    vsman, _ = ensure_download(manifest_download(fetch_json(VS_CHANNEL)),
                               layout.downloads / "vs-manifest")
    try:
        manifest = json.loads(vsman.read_text(encoding="utf-8"))
    except ValueError as err:
        raise Failure(f"Visual Studio's manifest isn't the JSON expected: {err}")
    if not isinstance(manifest, dict):
        raise Failure("Visual Studio's manifest isn't the JSON object expected")
    package, why = sdk_package(manifest)
    if why:
        note(why)
    payloads = sdk_payloads(package)
    folder = layout.downloads / "windows-sdk"

    def get(name: str) -> Path:
        path, _ = ensure_download(payload_download(package, payloads, name), folder)
        return path

    msi = get(SDK_TOOLS_MSI)
    for cab in media_cabinets(run([msiinfo, "export", str(msi), "Media"])):
        get(cab)
    partial = layout.root / "windows-sdk.partial"
    if partial.exists():
        shutil.rmtree(partial)
    partial.mkdir(parents=True)
    # msiextract finds the cabinets next to the MSI.
    run([msiextract, "-C", str(partial), str(msi)], cwd=folder)
    fxc = find_fxc(partial)
    if not fxc:
        raise Failure(f"{SDK_TOOLS_MSI} has no x64 fxc.exe with D3DCOMPILER_47.dll")
    record = {"package": package["id"], "version": package.get("version"),
              "fxc": fxc.relative_to(partial).as_posix()}
    (partial / layout.sdk_record.name).write_text(json.dumps(record, indent=2) + "\n")
    if layout.sdk_dir.exists():
        shutil.rmtree(layout.sdk_dir)
    partial.rename(layout.sdk_dir)
    return "done", f"unpacked fxc.exe from {package['id']} {package.get('version')}"


def installed_inno_setup(layout: Layout) -> str | None:
    """The SHA-256 of the Inno Setup installer that finished installing in the prefix."""
    if not layout.iscc_exe.is_file():
        return None
    try:
        return layout.inno_record.read_text(encoding="utf-8").strip() or None
    except OSError:
        return None


def step_inno_setup(layout: Layout, check: bool) -> tuple[str, str]:
    installed = installed_inno_setup(layout)
    if installed == INNO_SETUP.sha256:
        return "ok", f"Inno Setup {INNO_SETUP_VERSION} in {layout.prefix} ({INNO_DIR})"
    if not layout.iscc_exe.is_file():
        state = f"Inno Setup {INNO_SETUP_VERSION} isn't installed in {layout.prefix}"
    elif installed:
        state = (f"the Inno Setup in {layout.prefix} isn't {INNO_SETUP_VERSION}, the pinned one "
                 "(it was installed from another installer)")
    else:
        state = (f"there's an Inno Setup in {layout.prefix}, but no record that "
                 f"{INNO_SETUP_VERSION}'s installer finished installing it")
    if check:
        return "missing", state
    note(state + "; installing it")
    layout.inno_record.unlink(missing_ok=True)
    if not layout.wine.is_file():
        raise Failure("Wine isn't set up, so Inno Setup can't be installed")
    installer, _ = ensure_download(INNO_SETUP, layout.downloads / "inno-setup")
    note(f"installing Inno Setup {INNO_SETUP_VERSION} into {layout.prefix} under Wine "
         "(a new Wine prefix takes a minute)")
    env = wine_env(layout)
    run([str(layout.wine), "wineboot", "--init"], env=env)
    run([str(layout.wineserver), "-w"], env=env)
    # Inno Setup's installer starts itself again from a temporary folder, and under
    # Wine that fails (exit code 41) when its own path has a space in it, as
    # "Application Support" does. So it runs from C:\ in the prefix.
    staging = layout.prefix / "drive_c" / "lodestone-setup"
    staging.mkdir(parents=True, exist_ok=True)
    copy = staging / installer.name
    shutil.copyfile(installer, copy)
    try:
        if sha256(copy) != INNO_SETUP.sha256:
            raise Failure(f"the copy of {installer.name} in {staging} doesn't match its SHA-256")
        run([str(layout.wine), rf"C:\{staging.name}\{installer.name}", "/VERYSILENT",
             "/SUPPRESSMSGBOXES", "/NORESTART", "/CURRENTUSER", f"/DIR={INNO_DIR}", "/NOICONS"],
            env=env)
        run([str(layout.wineserver), "-w"], env=env)
    finally:
        shutil.rmtree(staging, ignore_errors=True)
    if not layout.iscc_exe.is_file():
        raise Failure(f"Inno Setup's installer finished, but there's no {layout.iscc_exe}")
    layout.inno_record.write_text(INNO_SETUP.sha256 + "\n", encoding="utf-8")
    return "done", f"installed Inno Setup {INNO_SETUP_VERSION} ({INNO_DIR} in the prefix)"


def wrapper(comment: list[str], layout: Layout, program: Path, defines: bool) -> str:
    """A bash script that runs `program` under Wine, with Unix paths in its arguments
    (anything starting with / that has another /) turned into Wine's Z:\\ paths, and
    with `defines`, those in Inno Setup's /DName=/path too. Switches (/T, /Q, /DBuild=1)
    pass through."""
    q = shlex.quote
    lines = ["#!/bin/bash", *(f"# {line}" for line in comment),
             "# Written by launcher/scripts/setup_cross_windows.py; run it again to rewrite it.",
             f"export WINEPREFIX={q(str(layout.prefix))}"]
    lines += [f"export {name}={q(value)}" for name, value in WINE_ENV.items()]
    lines += [f"WINE={q(str(layout.wine))}", f"PROGRAM={q(str(program))}",
              'win() { local p="$1"; printf \'Z:%s\' "${p//\\//\\\\}"; }',
              "args=()",
              'for a in "$@"; do']
    if defines:
        lines += ['  if [[ "$a" =~ ^(/D[A-Za-z_]+=)(/.*/.*)$ ]]; then '
                  'a="${BASH_REMATCH[1]}$(win "${BASH_REMATCH[2]}")"',
                  '  elif [[ "$a" == /*/* ]]; then a="$(win "$a")"; fi']
    else:
        lines += ['  if [[ "$a" == /*/* ]]; then a="$(win "$a")"; fi']
    lines += ['  args+=("$a")', "done", 'exec "$WINE" "$PROGRAM" "${args[@]}"']
    return "\n".join(lines) + "\n"


def write_if_changed(path: Path, text: str) -> bool:
    if path.is_file() and path.read_text(encoding="utf-8") == text and os.access(path, os.X_OK):
        return False
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    path.chmod(0o755)
    return True


def step_wrappers(layout: Layout, check: bool) -> tuple[str, str]:
    fxc = layout.fxc_exe()
    if not (fxc and layout.wine.is_file() and installed_inno_setup(layout) == INNO_SETUP.sha256):
        return "missing", "waiting for Wine, fxc.exe and Inno Setup"
    scripts = {
        layout.fxc_wrapper: wrapper(
            ["fxc.exe under Wine, for GPUI's shaders when building for Windows on a Mac",
             "(publish_release.py passes it to cargo as GPUI_FXC_PATH)."], layout, fxc, False),
        layout.iscc_wrapper: wrapper(
            ["Inno Setup 6's compiler under Wine, for the Windows installer on a Mac",
             "(publish_release.py runs it as ISCC)."], layout, layout.iscc_exe, True),
    }
    stale = [p for p, text in scripts.items()
             if not (p.is_file() and p.read_text(encoding="utf-8") == text)]
    if not stale:
        return "ok", f"{layout.fxc_wrapper} and {layout.iscc_wrapper}"
    if check:
        return "missing", "out of date: " + ", ".join(p.name for p in stale)
    for path, text in scripts.items():
        write_if_changed(path, text)
    return "done", "wrote " + ", ".join(str(p) for p in stale)


def step_crt(check: bool, accept: bool) -> tuple[str, str]:
    cache = pr.xwin_cache_dir()
    if pr.has_xwin_crt():
        return "ok", f"in cargo-xwin's cache ({cache})"
    if not accept:
        return "missing", (
            "Microsoft's C runtime (MSVC CRT) and Windows SDK libraries and headers, which "
            "Windows programs link against, aren't in cargo-xwin's cache. cargo-xwin downloads "
            "them from Microsoft's Visual Studio servers; they're under Microsoft's license "
            f"terms for them ({pr.MICROSOFT_LICENSE}). Once you've read and accepted them, run "
            "this again with --accept-microsoft-license"
        )
    if check:
        return "missing", "not downloaded yet (run this without --check to download them)"
    note(f"downloading them with cargo-xwin into {cache} (about 1 GB unpacked)")
    env = dict(os.environ, XWIN_ACCEPT_LICENSE="1", PATH=pr.cross_tool_path())
    run(["cargo", "xwin", "cache", "xwin"], cwd=ROOT, env=env, quiet=False)
    if not pr.has_xwin_crt():
        raise Failure(f"cargo-xwin finished, but {cache / 'xwin' / 'DONE'} doesn't list x86_64")
    return "done", f"downloaded into {cache}"


# The steps that must all be there before the wrappers run anything under Wine: Wine
# creates (or upgrades) its prefix when it runs, so a check never runs it before.
WINE_STEPS = ("Wine", "fxc.exe", "Inno Setup", "Wrappers")


def final_check(layout: Layout, wine_ready: bool) -> list[tuple[str, bool | None, str]]:
    """Runs fxc and Inno Setup's compiler under Wine (only when `wine_ready`: every
    WINE_STEPS step is there), and publish_release.py's own check of what a Windows
    build needs. Each result is ok (True), failed (False) or not run (None)."""
    results: list[tuple[str, bool | None, str]] = []
    for name, path, expect in (("fxc /?", layout.fxc_wrapper, "Direct3D Shader Compiler"),
                               ("iscc /?", layout.iscc_wrapper, "Inno Setup 6")):
        if not wine_ready:
            results.append((name, None, f"waiting for {', '.join(WINE_STEPS)}"))
            continue
        # Both print their usage and exit with 1.
        try:
            result = subprocess.run([str(path), "/?"], text=True, errors="replace",
                                    stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                    stderr=subprocess.STDOUT, timeout=300)
        except subprocess.TimeoutExpired:
            results.append((name, False, f"{path} didn't finish within 5 minutes"))
            continue
        except OSError as err:
            results.append((name, False, f"couldn't run {path}: {err}"))
            continue
        out = result.stdout or ""
        first = next((line.strip() for line in out.splitlines() if expect in line), None)
        results.append((name, bool(first), first or out.strip()[:300]))
    os.environ.setdefault(pr.CROSS_WINDOWS_DIR_ENV, str(layout.root))
    problems = pr.cross_windows_problems(TARGET, installer=True)
    results.append(("publish_release.py's check", not problems,
                    "nothing missing" if not problems else "; ".join(problems)))
    return results


def main(argv: list[str]) -> int:
    sys.stdout.reconfigure(line_buffering=True)
    parser = argparse.ArgumentParser(
        description="Set up this Mac to build Lodestone's Windows releases "
                    "(launcher/docs/cross-windows.md).")
    parser.add_argument("--check", action="store_true",
                        help="only report what's there; download and install nothing")
    parser.add_argument("--accept-microsoft-license", action="store_true",
                        help="you have read and accept Microsoft's license terms for the "
                             f"MSVC C runtime and Windows SDK libraries ({pr.MICROSOFT_LICENSE}),"
                             " which cargo-xwin downloads, and the Windows SDK's "
                             f"({WINDOWS_SDK_LICENSE}), which fxc.exe and D3DCOMPILER_47.dll "
                             "come under: let this download them")
    parser.add_argument("--dir", type=Path,
                        help="the folder to set up (default: $LODESTONE_CROSS_WINDOWS_DIR, or "
                             f"{pr.cross_windows_dir()})")
    args = parser.parse_args(argv)
    if sys.platform != "darwin":
        raise Failure("this sets up a Mac; on Windows, publish_release.py builds natively")
    # publish_release.py finds the folder by itself only when it's this one.
    found_by_default = pr.cross_windows_dir()
    if args.dir:
        os.environ[pr.CROSS_WINDOWS_DIR_ENV] = str(args.dir.expanduser().resolve())
    layout = Layout(pr.cross_windows_dir())
    say(("Checking" if args.check else "Setting up") + f" Windows builds in {layout.root}")
    if not args.check:
        layout.root.mkdir(parents=True, exist_ok=True)

    steps = [
        ("Rosetta 2", lambda: step_rosetta(args.check)),
        ("Homebrew formulas", lambda: step_brew(args.check)),
        ("cargo-xwin", lambda: step_cargo_xwin(args.check)),
        ("Rust target", lambda: step_rust_target(args.check)),
        ("Wine", lambda: step_wine(layout, args.check)),
        ("fxc.exe", lambda: step_fxc(layout, args.check, args.accept_microsoft_license)),
        ("Inno Setup", lambda: step_inno_setup(layout, args.check)),
        ("Wrappers", lambda: step_wrappers(layout, args.check)),
        ("Microsoft CRT and SDK", lambda: step_crt(args.check, args.accept_microsoft_license)),
    ]
    results = []
    for name, action in steps:
        say(name)
        try:
            state, detail = action()
        except Failure as err:
            state, detail = "failed", str(err)
        except Exception as err:  # noqa: BLE001  (report it, and carry on with the rest)
            state, detail = "failed", f"{type(err).__name__}: {err}"
        note(f"{state}: {detail}")
        results.append((name, state, detail))

    say("Final check")
    states = {name: state for name, state, _ in results}
    wine_ready = all(states.get(name) in ("ok", "done") for name in WINE_STEPS)
    checks = final_check(layout, wine_ready)
    for name, ok, detail in checks:
        note(f"{'ok' if ok else 'not run' if ok is None else 'FAILED'}: {name}: {detail}")

    print("\nSummary:")
    for name, state, detail in results:
        print(f"  {state:<8} {name}: {detail.splitlines()[0] if detail else ''}")
    ready = (all(state in ("ok", "done") for _, state, _ in results)
             and all(ok is True for _, ok, _ in checks))
    changed = [name for name, state, _ in results if state == "done"]
    if changed:
        print("\nSet up now: " + ", ".join(changed) + ".")
    elif not args.check and ready:
        print("\nEverything was already set up; nothing changed.")
    if ready and layout.root == found_by_default:
        print("Ready: `publish_release.py --target x86_64-pc-windows-msvc` finds these by itself.")
    elif ready:
        print(f"Ready, in a folder publish_release.py doesn't look in by itself. Set "
              f"{pr.CROSS_WINDOWS_DIR_ENV} for it (in your shell's profile, to keep it):")
        print(f"    export {pr.CROSS_WINDOWS_DIR_ENV}={shlex.quote(str(layout.root))}")
    else:
        print("Not ready; see the lines above"
              + (" (run it without --check to set up what's missing)." if args.check else "."))
    return 0 if ready else 1


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except Failure as err:
        print(f"error: {err}", file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        sys.exit(130)
