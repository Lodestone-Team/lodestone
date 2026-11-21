"""Tests for publish_release.py's manifest logic: python3 launcher/scripts/test_publish_release.py"""

import os
import re
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path
from unittest import mock

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


class InfoPlist(unittest.TestCase):
    def test_the_app_opens_invite_links(self):
        types = pr.info_plist(10, "0.1.0")["CFBundleURLTypes"]
        self.assertEqual(types[0]["CFBundleURLSchemes"], ["lodestone"])
        self.assertEqual(types[0]["CFBundleURLName"], pr.BUNDLE_ID)


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
    record = {"product": pr.PRODUCT.key, "build": build, "commit": commit, "version": "0.1.0",
              "assets": {}, "files": {}}
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

    def stage_host(self, folder, build=7, keys=("windows-x86_64",)):
        pr.use_product("host")
        try:
            return staged(folder, build, list(keys))
        finally:
            pr.use_product("launcher")

    def test_a_host_dry_run_isnt_uploaded_as_the_launcher(self):
        # Both products dry-run into the same folder: --product host --dry-run, then
        # --tag launcher-v1.0.0 would upload the Host as LodestoneLauncherSetup.exe.
        record = self.stage_host(self.dir / "s")
        self.assertEqual(record["product"], "host")
        with self.assertRaisesRegex(pr.Failure, "holds a Lodestone Host build, not Lodestone "
                                                "Launcher's. Run this --product host"):
            pr.check_candidate(self.dir / "s", 7, "abc")
        with self.assertRaisesRegex(pr.Failure, "holds a Lodestone Host build"):
            pr.read_stage(self.dir / "s")

    def test_a_launcher_dry_run_isnt_uploaded_as_the_host(self):
        staged(self.dir / "s", 7, ["macos-aarch64"])
        pr.use_product("host")
        try:
            with self.assertRaisesRegex(pr.Failure, "holds a Lodestone Launcher build, not "
                                                    "Lodestone Host's. Run this without"):
                pr.check_candidate(self.dir / "s", 7, "abc")
            with self.assertRaisesRegex(pr.Failure, "holds a Lodestone Launcher build"):
                pr.read_stage(self.dir / "s")
        finally:
            pr.use_product("launcher")

    def test_a_record_from_before_products_is_told_apart_by_its_files(self):
        record = self.stage_host(self.dir / "s")
        del record["product"]
        (self.dir / "s" / pr.STAGE_RECORD).write_text(pr.json.dumps(record))
        with self.assertRaisesRegex(pr.Failure, "holds a Lodestone Host build"):
            pr.check_candidate(self.dir / "s", 7, "abc")
        record = staged(self.dir / "l", 7, ["macos-aarch64"])
        del record["product"]
        (self.dir / "l" / pr.STAGE_RECORD).write_text(pr.json.dumps(record))
        self.assertEqual(pr.check_candidate(self.dir / "l", 7, "abc")["build"], 7)

    def test_a_record_that_names_the_wrong_product_for_its_files_is_refused(self):
        record = self.stage_host(self.dir / "s")
        record["product"] = "launcher"
        (self.dir / "s" / pr.STAGE_RECORD).write_text(pr.json.dumps(record))
        with self.assertRaisesRegex(pr.Failure, "not Lodestone Launcher's"):
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
                "assets": self.folder.assets()}

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

    def create(self, commit, force=False):
        self.created = commit
        self._tag_commit = commit
        self.exists = True

    def published_releases(self):
        return self.others


class RacedRelease(FakeRelease):
    """A release where another upload (`meanwhile`) runs between this upload's reading
    of the manifest and its writing of it, the first time."""

    def __init__(self, *args, meanwhile=None, **kw):
        super().__init__(*args, **kw)
        self.meanwhile = meanwhile

    def upload(self, path):
        if path.name == pr.MANIFEST and self.meanwhile:
            meanwhile, self.meanwhile = self.meanwhile, None
            meanwhile()
        super().upload(path)


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

    def test_uploads_at_the_same_time_keep_each_others_platforms(self):
        # The Mac upload reads the manifest, the Windows upload then writes its own,
        # and the Mac's (without Windows) replaces it. Windows' files are in the
        # release, so the Mac upload puts it back.
        windows = self.stage_win(10)
        path = self.dir / "launcher-v0.2.0"
        other = FakeRelease(path, "launcher-v0.2.0", tag_commit="abc")
        mac = RacedRelease(path, "launcher-v0.2.0", tag_commit="abc",
                           meanwhile=lambda: pr.publish(other, [windows]))
        manifest = pr.publish(mac, [self.stage_mac(10)])
        self.assertEqual(set(manifest["assets"]), {"macos-aarch64", "macos-x86_64", "windows-x86_64"})
        self.assertEqual(manifest["assets"]["windows-x86_64"], windows[1]["assets"]["windows-x86_64"])
        self.assertEqual(mac.manifest(), manifest)

    def test_the_releases_files_fill_in_what_its_manifest_lost(self):
        # --clobber deletes latest.json before uploading it again: an upload that reads
        # it then starts from nothing.
        release = self.release(tag_commit="abc")
        windows = self.stage_win(10)
        pr.publish(release, [windows])
        (release.folder.path / pr.MANIFEST).unlink()
        manifest = pr.publish(release, [self.stage_mac(10)])
        self.assertEqual(manifest["assets"]["windows-x86_64"], windows[1]["assets"]["windows-x86_64"])
        self.assertEqual(set(manifest["assets"]), {"macos-aarch64", "macos-x86_64", "windows-x86_64"})

    def test_a_platform_with_files_of_two_builds_isnt_guessed(self):
        release = self.release(tag_commit="abc")
        pr.publish(release, [self.stage_win(10)])
        (release.folder.path / pr.asset_name(9, "windows-x86_64")).write_bytes(b"old")
        (release.folder.path / pr.MANIFEST).unlink()
        manifest = pr.publish(release, [self.stage_mac(10)])
        self.assertNotIn("windows-x86_64", manifest["assets"])

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


