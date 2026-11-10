#!/usr/bin/env python3
"""Rebuilds Lodestone Launcher or Lodestone Host when you save a file, and runs it again.

It watches the launcher workspace (`launcher/crates`, `launcher/assets`) and the crates
it shares with the core (`crates/`). After each save it runs an incremental
`cargo build`. When the build works, it stops the running app and starts the new one;
when it fails, the old app keeps running and the errors are shown. It uses only
Python's standard library and the Rust toolchain. `launcher/docs/dev-loop.md` explains
the workflow and the build times.

Usage, from anywhere in the repository:

    python3 launcher/scripts/dev_loop.py launcher                    # run the launcher
    python3 launcher/scripts/dev_loop.py launcher --scene settings   # open a scene
    python3 launcher/scripts/dev_loop.py launcher --scene settings --png out.png
    python3 launcher/scripts/dev_loop.py host                        # run the Host
    python3 launcher/scripts/dev_loop.py host --png out.png -- --settings general
    python3 launcher/scripts/dev_loop.py launcher --once             # build, run, exit

`--scene NAME` builds with the `screenshot` feature and opens a window on one of the
offscreen scenes (`crates/lodestone-launcher/src/preview.rs` lists them), with made-up
accounts and instances. So the window opens on the page you are changing, every time.
`--png FILE` renders the window offscreen to FILE after each build instead of opening
it, and opens the picture once (most image viewers then show each new version;
`--no-open` skips that, for scripts and coding agents).
Arguments after `--` go to the app (for example `--locale fr`, `--zoom 1.5`, or the
Host's `--settings general`, `--tab console`, `--demo`).

The apps never use your real data here. The launcher gets its own data folder
(`launcher/target/dev-loop/launcher`, kept between runs, or `--data DIR`). The Host
gets its own core data, Host data and launcher folders under
`launcher/target/dev-loop/host`, and its core listens on port 16762
(`--core-port`), not 16662. The Host's core is built once at the start, without Deno,
and again when a file in `core/` changes.
"""

import argparse
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LAUNCHER = ROOT / "launcher"
EXE = ".exe" if sys.platform == "win32" else ""
# What a save in these folders can change. `target` folders are skipped.
WATCH = [LAUNCHER / "crates", LAUNCHER / "assets", LAUNCHER / "Cargo.toml", ROOT / "crates"]
CORE_WATCH = [ROOT / "core", ROOT / "Cargo.toml"]
EXTENSIONS = {".rs", ".toml", ".json", ".png", ".svg", ".ttf", ".otf", ".jpg", ".md"}
POLL_SECONDS = 0.25
# Saves within this long of each other are one change (an editor saving many files).
SETTLE_SECONDS = 0.3


def say(text: str) -> None:
    print(f"[dev-loop {time.strftime('%H:%M:%S')}] {text}", flush=True)


def snapshot(paths: list[Path]) -> dict[str, float]:
    """The modification time of every watched file."""
    seen: dict[str, float] = {}
    stack = list(paths)
    while stack:
        path = stack.pop()
        try:
            if path.is_file():
                seen[str(path)] = path.stat().st_mtime
                continue
            with os.scandir(path) as entries:
                for entry in entries:
                    if entry.is_dir(follow_symlinks=False):
                        if entry.name not in ("target", "node_modules") and not entry.name.startswith("."):
                            stack.append(Path(entry.path))
                    elif os.path.splitext(entry.name)[1] in EXTENSIONS:
                        try:
                            seen[entry.path] = entry.stat().st_mtime
                        except FileNotFoundError:
                            pass
        except (FileNotFoundError, NotADirectoryError):
            pass
    return seen


def changed(before: dict[str, float], after: dict[str, float]) -> list[str]:
    names = {name for name in before.keys() | after.keys() if before.get(name) != after.get(name)}
    return sorted(names)


def cargo_build(args: list[str], cwd: Path) -> bool:
    started = time.monotonic()
    say("cargo build " + " ".join(args))
    result = subprocess.run(["cargo", "build", *args], cwd=cwd)
    seconds = time.monotonic() - started
    if result.returncode == 0:
        say(f"built in {seconds:.1f} s")
        return True
    say(f"build failed after {seconds:.1f} s; fix it and save again")
    return False


def stop(process: subprocess.Popen | None) -> None:
    """Stops the app: asks first, then forces it after three seconds."""
    if process is None or process.poll() is not None:
        return
    if sys.platform == "win32":
        process.terminate()
    else:
        process.send_signal(signal.SIGTERM)
    try:
        process.wait(timeout=3)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()


def open_file(path: Path) -> None:
    if sys.platform == "darwin":
        subprocess.run(["open", str(path)])
    elif sys.platform == "win32":
        os.startfile(path)  # type: ignore[attr-defined]
    else:
        subprocess.run(["xdg-open", str(path)])


