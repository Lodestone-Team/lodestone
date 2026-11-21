"""Tests for windows_vm.py: python3 launcher/scripts/test_windows_vm.py

No VM and no Fusion: the answer file, the .vmx, the staged-build checks and the
password handling run against stand-ins."""

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path
from unittest import mock

import windows_vm as vm

NS = {"u": "urn:schemas-microsoft-com:unattend"}


class AnswerFile(unittest.TestCase):
    def setUp(self):
        self.pw = "Abc123def456GHI789jkl012"
        self.xml = vm.autounattend(self.pw)
        self.root = ET.fromstring(self.xml.encode("utf-8"))

    def passes(self, name):
        return self.root.find(f"u:settings[@pass='{name}']", NS)

    def test_is_well_formed_and_for_arm64(self):
        components = self.root.findall(".//u:component", NS)
        self.assertTrue(components)
        for c in components:
            self.assertEqual(c.get("processorArchitecture"), "arm64")

    def test_accepts_the_license_and_picks_pro(self):
        self.assertEqual(self.root.find(".//u:UserData/u:AcceptEula", NS).text, "true")
        self.assertEqual(self.root.find(".//u:ProductKey/u:Key", NS).text,
                         vm.PRO_GENERIC_KEY)

    def test_skips_the_tpm_and_secure_boot_checks_in_setup(self):
        pe = self.passes("windowsPE")
        paths = [p.text for p in pe.findall(".//u:RunSynchronousCommand/u:Path", NS)]
        for check in ("BypassTPMCheck", "BypassSecureBootCheck", "BypassRAMCheck"):
            self.assertTrue(any(check in p and "LabConfig" in p for p in paths), check)

    def test_makes_the_account_signs_it_in_and_runs_the_first_logon_script(self):
        oobe = self.passes("oobeSystem")
        account = oobe.find(".//u:LocalAccount", NS)
        self.assertEqual(account.find("u:Name", NS).text, vm.USER)
        self.assertEqual(account.find("u:Group", NS).text, "Administrators")
        self.assertEqual(account.find("u:Password/u:Value", NS).text, self.pw)
        autologon = oobe.find(".//u:AutoLogon", NS)
        self.assertEqual(autologon.find("u:Enabled", NS).text, "true")
        self.assertEqual(autologon.find("u:Username", NS).text, vm.USER)
        command = oobe.find(".//u:FirstLogonCommands//u:CommandLine", NS).text
        self.assertIn(r"lodestone-vm\firstlogon.ps1", command)
        self.assertIn("-ExecutionPolicy Bypass", command)
        self.assertNotIn("Set-ExecutionPolicy", self.xml)

    def test_escapes_a_password_with_xml_characters(self):
        root = ET.fromstring(vm.autounattend("Tr&st<1>").encode("utf-8"))
        self.assertEqual(root.find(".//u:LocalAccount/u:Password/u:Value", NS).text,
                         "Tr&st<1>")

    def test_wipes_disk_0_and_installs_to_its_third_partition(self):
        disk = self.root.find(".//u:DiskConfiguration/u:Disk", NS)
        self.assertEqual(disk.find("u:DiskID", NS).text, "0")
        self.assertEqual(disk.find("u:WillWipeDisk", NS).text, "true")
        types = [p.find("u:Type", NS).text for p in disk.findall(".//u:CreatePartition", NS)]
        self.assertEqual(types, ["EFI", "MSR", "Primary"])
        target = self.root.find(".//u:InstallTo", NS)
        self.assertEqual(target.find("u:PartitionID", NS).text, "3")


class FirstLogon(unittest.TestCase):
    def test_never_changes_the_execution_policy(self):
        self.assertNotIn("Set-ExecutionPolicy", vm.FIRST_LOGON)

    def test_installs_tools_marks_ready_and_restarts(self):
        script = vm.FIRST_LOGON
        self.assertIn(r"lodestone-vm\tools\setup.exe", script)
        self.assertIn("ready.txt", script)
        self.assertLess(script.index("ready.txt"), script.index("Restart-Computer"))
        self.assertIn("AutoLogonCount", script)