class Versions(unittest.TestCase):
    def test_bare_versions_name_launcher_tags(self):
        self.assertEqual(pr.normalize_tag("1.0.0"), "launcher-v1.0.0")
        self.assertEqual(pr.normalize_tag("v1.0.0-beta.2"), "launcher-v1.0.0-beta.2")
        self.assertEqual(pr.normalize_tag("launcher-v1.0.0"), "launcher-v1.0.0")
        self.assertEqual(pr.normalize_tag("draft"), "draft")

    def test_versions_sort_with_betas_before_their_version(self):
        tags = ["launcher-v0.10.0", "launcher-v0.2.0", "launcher-v0.10.0-beta.10",
                "launcher-v0.10.0-beta.2", "launcher-v0.9.1"]
        self.assertEqual(sorted(tags, key=pr.version_order),
                         ["launcher-v0.2.0", "launcher-v0.9.1", "launcher-v0.10.0-beta.2",
                          "launcher-v0.10.0-beta.10", "launcher-v0.10.0"])

    def test_commits_are_classified(self):
        self.assertEqual(pr.classify("Launcher: bring the window back after a crash"), "fix")
        self.assertEqual(pr.classify("Performance monitor: correct readings on Windows"), "fix")
        self.assertEqual(pr.classify("Launcher: find the mod that crashes an instance"), "feature")
        self.assertEqual(pr.classify("feat: a new thing"), "feature")
        self.assertEqual(pr.classify("fix(updater): retry"), "fix")
        self.assertEqual(pr.classify("refactor!: settings moved"), "breaking")
        self.assertEqual(pr.classify("chore: bump deps"), "none")
        self.assertEqual(pr.classify("Settings: new layout", "BREAKING CHANGE: old files"), "breaking")

    def test_bumps_follow_semver_and_its_rule_before_1_0(self):
        self.assertEqual(pr.bump((0, 2, 3), "fix"), (0, 2, 4))
        self.assertEqual(pr.bump((0, 2, 3), "feature"), (0, 3, 0))
        self.assertEqual(pr.bump((0, 2, 3), "breaking"), (0, 3, 0))
        self.assertEqual(pr.bump((1, 2, 3), "breaking"), (2, 0, 0))

    def commits(self, *subjects):
        return [(f"{i:040d}", subject, "") for i, subject in enumerate(subjects)]

    def test_the_first_release_takes_the_workspace_version(self):
        tag, why = pr.suggest_version([], self.commits("Launcher: a thing"), "0.1.0", False)
        self.assertEqual(tag, "launcher-v0.1.0")
        self.assertIn("first", why)
        self.assertEqual(pr.suggest_version([], [], "0.1.0", True)[0], "launcher-v0.1.0-beta.1")

    def test_features_bump_the_minor_version_and_fixes_the_patch(self):
        tags = ["launcher-v0.2.0", "launcher-v0.1.0", "v9.0.0"]
        features = self.commits("Launcher: fix a crash", "Launcher: a new setting")
        self.assertEqual(pr.suggest_version(tags, features, "0.1.0", False)[0], "launcher-v0.3.0")
        fixes = self.commits("Launcher: fix a crash")
        self.assertEqual(pr.suggest_version(tags, fixes, "0.1.0", False)[0], "launcher-v0.2.1")
        self.assertEqual(pr.suggest_version(tags, fixes, "0.1.0", True)[0], "launcher-v0.2.1-beta.1")

    def test_a_beta_line_carries_on_or_finishes(self):
        tags = ["launcher-v0.2.0", "launcher-v0.3.0-beta.1", "launcher-v0.3.0-beta.2"]
        commits = self.commits("Launcher: a new setting")
        self.assertEqual(pr.suggest_version(tags, commits, "0.1.0", True)[0], "launcher-v0.3.0-beta.3")
        self.assertEqual(pr.suggest_version(tags, commits, "0.1.0", False)[0], "launcher-v0.3.0")

    def test_a_named_version_must_be_newer(self):
        tags = ["launcher-v0.2.0", "launcher-v0.3.0-beta.2"]
        self.assertEqual(pr.version_problem("launcher-v0.3.0", tags), (None, None))
        self.assertEqual(pr.version_problem("launcher-v0.3.0-beta.3", tags), (None, None))
        self.assertEqual(pr.version_problem("launcher-v0.2.1", tags), (None, None))
        self.assertIn("already exists", pr.version_problem("launcher-v0.2.0", tags)[1])
        self.assertIn("too old", pr.version_problem("launcher-v0.1.5", tags)[0])
        self.assertIn("beta.2 already exists", pr.version_problem("launcher-v0.3.0-beta.1", tags)[0])
        self.assertIn("too old", pr.version_problem("launcher-v0.2.0-beta.5", tags)[0])

    def test_skipping_versions_is_only_noted(self):
        tags = ["launcher-v0.2.0"]
        problem, note = pr.version_problem("launcher-v0.5.0", tags)
        self.assertIsNone(problem)
        self.assertIn("skips versions", note)
        self.assertEqual(pr.version_problem("launcher-v1.0.0", tags), (None, None))
        self.assertEqual(pr.version_problem("launcher-v1.0.0", []), (None, None))


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


def staged_host(folder, build, keys, commit="abc", extras=()):
    """A Host build staged as `stage` leaves it: each platform's Host and its core."""
    record = staged(folder, build, keys, commit, extras)
    for key in keys:
        core = folder / pr.core_name(build, key)
        core.write_bytes(f"core {key} {build}".encode())
        record["assets"][key]["core"] = {"name": core.name, "size": core.stat().st_size,
                                         "sha256": pr.sha256(core)}
        record["files"][core.name] = pr.sha256(core)
    (folder / pr.STAGE_RECORD).write_text(pr.json.dumps(record))
    return record


