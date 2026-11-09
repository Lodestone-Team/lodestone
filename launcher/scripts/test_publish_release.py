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
        self.assertEqual(m["assets"]["windows-x86_64"]["name"], "lodestone-launcher-10-windows-x86_64-standalone.exe")
        self.assertEqual(m["version"], "0.1.0")

    def test_publishing_one_platform_keeps_the_others(self):
        m = pr.updated_manifest(None, "windows-x86_64", entry(10), "0.1.0", "t1")
        m = pr.updated_manifest(m, "macos-aarch64", entry(12, "macos-aarch64", "d"), "0.2.0", "t2")
        self.assertEqual(set(m["assets"]), {"macos-aarch64", "windows-x86_64"})
        self.assertEqual(m["assets"]["windows-x86_64"]["build"], 10)
        self.assertEqual((m["build"], m["commit"], m["version"]), (12, "d", "0.2.0"))
        self.assertEqual(m["assets"]["macos-aarch64"]["name"], "lodestone-launcher-12-macos-aarch64-standalone")

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
            ["lodestone-launcher-3-windows-x86_64-standalone.exe",
             "lodestone-launcher-5-windows-x86_64-standalone.exe"],
        )

    def test_keeps_the_build_the_manifest_points_at(self):
        names = [pr.asset_name(b, "macos-aarch64") for b in (3, 5, 9, 12)]
        self.assertEqual(pr.stale_names(names, "macos-aarch64", {3}), ["lodestone-launcher-5-macos-aarch64-standalone"])


def staged(folder, build, keys, commit="abc", extras=()):
    """A staged folder as `stage` leaves it, with stand-in files."""
    folder.mkdir(parents=True, exist_ok=True)
    record = {"build": build, "commit": commit, "version": "0.1.0", "assets": {}, "files": {}}
    for key in keys:
        binary = folder / pr.asset_name(build, key)
        binary.write_bytes(f"{key} {build}".encode())
        record["assets"][key] = pr.describe(binary, build, commit)
        record["files"][binary.name] = pr.sha256(binary)
    for name in extras:
        (folder / name).write_bytes(name.encode())
        record["files"][name] = pr.sha256(folder / name)
    (folder / pr.STAGE_RECORD).write_text(pr.json.dumps(record))
    return record


