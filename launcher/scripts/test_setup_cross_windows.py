"""Tests for setup_cross_windows.py: python3 launcher/scripts/test_setup_cross_windows.py

No network and no installs: the pieces that choose what to download, and the wrappers
it writes, run against stand-ins."""

import io
import os
import subprocess
import sys
import tempfile
import unittest
import urllib.error
import urllib.request
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

import publish_release as pr
import setup_cross_windows as setup


class Layout(unittest.TestCase):
    def test_publish_release_finds_the_wrappers_where_setup_writes_them(self):
        with tempfile.TemporaryDirectory() as tmp:
            old = os.environ.get(pr.CROSS_WINDOWS_DIR_ENV)
            os.environ[pr.CROSS_WINDOWS_DIR_ENV] = tmp
            try:
                layout = setup.Layout(pr.cross_windows_dir())
                self.assertEqual(layout.fxc_wrapper, Path(tmp) / "bin" / "fxc")
                self.assertIn(str(layout.iscc_wrapper), pr.iscc_candidates())
            finally:
                if old is None:
                    del os.environ[pr.CROSS_WINDOWS_DIR_ENV]
                else:
                    os.environ[pr.CROSS_WINDOWS_DIR_ENV] = old

    def test_downloads_are_pinned(self):
        for download in (setup.WINE, setup.INNO_SETUP):
            self.assertTrue(download.url.startswith("https://github.com/"))
            self.assertRegex(download.sha256, r"^[0-9a-f]{64}$")
            self.assertTrue(download.url.endswith("/" + download.name))
        self.assertEqual(setup.CARGO_XWIN_VERSION, pr.CARGO_XWIN_VERSION)


