#!/usr/bin/env python3
"""Makes and drives a Windows 11 on Arm VM in VMware Fusion, to test Windows builds.

A Mac builds the Launcher's and the Host's Windows releases
(`publish_release.py --target x86_64-pc-windows-msvc`), but can't run them. This
script gives it a Windows to run them in (launcher/docs/cross-windows.md, "Testing
in the VM from the Mac"): a Windows 11 on Arm VM in VMware Fusion (free), whose
Direct3D 11 GPUI draws with, and whose Prism emulator runs the x64 programs.

    python3 launcher/scripts/windows_vm.py create --iso Win11_arm64.iso --accept-windows-license
    python3 launcher/scripts/windows_vm.py status
    python3 launcher/scripts/windows_vm.py start | stop | reset
    python3 launcher/scripts/windows_vm.py screenshot vm.png
    python3 launcher/scripts/windows_vm.py test launcher --staged <release-candidate dir>

`create` installs Windows with no clicks: a local account (`lodestone`) that signs in
by itself, VMware Tools (network, display driver, and the guest operations `vmrun`
drives), no sleep, no automatic updates, no firewall prompts, and a snapshot of the
signed-in desktop ("fresh") that `test` goes back to. Windows' license terms are accepted in the
unattended install only with --accept-windows-license, which is the developer's call;
it runs unactivated (with a watermark) unless they add a key.

The ISO is Microsoft's "Windows 11 (multi-edition ISO for Arm64)"
(microsoft.com/software-download/windows11arm64), downloaded by the developer or with
their OK; the script never downloads it. It installs Windows 11 Pro.

The account's password is made up here and kept in the macOS Keychain (service
"Lodestone Windows test VM"); $LODESTONE_VM_PASSWORD wins over it when set. It is never
printed, though vmrun only takes it on its command line (-gp). The unattended-install
disc that holds it is deleted once Windows is in, or when create fails.

Needs VMware Fusion in /Applications (its vmrun, vmcli, vmware-vdiskmanager and
mkisofs). Uses only the standard library. PowerShell scripts in the VM run with
`-ExecutionPolicy Bypass` for that one process; the VM's execution policy is never
changed.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import secrets
import shutil
import string
import subprocess
import sys
import tempfile
import time
import zipfile
from pathlib import Path
from xml.sax.saxutils import escape

FUSION_APP = Path("/Applications/VMware Fusion.app")
FUSION = FUSION_APP / "Contents" / "Library"
VMRUN = FUSION / "vmrun"
VMCLI = FUSION / "vmcli"
VDISKMANAGER = FUSION / "vmware-vdiskmanager"
MKISOFS = FUSION / "mkisofs"
TOOLS_ISO = FUSION / "isoimages" / "arm64" / "windows.iso"

SCRIPTS = Path(__file__).resolve().parent
VM_NAME = "Lodestone Windows Test"
DEFAULT_VMX = (Path.home() / "Virtual Machines.localized" / f"{VM_NAME}.vmwarevm"
               / f"{VM_NAME}.vmx")
USER = "lodestone"
KEYCHAIN_SERVICE = "Lodestone Windows test VM"
SNAPSHOT = "fresh"
MARKER = "lodestone.testVM"
COMPUTER_NAME = "LODESTONE-VM"

# Microsoft's generic installation key for Windows 11 Pro: it picks the edition on the
# multi-edition ISO and doesn't activate anything.
# learn.microsoft.com/windows-server/get-started/kms-client-activation-keys lists
# the KMS ones; this is the published default "generic key" for Pro.
PRO_GENERIC_KEY = "VK7JG-NPHTM-C97JM-9MPGT-3V66T"

# In the guest: where the first sign-in leaves its log and the "ready" mark.
GUEST_SETUP_DIR = r"C:\lodestone-vm"
GUEST_READY = GUEST_SETUP_DIR + r"\ready.txt"
GUEST_TEST_DIR = r"C:\lodestone-test"
POWERSHELL = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe"
CMD = r"C:\Windows\System32\cmd.exe"

# The El Torito boot image on Microsoft's ISO that boots without asking for a key.
NOPROMPT_BOOT = "efi/microsoft/boot/efisys_noprompt.bin"


class Failure(Exception):
    pass


class LoginFailure(Failure):
    """The VM refused the test account's password: waiting won't fix that."""


# What vmrun says when the guest refuses the account or its password.
LOGIN_REFUSED = ("Invalid user name or password", "VIX_E_GUEST_USER_PERMISSIONS",
                 "The user name or password is incorrect")


def say(message: str) -> None:
    print(message, flush=True)


# --- Fusion's tools ---

def need_fusion() -> None:
    missing = [p.name for p in (VMRUN, VMCLI, VDISKMANAGER, MKISOFS) if not p.exists()]
    if missing:
        raise Failure(
            f"VMware Fusion isn't installed in {FUSION_APP} (no {', '.join(missing)}). "
            "Install it from Broadcom's support portal (free, \"for Personal Use\"); "
            "launcher/docs/cross-windows.md says how.")


def run(cmd: list[str], check: bool = True, timeout: float | None = None,
        secret: str | None = None) -> subprocess.CompletedProcess:
    """Runs a command and returns its output. `secret` is masked in any error.

    Output goes through temporary files, not pipes: `vmrun start` leaves the VM's
    process running with whatever it was given as output, and a pipe would then only
    close when the VM stops."""
    with tempfile.TemporaryFile() as out, tempfile.TemporaryFile() as err:
        try:
            process = subprocess.run(cmd, stdin=subprocess.DEVNULL, stdout=out,
                                     stderr=err, timeout=timeout)
        except subprocess.TimeoutExpired:
            raise Failure(f"{shown(cmd, secret)} took longer than {timeout:.0f}s")
        out.seek(0)
        err.seek(0)
        result = subprocess.CompletedProcess(
            cmd, process.returncode, out.read().decode("utf-8", "replace"),
            err.read().decode("utf-8", "replace"))
    if check and result.returncode != 0:
        out = (result.stdout + result.stderr).strip()
        if secret:
            out = out.replace(secret, "********")
        raise Failure(f"{shown(cmd, secret)} failed ({result.returncode}): {out}")
    return result


def shown(cmd: list[str], secret: str | None) -> str:
    parts = [Path(cmd[0]).name] + cmd[1:]
    return " ".join("********" if secret and p == secret else p for p in parts)


def vmrun(*args: str, check: bool = True, timeout: float | None = None,
          ) -> subprocess.CompletedProcess:
    return run([str(VMRUN), "-T", "fusion", *args], check=check, timeout=timeout)


class Guest:
    """vmrun's guest operations, signed in as the test account."""

    def __init__(self, vmx: Path, user: str, password: str):
        self.vmx, self.user, self.password = vmx, user, password

    def run(self, command: str, *args: str, check: bool = True,
            timeout: float | None = None) -> subprocess.CompletedProcess:
        # vmrun takes the guest password only on its command line (-gp), where other
        # local processes can see it while it runs; it's a throwaway test account's.
        cmd = [str(VMRUN), "-T", "fusion", "-gu", self.user, "-gp", self.password,
               command, str(self.vmx), *args]
        result = run(cmd, check=False, timeout=timeout, secret=self.password)
        said = self.masked(result)
        if any(text in said for text in LOGIN_REFUSED):
            raise LoginFailure(
                f"The VM refused the {self.user} account's password. It comes from "
                f"$LODESTONE_VM_PASSWORD if that's set, else the Keychain item "
                f"\"{KEYCHAIN_SERVICE}\"; they must match what `create` set.")
        if check and result.returncode != 0:
            raise Failure(f"vmrun {command} failed ({result.returncode}): {said}")
        return result

    def masked(self, result: subprocess.CompletedProcess) -> str:
        return (result.stdout + result.stderr).strip().replace(self.password, "********")

    def exists(self, path: str) -> bool:
        result = self.run("fileExistsInGuest", path, check=False, timeout=60)
        return result.returncode == 0 and "does not exist" not in result.stdout

    def dir_exists(self, path: str) -> bool:
        # fileExistsInGuest says "does not exist" for a folder.
        result = self.run("directoryExistsInGuest", path, check=False, timeout=60)
        return result.returncode == 0 and "does not exist" not in result.stdout

    def processes(self) -> str:
        result = self.run("listProcessesInGuest", check=False, timeout=60)
        return result.stdout if result.returncode == 0 else ""

    def mkdir(self, path: str) -> None:
        if not self.dir_exists(path):
            self.run("createDirectoryInGuest", path, timeout=60)

    def copy_in(self, host: Path, guest: str) -> None:
        self.run("copyFileFromHostToGuest", str(host), guest, timeout=600)

    def copy_out(self, guest: str, host: Path) -> None:
        self.run("copyFileFromGuestToHost", guest, str(host), timeout=600)

    def program(self, command_line: str, interactive: bool = True,
                timeout: float = 300) -> subprocess.CompletedProcess:
        """Runs a command line in the guest through cmd.exe. vmrun hands a program its
        arguments as one string, which cmd parses again; PowerShell started directly
        gets it as a single argument and exits. With `interactive`, it runs in the
        signed-in desktop session (as the account, not elevated: UAC applies)."""
        flags = ["-interactive"] if interactive else []
        return self.run("runProgramInGuest", *flags, CMD, f"/c {command_line}",
                        check=False, timeout=timeout)

    def powershell_file(self, script: str, args: str = "", interactive: bool = True,
                        timeout: float = 300) -> subprocess.CompletedProcess:
        """Runs a .ps1 in the guest, without changing the VM's execution policy (only
        this one process bypasses it)."""
        return self.program(f"{POWERSHELL} -NoProfile -NonInteractive -ExecutionPolicy "
                            f"Bypass -File {script} {args}".rstrip(),
                            interactive=interactive, timeout=timeout)