class Password(unittest.TestCase):
    def test_made_up_passwords_are_long_and_mixed(self):
        for _ in range(50):
            pw = vm.make_password()
            self.assertEqual(len(pw), 24)
            self.assertTrue(pw.isalnum())  # nothing to escape in XML or a command line
            self.assertTrue(any(c.islower() for c in pw))
            self.assertTrue(any(c.isupper() for c in pw))
            self.assertTrue(any(c.isdigit() for c in pw))

    def test_environment_wins_over_the_keychain(self):
        with mock.patch.dict("os.environ", {"LODESTONE_VM_PASSWORD": "fromenv"}), \
                mock.patch.object(vm, "keychain_password", return_value="fromkeychain"):
            self.assertEqual(vm.password(), "fromenv")

    def test_no_password_anywhere_fails(self):
        with mock.patch.dict("os.environ", {}, clear=True), \
                mock.patch.object(vm, "keychain_password", return_value=None):
            with self.assertRaises(vm.Failure):
                vm.password()

    def test_guest_failures_never_show_the_password(self):
        def fake_run(cmd, stdout, stderr, **kwargs):
            stderr.write(b"Error: bad login for sekrit123")
            return subprocess.CompletedProcess(cmd, 255)

        with mock.patch.object(subprocess, "run", side_effect=fake_run):
            guest = vm.Guest(Path("/vm.vmx"), "lodestone", "sekrit123")
            with self.assertRaises(vm.Failure) as caught:
                guest.run("fileExistsInGuest", r"C:\x")
        self.assertNotIn("sekrit123", str(caught.exception))
        self.assertIn("********", str(caught.exception))