class Stage(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def test_a_staged_folder_reads_back(self):
        staged(self.dir / "s", 7, ["windows-x86_64"], extras=[pr.installer_name(7, "windows-x86_64")])
        record = pr.read_stage(self.dir / "s")
        self.assertEqual(set(record["files"]), {pr.asset_name(7, "windows-x86_64"),
                                                pr.installer_name(7, "windows-x86_64")})

    def test_a_changed_file_is_refused(self):
        staged(self.dir / "s", 7, ["windows-x86_64"])
        (self.dir / "s" / pr.asset_name(7, "windows-x86_64")).write_bytes(b"something else")
        with self.assertRaisesRegex(pr.Failure, "changed since it was staged"):
            pr.read_stage(self.dir / "s")

    def test_publishing_needs_a_dry_run(self):
        with self.assertRaisesRegex(pr.Failure, "no dry run"):
            pr.check_candidate(self.dir / "s", 7, "abc")

    def test_the_dry_runs_files_can_be_published(self):
        staged(self.dir / "s", 7, ["macos-aarch64"])
        self.assertEqual(pr.check_candidate(self.dir / "s", 7, "abc")["build"], 7)

    def test_a_tested_copy_that_updated_itself_doesnt_matter(self):
        record = staged(self.dir / "s", 7, ["macos-aarch64"])
        copies = pr.save_test_copies(self.dir / "s", self.dir / "t", record)
        copies[0].write_bytes(b"downloaded by the updater")
        pr.check_candidate(self.dir / "s", 7, "abc")

    def test_another_build_is_refused(self):
        staged(self.dir / "s", 7, ["macos-aarch64"])
        with self.assertRaisesRegex(pr.Failure, "last dry run was build 7"):
            pr.check_candidate(self.dir / "s", 8, "abc")

    def test_a_staged_file_changed_after_the_dry_run_is_refused(self):
        staged(self.dir / "s", 7, ["macos-aarch64"])
        (self.dir / "s" / pr.asset_name(7, "macos-aarch64")).write_bytes(b"rebuilt")
        with self.assertRaisesRegex(pr.Failure, "isn't the build that was tested"):
            pr.check_candidate(self.dir / "s", 7, "abc")


class Publish(unittest.TestCase):
    """Publishing and promoting, with folders standing in for the releases."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)
        self.beta = pr.Folder(self.dir / "beta")
        self.stable = pr.Folder(self.dir / "stable")

    def tearDown(self):
        self.tmp.cleanup()

    def stage_both(self, build, commit="abc"):
        mac = self.dir / f"mac{build}"
        win = self.dir / f"win{build}"
        staged(mac, build, ["macos-aarch64", "macos-x86_64"], commit,
               extras=[pr.dmg_name(build, ["macos-aarch64", "macos-x86_64"])])
        staged(win, build, ["windows-x86_64"], commit,
               extras=[pr.installer_name(build, "windows-x86_64")])
        return [(mac, pr.read_stage(mac)), (win, pr.read_stage(win))]

    def test_platforms_staged_apart_publish_together(self):
        manifest = pr.publish(self.beta, self.stage_both(10))
        self.assertEqual(set(manifest["assets"]), {"macos-aarch64", "macos-x86_64", "windows-x86_64"})
        names = self.beta.names()
        self.assertIn(pr.STABLE_DMG, names)
        self.assertIn(pr.STABLE_INSTALLER, names)
        self.assertIn("lodestone-launcher-10-macos-universal.dmg", names)
        self.assertEqual(self.beta.manifest(), manifest)

    def test_a_channel_refuses_a_build_it_already_has(self):
        pr.publish(self.beta, self.stage_both(10))
        with self.assertRaisesRegex(pr.Failure, "already at build 10"):
            pr.publish(self.beta, self.stage_both(10))

    def test_old_builds_and_their_disk_images_are_cleaned_up(self):
        for build in (10, 11, 12):
            pr.publish(self.beta, self.stage_both(build))
        names = self.beta.names()
        self.assertNotIn("lodestone-launcher-10-macos-universal.dmg", names)
        self.assertNotIn(pr.asset_name(10, "windows-x86_64"), names)
        self.assertIn("lodestone-launcher-11-macos-universal.dmg", names)
        self.assertIn(pr.installer_name(12, "windows-x86_64"), names)

    def test_promoting_copies_betas_files_byte_for_byte(self):
        pr.publish(self.beta, self.stage_both(10))
        manifest = pr.promote(self.beta, self.stable, None, False)
        self.assertEqual(manifest["assets"], self.beta.manifest()["assets"])
        for name in self.stable.names():
            if name != pr.MANIFEST:
                self.assertEqual((self.stable.path / name).read_bytes(),
                                 (self.beta.path / name).read_bytes(), name)
        self.assertIn(pr.STABLE_DMG, self.stable.names())

    def test_promoting_one_platform_leaves_the_others(self):
        pr.publish(self.beta, self.stage_both(10))
        pr.promote(self.beta, self.stable, ["windows-x86_64"], False)
        pr.publish(self.beta, self.stage_both(11))
        manifest = pr.promote(self.beta, self.stable, ["windows-x86_64"], False)
        self.assertEqual(set(manifest["assets"]), {"windows-x86_64"})
        self.assertEqual(manifest["assets"]["windows-x86_64"]["build"], 11)
        self.assertNotIn(pr.STABLE_DMG, self.stable.names())

    def test_a_tampered_beta_file_isnt_promoted(self):
        pr.publish(self.beta, self.stage_both(10))
        (self.beta.path / pr.asset_name(10, "windows-x86_64")).write_bytes(b"not what was published")
        with self.assertRaisesRegex(pr.Failure, "doesn't match beta's manifest"):
            pr.promote(self.beta, self.stable, None, False)
        self.assertEqual(self.stable.names(), [])

    def test_promoting_twice_is_refused(self):
        pr.publish(self.beta, self.stage_both(10))
        pr.promote(self.beta, self.stable, None, False)
        with self.assertRaisesRegex(pr.Failure, "already at build 10"):
            pr.promote(self.beta, self.stable, None, False)

    def test_a_platform_beta_doesnt_have_is_refused(self):
        pr.publish(self.beta, self.stage_both(10))
        with self.assertRaisesRegex(pr.Failure, "no build for linux-x86_64"):
            pr.promote(self.beta, self.stable, ["linux-x86_64"], False)


class Installer(unittest.TestCase):
    def test_only_x64_windows_has_an_installer(self):
        self.assertTrue(pr.has_installer("windows-x86_64"))
        self.assertFalse(pr.has_installer("windows-aarch64"))
        self.assertFalse(pr.has_installer("macos-aarch64"))

    def test_installers_and_disk_images_have_fixed_names_too(self):
        self.assertEqual(pr.alias_for(pr.installer_name(7, "windows-x86_64")), pr.STABLE_INSTALLER)
        self.assertEqual(pr.alias_for(pr.dmg_name(7, ["macos-aarch64"])), pr.STABLE_DMG)
        self.assertIsNone(pr.alias_for(pr.asset_name(7, "windows-x86_64")))

    def test_old_installers_are_cleaned_up_with_their_builds(self):
        names = []
        for build in (3, 5, 9):
            names += [pr.asset_name(build, "windows-x86_64"), pr.installer_name(build, "windows-x86_64")]
        names += [pr.STABLE_INSTALLER, "latest.json"]
        self.assertEqual(
            pr.stale_names(names, "windows-x86_64", set()),
            ["lodestone-launcher-3-windows-x86_64-standalone.exe",
             "lodestone-launcher-3-windows-x86_64-setup.exe"],
        )


class Names(unittest.TestCase):
    def test_standalone_and_installer_names_say_which_is_which(self):
        self.assertEqual(pr.asset_name(7, "windows-x86_64"),
                         "lodestone-launcher-7-windows-x86_64-standalone.exe")
        self.assertEqual(pr.asset_name(7, "macos-aarch64"),
                         "lodestone-launcher-7-macos-aarch64-standalone")
        self.assertEqual(pr.installer_name(7, "windows-x86_64"),
                         "lodestone-launcher-7-windows-x86_64-setup.exe")
        self.assertEqual(pr.dmg_name(7, ["macos-aarch64"]), "lodestone-launcher-7-macos-aarch64.dmg")
        self.assertEqual(pr.dmg_name(7, ["macos-aarch64", "macos-x86_64"]),
                         "lodestone-launcher-7-macos-universal.dmg")

    def test_files_from_before_the_standalone_suffix_are_cleaned_up_too(self):
        names = ["lodestone-launcher-531-windows-x86_64.exe",
                 "lodestone-launcher-554-windows-x86_64.exe",
                 "lodestone-launcher-554-windows-x86_64-setup.exe",
                 pr.asset_name(600, "windows-x86_64"), pr.installer_name(600, "windows-x86_64"),
                 pr.STABLE_INSTALLER, "latest.json"]
        self.assertEqual(pr.stale_names(names, "windows-x86_64", set()),
                         ["lodestone-launcher-531-windows-x86_64.exe"])


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