# --- The password ---

def make_password() -> str:
    alphabet = string.ascii_letters + string.digits
    while True:
        pw = "".join(secrets.choice(alphabet) for _ in range(24))
        if (any(c.islower() for c in pw) and any(c.isupper() for c in pw)
                and any(c.isdigit() for c in pw)):
            return pw


def keychain_password() -> str | None:
    result = run(["security", "find-generic-password", "-s", KEYCHAIN_SERVICE,
                  "-a", USER, "-w"], check=False)
    return result.stdout.strip() or None if result.returncode == 0 else None


def store_password(password: str) -> None:
    """Into the login Keychain, through `security -i`'s standard input rather than
    its command line, where other processes could see it."""
    command = (f'add-generic-password -U -s "{KEYCHAIN_SERVICE}" -a {USER} '
               f'-l "{KEYCHAIN_SERVICE}" -w {password}\n')
    result = subprocess.run(["security", "-i"], input=command, capture_output=True,
                            text=True, timeout=60)
    if result.returncode != 0 or result.stderr.strip():
        said = (result.stdout + result.stderr).strip().replace(password, "********")
        raise Failure(f"Couldn't save the VM's password in the Keychain: {said}")


def saved_password() -> str | None:
    """$LODESTONE_VM_PASSWORD wins over the Keychain, for `create` and the rest alike."""
    return os.environ.get("LODESTONE_VM_PASSWORD") or keychain_password()