class Host(unittest.TestCase):
    """--product host: Lodestone Host, with Lodestone Core beside it."""

    def setUp(self):
        pr.use_product("host")
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)

    def tearDown(self):
        pr.use_product("launcher")
        self.tmp.cleanup()

    def test_names_tags_and_files_are_the_hosts(self):
        self.assertEqual(pr.normalize_tag("0.6.0"), "host-v0.6.0")
        self.assertEqual(pr.normalize_tag("host-v0.6.0-beta.1"), "host-v0.6.0-beta.1")
        pr.check_tag("host-v0.6.0")
        with self.assertRaisesRegex(pr.Failure, "isn't a Lodestone Host version"):
            pr.check_tag("launcher-v0.6.0")
        self.assertEqual(pr.asset_name(7, "windows-x86_64"),
                         "lodestone-host-7-windows-x86_64-standalone.exe")
        self.assertEqual(pr.core_name(7, "windows-x86_64"), "lodestone-host-7-windows-x86_64-core.exe")
        self.assertEqual(pr.core_name(7, "macos-aarch64"), "lodestone-host-7-macos-aarch64-core")
        self.assertEqual(pr.dmg_name(7, ["macos-aarch64", "macos-x86_64"]),
                         "lodestone-host-7-macos-universal.dmg")
        self.assertEqual(pr.alias_for(pr.installer_name(7, "windows-x86_64")), "LodestoneHostSetup.exe")
        self.assertEqual(pr.alias_for(pr.dmg_name(7, ["macos-aarch64"])), "LodestoneHost.dmg")
        self.assertIsNone(pr.alias_for(pr.core_name(7, "windows-x86_64")))
        self.assertEqual(pr.release_title("host-v0.6.0"), "Lodestone Host 0.6.0")
        notes = pr.release_notes("host-v0.6.0", "abc")
        base = "https://github.com/Lodestone-Team/lodestone/releases/download/host-v0.6.0"
        self.assertIn(f"[LodestoneHost.dmg]({base}/LodestoneHost.dmg)", notes)
        self.assertIn(f"[LodestoneHostSetup.exe]({base}/LodestoneHostSetup.exe)", notes)
        self.assertNotIn("administrator", notes)
        self.assertNotIn("Applications", notes)
        self.assertEqual(pr.LAUNCHER_PATHS, ["launcher", "core", "crates"])

    def test_the_core_has_no_build_of_its_own_but_is_cleaned_up_with_its_host(self):
        self.assertEqual(pr.file_build(pr.asset_name(9, "macos-aarch64"), "macos-aarch64"), 9)
        self.assertIsNone(pr.file_build(pr.core_name(9, "macos-aarch64"), "macos-aarch64"))
        self.assertIsNone(pr.file_build(pr.asset_name(9, "macos-aarch64").replace("host", "launcher"),
                                        "macos-aarch64"))
        names = [n for b in (3, 5, 9) for n in (pr.asset_name(b, "macos-aarch64"),
                                                pr.core_name(b, "macos-aarch64"))]
        self.assertEqual(pr.stale_names(names, "macos-aarch64", set()),
                         ["lodestone-host-3-macos-aarch64-standalone",
                          "lodestone-host-3-macos-aarch64-core"])
        self.assertEqual(pr.release_platforms({"assets": [{"name": n} for n in names]}),
                         ["macos-aarch64"])

    def test_the_app_replaces_the_tauri_host(self):
        plist = pr.info_plist(10, "0.6.0")
        self.assertEqual(plist["CFBundleIdentifier"], "com.lodestone.cc")
        self.assertEqual(plist["CFBundleExecutable"], "lodestone-host")
        self.assertEqual(plist["CFBundleName"], "Lodestone Host")
        self.assertNotIn("CFBundleURLTypes", plist)
        self.assertEqual(pr.core_identifier(), "com.lodestone.cc.core")

    def test_a_staged_core_is_checked_too(self):
        staged_host(self.dir / "s", 7, ["macos-aarch64"])
        record = pr.read_stage(self.dir / "s")
        self.assertEqual(record["assets"]["macos-aarch64"]["core"]["name"],
                         "lodestone-host-7-macos-aarch64-core")
        (self.dir / "s" / pr.core_name(7, "macos-aarch64")).write_bytes(b"rebuilt")
        with self.assertRaisesRegex(pr.Failure, "changed since it was staged"):
            pr.read_stage(self.dir / "s")

    def test_the_manifest_lists_the_core_and_promotion_copies_it(self):
        win = self.dir / "win"
        staged_host(win, 10, ["windows-x86_64"], extras=[pr.installer_name(10, "windows-x86_64")])
        beta = FakeRelease(self.dir / "beta", "host-v0.6.0-beta.1", tag_commit="abc", draft=False)
        manifest = pr.publish(beta, [(win, pr.read_stage(win))], tag=beta.tag)
        core = manifest["assets"]["windows-x86_64"]["core"]
        self.assertEqual(core["name"], "lodestone-host-10-windows-x86_64-core.exe")
        self.assertIn(core["name"], beta.names())
        self.assertIn("LodestoneHostSetup.exe", beta.names())
        stable = FakeRelease(self.dir / "stable", "host-v0.6.0")
        copied = pr.copy_release(beta, stable, None, False, tag=stable.tag)
        self.assertEqual(copied["assets"], manifest["assets"])
        self.assertEqual((stable.folder.path / core["name"]).read_bytes(),
                         (beta.folder.path / core["name"]).read_bytes())
        # a core that isn't what the manifest says isn't copied
        (beta.folder.path / core["name"]).write_bytes(b"tampered")
        with self.assertRaisesRegex(pr.Failure, "core.exe doesn't match its manifest"):
            pr.copy_release(beta, FakeRelease(self.dir / "other", "host-v0.6.1"), None, False)

    def test_a_platform_the_manifest_lost_comes_back_with_its_core(self):
        win, mac = self.dir / "win", self.dir / "mac"
        staged_host(win, 10, ["windows-x86_64"])
        staged_host(mac, 10, ["macos-aarch64"])
        release = FakeRelease(self.dir / "r", "host-v0.6.0", tag_commit="abc")
        first = pr.publish(release, [(win, pr.read_stage(win))])
        (release.folder.path / pr.MANIFEST).unlink()
        manifest = pr.publish(release, [(mac, pr.read_stage(mac))])
        self.assertEqual(manifest["assets"]["windows-x86_64"], first["assets"]["windows-x86_64"])
        self.assertIn("core", manifest["assets"]["windows-x86_64"])
        # a Host binary without its core yet is still uploading: it's left out
        (release.folder.path / pr.core_name(10, "windows-x86_64")).unlink()
        (release.folder.path / pr.MANIFEST).unlink()
        manifest = pr.publish(release, [(mac, pr.read_stage(mac))], force=True)
        self.assertNotIn("windows-x86_64", manifest["assets"])

    def test_the_host_has_no_old_channel_to_bridge(self):
        with self.assertRaisesRegex(pr.Failure, "no old update channel"):
            pr.main(["--product", "host", "--bridge", "0.6.0"])

    def test_the_checklist_is_the_hosts(self):
        lines = pr.checklist(12, "0123456789", "macos-aarch64")
        self.assertIn('Settings › Updates shows this build and commit ("Build 12 · 0123456").', lines)
        self.assertTrue(any("EULA" in line for line in lines))
        self.assertFalse(any(line.startswith("Windows:") for line in lines))

    def test_the_core_links_the_c_runtime_statically_on_windows_only(self):
        # LS-40: the core builds from the root workspace, where the launcher's
        # .cargo/config.toml (and its +crt-static) doesn't apply.
        cmd = pr.core_build_command("release", "x86_64-pc-windows-msvc")
        self.assertIn("--no-default-features", cmd)
        self.assertEqual(cmd[cmd.index("--target") + 1], "x86_64-pc-windows-msvc")
        config = tomllib.loads(cmd[cmd.index("--config") + 1])
        launcher = tomllib.loads((pr.LAUNCHER / ".cargo" / "config.toml").read_text())
        # the same target table the launcher's config uses, so only MSVC builds get it
        self.assertEqual(set(config["target"]), set(launcher["target"]))
        (scope,) = config["target"]
        self.assertEqual(scope, 'cfg(all(windows, target_env = "msvc"))')
        self.assertEqual(config["target"][scope]["rustflags"], ["-C", "target-feature=+crt-static"])
        self.assertNotIn("--target", pr.core_build_command("dev-build", None))


