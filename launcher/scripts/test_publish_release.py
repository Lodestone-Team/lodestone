"""Tests for publish_release.py's manifest logic: python3 launcher/scripts/test_publish_release.py"""

import tempfile
import unittest
from pathlib import Path

import publish_release as pr


def entry(build, key="windows-x86_64", commit="c"):
    return {"name": pr.asset_name(build, key), "size": 3, "sha256": "0" * 64,
            "build": build, "commit": commit}


class Manifest(unittest.TestCase):
    def test_the_first_publish_starts_the_manifest(self):
        m = pr.updated_manifest(None, "windows-x86_64", entry(10), "0.1.0", "now")
        self.assertEqual(m["build"], 10)
        self.assertEqual(m["assets"]["windows-x86_64"]["name"], "lodestone-launcher-10-windows-x86_64.exe")
        self.assertEqual(m["version"], "0.1.0")

    def test_publishing_one_platform_keeps_the_others(self):
        m = pr.updated_manifest(None, "windows-x86_64", entry(10), "0.1.0", "t1")
        m = pr.updated_manifest(m, "macos-aarch64", entry(12, "macos-aarch64", "d"), "0.2.0", "t2")
        self.assertEqual(set(m["assets"]), {"macos-aarch64", "windows-x86_64"})
        self.assertEqual(m["assets"]["windows-x86_64"]["build"], 10)
        self.assertEqual((m["build"], m["commit"], m["version"]), (12, "d", "0.2.0"))
        self.assertEqual(m["assets"]["macos-aarch64"]["name"], "lodestone-launcher-12-macos-aarch64")

    def test_an_older_platform_doesnt_lower_the_top_build(self):
        m = pr.updated_manifest(None, "macos-aarch64", entry(12, "macos-aarch64", "d"), "0.2.0", "t1")
        m = pr.updated_manifest(m, "windows-x86_64", entry(11), "0.1.0", "t2")
        self.assertEqual((m["build"], m["commit"], m["version"]), (12, "d", "0.2.0"))

    def test_assets_written_without_their_own_build_get_the_manifests(self):
        old = {"build": 7, "commit": "a", "version": "0.1.0",
               "assets": {"macos-aarch64": {"name": "x", "size": 1, "sha256": "0" * 64}}}
        m = pr.updated_manifest(old, "windows-x86_64", entry(9), "0.1.0", "t")
        self.assertEqual(m["assets"]["macos-aarch64"]["build"], 7)
        self.assertEqual(m["assets"]["macos-aarch64"]["commit"], "a")


class Cleanup(unittest.TestCase):
    def test_keeps_the_two_newest_of_this_platform_only(self):
        names = [pr.asset_name(b, "windows-x86_64") for b in (3, 5, 9, 12)]
        names += [pr.asset_name(4, "macos-aarch64"), "latest.json"]
        self.assertEqual(
            pr.stale_names(names, "windows-x86_64", set()),
            ["lodestone-launcher-3-windows-x86_64.exe", "lodestone-launcher-5-windows-x86_64.exe"],
        )

    def test_keeps_the_build_the_manifest_points_at(self):
        names = [pr.asset_name(b, "macos-aarch64") for b in (3, 5, 9, 12)]
        self.assertEqual(pr.stale_names(names, "macos-aarch64", {3}), ["lodestone-launcher-5-macos-aarch64"])


class Candidate(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.target = Path(self.tmp.name)
        self.binary = self.target / pr.asset_name(7, "windows-x86_64")
        self.binary.write_bytes(b"tested")
        self.entry = pr.describe(self.binary, 7, "abc")

    def tearDown(self):
        self.tmp.cleanup()

    def test_publishing_needs_a_dry_run(self):
        with self.assertRaisesRegex(pr.Failure, "no dry run"):
            pr.check_candidate(self.target, self.entry, "windows-x86_64")

    def test_the_dry_runs_file_can_be_published(self):
        copy = pr.save_candidate(self.target, self.binary, self.entry, "windows-x86_64")
        self.assertEqual(copy.read_bytes(), b"tested")
        pr.check_candidate(self.target, self.entry, "windows-x86_64")

    def test_a_tested_copy_that_updated_itself_doesnt_matter(self):
        copy = pr.save_candidate(self.target, self.binary, self.entry, "windows-x86_64")
        copy.write_bytes(b"downloaded by the updater")
        pr.check_candidate(self.target, self.entry, "windows-x86_64")

    def test_a_different_file_is_refused(self):
        pr.save_candidate(self.target, self.binary, self.entry, "windows-x86_64")
        self.binary.write_bytes(b"rebuilt after a change")
        changed = pr.describe(self.binary, 7, "abc")
        with self.assertRaisesRegex(pr.Failure, "isn't the file the dry run made"):
            pr.check_candidate(self.target, changed, "windows-x86_64")

    def test_another_build_is_refused(self):
        pr.save_candidate(self.target, self.binary, self.entry, "windows-x86_64")
        other = dict(self.entry, build=8)
        with self.assertRaisesRegex(pr.Failure, "last dry run was build 7"):
            pr.check_candidate(self.target, other, "windows-x86_64")


class Platforms(unittest.TestCase):
    def test_matches_rusts_os_and_arch_names(self):
        self.assertEqual(pr.key_for_target("x86_64-pc-windows-msvc"), "windows-x86_64")
        self.assertEqual(pr.key_for_target("aarch64-pc-windows-msvc"), "windows-aarch64")
        self.assertEqual(pr.key_for_target("aarch64-apple-darwin"), "macos-aarch64")
        self.assertEqual(pr.key_for_target("x86_64-apple-darwin"), "macos-x86_64")
        self.assertEqual(pr.key_for_target("x86_64-unknown-linux-gnu"), "linux-x86_64")
        with self.assertRaises(pr.Failure):
            pr.key_for_target("wasm32-unknown-unknown")


if __name__ == "__main__":
    unittest.main()