def password() -> str:
    pw = saved_password()
    if not pw:
        raise Failure(f"No password for the VM's account: neither $LODESTONE_VM_PASSWORD "
                      f"nor the Keychain item \"{KEYCHAIN_SERVICE}\" has one.")
    return pw


# --- The unattended install ---

def autounattend(pw: str) -> str:
    """Windows Setup's answer file: wipe the disk, install Windows 11 Pro, make the
    local account, sign it in, and run firstlogon.ps1 from the same disc."""
    arch = ('processorArchitecture="arm64" publicKeyToken="31bf3856ad364e35" '
            'language="neutral" versionScope="nonSxS" '
            'xmlns:wcm="http://schemas.microsoft.com/WMIConfig/2002/State"')
    intl = """<InputLocale>en-US</InputLocale>
      <SystemLocale>en-US</SystemLocale>
      <UILanguage>en-US</UILanguage>
      <UserLocale>en-US</UserLocale>"""
    secret = f"<Value>{escape(pw)}</Value><PlainText>true</PlainText>"
    # Windows 11's TPM, Secure Boot and memory checks: the VM has no virtual TPM
    # (Fusion only adds one with an encrypted VM), so Setup is told to skip them.
    labconfig = [f'reg.exe add "HKLM\\SYSTEM\\Setup\\LabConfig" /v {name} '
                 '/t REG_DWORD /d 1 /f'
                 for name in ("BypassTPMCheck", "BypassSecureBootCheck",
                              "BypassRAMCheck")]
    pe_commands = "\n".join(
        f"""        <RunSynchronousCommand wcm:action="add">
          <Order>{i}</Order>
          <Path>{cmd}</Path>
        </RunSynchronousCommand>""" for i, cmd in enumerate(labconfig, 1))
    # The disc's drive letter isn't known in advance: look for the script on each.
    first_logon = (r"cmd.exe /c for %d in (D E F G H I J K) do @if exist "
                   r"%d:\lodestone-vm\firstlogon.ps1 "
                   f"{POWERSHELL} -NoProfile -ExecutionPolicy Bypass "
                   r"-File %d:\lodestone-vm\firstlogon.ps1 -Media %d:")
    return f"""<?xml version="1.0" encoding="utf-8"?>
<unattend xmlns="urn:schemas-microsoft-com:unattend">
  <settings pass="windowsPE">
    <component name="Microsoft-Windows-International-Core-WinPE" {arch}>
      <SetupUILanguage><UILanguage>en-US</UILanguage></SetupUILanguage>
      {intl}
    </component>
    <component name="Microsoft-Windows-Setup" {arch}>
      <RunSynchronous>
{pe_commands}
      </RunSynchronous>
      <DiskConfiguration>
        <Disk wcm:action="add">
          <DiskID>0</DiskID>
          <WillWipeDisk>true</WillWipeDisk>
          <CreatePartitions>
            <CreatePartition wcm:action="add"><Order>1</Order><Type>EFI</Type><Size>300</Size></CreatePartition>
            <CreatePartition wcm:action="add"><Order>2</Order><Type>MSR</Type><Size>16</Size></CreatePartition>
            <CreatePartition wcm:action="add"><Order>3</Order><Type>Primary</Type><Extend>true</Extend></CreatePartition>
          </CreatePartitions>
          <ModifyPartitions>
            <ModifyPartition wcm:action="add"><Order>1</Order><PartitionID>1</PartitionID><Format>FAT32</Format><Label>System</Label></ModifyPartition>
            <ModifyPartition wcm:action="add"><Order>2</Order><PartitionID>3</PartitionID><Format>NTFS</Format><Label>Windows</Label><Letter>C</Letter></ModifyPartition>
          </ModifyPartitions>
        </Disk>
      </DiskConfiguration>
      <ImageInstall>
        <OSImage>
          <InstallTo><DiskID>0</DiskID><PartitionID>3</PartitionID></InstallTo>
        </OSImage>
      </ImageInstall>
      <UserData>
        <AcceptEula>true</AcceptEula>
        <FullName>{USER}</FullName>
        <Organization>Lodestone</Organization>
        <ProductKey><Key>{PRO_GENERIC_KEY}</Key><WillShowUI>OnError</WillShowUI></ProductKey>
      </UserData>
    </component>
  </settings>
  <settings pass="specialize">
    <component name="Microsoft-Windows-Shell-Setup" {arch}>
      <ComputerName>{COMPUTER_NAME}</ComputerName>
      <TimeZone>UTC</TimeZone>
    </component>
    <component name="Microsoft-Windows-Deployment" {arch}>
      <RunSynchronous>
        <RunSynchronousCommand wcm:action="add">
          <Order>1</Order>
          <Path>reg.exe add "HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\OOBE" /v BypassNRO /t REG_DWORD /d 1 /f</Path>
        </RunSynchronousCommand>
      </RunSynchronous>
    </component>
  </settings>
  <settings pass="oobeSystem">
    <component name="Microsoft-Windows-International-Core" {arch}>
      {intl}
    </component>
    <component name="Microsoft-Windows-Shell-Setup" {arch}>
      <OOBE>
        <HideEULAPage>true</HideEULAPage>
        <HideOEMRegistrationScreen>true</HideOEMRegistrationScreen>
        <HideOnlineAccountScreens>true</HideOnlineAccountScreens>
        <HideWirelessSetupInOOBE>true</HideWirelessSetupInOOBE>
        <ProtectYourPC>3</ProtectYourPC>
      </OOBE>
      <UserAccounts>
        <LocalAccounts>
          <LocalAccount wcm:action="add">
            <Name>{USER}</Name>
            <DisplayName>{USER}</DisplayName>
            <Group>Administrators</Group>
            <Password>{secret}</Password>
          </LocalAccount>
        </LocalAccounts>
      </UserAccounts>
      <AutoLogon>
        <Enabled>true</Enabled>
        <Username>{USER}</Username>
        <Password>{secret}</Password>
        <LogonCount>9999999</LogonCount>
      </AutoLogon>
      <FirstLogonCommands>
        <SynchronousCommand wcm:action="add">
          <Order>1</Order>
          <CommandLine>{first_logon}</CommandLine>
          <Description>Lodestone test VM setup</Description>
        </SynchronousCommand>
      </FirstLogonCommands>
    </component>
  </settings>
</unattend>
"""