XWIN_ENV_SAMPLE = '''export AR_x86_64_pc_windows_msvc="llvm-lib";
export CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER="lld-link";
export CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS="-C target-feature=+crt-static --cfg windows_slim_errors -C linker-flavor=lld-link -C link-arg=-nodefaultlib:ucrt -C link-arg=-defaultlib:libucrt -Lnative=/c/crt/lib/x86_64";
export CC_x86_64_pc_windows_msvc="clang-cl";
export CFLAGS_x86_64_pc_windows_msvc="--target=x86_64-pc-windows-msvc /imsvc /c/crt/include ";
export LIB="/c/crt/lib/x86_64;/c/sdk/lib/um/x86_64";
export PATH="/usr/bin:/bin:/c";
export RUSTFLAGS="";
export TARGET_CC="clang-cl";
'''

# What it prints in the root workspace (no rustflags config) with
# RUSTFLAGS=CORE_XWIN_RUSTFLAGS in its environment.
XWIN_CORE_ENV_SAMPLE = XWIN_ENV_SAMPLE.replace(
    "+crt-static --cfg windows_slim_errors", "+crt-static")

WIN = "x86_64-pc-windows-msvc"
MAC = "aarch64-apple-darwin"


def executable(path, text="#!/bin/sh\nexit 0\n"):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    path.chmod(0o755)
    return path


