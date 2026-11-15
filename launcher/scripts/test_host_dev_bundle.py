"""Tests for host_dev_bundle.py: python3 -m unittest test_host_dev_bundle (in launcher/scripts)"""

import plistlib
import tempfile
import tomllib
import unittest
from pathlib import Path
from unittest import mock

import host_dev_bundle as hdb


class Build(unittest.TestCase):
    def commands(self, release):
        ran = []
        with mock.patch.object(hdb, "run", lambda cmd, cwd: ran.append((cmd, cwd))):
            hdb.build(release)
        return {cwd: cmd for cmd, cwd in ran}

    def test_a_release_host_keeps_debug_assertions(self):
        # LS-117: launchd starts the login item without the bundle's LSEnvironment, and
        # a plain release Host uses the installed Host's login item and ~/.lodestone.
        host = self.commands(True)[hdb.LAUNCHER]
        self.assertNotIn("--release", host)
        self.assertEqual(host[host.index("--profile") + 1], hdb.RELEASE_PROFILE)
        config = {}
        for i, arg in enumerate(host):
            if arg == "--config":
                config.update(tomllib.loads(host[i + 1])["profile"][hdb.RELEASE_PROFILE])
        self.assertEqual(config, {"inherits": "release", "debug-assertions": True})

    def test_the_core_builds_without_default_features(self):
        for release in (False, True):
            core = self.commands(release)[hdb.ROOT]
            self.assertIn("--no-default-features", core)
            self.assertEqual("--release" in core, release)

    def test_a_debug_build_is_a_plain_one(self):
        self.assertEqual(self.commands(False)[hdb.LAUNCHER],
                         ["cargo", "build", "-p", "lodestone-host"])


class Bundle(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)
        self.root = self.dir / "repo"
        patches = [mock.patch.object(hdb, "ROOT", self.root),
                   mock.patch.object(hdb, "LAUNCHER", self.root / "launcher"),
                   mock.patch.object(hdb, "ICON", self.dir / "icon.icns"),
                   mock.patch.object(hdb, "run", lambda cmd, cwd: None)]
        for patch in patches:
            patch.start()
            self.addCleanup(patch.stop)
        (self.dir / "icon.icns").write_bytes(b"icon")

    def tearDown(self):
        self.tmp.cleanup()

    def program(self, path, content):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)

    def test_a_release_bundle_never_takes_target_releases_host(self):
        launcher, root = self.root / "launcher" / "target", self.root / "target"
        self.program(launcher / "release" / "lodestone-host", b"a release Host")
        self.program(root / "release" / "lodestone_core", b"core")
        with self.assertRaises(SystemExit):
            hdb.bundle(self.dir / "out", self.dir / "data", release=True)
        self.program(launcher / hdb.RELEASE_PROFILE / "lodestone-host", b"a dev-bundle Host")
        app = hdb.bundle(self.dir / "out", self.dir / "data", release=True)
        macos = app / "Contents" / "MacOS"
        self.assertEqual((macos / "lodestone-host").read_bytes(), b"a dev-bundle Host")
        self.assertEqual((macos / "lodestone_core").read_bytes(), b"core")
        with (app / "Contents" / "Info.plist").open("rb") as file:
            plist = plistlib.load(file)
        self.assertEqual(plist["CFBundleIdentifier"], hdb.BUNDLE_ID)
        self.assertEqual(plist["LSEnvironment"]["LODESTONE_PATH"], str((self.dir / "data").resolve()))


if __name__ == "__main__":
    unittest.main()