FIRST_LOGON = r"""# Runs once, at the test account's first sign-in (autounattend.xml's
# FirstLogonCommands), from the unattended-install disc. Made by windows_vm.py.
param([string]$Media)
New-Item -ItemType Directory -Force 'C:\lodestone-vm', 'C:\lodestone-test' | Out-Null
Start-Transcript 'C:\lodestone-vm\firstlogon.log' -Append

# Stay signed in: Setup's auto sign-in counts down, so take the count away; tests
# need the desktop session (UI Automation sees no windows without one).
$winlogon = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon'
Set-ItemProperty $winlogon -Name AutoAdminLogon -Value '1'
Remove-ItemProperty $winlogon -Name AutoLogonCount -ErrorAction SilentlyContinue

# Never sleep or turn the screen off, no screen saver, and no sign-in on wake.
powercfg /change standby-timeout-ac 0
powercfg /change monitor-timeout-ac 0
powercfg /change hibernate-timeout-ac 0
powercfg /hibernate off
powercfg /SETACVALUEINDEX SCHEME_CURRENT SUB_NONE CONSOLELOCK 0
powercfg /SETACTIVE SCHEME_CURRENT
reg.exe add 'HKCU\Control Panel\Desktop' /v ScreenSaveActive /t REG_SZ /d 0 /f
reg.exe add 'HKCU\Control Panel\Desktop' /v ScreenSaverIsSecure /t REG_SZ /d 0 /f

# Windows Firewall: don't ask when a program starts listening (the core listens on
# every interface); nobody is there to answer, and the dialog covers the screenshots.
Set-NetFirewallProfile -All -NotifyOnListen False

# Windows Update: no automatic updates, so the snapshot stays as it was made and
# nothing restarts it in the middle of a test.
reg.exe add 'HKLM\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU' /v NoAutoUpdate /t REG_DWORD /d 1 /f
reg.exe add 'HKLM\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU' /v NoAutoRebootWithLoggedOnUsers /t REG_DWORD /d 1 /f
# No "Let's finish setting up your device" screen.
reg.exe add 'HKCU\Software\Microsoft\Windows\CurrentVersion\UserProfileEngagement' /v ScoobeSystemSettingEnabled /t REG_DWORD /d 0 /f

# VMware Tools: the network and display drivers, and the guest operations vmrun uses.
$tools = Join-Path $Media 'lodestone-vm\tools\setup.exe'
$p = Start-Process $tools -ArgumentList '/S /v"/qn REBOOT=R"' -Wait -PassThru
"VMware Tools setup exited with $($p.ExitCode)"

Set-Content 'C:\lodestone-vm\ready.txt' (Get-Date -Format o)
Stop-Transcript
Restart-Computer -Force
"""