class CrossWindows(unittest.TestCase):
    """Windows builds on a Mac (launcher/docs/cross-windows.md)."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)
        self.cross_dir = self.dir / "cross"
        # A machine with nothing set up: no tools on PATH, no Homebrew, an empty
        # home, cargo-xwin cache and setup folder; not Windows, whatever runs the test.
        empty = self.dir / "empty-path"
        empty.mkdir()
        self.env = mock.patch.dict(os.environ, {
            "PATH": str(empty), "HOME": str(self.dir / "home"),
            "CARGO_HOME": str(self.dir / "cargo"), "XWIN_CACHE_DIR": str(self.dir / "xwin"),
            pr.CROSS_WINDOWS_DIR_ENV: str(self.cross_dir), "HOMEBREW_PREFIX": "",
        })
        self.env.start()
        for name in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "XWIN_ACCEPT_LICENSE",
                     "GPUI_FXC_PATH", "ISCC", "LOCALAPPDATA", "ProgramFiles(x86)",
                     "ProgramFiles", "TARGET_CC"):
            os.environ.pop(name, None)
        self.patches = [mock.patch.object(pr, "HOMEBREW_PREFIXES", ()),
                        mock.patch.object(pr, "on_windows", lambda: False)]
        for patch in self.patches:
            patch.start()

    def tearDown(self):
        for patch in reversed(self.patches):
            patch.stop()
        self.env.stop()
        self.tmp.cleanup()

    def set_up_everything(self):
        """What setup_cross_windows.py leaves: the tools, its wrappers, the CRT."""
        tools = self.dir / "tools"
        for name in ("clang-cl", "llvm-lib", "lld-link"):
            executable(tools / name)
        executable(self.dir / "cargo" / "bin" / "cargo-xwin")
        executable(self.cross_dir / "bin" / "fxc")
        executable(self.cross_dir / "bin" / "iscc")
        (self.dir / "xwin" / "xwin").mkdir(parents=True)
        (self.dir / "xwin" / "xwin" / "DONE").write_text("x86_64 aarch64\nsome.msi\n")
        os.environ["PATH"] = str(tools)

    def fake_run(self, calls, out=XWIN_ENV_SAMPLE, root_out=XWIN_CORE_ENV_SAMPLE):
        def run(cmd, cwd=pr.ROOT, env=None, quiet=True, shown=None):
            calls.append((cmd, cwd, env))
            if cmd[:3] != ["cargo", "xwin", "env"]:
                return ""
            # cargo-xwin reads the workspace's config: only the launcher's has rustflags
            return out if cwd == pr.LAUNCHER else root_out
        return run

    # The environment cargo-xwin prints

    def test_xwin_env_is_parsed_without_the_empty_rustflags(self):
        env = pr.parse_xwin_env("Some progress line\n" + XWIN_ENV_SAMPLE)
        self.assertEqual(env["CC_x86_64_pc_windows_msvc"], "clang-cl")
        self.assertEqual(env["LIB"], "/c/crt/lib/x86_64;/c/sdk/lib/um/x86_64")
        self.assertEqual(env["CFLAGS_x86_64_pc_windows_msvc"],
                         "--target=x86_64-pc-windows-msvc /imsvc /c/crt/include ")
        self.assertEqual(env["PATH"], "/usr/bin:/bin:/c")
        # RUSTFLAGS="" would replace the config's +crt-static: VCRUNTIME140.dll.
        self.assertNotIn("RUSTFLAGS", env)
        self.assertEqual(len(env), 8)

    # Which builds get it

    def test_macos_only_and_windows_hosts_check_and_run_nothing(self):
        def refuse(*args, **kwargs):
            raise AssertionError("ran a command")
        with mock.patch.object(pr, "run", refuse):
            self.assertEqual(pr.cross_windows_builds([None], True), {})
            self.assertEqual(pr.cross_windows_builds([MAC, "x86_64-apple-darwin"], True), {})
            with mock.patch.object(pr, "on_windows", lambda: True):
                self.assertEqual(pr.cross_windows_builds([WIN], True), {})
                self.assertFalse(pr.is_cross_windows(WIN))

    def test_only_x64_windows_is_cross_built(self):
        with self.assertRaisesRegex(pr.Failure, "only x86_64-pc-windows-msvc .* not aarch64-pc-windows-msvc"):
            pr.cross_windows_builds(["aarch64-pc-windows-msvc"], False)

    def test_a_ready_machine_gets_cargo_xwins_environment_and_the_setup_folders_fxc(self):
        self.set_up_everything()
        calls = []
        with mock.patch.object(pr, "run", self.fake_run(calls)), \
                mock.patch.object(pr, "has_rust_target", lambda target: True):
            cross = pr.cross_windows_builds([MAC, WIN], True)
        self.assertEqual(list(cross), [WIN])
        build = cross[WIN]
        self.assertEqual(build.fxc, str(self.cross_dir / "bin" / "fxc"))
        self.assertEqual(build.env["TARGET_CC"], "clang-cl")
        self.assertNotIn("RUSTFLAGS", build.env)
        self.assertEqual(build.gpui_env()["GPUI_FXC_PATH"], build.fxc)
        self.assertNotIn("GPUI_FXC_PATH", build.env)
        # the launcher has no core: one environment, for the launcher's workspace
        self.assertIsNone(build.core_env)
        (cmd, cwd, env), = calls
        self.assertEqual(cmd, ["cargo", "xwin", "env", "--target", WIN])
        self.assertEqual(cwd, pr.LAUNCHER)
        self.assertNotIn("RUSTFLAGS", env)
        # cargo-xwin found after the developer's PATH, not before it
        self.assertEqual(env["PATH"].split(os.pathsep),
                         [str(self.dir / "tools"), str(self.dir / "cargo" / "bin")])

    def test_the_hosts_core_gets_the_root_workspaces_environment_not_the_launchers(self):
        pr.use_product("host")
        self.addCleanup(pr.use_product, "launcher")
        self.set_up_everything()
        calls = []
        with mock.patch.object(pr, "run", self.fake_run(calls)), \
                mock.patch.object(pr, "has_rust_target", lambda target: True):
            build = pr.cross_windows_builds([WIN], True)[WIN]
        (_, app_cwd, app_call), (cmd, core_cwd, core_call) = calls
        self.assertEqual((app_cwd, core_cwd), (pr.LAUNCHER, pr.ROOT))
        self.assertEqual(cmd, ["cargo", "xwin", "env", "--target", WIN])
        # cargo-xwin sees the static C runtime for the core, and only in that call
        self.assertEqual(core_call["RUSTFLAGS"], "-C target-feature=+crt-static")
        self.assertNotIn("RUSTFLAGS", app_call)
        self.assertNotIn("RUSTFLAGS", build.core_env)
        app_flags = build.env["CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS"]
        core_flags = build.core_env["CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS"]
        self.assertIn("--cfg windows_slim_errors", app_flags)
        self.assertNotIn("windows_slim_errors", core_flags)
        for flag in ("+crt-static", "-defaultlib:libucrt", "-nodefaultlib:ucrt"):
            self.assertIn(flag, core_flags)
        self.assertEqual(build.core_env["TARGET_CC"], "clang-cl")

    def test_one_message_names_everything_missing_and_the_setup_script(self):
        os.environ["RUSTFLAGS"] = "-C opt-level=1"
        with mock.patch.object(pr, "has_rust_target", lambda target: False), \
                self.assertRaises(pr.Failure) as raised:
            pr.cross_windows_builds([WIN], True)
        message = str(raised.exception)
        for expected in ("can't build windows-x86_64 yet", "cargo-xwin isn't installed",
                         "--version " + pr.CARGO_XWIN_VERSION, "clang-cl isn't on PATH",
                         "llvm-lib isn't on PATH", "lld-link isn't on PATH or in Homebrew's lld",
                         "rustup target add x86_64-pc-windows-msvc",
                         "--accept-microsoft-license", pr.MICROSOFT_LICENSE,
                         f"no {self.cross_dir / 'bin' / 'fxc'}", "Inno Setup isn't set up",
                         "RUSTFLAGS is set", "python3 launcher/scripts/setup_cross_windows.py",
                         "launcher/docs/cross-windows.md", str(self.cross_dir)):
            self.assertIn(expected, message)
        self.assertEqual(message.count("\n  - "), 9)

    def test_no_installer_needs_no_inno_setup(self):
        self.set_up_everything()
        (self.cross_dir / "bin" / "iscc").unlink()
        with mock.patch.object(pr, "has_rust_target", lambda target: True):
            self.assertEqual(pr.cross_windows_problems(WIN, installer=False), [])
            (problem,) = pr.cross_windows_problems(WIN, installer=True)
        self.assertIn("pass --no-installer", problem)

    def test_the_license_can_be_accepted_for_cargo_xwin_to_download_the_crt(self):
        self.set_up_everything()
        (self.dir / "xwin" / "xwin" / "DONE").unlink()
        with mock.patch.object(pr, "has_rust_target", lambda target: True):
            (problem,) = pr.cross_windows_problems(WIN, installer=True)
            self.assertIn("Microsoft's license", problem)
            os.environ["XWIN_ACCEPT_LICENSE"] = "1"
            self.assertEqual(pr.cross_windows_problems(WIN, installer=True), [])

    def test_a_cache_for_another_architecture_doesnt_count(self):
        self.set_up_everything()
        (self.dir / "xwin" / "xwin" / "DONE").write_text("aarch64\n")
        self.assertFalse(pr.has_xwin_crt())

    # Finding the tools

    def test_the_setup_folder_defaults_to_application_support(self):
        del os.environ[pr.CROSS_WINDOWS_DIR_ENV]
        with mock.patch.object(pr.sys, "platform", "darwin"):
            self.assertEqual(pr.cross_windows_dir(), self.dir / "home" / "Library" /
                             "Application Support" / "Lodestone" / "cross-windows")
        os.environ[pr.CROSS_WINDOWS_DIR_ENV] = "~/elsewhere"
        self.assertEqual(pr.cross_windows_dir(), self.dir / "home" / "elsewhere")

    def test_homebrews_keg_only_llvm_and_lld_are_found(self):
        brew = self.dir / "brew"
        executable(brew / "opt" / "llvm" / "bin" / "clang-cl")
        executable(brew / "opt" / "lld" / "bin" / "lld-link")
        with mock.patch.object(pr, "HOMEBREW_PREFIXES", (str(brew),)):
            self.assertEqual(pr.find_cross_tool("clang-cl"), str(brew / "opt" / "llvm" / "bin" / "clang-cl"))
            self.assertIsNone(pr.find_cross_tool("llvm-lib"))
            path = pr.cross_tool_path().split(os.pathsep)
        self.assertEqual(path[1:], [str(brew / "opt" / "llvm" / "bin"), str(brew / "opt" / "lld" / "bin")])

    def test_gpui_fxc_path_overrides_the_setup_folder(self):
        self.set_up_everything()
        own = executable(self.dir / "my-fxc")
        os.environ["GPUI_FXC_PATH"] = str(own)
        self.assertEqual(pr.find_fxc(), (str(own), None))
        os.environ["GPUI_FXC_PATH"] = str(self.dir / "missing")
        fxc, problem = pr.find_fxc()
        self.assertIsNone(fxc)
        self.assertIn("GPUI_FXC_PATH is", problem)

    def test_iscc_is_found_in_the_setup_folder_off_windows_only(self):
        with self.assertRaisesRegex(pr.Failure, "setup_cross_windows.py"):
            pr.find_iscc()
        wrapper = executable(self.cross_dir / "bin" / "iscc")
        self.assertEqual(pr.find_iscc(), wrapper)
        os.environ["ISCC"] = str(executable(self.dir / "own-iscc"))
        self.assertEqual(pr.find_iscc(), self.dir / "own-iscc")
        del os.environ["ISCC"]
        with mock.patch.object(pr, "on_windows", lambda: True):
            self.assertNotIn(str(wrapper), pr.iscc_candidates())
            with self.assertRaisesRegex(pr.Failure, "winget install"):
                pr.find_iscc()

    # The builds

    def cross_build(self):
        return pr.CrossBuild(env={"TARGET_CC": "clang-cl", "PATH": "/x",
                                  "CC_x86_64_pc_windows_msvc": "clang-cl",
                                  "WORKSPACE": "launcher"}, fxc="/f/fxc",
                             core_env={"TARGET_CC": "clang-cl", "PATH": "/x",
                                       "WORKSPACE": "root"})

    def built(self, calls, data=b"MZ static"):
        def copy_built(workspace, program, target, key, copy, profile="release"):
            copy.write_bytes(data)
            return copy
        return mock.patch.object(pr, "copy_built", copy_built)

    def test_a_cross_build_is_the_same_cargo_command_with_cargo_xwins_environment(self):
        calls = []
        with mock.patch.object(pr, "run", self.fake_run(calls)), self.built(calls):
            pr.build_binary(WIN, 9, "abc", "windows-x86_64", self.dir, True, self.cross_build())
            pr.build_binary(WIN, 9, "abc", "windows-x86_64", self.dir, True)
        (cross_cmd, cross_cwd, cross_env), (cmd, cwd, env) = calls
        self.assertEqual(cross_cmd, ["cargo", "build", "--profile", "dev-build", "--locked",
                                     "-p", "lodestone-launcher", "--target", WIN])
        self.assertEqual((cross_cmd, cross_cwd), (cmd, cwd))
        self.assertEqual(cross_env["TARGET_CC"], "clang-cl")
        self.assertEqual(cross_env["GPUI_FXC_PATH"], "/f/fxc")
        self.assertEqual(cross_env["PATH"], "/x")
        self.assertEqual(cross_env["LODESTONE_LAUNCHER_CHANNEL"], "dev")
        self.assertEqual(cross_env["LODESTONE_LAUNCHER_BUILD"], "9")
        self.assertNotIn("TARGET_CC", env)
        self.assertNotIn("GPUI_FXC_PATH", env)
        self.assertEqual(env, {**os.environ, "LODESTONE_LAUNCHER_BUILD": "9",
                               "LODESTONE_LAUNCHER_COMMIT": "abc",
                               "LODESTONE_LAUNCHER_CHANNEL": "dev"})

    def test_the_cores_cross_build_gets_cargo_xwins_environment_without_fxc(self):
        calls = []
        with mock.patch.object(pr, "run", self.fake_run(calls)), self.built(calls):
            pr.build_core(WIN, 9, "windows-x86_64", self.dir, False, self.cross_build())
            pr.build_core(MAC, 9, "macos-aarch64", self.dir, False)
        (cross_cmd, cross_cwd, cross_env), (cmd, cwd, env) = calls
        self.assertEqual(cross_cmd, pr.core_build_command("release", WIN))
        self.assertEqual(cross_cwd, pr.ROOT)
        self.assertEqual(cross_env["TARGET_CC"], "clang-cl")
        self.assertEqual(cross_env["WORKSPACE"], "root")
        self.assertNotIn("GPUI_FXC_PATH", cross_env)
        # unchanged: the core's macOS build inherits the environment, as before
        self.assertEqual((cmd, cwd, env), (pr.core_build_command("release", MAC), pr.ROOT, None))

    def test_a_cross_build_without_the_cores_environment_is_refused(self):
        cross = pr.CrossBuild(env={"TARGET_CC": "clang-cl"}, fxc="/f/fxc")
        with mock.patch.object(pr, "run", self.fake_run([])), self.built([]), \
                self.assertRaisesRegex(pr.Failure, "no cargo-xwin environment for the core"):
            pr.build_core(WIN, 9, "windows-x86_64", self.dir, False, cross)

    def test_open_with_only_a_windows_build_is_refused_before_building(self):
        def run(cmd, cwd=pr.ROOT, env=None, quiet=True, shown=None):
            return {"rev-parse": "abc1234def", "rev-list": "9"}.get(cmd[1], "")

        def refuse(*args, **kwargs):
            raise AssertionError("started the build")
        with mock.patch.object(pr, "run", run), \
                mock.patch.object(pr, "workspace_version", lambda: "0.1.0"), \
                mock.patch.object(pr, "host_target", lambda: MAC), \
                mock.patch.object(pr, "cross_windows_builds", refuse), \
                mock.patch.object(pr, "stage", refuse), \
                self.assertRaises(pr.Failure) as raised:
            pr.main(["--dev", "--dry-run", "--open", "--allow-dirty", "--target", WIN])
        message = str(raised.exception)
        self.assertIn("nothing to open", message)
        self.assertIn("Drop --open", message)
        self.assertIn("tested on Windows", message)

    def test_a_cross_build_that_needs_vcruntime_is_refused(self):
        with mock.patch.object(pr, "run", self.fake_run([])), \
                self.built([], b"MZ ... VCRUNTIME140.dll ..."), \
                self.assertRaisesRegex(pr.Failure, "links the C runtime dynamically"):
            pr.build_binary(WIN, 9, "abc", "windows-x86_64", self.dir, False, self.cross_build())

    def test_mixed_targets_each_get_only_their_own_environment(self):
        pr.use_product("host")
        self.addCleanup(pr.use_product, "launcher")
        seen = []

        def build_binary(target, build, commit, key, into, dev=False, cross=None):
            seen.append(("app", target, cross))
            path = into / pr.asset_name(build, key)
            path.write_bytes(b"app")
            return path

        def build_core(target, build, key, into, dev=False, cross=None):
            seen.append(("core", target, cross))
            path = into / pr.core_name(build, key)
            path.write_bytes(b"core")
            return path

        def build_dmg(app, name, identity, into):
            (into / name).write_bytes(b"dmg")
            return into / name

        cross = {WIN: self.cross_build()}
        with mock.patch.object(pr, "build_binary", build_binary), \
                mock.patch.object(pr, "build_core", build_core), \
                mock.patch.object(pr, "host_target", lambda: MAC), \
                mock.patch.object(pr, "codesign", lambda *a, **k: None), \
                mock.patch.object(pr, "build_app", lambda *a, **k: self.dir / "app"), \
                mock.patch.object(pr, "build_dmg", build_dmg), \
                mock.patch.object(pr, "notarize", lambda dmg: False):
            record = pr.stage([MAC, WIN], 9, "abc", "0.1.0", self.dir / "stage", False, True,
                              cross)
        self.assertEqual(seen, [("app", MAC, None), ("core", MAC, None),
                                ("app", WIN, cross[WIN]), ("core", WIN, cross[WIN])])
        self.assertEqual(set(record["assets"]), {"macos-aarch64", "windows-x86_64"})


def staged_dev(folder, build, keys, commit="abc1234def", extras=()):
    """A developer build staged as `stage(dev=True)` leaves it."""
    record = staged(folder, build, keys, commit, extras)
    record["channel"] = pr.DEV_CHANNEL
    (folder / pr.STAGE_RECORD).write_text(pr.json.dumps(record))
    return record


class FakeDevRelease(pr.DevRelease):
    """A developer build's release with a folder for its files and no GitHub."""

    def __init__(self, path, tag, exists=False, others=()):
        super().__init__("test/repo", tag)
        self.folder = pr.Folder(path)
        self.exists = exists
        self._tag_commit = None
        self.others = list(others)
        self.created = None
        self.pruned = False

    def info(self, fresh=False):
        if not self.exists:
            return None
        return {"tagName": self.tag, "isDraft": False, "isPrerelease": True,
                "url": f"release {self.tag}",
                "assets": self.folder.assets()}

    names = FakeRelease.names
    download = FakeRelease.download
    manifest = FakeRelease.manifest
    upload = FakeRelease.upload
    delete = FakeRelease.delete
    tag_commit = FakeRelease.tag_commit

    def create(self, commit):
        self.created = commit
        self._tag_commit = commit
        self.exists = True

    def published_releases(self):
        return self.others

    def finished(self, manifest):
        self.pruned = True


