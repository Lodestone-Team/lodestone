"""Tests for publish_release.py's manifest logic: python3 launcher/scripts/test_publish_release.py"""

import os
import sys
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


class FakeRelease(pr.VersionRelease):
    """A version's release with a folder for its files and no GitHub."""

    def __init__(self, path, tag, tag_commit=None, draft=True, prerelease=None, others=()):
        super().__init__("test/repo", tag)
        self.folder = pr.Folder(path)
        self._tag_commit = tag_commit
        self.exists = tag_commit is not None
        self.draft = draft
        self._prerelease = pr.is_prerelease_tag(tag) if prerelease is None else prerelease
        self.others = list(others)
        self.created = None

    def info(self, fresh=False):
        if not self.exists:
            return None
        return {"tagName": self.tag, "isDraft": self.draft, "isPrerelease": self._prerelease,
                "url": f"release {self.tag}",
                "assets": [{"name": name} for name in self.folder.names()]}

    def names(self):
        return self.folder.names() if self.exists else []

    def download(self, name, into):
        return self.folder.download(name, into)

    def manifest(self):
        return self.folder.manifest() if self.exists else None

    def upload(self, path):
        self.folder.upload(path)

    def delete(self, name):
        self.folder.delete(name)

    def tag_commit(self):
        return self._tag_commit

    def create(self, commit):
        self.created = commit
        self._tag_commit = commit
        self.exists = True

    def published_releases(self):
        return self.others


def published(tag, builds, prerelease=False, draft=False):
    """A GitHub release object as the API lists it."""
    return {"tag_name": tag, "draft": draft, "prerelease": prerelease,
            "assets": [{"name": pr.asset_name(build, key)} for key, build in builds.items()]
            + [{"name": "latest.json"}]}