def make_setup_iso(folder: Path, windows_iso: Path, pw: str) -> Path:
    """Microsoft's ISO again, with three changes: it boots with
    efisys_noprompt.bin (no "Press any key to boot from CD or DVD", which nothing can
    answer: Fusion's vmcli has no working key presses), autounattend.xml at its root
    (Setup reads it from there), and lodestone-vm/ (firstlogon.ps1 and VMware Tools'
    installer). UDF, so install.wim (over 4 GB) fits."""
    iso = folder / "windows-setup.iso"
    with tempfile.TemporaryDirectory() as staging, tempfile.TemporaryDirectory() as tools, \
            tempfile.TemporaryDirectory() as win:
        stage = Path(staging)  # gone afterwards, whatever happens: it holds the password
        (stage / "lodestone-vm" / "tools").mkdir(parents=True)
        (stage / "autounattend.xml").write_text(autounattend(pw), encoding="utf-8")
        (stage / "lodestone-vm" / "firstlogon.ps1").write_text(
            FIRST_LOGON.replace("\n", "\r\n"), encoding="utf-8")
        mount(TOOLS_ISO, Path(tools))
        try:
            shutil.copyfile(Path(tools) / "setup.exe",
                            stage / "lodestone-vm" / "tools" / "setup.exe")
        finally:
            unmount(Path(tools))
        mount(windows_iso, Path(win))
        try:
            if not (Path(win) / NOPROMPT_BOOT).is_file():
                raise Failure(f"{windows_iso} has no {NOPROMPT_BOOT}: is it Microsoft's "
                              "Windows 11 Arm64 ISO?")
            if not (Path(win) / "sources" / "install.wim").is_file():
                raise Failure(f"{windows_iso} has no sources/install.wim.")
            run([str(MKISOFS), "-quiet", "-iso-level", "3", "-udf", "-J", "-joliet-long",
                 "-V", "WIN11_LODESTONE", "-eltorito-platform", "efi",
                 "-b", NOPROMPT_BOOT, "-no-emul-boot", "-o", str(iso),
                 "-graft-points", f"/={win}",
                 f"autounattend.xml={stage / 'autounattend.xml'}",
                 f"lodestone-vm/={stage / 'lodestone-vm'}/"], timeout=1800)
        finally:
            unmount(Path(win))
    iso.chmod(0o600)
    return iso


def mount(image: Path, at: Path) -> None:
    run(["hdiutil", "attach", "-readonly", "-nobrowse", "-noverify",
         "-mountpoint", str(at), str(image)], timeout=300)


def unmount(at: Path) -> None:
    run(["hdiutil", "detach", str(at)], check=False, timeout=300)


# --- The VM ---

def vmx_text(setup_iso: Path, cpus: int, memory_gb: int) -> str:
    entries = {
        ".encoding": "UTF-8",
        "config.version": "8",
        "virtualHW.version": "22",
        "displayName": VM_NAME,
        MARKER: "TRUE",  # made by windows_vm.py: `reset` and `test` only touch such VMs
        "guestOS": "arm-windows11-64",
        "firmware": "efi",
        "numvcpus": str(cpus),
        "cpuid.coresPerSocket": str(cpus),
        "memsize": str(memory_gb * 1024),
        "nvram": f"{VM_NAME}.nvram",
        "pciBridge0.present": "TRUE",
        **{f"pciBridge{n}.{k}": v for n in (4, 5, 6, 7) for k, v in
           (("present", "TRUE"), ("virtualDev", "pcieRootPort"), ("functions", "8"))},
        "vmci0.present": "TRUE",
        "hpet0.present": "TRUE",
        "svga.present": "TRUE",
        "mks.enable3d": "TRUE",
        "svga.graphicsMemoryKB": "8388608",
        "nvme0.present": "TRUE",
        "nvme0:0.present": "TRUE",
        "nvme0:0.fileName": "Windows.vmdk",
        "sata0.present": "TRUE",
        "sata0:0.present": "TRUE",
        "sata0:0.deviceType": "cdrom-image",
        "sata0:0.fileName": setup_iso.name,
        "sata0:0.startConnected": "TRUE",
        "usb.present": "TRUE",
        "ehci.present": "TRUE",
        "usb_xhci.present": "TRUE",
        "ethernet0.present": "TRUE",
        "ethernet0.connectionType": "nat",
        "ethernet0.addressType": "generated",
        "ethernet0.virtualDev": "vmxnet3",
        "ethernet0.linkStatePropagation.enable": "TRUE",
        "tools.syncTime": "TRUE",
        "tools.upgrade.policy": "manual",
    }
    return "".join(f'{k} = "{v}"\n' for k, v in entries.items())


def set_vmx(vmx: Path, changes: dict[str, str | None]) -> None:
    """Edits the .vmx (the VM must be off): sets keys, or removes those set to None."""
    lines = vmx.read_text(encoding="utf-8").splitlines()
    keys = {k.lower() for k in changes}
    kept = [line for line in lines
            if line.split("=", 1)[0].strip().lower() not in keys]
    kept += [f'{k} = "{v}"' for k, v in changes.items() if v is not None]
    vmx.write_text("\n".join(kept) + "\n", encoding="utf-8")


def running(vmx: Path) -> bool:
    out = vmrun("list", check=False).stdout
    return any(Path(line.strip()).resolve() == vmx.resolve()
               for line in out.splitlines()[1:] if line.strip())


def start(vmx: Path) -> None:
    if not running(vmx):
        # nogui: no Fusion window; the VM runs in the background.
        vmrun("start", str(vmx), "nogui", timeout=300)


