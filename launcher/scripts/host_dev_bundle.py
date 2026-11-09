#!/usr/bin/env python3
"""Builds a runnable `Lodestone Host Dev.app` (macOS) for trying the GPUI Host.

The app holds the Host (`lodestone-host`) and the core it runs as a child process
(`lodestone_core`, built without the `deno` feature, as every Host build is). Its
Info.plist points the core's data folder and the Host's own folder (logs, the
single-instance socket) into the output folder, so a test never touches
`~/.lodestone` or the installed Host's data. It is signed ad hoc, for this Mac only.

Nothing is published. For a release build of the Host, see the parity plan
(`launcher/docs/host-parity.md`): its packaging is still to come.

Usage, from anywhere in the repository:

    python3 launcher/scripts/host_dev_bundle.py              # into a new temp folder
    python3 launcher/scripts/host_dev_bundle.py --out DIR    # into DIR
    python3 launcher/scripts/host_dev_bundle.py --release    # optimised builds
    python3 launcher/scripts/host_dev_bundle.py --no-build   # bundle what is built
    python3 launcher/scripts/host_dev_bundle.py --data DIR   # the core's data folder
    python3 launcher/scripts/host_dev_bundle.py --open       # and open the app

Then `open "<out>/Lodestone Host Dev.app"`. The core's output is in
`<out>/host/logs/core.log`.
"""

import argparse
import os
import plistlib
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

APP_NAME = "Lodestone Host Dev"
BUNDLE_ID = "dev.lodestone.host.dev"
ROOT = Path(__file__).resolve().parents[2]
LAUNCHER = ROOT / "launcher"
ICON = LAUNCHER / "installer" / "macos" / "HostIcon.icns"


def run(cmd: list[str], cwd: Path) -> None:
    print("+", " ".join(cmd), flush=True)
    subprocess.run(cmd, cwd=cwd, check=True)


def build(release: bool) -> None:
    profile = ["--release"] if release else []
    # the core: never with Deno
    run(["cargo", "build", "-p", "lodestone_core", "--no-default-features", *profile], ROOT)
    run(["cargo", "build", "-p", "lodestone-host", *profile], LAUNCHER)


def info_plist(data: Path, host: Path) -> dict:
    return {
        "CFBundleDevelopmentRegion": "en",
        "CFBundleDisplayName": APP_NAME,
        "CFBundleExecutable": "lodestone-host",
        "CFBundleIconFile": "AppIcon",
        "CFBundleIdentifier": BUNDLE_ID,
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundleName": APP_NAME,
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": "0.0.0",
        "CFBundleVersion": "0",
        "LSMinimumSystemVersion": "10.15.7",
        "NSHighResolutionCapable": True,
        # Where the core keeps its data and the Host its logs, for this bundle only.
        # macOS passes these to the app when it is opened from Finder or `open`.
        "LSEnvironment": {
            "LODESTONE_PATH": str(data),
            "LODESTONE_HOST_DATA_DIR": str(host),
        },
    }


def bundle(out: Path, data: Path, release: bool) -> Path:
    profile = "release" if release else "debug"
    host_exe = LAUNCHER / "target" / profile / "lodestone-host"
    core_exe = ROOT / "target" / profile / "lodestone_core"
    for exe in (host_exe, core_exe):
        if not exe.is_file():
            sys.exit(f"{exe} isn't built: run without --no-build")
    app = out / f"{APP_NAME}.app"
    if app.exists():
        shutil.rmtree(app)
    macos = app / "Contents" / "MacOS"
    resources = app / "Contents" / "Resources"
    macos.mkdir(parents=True)
    resources.mkdir()
    shutil.copy2(host_exe, macos / "lodestone-host")
    # next to the Host's executable, where it looks first
    shutil.copy2(core_exe, macos / "lodestone_core")
    shutil.copyfile(ICON, resources / "AppIcon.icns")
    host = out / "host"
    data.mkdir(parents=True, exist_ok=True)
    host.mkdir(parents=True, exist_ok=True)
    with (app / "Contents" / "Info.plist").open("wb") as file:
        plistlib.dump(info_plist(data.resolve(), host.resolve()), file)
    (app / "Contents" / "PkgInfo").write_text("APPL????", encoding="ascii")
    # ad hoc: runs on this Mac; the core inside is signed with the app
    run(["codesign", "--force", "--deep", "--sign", "-", str(app)], out)
    return app


def main() -> None:
    if sys.platform != "darwin":
        sys.exit("The dev bundle is a macOS app. Elsewhere, run the Host with "
                 "`cargo run -p lodestone-host` in launcher/ (it finds the core in "
                 "target/debug).")
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--out", type=Path, help="the folder for the app (default: a new temp folder)")
    parser.add_argument("--data", type=Path, help="the core's data folder (default: <out>/data)")
    parser.add_argument("--release", action="store_true", help="optimised builds")
    parser.add_argument("--no-build", action="store_true", help="bundle what is already built")
    parser.add_argument("--open", action="store_true", help="open the app afterwards")
    args = parser.parse_args()

    out = (args.out or Path(tempfile.mkdtemp(prefix="lodestone-host-dev-"))).resolve()
    out.mkdir(parents=True, exist_ok=True)
    data = (args.data or out / "data").resolve()
    home_data = Path.home() / ".lodestone"
    if data == home_data.resolve() if home_data.exists() else data == home_data:
        sys.exit("The dev bundle doesn't use ~/.lodestone, the real core's data. Pick another --data.")
    if not args.no_build:
        build(args.release)
    app = bundle(out, data, args.release)
    print(f"\nBuilt {app}")
    print(f"The core's data: {data}")
    print(f"The core's log:  {out / 'host' / 'logs' / 'core.log'}")
    print(f'Open it with:    open "{app}"')
    if args.open:
        subprocess.run(["open", str(app)], check=True)


if __name__ == "__main__":
    os.umask(0o022)
    main()