class Publish(unittest.TestCase):
    """Uploading builds to a version's release, with folders standing in for GitHub."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def stage_mac(self, build, commit="abc"):
        mac = self.dir / f"mac{build}{commit}"
        staged(mac, build, ["macos-aarch64", "macos-x86_64"], commit,
               extras=[pr.dmg_name(build, ["macos-aarch64", "macos-x86_64"])])
        return mac, pr.read_stage(mac)

    def stage_win(self, build, commit="abc"):
        win = self.dir / f"win{build}{commit}"
        staged(win, build, ["windows-x86_64"], commit,
               extras=[pr.installer_name(build, "windows-x86_64")])
        return win, pr.read_stage(win)

    def release(self, tag="launcher-v0.2.0", **kw):
        return FakeRelease(self.dir / tag, tag, **kw)

    def test_the_first_upload_tags_the_commit_and_drafts_the_release(self):
        release = self.release()
        manifest = pr.publish(release, [self.stage_win(10)], tag=release.tag)
        self.assertEqual(release.created, "abc")
        self.assertEqual(manifest["tag"], "launcher-v0.2.0")
        self.assertIn(pr.STABLE_INSTALLER, release.names())

    def test_platforms_from_different_machines_end_up_in_one_manifest(self):
        release = self.release(tag_commit="abc")
        pr.publish(release, [self.stage_win(10)])
        manifest = pr.publish(release, [self.stage_mac(10)])
        self.assertEqual(set(manifest["assets"]), {"macos-aarch64", "macos-x86_64", "windows-x86_64"})
        self.assertIn("lodestone-launcher-10-macos-universal.dmg", release.names())
        self.assertIn(pr.STABLE_DMG, release.names())

    def test_a_build_of_another_commit_is_refused(self):
        release = self.release(tag_commit="abc")
        with self.assertRaisesRegex(pr.Failure, "is on abc.*build of def"):
            pr.publish(release, [self.stage_win(11, commit="def")])

    def test_a_platform_the_release_has_is_refused_or_skipped(self):
        release = self.release(tag_commit="abc")
        pr.publish(release, [self.stage_win(10)])
        with self.assertRaisesRegex(pr.Failure, "already has windows-x86_64"):
            pr.publish(release, [self.stage_win(10)])
        # CI's upload leaves what a developer uploaded first, and adds the rest.
        before = (release.folder.path / pr.asset_name(10, "windows-x86_64")).read_bytes()
        manifest = pr.publish(release, [self.stage_mac(10), self.stage_win(10)], skip_existing=True)
        self.assertEqual(set(manifest["assets"]), {"macos-aarch64", "macos-x86_64", "windows-x86_64"})
        self.assertEqual((release.folder.path / pr.asset_name(10, "windows-x86_64")).read_bytes(), before)
        self.assertIsNone(pr.publish(release, [self.stage_win(10)], skip_existing=True))
        # --force replaces it.
        pr.publish(release, [self.stage_win(10)], force=True)

    def test_a_build_launchers_wouldnt_update_to_is_refused(self):
        stable = published("launcher-v0.3.0", {"windows-x86_64": 20})
        beta = published("launcher-v0.4.0-beta.1", {"windows-x86_64": 30}, prerelease=True)
        draft = published("launcher-v0.5.0", {"windows-x86_64": 90}, draft=True)
        # A stable version competes with stable ones only.
        release = self.release("launcher-v0.3.1", tag_commit="abc", others=[stable, beta, draft])
        with self.assertRaisesRegex(pr.Failure, "launcher-v0.3.0 is published with windows-x86_64 build 20"):
            pr.publish(release, [self.stage_win(20)])
        pr.publish(release, [self.stage_win(21)])
        # A beta competes with every published release.
        release = self.release("launcher-v0.4.0-beta.2", tag_commit="abc", others=[stable, beta])
        with self.assertRaisesRegex(pr.Failure, "beta launchers wouldn't update"):
            pr.publish(release, [self.stage_win(25)])
        pr.publish(release, [self.stage_win(31)])

    def test_other_platforms_builds_dont_count(self):
        mac = published("launcher-v0.3.0", {"macos-aarch64": 50})
        release = self.release("launcher-v0.3.1", tag_commit="abc", others=[mac])
        pr.publish(release, [self.stage_win(10)])

    def test_promoting_copies_a_betas_files_byte_for_byte(self):
        beta = self.release("launcher-v0.2.0-beta.1", tag_commit="abc", draft=False)
        pr.publish(beta, [self.stage_mac(10), self.stage_win(10)])
        stable = self.release("launcher-v0.2.0")
        manifest = pr.copy_release(beta, stable, None, False, tag=stable.tag)
        self.assertEqual(stable.created, "abc")
        self.assertEqual(manifest["assets"], beta.manifest()["assets"])
        self.assertEqual(manifest["tag"], "launcher-v0.2.0")
        for name in stable.names():
            if name != pr.MANIFEST:
                self.assertEqual((stable.folder.path / name).read_bytes(),
                                 (beta.folder.path / name).read_bytes(), name)

    def test_a_tampered_file_isnt_copied(self):
        beta = self.release("launcher-v0.2.0-beta.1", tag_commit="abc", draft=False)
        pr.publish(beta, [self.stage_win(10)])
        (beta.folder.path / pr.asset_name(10, "windows-x86_64")).write_bytes(b"not what was published")
        stable = self.release("launcher-v0.2.0")
        with self.assertRaisesRegex(pr.Failure, "doesn't match its manifest"):
            pr.copy_release(beta, stable, None, False)
        self.assertEqual(stable.names(), [])

    def test_a_platform_the_source_doesnt_have_is_refused(self):
        beta = self.release("launcher-v0.2.0-beta.1", tag_commit="abc", draft=False)
        pr.publish(beta, [self.stage_win(10)])
        with self.assertRaisesRegex(pr.Failure, "no build for linux-x86_64"):
            pr.copy_release(beta, self.release(), ["linux-x86_64"], False)

    def test_copying_from_a_draft_is_refused(self):
        with self.assertRaisesRegex(pr.Failure, "still a draft"):
            pr.published_source(self.release("launcher-v0.2.0-beta.1", tag_commit="abc"), None)
        with self.assertRaisesRegex(pr.Failure, "bridge a stable version"):
            pr.published_source(self.release("launcher-v0.2.0-beta.1", tag_commit="abc",
                                             draft=False), False)


class Folders(unittest.TestCase):
    """Test folders, and the legacy release, hold a run of builds."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)
        self.folder = pr.Folder(self.dir / "out")

    def tearDown(self):
        self.tmp.cleanup()

    def stage(self, build):
        win = self.dir / f"win{build}"
        staged(win, build, ["windows-x86_64"], extras=[pr.installer_name(build, "windows-x86_64")])
        return [(win, pr.read_stage(win))]

    def test_a_build_it_already_has_is_refused(self):
        pr.publish(self.folder, self.stage(10))
        with self.assertRaisesRegex(pr.Failure, "already at build 10"):
            pr.publish(self.folder, self.stage(10))

    def test_old_builds_are_cleaned_up(self):
        for build in (10, 11, 12):
            pr.publish(self.folder, self.stage(build))
        names = self.folder.names()
        self.assertNotIn(pr.asset_name(10, "windows-x86_64"), names)
        self.assertIn(pr.asset_name(11, "windows-x86_64"), names)
        self.assertIn(pr.installer_name(12, "windows-x86_64"), names)


class Tags(unittest.TestCase):
    def test_versions_and_betas(self):
        pr.check_tag("launcher-v0.2.0")
        pr.check_tag("launcher-v1.10.3-beta.4")
        for bad in ("v0.2.0", "launcher-0.2.0", "launcher-v0.2", "launcher-latest", "launcher-v0.2.0-"):
            with self.assertRaises(pr.Failure, msg=bad):
                pr.check_tag(bad)
        self.assertFalse(pr.is_prerelease_tag("launcher-v0.2.0"))
        self.assertTrue(pr.is_prerelease_tag("launcher-v0.2.0-rc.1"))

    def test_a_version_the_workspace_doesnt_have_gets_a_note(self):
        self.assertIsNone(pr.tag_version_note("launcher-v0.2.0-beta.1", "0.2.0"))
        self.assertIn("launcher/Cargo.toml says 0.1.0", pr.tag_version_note("launcher-v0.2.0", "0.1.0"))

    def test_only_standalone_binaries_have_a_build_for_ordering(self):
        self.assertEqual(pr.file_build(pr.asset_name(7, "windows-x86_64"), "windows-x86_64"), 7)
        self.assertIsNone(pr.file_build(pr.installer_name(7, "windows-x86_64"), "windows-x86_64"))
        self.assertIsNone(pr.file_build(pr.asset_name(7, "macos-aarch64"), "windows-x86_64"))
        self.assertIsNone(pr.file_build(pr.dmg_name(7, ["macos-aarch64"]), "macos-aarch64"))