class Dev(unittest.TestCase):
    """--dev: developer builds, for developers' own launchers."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def stage(self, name, build=40, keys=("windows-x86_64",), commit="abc1234def", dev=True):
        make = staged_dev if dev else staged
        make(self.dir / name, build, list(keys), commit)
        return self.dir / name, pr.read_stage(self.dir / name)

    def test_a_developer_build_is_tagged_by_build_and_commit(self):
        self.assertEqual(pr.dev_tag(3612, "ab12cd3ef0"), "launcher-dev-3612-ab12cd3")
        self.assertTrue(pr.is_dev_tag("launcher-dev-3612-ab12cd3"))
        self.assertFalse(pr.is_dev_tag("launcher-v1.0.0"))
        self.assertEqual(pr.dev_release_title("launcher-dev-3612-ab12cd3"),
                         "Lodestone Launcher developer build 3612 (ab12cd3)")
        self.assertEqual(pr.dev_tag_of([self.stage("a")]), "launcher-dev-40-abc1234")

    def test_its_release_is_made_published_and_old_ones_are_pruned(self):
        win = self.stage("win")
        release = FakeDevRelease(self.dir / "rel", pr.dev_tag_of([win]))
        manifest = pr.publish(release, [win], tag=release.tag)
        self.assertEqual(release.created, "abc1234def")
        self.assertEqual(manifest["tag"], "launcher-dev-40-abc1234")
        self.assertTrue(release.pruned)
        # another platform's build of the same commit joins it
        mac = self.stage("mac", keys=("macos-aarch64",))
        manifest = pr.publish(release, [mac])
        self.assertEqual(set(manifest["assets"]), {"windows-x86_64", "macos-aarch64"})
        with self.assertRaisesRegex(pr.Failure, "already has windows-x86_64"):
            pr.publish(release, [win])
        self.assertIsNone(pr.publish(release, [win], skip_existing=True))

    def test_another_commits_build_goes_to_its_own_release(self):
        release = FakeDevRelease(self.dir / "rel", "launcher-dev-40-abc1234")
        with self.assertRaisesRegex(pr.Failure, "go to launcher-dev-41-def5678"):
            pr.publish(release, [self.stage("other", 41, commit="def5678aaa")])
        with self.assertRaisesRegex(pr.Failure, "different commits"):
            pr.dev_tag_of([self.stage("a"), self.stage("b", 41, commit="def5678aaa")])

    def test_developer_builds_and_versions_dont_mix(self):
        version = FakeRelease(self.dir / "v", "launcher-v0.2.0", tag_commit="abc1234def")
        with self.assertRaisesRegex(pr.Failure, "only goes to a developer build's release"):
            pr.publish(version, [self.stage("dev")])
        with self.assertRaisesRegex(pr.Failure, "isn't a developer build"):
            pr.dev_tag_of([self.stage("plain", dev=False)])

    def test_a_build_developers_wouldnt_update_to_is_only_noted(self):
        newer = published("launcher-v0.3.0", {"windows-x86_64": 90})
        win = self.stage("win")
        release = FakeDevRelease(self.dir / "rel", pr.dev_tag_of([win]), others=[newer])
        self.assertIsNotNone(pr.publish(release, [win]))

    def test_developers_read_every_release_and_developer_build(self):
        releases = [
            published("launcher-dev-50-aaaaaaa", {"windows-x86_64": 50}, prerelease=True),
            published("launcher-v0.4.0-beta.1", {"windows-x86_64": 45}, prerelease=True),
            published("launcher-v0.5.0", {"windows-x86_64": 99}, draft=True),
        ]
        self.assertEqual(pr.newest_for_developers(releases, "windows-x86_64", "x"),
                         (50, "launcher-dev-50-aaaaaaa"))
        self.assertEqual(pr.newest_for_developers(releases, "windows-x86_64",
                                                  "launcher-dev-50-aaaaaaa"),
                         (45, "launcher-v0.4.0-beta.1"))
        # versions never compete with developer builds
        self.assertEqual(pr.newest_published(releases, "windows-x86_64", True, "x"),
                         (45, "launcher-v0.4.0-beta.1"))

    def test_only_the_newest_developer_builds_are_kept(self):
        def dev(n, **kw):
            return dict(published(f"launcher-dev-{n}-aaaaaaa", {}, prerelease=True, **kw),
                        created_at=f"2026-11-{n:02d}T00:00:00Z")
        releases = [dev(n) for n in range(1, 9)] + [
            dict(published("launcher-v0.1.0", {}), created_at="2026-10-01T00:00:00Z"),
            dict(published("launcher-dev-0-draft", {}, prerelease=True, draft=True),
                 created_at="2026-10-01T00:00:00Z"),
        ]
        self.assertEqual(pr.old_dev_builds(releases, keep=5),
                         ["launcher-dev-3-aaaaaaa", "launcher-dev-2-aaaaaaa",
                          "launcher-dev-1-aaaaaaa"])
        # the build just uploaded is always kept, and counts among them
        self.assertEqual(pr.old_dev_builds(releases, "launcher-dev-1-aaaaaaa", keep=2),
                         [f"launcher-dev-{n}-aaaaaaa" for n in range(7, 1, -1)])

    def test_dev_leaves_out_tag_and_version_modes(self):
        with self.assertRaisesRegex(pr.Failure, "leave out --tag"):
            pr.main(["--dev", "--tag", "1.0.0"])
        with self.assertRaisesRegex(pr.Failure, "use it with --dry-run"):
            pr.main(["--dev", "--find-draft"])

    def test_the_hosts_developer_builds(self):
        pr.use_product("host")
        try:
            self.assertEqual(pr.dev_tag(12, "ab12cd3ef0"), "host-dev-12-ab12cd3")
            self.assertIn("Settings › Updates › **Developer builds**",
                          pr.dev_release_notes("host-dev-12-ab12cd3", "ab12cd3ef0"))
        finally:
            pr.use_product("launcher")


class Workflows(unittest.TestCase):
    """The release workflows' steps that run this script."""

    def test_a_failure_isnt_hidden_by_a_pipe(self):
        # LS-34: a step without `shell: bash` runs as `bash -e` without pipefail, so
        # `publish_release.py ... | tee` passed even when the script exited 1.
        folder = pr.ROOT / ".github" / "workflows"
        checked = 0
        for path in sorted(folder.glob("*.yml")):
            steps = re.split(r"\n(?=\s*- (?:name|uses):)", path.read_text())
            for text in steps:
                if "publish_release.py" in text and re.search(r"\|\s*tee\b", text):
                    checked += 1
                    with self.subTest(workflow=path.name, step=text.strip().splitlines()[0]):
                        self.assertTrue(re.search(r"^\s*shell: bash\s*$", text, re.M)
                                        or "pipefail" in text)
        self.assertGreater(checked, 0)

    def test_builds_keep_the_drafts_prerelease_flag(self):
        # LS-66: the core and dashboard builds updated release-draft.yml's stable
        # vX.Y.Z draft with `prerelease: true`, so it was published as a pre-release.
        folder = pr.ROOT / ".github" / "workflows"
        for path in sorted(folder.glob("*.yml")):
            with self.subTest(workflow=path.name):
                self.assertNotRegex(path.read_text(), r"(?m)^\s*prerelease: true\s*$")
        release = (folder / "release.yml").read_text()
        self.assertEqual(release.count("prerelease: ${{ needs.draft.outputs.prerelease }}"), 2)


if __name__ == "__main__":
    unittest.main()