class WindowsSdk(unittest.TestCase):
    def test_the_pinned_sdk_is_taken_or_else_the_newest(self):
        manifest = {"packages": [{"id": "Win11SDK_10.0.22621"}, {"id": "Win11SDK_10.0.26100"},
                                 {"id": "Win10SDK_10.0.19041"}]}
        self.assertEqual(setup.sdk_package(manifest), ({"id": "Win11SDK_10.0.26100"}, None))
        manifest["packages"][1]["id"] = "Win11SDK_10.0.28000"
        package, why = setup.sdk_package(manifest)
        self.assertEqual(package["id"], "Win11SDK_10.0.28000")
        self.assertIn("no longer has Win11SDK_10.0.26100", why)
        with self.assertRaises(setup.Failure):
            setup.sdk_package({"packages": [{"id": "Win10SDK_10.0.19041"}]})

    def test_payloads_are_found_by_file_name(self):
        package = {"payloads": [{"fileName": "Installers/a b.msi", "url": "u1"},
                                {"fileName": "Installers\\x.cab", "url": "u2"}]}
        self.assertEqual(set(setup.sdk_payloads(package)), {"a b.msi", "x.cab"})

    def test_the_cabinets_come_from_the_msis_media_table(self):
        table = ("DiskId\tLastSequence\tDiskPrompt\tCabinet\tVolumeLabel\tSource\n"
                 "i2\ti4\tL64\tS255\tS32\tS72\nMedia\tDiskId\n1\t0\t\t\t\t\n"
                 "2\t6\t\t2630bae9681db6a9f6722366f47d055c.cab\t\t\n"
                 "3\t8\t\t26ea25236f12b23db661acf268a70cfa.cab\t\t\n")
        self.assertEqual(setup.media_cabinets(table), ["2630bae9681db6a9f6722366f47d055c.cab",
                                                       "26ea25236f12b23db661acf268a70cfa.cab"])

    def test_fxc_is_the_x64_one_with_its_compiler_dll(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            kits = root / "Program Files" / "Windows Kits" / "10" / "bin"
            for arch in ("x86", "x64", "arm64"):
                (kits / "10.0.26100.0" / arch).mkdir(parents=True)
                (kits / "10.0.26100.0" / arch / "fxc.exe").write_bytes(b"MZ")
            self.assertIsNone(setup.find_fxc(root))
            (kits / "10.0.26100.0" / "x64" / "d3dcompiler_47.dll").write_bytes(b"MZ")
            self.assertEqual(setup.find_fxc(root), kits / "10.0.26100.0" / "x64" / "fxc.exe")


@unittest.skipIf(os.name == "nt", "the wrappers are bash scripts")
class Wrappers(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.layout = setup.Layout(Path(self.tmp.name) / "set up here")
        # A stand-in for Wine that prints what it was given, one argument a line.
        self.layout.wine.parent.mkdir(parents=True)
        self.layout.wine.write_text('#!/bin/sh\nprintf "WINEPREFIX=%s\\n" "$WINEPREFIX"\n'
                                    'for a in "$@"; do printf "%s\\n" "$a"; done\n')
        self.layout.wine.chmod(0o755)

    def tearDown(self):
        self.tmp.cleanup()

    def run_wrapper(self, defines, *args):
        program = self.layout.iscc_exe
        script = Path(self.tmp.name) / "wrapper"
        setup.write_if_changed(script, setup.wrapper(["test"], self.layout, program, defines))
        out = subprocess.run([str(script), *args], capture_output=True, text=True, check=True)
        return out.stdout.splitlines()

    def test_iscc_gets_wine_paths_in_bare_arguments_and_defines(self):
        lines = self.run_wrapper(True, "/Q", "/DSourceExe=/a b/stage/x.exe", "/DBuild=12",
                                 "/DOutputName=lodestone-1-setup", "/repo/lodestone-launcher.iss")
        self.assertEqual(lines[0], f"WINEPREFIX={self.layout.prefix}")
        self.assertEqual(lines[1], str(self.layout.iscc_exe))
        self.assertEqual(lines[2:], ["/Q", r"/DSourceExe=Z:\a b\stage\x.exe", "/DBuild=12",
                                     "/DOutputName=lodestone-1-setup",
                                     r"Z:\repo\lodestone-launcher.iss"])

    def test_fxc_passes_its_switches_through(self):
        lines = self.run_wrapper(False, "/T", "vs_5_0", "/E", "quad_vs", "/Fh",
                                 "/target/out/quad_vs.h", "/O3", "/src/shaders.hlsl")
        self.assertEqual(lines[2:], ["/T", "vs_5_0", "/E", "quad_vs", "/Fh",
                                     r"Z:\target\out\quad_vs.h", "/O3", r"Z:\src\shaders.hlsl"])

    def test_writing_the_same_wrapper_again_changes_nothing(self):
        path = Path(self.tmp.name) / "w"
        text = setup.wrapper(["x"], self.layout, self.layout.iscc_exe, True)
        self.assertTrue(setup.write_if_changed(path, text))
        self.assertFalse(setup.write_if_changed(path, text))


class Output(io.StringIO):
    """Standard output for main(), which asks for line buffering."""

    def reconfigure(self, **kwargs):
        pass


class Safety(unittest.TestCase):
    """What it downloads, what it asks first, and what --check leaves alone."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.layout = setup.Layout(Path(self.tmp.name) / "set up here")

    def tearDown(self):
        self.tmp.cleanup()

    def test_fxc_waits_for_the_license_and_downloads_nothing_without_it(self):
        def refuse(*args, **kwargs):
            raise AssertionError("downloaded something")
        with mock.patch.object(setup, "fetch", refuse), \
                mock.patch.object(setup, "ensure_download", refuse):
            for check in (False, True):
                state, detail = setup.step_fxc(self.layout, check, accept=False)
                self.assertEqual(state, "missing")
                self.assertIn(setup.WINDOWS_SDK_LICENSE, detail)
                self.assertIn("--accept-microsoft-license", detail)
        self.assertFalse(self.layout.root.exists())

    def test_only_https_is_downloaded(self):
        for url in ("http://example.com/x.msi", "ftp://x/y", None, 3):
            with self.assertRaisesRegex(setup.Failure, "isn't an https URL"):
                setup.check_https(url)
        handler = setup.HttpsOnlyRedirects()
        request = urllib.request.Request("https://example.com/a")
        with self.assertRaisesRegex(setup.Failure, "isn't https"):
            handler.redirect_request(request, None, 302, "Found", {}, "http://example.com/b")

    def test_the_manifest_is_checked_against_the_channel(self):
        channel = {"channelItems": [{"type": "Manifest", "id": setup.VS_MANIFEST_ID,
                                     "payloads": [{"url": "https://x/vs.vsman",
                                                   "sha256": "ab" * 32}]}]}
        download = setup.manifest_download(channel)
        self.assertEqual((download.url, download.sha256), ("https://x/vs.vsman", "ab" * 32))
        for broken in ({}, {"channelItems": "no"}, {"channelItems": [{"type": "Manifest"}]},
                       {"channelItems": [{"type": "Manifest", "id": setup.VS_MANIFEST_ID}]},
                       {"channelItems": [{"type": "Manifest", "id": setup.VS_MANIFEST_ID,
                                          "payloads": [{"url": "https://x/vs.vsman"}]}]}):
            with self.assertRaises(setup.Failure):
                setup.manifest_download(broken)
        channel["channelItems"][0]["payloads"][0]["url"] = "http://x/vs.vsman"
        with self.assertRaisesRegex(setup.Failure, "https"):
            setup.manifest_download(channel)

    def test_a_manifest_of_another_shape_is_a_failure_not_a_traceback(self):
        package = {"id": "Win11SDK_10.0.26100"}
        with self.assertRaises(setup.Failure):
            setup.sdk_payloads(package)
        with self.assertRaises(setup.Failure):
            setup.sdk_package({"packages": None})
        payloads = setup.sdk_payloads({"payloads": [{"fileName": r"Installers\a.msi"}]})
        with self.assertRaisesRegex(setup.Failure, "without a URL and SHA-256"):
            setup.payload_download(package, payloads, "a.msi")
        with self.assertRaisesRegex(setup.Failure, "has no b.msi"):
            setup.payload_download(package, payloads, "b.msi")

    def test_a_failed_download_is_a_failure_and_leaves_no_partial_file(self):
        folder = Path(self.tmp.name) / "d"

        def offline(url, timeout):
            raise urllib.error.URLError("no network")
        download = setup.Download("https://example.com/x.exe", "00" * 32, "x.exe")
        with mock.patch.object(setup, "open_url", offline), \
                self.assertRaisesRegex(setup.Failure, "couldn't download https://example.com/x.exe"):
            setup.ensure_download(download, folder)
        self.assertEqual(list(folder.iterdir()), [])
        with mock.patch.object(setup, "open_url", offline), \
                self.assertRaisesRegex(setup.Failure, "couldn't download"):
            setup.fetch("https://example.com/channel")

    def test_inno_setup_counts_only_when_the_pinned_installer_finished(self):
        self.layout.iscc_exe.parent.mkdir(parents=True)
        self.layout.iscc_exe.write_bytes(b"MZ")
        state, detail = setup.step_inno_setup(self.layout, check=True)
        self.assertEqual(state, "missing")
        self.assertIn("no record", detail)
        self.layout.inno_record.write_text("cd" * 32 + "\n")
        state, detail = setup.step_inno_setup(self.layout, check=True)
        self.assertEqual(state, "missing")
        self.assertIn(f"isn't {setup.INNO_SETUP_VERSION}", detail)
        self.layout.inno_record.write_text(setup.INNO_SETUP.sha256 + "\n")
        self.assertEqual(setup.step_inno_setup(self.layout, check=True)[0], "ok")

    def test_the_final_check_runs_nothing_under_wine_until_it_is_all_there(self):
        def refuse(*args, **kwargs):
            raise AssertionError("ran a wrapper")
        with mock.patch.object(setup.subprocess, "run", refuse), \
                mock.patch.object(setup.pr, "cross_windows_problems", lambda t, installer: []), \
                mock.patch.dict(os.environ, {pr.CROSS_WINDOWS_DIR_ENV: str(self.layout.root)}):
            checks = setup.final_check(self.layout, wine_ready=False)
        self.assertEqual([ok for _, ok, _ in checks], [None, None, True])

    def test_a_wrapper_that_hangs_or_wont_run_is_a_failed_check(self):
        for error in (subprocess.TimeoutExpired("fxc", 300), PermissionError("denied")):
            def run(*args, **kwargs):
                raise error
            with mock.patch.object(setup.subprocess, "run", run), \
                    mock.patch.object(setup.pr, "cross_windows_problems", lambda t, installer: []), \
                    mock.patch.dict(os.environ, {pr.CROSS_WINDOWS_DIR_ENV: str(self.layout.root)}):
                checks = setup.final_check(self.layout, wine_ready=True)
            self.assertEqual([ok for _, ok, _ in checks], [False, False, True])

    def test_a_folder_of_its_own_ends_with_the_variable_to_set(self):
        root = Path(self.tmp.name) / "my folder"
        steps = ("step_rosetta", "step_brew", "step_cargo_xwin", "step_rust_target",
                 "step_wine", "step_fxc", "step_inno_setup", "step_wrappers", "step_crt")
        patches = [mock.patch.object(setup, name, lambda *a, **k: ("ok", "there"))
                   for name in steps]
        patches.append(mock.patch.object(setup, "final_check",
                                         lambda layout, ready: [("x", True, "fine")]))
        patches.append(mock.patch.object(setup.sys, "platform", "darwin"))
        env = {k: v for k, v in os.environ.items() if k != pr.CROSS_WINDOWS_DIR_ENV}
        patches.append(mock.patch.dict(os.environ, env, clear=True))
        for patch in patches:
            patch.start()
        try:
            out = Output()
            with redirect_stdout(out):
                self.assertEqual(setup.main(["--check", "--dir", str(root)]), 0)
        finally:
            for patch in reversed(patches):
                patch.stop()
        text = out.getvalue()
        self.assertNotIn("finds these by itself", text)
        self.assertIn(f"export {pr.CROSS_WINDOWS_DIR_ENV}='{root.resolve()}'", text)


if __name__ == "__main__":
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    unittest.main()