class Vmx(unittest.TestCase):
    def test_describes_an_efi_arm_windows_11_vm_with_one_disc(self):
        text = vm.vmx_text(Path("/somewhere/windows-setup.iso"), 4, 8)
        entries = dict(line.split(" = ", 1) for line in text.splitlines())
        self.assertEqual(entries["guestOS"], '"arm-windows11-64"')
        self.assertEqual(entries["firmware"], '"efi"')
        self.assertEqual(entries["memsize"], '"8192"')
        self.assertEqual(entries["numvcpus"], '"4"')
        self.assertEqual(entries["nvme0:0.fileName"], '"Windows.vmdk"')
        self.assertEqual(entries["sata0:0.fileName"], '"windows-setup.iso"')
        self.assertEqual(entries["ethernet0.virtualDev"], '"vmxnet3"')
        self.assertNotIn("sata0:1.present", entries)

    def test_set_vmx_replaces_and_removes_keys_whatever_their_case(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "x.vmx"
            path.write_text('a = "1"\nSATA0:0.present = "TRUE"\nb = "2"\n')
            vm.set_vmx(path, {"sata0:0.present": None, "b": "3", "c": "4"})
            self.assertEqual(path.read_text(), 'a = "1"\nb = "3"\nc = "4"\n')


class StagedBuild(unittest.TestCase):
    def make(self, tmp, product="launcher", files=None, corrupt=None):
        stage = Path(tmp) / "release-candidate" / "stage"
        stage.mkdir(parents=True)
        files = files or {f"lodestone-{product}-4487-windows-x86_64-standalone.exe": b"exe",
                          f"lodestone-{product}-4487-windows-x86_64-setup.exe": b"setup",
                          f"lodestone-{product}-4487-macos-universal.dmg": b"dmg"}
        record = {"product": product, "build": 4487, "commit": "a" * 40, "files": {}}
        for name, data in files.items():
            (stage / name).write_bytes(data)
            record["files"][name] = hashlib.sha256(data).hexdigest()
        if corrupt:
            (stage / corrupt).write_bytes(b"changed")
        (stage / "stage.json").write_text(json.dumps(record))
        return stage

    def test_takes_the_release_candidate_folder_and_keeps_only_windows_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            stage = self.make(tmp)
            record, folder = vm.staged_build(stage.parent, "launcher")
            self.assertEqual(folder, stage)
            self.assertEqual(sorted(record["files"]), [
                "lodestone-launcher-4487-windows-x86_64-setup.exe",
                "lodestone-launcher-4487-windows-x86_64-standalone.exe"])

    def test_refuses_a_changed_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            stage = self.make(tmp, corrupt="lodestone-launcher-4487-windows-x86_64-setup.exe")
            with self.assertRaises(vm.Failure):
                vm.staged_build(stage, "launcher")

    def test_refuses_the_other_product(self):
        with tempfile.TemporaryDirectory() as tmp:
            stage = self.make(tmp, product="host")
            with self.assertRaises(vm.Failure):
                vm.staged_build(stage, "launcher")

    def test_refuses_a_dry_run_without_windows(self):
        with tempfile.TemporaryDirectory() as tmp:
            stage = self.make(tmp, files={"lodestone-launcher-4487-macos-universal.dmg": b"d"})
            with self.assertRaises(vm.Failure):
                vm.staged_build(stage, "launcher")


class Results(unittest.TestCase):
    def test_backslash_names_from_windows_unpack_into_folders(self):
        with tempfile.TemporaryDirectory() as tmp:
            archive = Path(tmp) / "r.zip"
            with vm.zipfile.ZipFile(archive, "w") as z:
                z.writestr("results.json", "{}")
                z.writestr("logs\\host.log", "hi")
            out = Path(tmp) / "out"
            out.mkdir()
            vm.unzip_results(archive, out)
            self.assertEqual((out / "logs" / "host.log").read_text(), "hi")
            self.assertTrue((out / "results.json").is_file())

    def test_refuses_names_that_climb_out(self):
        with tempfile.TemporaryDirectory() as tmp:
            archive = Path(tmp) / "r.zip"
            with vm.zipfile.ZipFile(archive, "w") as z:
                z.writestr("..\\evil.txt", "x")
            out = Path(tmp) / "out"
            out.mkdir()
            with self.assertRaises(vm.Failure):
                vm.unzip_results(archive, out)
            self.assertFalse((Path(tmp) / "evil.txt").exists())

    def test_results_folder_only_replaces_its_own(self):
        with tempfile.TemporaryDirectory() as tmp:
            staged = Path(tmp) / "rc" / "stage"
            staged.mkdir(parents=True)
            mine = Path(tmp) / "mine"
            mine.mkdir()
            (mine / "precious.txt").write_text("keep")
            with self.assertRaises(vm.Failure):
                vm.prepare_results_folder(mine, staged)
            self.assertTrue((mine / "precious.txt").exists())
            with self.assertRaises(vm.Failure):
                vm.prepare_results_folder(staged.parent, staged)
            out = Path(tmp) / "results"
            vm.prepare_results_folder(out, staged)
            (out / "old.png").write_text("old")
            vm.prepare_results_folder(out, staged)  # its own: emptied
            self.assertFalse((out / "old.png").exists())


class Create(unittest.TestCase):
    def test_needs_the_developers_ok_for_windows_license(self):
        with mock.patch.object(vm, "need_fusion"), tempfile.TemporaryDirectory() as tmp:
            iso = Path(tmp) / "win.iso"
            iso.write_bytes(b"iso")
            args = vm.argparse.Namespace(vmx=Path(tmp) / "vm" / "x.vmx", iso=iso,
                                         accept_windows_license=False, cpus=4,
                                         memory_gb=8, disk_gb=64, timeout=120)
            with self.assertRaises(vm.Failure) as caught:
                vm.create(args)
            self.assertIn("--accept-windows-license", str(caught.exception))
            self.assertFalse((Path(tmp) / "vm").exists())

    def test_a_failed_install_deletes_the_disc_even_if_stopping_is_interrupted(self):
        with mock.patch.object(vm, "need_fusion"), tempfile.TemporaryDirectory() as tmp:
            iso = Path(tmp) / "win.iso"
            iso.write_bytes(b"iso")
            vmx = Path(tmp) / "vm" / "x.vmx"
            args = vm.argparse.Namespace(vmx=vmx, iso=iso, accept_windows_license=True,
                                         cpus=4, memory_gb=8, disk_gb=64, timeout=120)

            def half_made(args, vmx, folder, iso, pw):
                (folder / "windows-setup.iso").write_bytes(b"holds the password")
                vmx.write_text("")
                raise vm.Failure("Setup went wrong")

            with mock.patch.object(vm, "saved_password", return_value="Abc123"), \
                    mock.patch.object(vm, "store_password"), \
                    mock.patch.object(vm, "install_windows", side_effect=half_made), \
                    mock.patch.object(vm, "stop", side_effect=KeyboardInterrupt), \
                    mock.patch.object(vm, "say"):
                with self.assertRaises(KeyboardInterrupt):
                    vm.create(args)
            self.assertFalse((vmx.parent / "windows-setup.iso").exists())

    def test_refuses_a_password_the_keychain_cant_take(self):
        with mock.patch.object(vm, "need_fusion"), tempfile.TemporaryDirectory() as tmp:
            iso = Path(tmp) / "win.iso"
            iso.write_bytes(b"iso")
            args = vm.argparse.Namespace(vmx=Path(tmp) / "vm" / "x.vmx", iso=iso,
                                         accept_windows_license=True, cpus=4,
                                         memory_gb=8, disk_gb=64, timeout=120)
            with mock.patch.object(vm, "saved_password", return_value='has "quote'), \
                    mock.patch.object(vm, "store_password") as store:
                with self.assertRaises(vm.Failure):
                    vm.create(args)
                store.assert_not_called()

    def test_never_overwrites_a_vm(self):
        with mock.patch.object(vm, "need_fusion"), tempfile.TemporaryDirectory() as tmp:
            vmx = Path(tmp) / "x.vmx"
            vmx.write_text("")
            iso = Path(tmp) / "win.iso"
            iso.write_bytes(b"iso")
            args = vm.argparse.Namespace(vmx=vmx, iso=iso, accept_windows_license=True,
                                         cpus=4, memory_gb=8, disk_gb=64, timeout=120)
            with self.assertRaises(vm.Failure):
                vm.create(args)


class GuestScript(unittest.TestCase):
    def test_the_checklist_script_never_changes_the_policy_or_starts_a_server(self):
        script = (vm.SCRIPTS / "windows_vm_test.ps1").read_text(encoding="utf-8")
        self.assertNotIn("Set-ExecutionPolicy", script)
        for word in ("server-start", "'play'", '"play"', "eula"):
            self.assertNotIn(word, script.lower())


if __name__ == "__main__":
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    unittest.main()