class App:
    """How to build and start one of the two apps."""

    def __init__(self, args: argparse.Namespace):
        self.args = args
        self.png = Path(args.png).resolve() if args.png else None
        self.opened_png = False
        dev = LAUNCHER / "target" / "dev-loop"
        self.env = dict(os.environ)
        if args.app == "launcher":
            self.package = "lodestone-launcher"
            data = Path(args.data).resolve() if args.data else dev / "launcher"
            data.mkdir(parents=True, exist_ok=True)
            self.env["LODESTONE_LAUNCHER_DATA_DIR"] = str(data)
            self.env.setdefault("LODESTONE_LAUNCHER_TRACE_STARTUP", "1")
            self.screenshot = bool(args.scene or self.png)
        else:
            self.package = "lodestone-host"
            if args.scene:
                sys.exit("--scene is for the launcher; the Host takes its options after `--`.")
            host = Path(args.data).resolve() if args.data else dev / "host"
            for name, folder in (("LODESTONE_PATH", "core"), ("LODESTONE_HOST_DATA_DIR", "host"),
                                 ("LODESTONE_LAUNCHER_DATA_DIR", "launcher")):
                (host / folder).mkdir(parents=True, exist_ok=True)
                self.env[name] = str(host / folder)
            if Path(self.env["LODESTONE_PATH"]).resolve() == (Path.home() / ".lodestone").resolve():
                sys.exit("The dev loop doesn't use ~/.lodestone, the real core's data. Pick another --data.")
            self.env["LODESTONE_HOST_CORE_PORT"] = str(args.core_port)
            self.env["LODESTONE_HOST_CORE"] = str(ROOT / "target" / "debug" / f"lodestone_core{EXE}")
            self.screenshot = bool(self.png)
        self.exe = LAUNCHER / "target" / "debug" / f"{self.package}{EXE}"

    def build(self) -> bool:
        features = ["--features", "screenshot"] if self.screenshot else []
        return cargo_build(["-p", self.package, *features], LAUNCHER)

    def build_core(self) -> bool:
        # The Host's core: never with Deno.
        return cargo_build(["-p", "lodestone_core", "--no-default-features"], ROOT)

    def command(self) -> list[str]:
        command = [str(self.exe)]
        if self.args.scene:
            command += ["--scene", self.args.scene]
        if self.png:
            command += ["--screenshot", str(self.png)]
        return command + self.args.app_args

    def start(self) -> subprocess.Popen:
        say("starting " + " ".join(self.command()))
        return subprocess.Popen(self.command(), cwd=LAUNCHER, env=self.env)

    def render(self) -> bool:
        """`--png`: runs the app to write the picture, and waits for it to quit."""
        started = time.monotonic()
        before = self.png.stat().st_mtime if self.png.exists() else None
        result = subprocess.run(self.command(), cwd=LAUNCHER, env=self.env)
        after = self.png.stat().st_mtime if self.png.exists() else None
        if result.returncode != 0 or after is None or after == before:
            say(f"no picture written (exit code {result.returncode})")
            return False
        say(f"wrote {self.png} in {time.monotonic() - started:.1f} s")
        if not self.opened_png and not self.args.no_open:
            open_file(self.png)
            self.opened_png = True
        return True


def interrupted(*_) -> None:
    raise KeyboardInterrupt


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("app", choices=["launcher", "host"], help="which app to run")
    parser.add_argument("--scene", help="launcher: open this offscreen scene in a window")
    parser.add_argument("--png", metavar="FILE", help="render to FILE after each build instead of opening a window")
    parser.add_argument("--no-open", action="store_true", help="with --png: don't open the picture")
    parser.add_argument("--data", metavar="DIR", help="the app's data folder (default: under launcher/target/dev-loop)")
    parser.add_argument("--core-port", type=int, default=16762, help="host: the core's port (default: 16762)")
    parser.add_argument("--once", action="store_true", help="build and run once, without watching")
    parser.add_argument("app_args", nargs="*", help="after `--`: arguments for the app")
    args = parser.parse_args()
    app = App(args)
    # Stopped by a signal (a terminal closing, `kill`): stop the app too, as Ctrl+C does.
    signal.signal(signal.SIGTERM, interrupted)

    watched = WATCH + (CORE_WATCH if args.app == "host" else [])
    files = snapshot(watched)
    say(f"watching {len(files)} files; Ctrl+C stops")
    process = None
    try:
        built = (args.app != "host" or app.build_core()) and app.build()
        while True:
            if built:
                if app.png:
                    built = app.render()
                else:
                    stop(process)
                    process = app.start()
            if args.once:
                if process is not None:
                    process.wait()
                sys.exit(0 if built else 1)
            # Wait for a save, then for the saves to settle.
            while True:
                time.sleep(POLL_SECONDS)
                now = snapshot(watched)
                if changed(files, now):
                    break
                if process is not None and process.poll() is not None:
                    say(f"the app quit (exit code {process.returncode}); save a file to start it again")
                    process = None
            while True:
                time.sleep(SETTLE_SECONDS)
                settled = snapshot(watched)
                if settled == now:
                    break
                now = settled
            names = changed(files, now)
            files = now
            shown = ", ".join(os.path.relpath(name, ROOT) for name in names[:3])
            say(f"changed: {shown}{' and more' if len(names) > 3 else ''}")
            core_changed = args.app == "host" and any(
                Path(name).is_relative_to(ROOT / "core") or name == str(ROOT / "Cargo.toml") for name in names)
            built = (not core_changed or app.build_core()) and app.build()
    except KeyboardInterrupt:
        pass
    finally:
        stop(process)


if __name__ == "__main__":
    main()