class Drafts(unittest.TestCase):
    def test_the_newest_launcher_draft_is_picked(self):
        releases = [
            dict(published("launcher-v0.3.0", {"windows-x86_64": 30}), created_at="2026-11-01T00:00:00Z"),
            dict(published("launcher-v0.4.0-beta.1", {}, prerelease=True, draft=True),
                 created_at="2026-11-03T00:00:00Z"),
            dict(published("launcher-v0.3.1", {"macos-aarch64": 31}, draft=True),
                 created_at="2026-11-02T00:00:00Z"),
            dict(published("v0.6.0", {}, draft=True), created_at="2026-11-04T00:00:00Z"),
        ]
        draft, others = pr.pick_draft(releases)
        self.assertEqual(draft["tag_name"], "launcher-v0.4.0-beta.1")
        self.assertEqual([d["tag_name"] for d in others], ["launcher-v0.3.1"])

    def test_no_draft_says_how_to_start_one(self):
        with self.assertRaisesRegex(pr.Failure, "no launcher draft release"):
            pr.pick_draft([published("launcher-v0.3.0", {"windows-x86_64": 30})])

    def test_a_drafts_platforms_are_its_standalone_binaries(self):
        release = published("launcher-v0.3.1", {"windows-x86_64": 31, "macos-aarch64": 31}, draft=True)
        release["assets"] += [{"name": pr.installer_name(31, "windows-x86_64")},
                              {"name": pr.dmg_name(31, ["macos-aarch64", "macos-x86_64"])},
                              {"name": pr.STABLE_INSTALLER}]
        self.assertEqual(pr.release_platforms(release), ["macos-aarch64", "windows-x86_64"])


class Opening(unittest.TestCase):
    def test_the_checklist_fits_the_platform(self):
        windows = pr.checklist(12, "0123456789", "windows-x86_64")
        self.assertIn('Settings › General › Updates shows this build and commit ("Build 12 · 0123456").',
                      windows)
        self.assertTrue(any(line.startswith("Windows:") for line in windows))
        self.assertFalse(any(line.startswith("macOS:") for line in windows))
        mac = pr.checklist(12, "0123456789", "macos-aarch64")
        self.assertTrue(any(line.startswith("macOS:") for line in mac))
        self.assertFalse(any(line.startswith("Windows:") for line in mac))

    def test_a_running_launcher_is_left_alone(self):
        original = pr.running_launchers
        pr.running_launchers = lambda: ["4242 lodestone-launcher.exe"]
        try:
            with self.assertRaisesRegex(pr.Failure, "already running .*4242.*quit it"):
                pr.open_test_copy(Path(sys.executable))
        finally:
            pr.running_launchers = original

    def test_a_copy_that_quits_at_once_is_reported(self):
        original = pr.running_launchers
        pr.running_launchers = lambda: []
        try:
            with tempfile.TemporaryDirectory() as tmp:
                quits = Path(tmp) / ("quits.cmd" if os.name == "nt" else "quits.sh")
                quits.write_text("@exit /b 3\n" if os.name == "nt" else "#!/bin/sh\nexit 3\n")
                quits.chmod(0o755)
                with self.assertRaisesRegex(pr.Failure, "quit straight away"):
                    pr.open_test_copy(quits, wait=10)
        finally:
            pr.running_launchers = original

    def test_the_binary_for_this_machine_must_be_in_the_dry_run(self):
        with self.assertRaisesRegex(pr.Failure, "no build for this machine"):
            pr.test_binary(Path("."), {"assets": {"macos-aarch64": {"name": "x"}}}, "windows-x86_64")


class Restricting(unittest.TestCase):
    def test_the_universal_disk_image_goes_with_the_last_mac(self):
        record = {"assets": {"macos-aarch64": {}, "macos-x86_64": {}},
                  "files": {pr.asset_name(1, "macos-aarch64"): "a", pr.asset_name(1, "macos-x86_64"): "b",
                            pr.dmg_name(1, ["macos-aarch64", "macos-x86_64"]): "c"}}
        some = pr.restricted(record, {"macos-aarch64"})
        self.assertEqual(set(some["files"]), {pr.asset_name(1, "macos-x86_64"),
                                              "lodestone-launcher-1-macos-universal.dmg"})
        self.assertEqual(pr.restricted(record, {"macos-aarch64", "macos-x86_64"})["files"], {})


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