def stop(vmx: Path, hard: bool = False) -> None:
    if running(vmx):
        vmrun("stop", str(vmx), "hard" if hard else "soft", timeout=600)


def screenshot(vmx: Path, out: Path) -> None:
    """A picture of the VM's screen, without signing in (vmcli, through the MKS)."""
    run([str(VMCLI), str(vmx), "MKS", "captureScreenshot", str(out)], timeout=60)


def wait_for(what: str, ready, timeout: float, every: float = 15) -> None:
    end = time.monotonic() + timeout
    while True:
        try:
            if ready():
                return
        except LoginFailure:
            raise
        except Failure:
            pass  # a vmrun call that hung or failed while Windows starts or stops
        if time.monotonic() > end:
            raise Failure(f"Gave up waiting for {what} after {timeout / 60:.0f} minutes.")
        time.sleep(every)


def wait_for_desktop(vmx: Path, guest: Guest, timeout: float = 900) -> None:
    """The account's desktop (explorer.exe) is up, seen through a guest operation,
    which also means VMware Tools answers. (`vmrun checkToolsState` isn't a reliable
    sign: Fusion 26 says "installed" while Tools runs.)"""
    wait_for("the test account's desktop in the VM",
             lambda: "explorer.exe" in guest.processes().lower(), timeout)


def ensure_ours(vmx: Path) -> None:
    """`reset` and `test` power the VM off and roll it back: only do that to the VM
    `create` made, with its snapshot."""
    if not vmx.exists():
        raise Failure(f"No VM at {vmx}; `create` makes one.")
    text = vmx.read_text(encoding="utf-8", errors="replace")
    if f'{MARKER} = "TRUE"' not in text:
        raise Failure(f"{vmx} wasn't made by windows_vm.py create ({MARKER} isn't set), "
                      "so it won't power it off or roll it back.")
    snaps = vmrun("listSnapshots", str(vmx), check=False).stdout.splitlines()[1:]
    if SNAPSHOT not in [line.strip() for line in snaps]:
        raise Failure(f"{vmx} has no \"{SNAPSHOT}\" snapshot to go back to.")


def create(args) -> int:
    need_fusion()
    vmx: Path = args.vmx
    if not args.accept_windows_license:
        raise Failure(
            "The unattended install accepts Windows' license terms (AcceptEula) for "
            "you; that's your call. Pass --accept-windows-license to go ahead.")
    iso = args.iso.resolve()
    if not iso.is_file():
        raise Failure(f"No ISO at {iso}.")
    folder = vmx.parent
    if vmx.exists() or folder.exists():
        raise Failure(f"{folder} already exists (a VM, or what an unfinished create left). "
                      "Stop it if it runs (`stop`), delete the folder (Fusion's Virtual "
                      "Machine Library, or Finder), then run create again.")
    pw = saved_password() or make_password()
    if any(c.isspace() or c in "\"'\\" for c in pw):
        raise Failure("$LODESTONE_VM_PASSWORD can't have spaces, quotes or backslashes "
                      "(the Keychain's `security -i` can't take them).")
    store_password(pw)
    folder.mkdir(parents=True)
    try:
        install_windows(args, vmx, folder, iso, pw)
    except BaseException:
        # First, so nothing can skip it: the install disc holds the password. (macOS
        # lets a file the VM still has open be deleted.)
        try:
            (folder / "windows-setup.iso").unlink(missing_ok=True)
        except OSError:
            pass
        try:
            if vmx.exists():
                stop(vmx, hard=True)
        except Failure:
            pass
        finally:
            say(f"create stopped; delete {folder} before running it again.")
        raise
    try:
        screenshot(vmx, folder / "ready.png")
    except Failure:
        pass
    say(f"Ready: {vmx}\nSigned in as {USER}; snapshot \"{SNAPSHOT}\" is this desktop. "
        f"The password is in the Keychain (\"{KEYCHAIN_SERVICE}\").")
    return 0


def install_windows(args, vmx: Path, folder: Path, iso: Path, pw: str) -> None:
    say(f"Making the VM in {folder}")
    run([str(VDISKMANAGER), "-c", "-s", f"{args.disk_gb}GB", "-a", "lsilogic",
         "-t", "0", str(folder / "Windows.vmdk")])
    say("Making its install disc (Windows' ISO with the answer file)")
    setup_iso = make_setup_iso(folder, iso, pw)
    vmx.write_text(vmx_text(setup_iso, args.cpus, args.memory_gb), encoding="utf-8")

    say("Starting it; Windows Setup runs by itself (no window; `screenshot` "
        "to look)")
    start(vmx)

    guest = Guest(vmx, USER, pw)
    say("Installing Windows, then VMware Tools (about 20 to 40 minutes)")
    wait_for("Windows Setup and the first sign-in",
             lambda: guest.exists(GUEST_READY),
             timeout=args.timeout * 60, every=30)
    say("First sign-in done; it restarts once for VMware Tools")
    time.sleep(60)
    wait_for_desktop(vmx, guest)

    say("Taking the install disc out")
    stop(vmx)
    set_vmx(vmx, {"sata0:0.present": None, "sata0:0.deviceType": None,
                  "sata0:0.fileName": None, "sata0:0.startConnected": None})
    setup_iso.unlink()  # 8.5 GB, and its answer file holds the password
    start(vmx)
    wait_for_desktop(vmx, guest)
    vmrun("snapshot", str(vmx), SNAPSHOT, timeout=900)


def status(args) -> int:
    need_fusion()
    vmx: Path = args.vmx
    if not vmx.exists():
        say(f"No VM at {vmx} (`create` makes one).")
        return 1
    on = running(vmx)
    say(f"VM: {vmx}\nRunning: {'yes' if on else 'no'}")
    if on:
        say(f"VMware Tools: {vmrun('checkToolsState', str(vmx), check=False).stdout.strip()}")
    snaps = vmrun("listSnapshots", str(vmx), check=False).stdout.strip().splitlines()[1:]
    say(f"Snapshots: {', '.join(snaps) or 'none'}")
    return 0


# --- Testing a build ---

def staged_build(path: Path, product: str) -> tuple[dict, Path]:
    """A dry run's stage.json and its folder, from the release-candidate folder or its
    stage/ folder (publish_release.py --dry-run --target x86_64-pc-windows-msvc)."""
    for folder in (path, path / "stage"):
        if (folder / "stage.json").is_file():
            break
    else:
        raise Failure(f"No stage.json in {path} or {path / 'stage'}: give the dry run's "
                      "release-candidate folder.")
    record = json.loads((folder / "stage.json").read_text(encoding="utf-8"))
    if record.get("product", "launcher") != product:
        raise Failure(f"{folder} holds a {record.get('product')} build, not the {product}.")
    windows = {name: digest for name, digest in record["files"].items()
               if "-windows-" in name and name.endswith(".exe")}
    if not windows:
        raise Failure(f"The dry run in {folder} has no Windows build "
                      "(--target x86_64-pc-windows-msvc).")
    for name, digest in windows.items():
        if sha256(folder / name) != digest:
            raise Failure(f"{folder / name} isn't the file the dry run staged; dry-run again.")
    return {**record, "files": windows}, folder


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


RESULTS_MARK = ".windows-vm-test"


def prepare_results_folder(out: Path, staged: Path) -> None:
    """Empties `out` for this run, but only a folder this command made before (it
    has the mark) or an empty one: never a folder someone pointed --out at by mistake."""
    if out == staged or out in staged.parents:
        raise Failure(f"--out {out} holds the staged build; give a folder of its own.")
    if out.exists():
        if not out.is_dir():
            raise Failure(f"--out {out} is a file.")
        if any(out.iterdir()) and not (out / RESULTS_MARK).exists():
            raise Failure(f"{out} isn't empty and isn't an earlier results folder; "
                          "give a new or empty folder.")
        shutil.rmtree(out)
    out.mkdir(parents=True)
    (out / RESULTS_MARK).touch()


def unzip_results(archive: Path, out: Path) -> None:
    """Windows PowerShell 5.1's Compress-Archive names entries with backslashes
    (logs\\host.log); unpack them as folders, and nothing outside `out`."""
    root = out.resolve()
    with zipfile.ZipFile(archive) as z:
        for info in z.infolist():
            name = info.filename.replace("\\", "/")
            target = (root / name).resolve()
            if root not in target.parents:
                raise Failure(f"The results hold an odd file name: {info.filename!r}")
            if name.endswith("/"):
                target.mkdir(parents=True, exist_ok=True)
                continue
            target.parent.mkdir(parents=True, exist_ok=True)
            with z.open(info) as src, target.open("wb") as dst:
                shutil.copyfileobj(src, dst)


def test(args) -> int:
    need_fusion()
    vmx: Path = args.vmx
    if not args.keep:
        ensure_ours(vmx)
    elif not vmx.exists():
        raise Failure(f"No VM at {vmx}; `create` makes one.")
    record, folder = staged_build(args.staged.expanduser().resolve(), args.product)
    build, commit = str(record["build"]), record["commit"][:7]
    out = (args.out.expanduser() if args.out
           else folder.parent / f"windows-vm-test-{args.product}-{build}").resolve()
    prepare_results_folder(out, folder)
    guest = Guest(vmx, USER, password())

    if args.keep:
        say("Using the VM as it is (--keep)")
        start(vmx)
    else:
        say(f"Going back to the \"{SNAPSHOT}\" snapshot")
        reset(vmx)
    wait_for_desktop(vmx, guest, timeout=600)

    say(f"Copying build {build} ({commit}) in")
    guest.program(f"rmdir /s /q {GUEST_TEST_DIR}", interactive=False, timeout=120)
    guest.mkdir(GUEST_TEST_DIR)
    for name in record["files"]:
        guest.copy_in(folder / name, f"{GUEST_TEST_DIR}\\{name}")
    for script in ("ui_drive.ps1", "windows_vm_test.ps1"):
        guest.copy_in(SCRIPTS / script, f"{GUEST_TEST_DIR}\\{script}")

    say("Running the checklist in the VM (a few minutes)")
    flags = " -SkipInstaller" if args.skip_installer else ""
    stopped = None
    try:
        result = guest.powershell_file(
            f"{GUEST_TEST_DIR}\\windows_vm_test.ps1",
            f"-Product {args.product} -Dir {GUEST_TEST_DIR} -Build {build} -Commit {commit}"
            f"{flags}", timeout=1800)
        said = guest.masked(result)
    except LoginFailure:
        raise
    except Failure as error:  # it ran too long: collect what's there anyway
        stopped, said = error, str(error)
    try:
        screenshot(vmx, out / "vm-screen-after.png")
    except Failure:
        pass
    if stopped:
        try:
            guest.copy_out(GUEST_TEST_DIR + "\\results\\transcript.txt",
                           out / "transcript.txt")
        except Failure:
            pass
        raise Failure(f"The checklist didn't finish: {stopped}. What it left is in {out}.")

    zipped = GUEST_TEST_DIR + "\\results.zip"
    guest.program(f"{POWERSHELL} -NoProfile -NonInteractive -Command Compress-Archive "
                  f"-Force -Path {GUEST_TEST_DIR}\\results\\* -DestinationPath {zipped}",
                  interactive=False, timeout=300)
    local_zip = out / "results.zip"
    try:
        guest.copy_out(zipped, local_zip)
    except Failure as error:
        raise Failure(f"The checklist left no results ({error}). vmrun said: {said}")
    unzip_results(local_zip, out)
    local_zip.unlink()

    results_file = out / "results.json"
    if not results_file.is_file():
        raise Failure(f"The checklist stopped before writing results.json; see "
                      f"{out / 'transcript.txt'}.")
    summary = json.loads(results_file.read_text(encoding="utf-8-sig"))
    say(f"\n{summary['windows']} ({summary['arch']}), {args.product} build {build} ({commit}):")
    marks = {"pass": "ok   ", "fail": "FAIL ", "check": "check", "skip": "skip "}
    for r in summary["results"]:
        say(f"  {marks.get(r['status'], r['status'])}  {r['step']}: {r['detail']}")
    failed = [r for r in summary["results"] if r["status"] == "fail"]
    say(f"\nScreenshots and logs: {out}")
    say("Not checked here: launching a game, and anything that needs signing in; "
        "a release's final check is on a real x64 PC.")
    return 1 if failed else 0


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description="Makes and drives the Windows 11 on Arm test VM in VMware Fusion.")
    parser.add_argument("--vmx", type=Path,
                        default=Path(os.environ.get("LODESTONE_VM") or DEFAULT_VMX),
                        help=f"the VM (default $LODESTONE_VM, else {DEFAULT_VMX})")
    sub = parser.add_subparsers(dest="command", required=True)

    p = sub.add_parser("create", help="install Windows in a new VM, unattended")
    p.add_argument("--iso", type=Path, required=True,
                   help="Microsoft's Windows 11 Arm64 multi-edition ISO")
    p.add_argument("--accept-windows-license", action="store_true",
                   help="accept Windows' license terms in the unattended install")
    p.add_argument("--cpus", type=int, default=4)
    p.add_argument("--memory-gb", type=int, default=8)
    p.add_argument("--disk-gb", type=int, default=64)
    p.add_argument("--timeout", type=int, default=120,
                   help="minutes to wait for Windows Setup (default 120)")
    p.set_defaults(func=create)

    sub.add_parser("status", help="whether it runs, VMware Tools, snapshots"
                   ).set_defaults(func=status)
    sub.add_parser("start", help="start it in the background").set_defaults(
        func=lambda a: (need_fusion(), start(a.vmx), 0)[-1])
    sub.add_parser("stop", help="shut Windows down").set_defaults(
        func=lambda a: (need_fusion(), stop(a.vmx), 0)[-1])
    sub.add_parser("reset", help=f"go back to the \"{SNAPSHOT}\" snapshot and start"
                   ).set_defaults(func=lambda a: reset(a.vmx))
    p = sub.add_parser("test", help="run the release checklist on a dry run's Windows build")
    p.add_argument("product", choices=("launcher", "host"))
    p.add_argument("--staged", type=Path, required=True,
                   help="the dry run's release-candidate folder (or its stage/ folder)")
    p.add_argument("--out", type=Path,
                   help="where results go (default: in the release-candidate folder)")
    p.add_argument("--keep", action="store_true",
                   help=f"test in the VM as it is instead of the \"{SNAPSHOT}\" snapshot")
    p.add_argument("--skip-installer", action="store_true",
                   help="only the -standalone exe, not Setup")
    p.set_defaults(func=test)

    p = sub.add_parser("screenshot", help="save a picture of its screen")
    p.add_argument("out", type=Path)
    p.set_defaults(func=lambda a: (need_fusion(), screenshot(a.vmx, a.out), 0)[-1])

    args = parser.parse_args(argv)
    # vmrun lists running VMs by absolute path, and vmcli resolves relative paths
    # against its own folder: make every path absolute.
    args.vmx = args.vmx.expanduser().resolve()
    for name in ("iso", "out"):
        if isinstance(getattr(args, name, None), Path):
            setattr(args, name, getattr(args, name).expanduser().resolve())
    try:
        return args.func(args)
    except Failure as error:
        print(f"windows_vm: {error}", file=sys.stderr)
        return 1


def reset(vmx: Path) -> int:
    need_fusion()
    ensure_ours(vmx)
    stop(vmx, hard=True)
    vmrun("revertToSnapshot", str(vmx), SNAPSHOT, timeout=900)
    start(vmx)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
