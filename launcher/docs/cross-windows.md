# Windows builds from a Mac

A Mac builds the Windows releases of Lodestone Launcher and Lodestone Host, versions and
developer builds alike, with the installer, using the same script as every other build:

```bash
python3 launcher/scripts/publish_release.py --dev --dry-run --target x86_64-pc-windows-msvc
python3 launcher/scripts/publish_release.py --product host --dry-run --tag draft --target x86_64-pc-windows-msvc
```

It stages the same files a Windows machine would: the standalone
`lodestone-launcher-<build>-windows-x86_64-standalone.exe` (the Host's, and its
`-core.exe`), and the installer `-setup.exe`. A Mac can't really run them, so they are
tested on Windows before they're uploaded ("Testing" below). Uploading works as usual:
`--dev` or `--tag ...` with the same `--target` options as the dry run, from the same
checkout without changes.

## How it works

- **Compiling and linking.** `cargo-xwin` gives cargo what it needs to build for
  `x86_64-pc-windows-msvc` on a Mac: clang-cl as the C compiler, lld-link as the linker,
  and Microsoft's C runtime and Windows SDK libraries and headers, which it downloads
  into its cache. `publish_release.py` runs `cargo xwin env --target ...` and builds with
  the same `cargo build` command as on Windows, with that environment added to the
  Windows builds only. Each Mac build in the same run (`--target aarch64-apple-darwin
  --target x86_64-pc-windows-msvc` works) gets none of it.
- **One environment per workspace.** `cargo xwin env` bakes the rustflags of the
  workspace it runs in into `CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS`. So it runs
  twice for the Host: in `launcher/` for the Host (with the launcher's `+crt-static` and
  `windows_slim_errors`), and in the root workspace for the core, with
  `RUSTFLAGS=-C target-feature=+crt-static` in that call only. The core then gets the
  static C runtime (and cargo-xwin's `-defaultlib:libucrt`) and none of the launcher's
  flags, as when Host - Release builds it on Windows.
- **The static C runtime.** `cargo xwin env` sets `RUSTFLAGS=""`, which would replace
  every `rustflags` setting in cargo's config, among them `+crt-static`
  (`launcher/.cargo/config.toml` for the Launcher and the Host, `--config` for the core).
  The programs would then need VCRUNTIME140.dll from the Visual C++ Redistributable,
  which a fresh Windows doesn't have. So the script leaves that variable out, refuses to
  start while `RUSTFLAGS` is set, and refuses a built program that imports
  VCRUNTIME140.dll.
- **GPUI's shaders.** GPUI (our fork's `lodestone/cross-windows-shaders`) compiles its
  HLSL shaders whenever the target is Windows, with Microsoft's `fxc.exe`, which off
  Windows it runs from `GPUI_FXC_PATH`. fxc.exe runs under Wine, with its own
  D3DCOMPILER_47.dll beside it, so the shaders are byte for byte what Windows would make.
- **The installer.** Inno Setup's compiler (ISCC) runs under Wine, from the same `.iss`
  files (`launcher/installer/windows`).
- `bin/fxc` and `bin/iscc` in the setup folder run those two under Wine, turning the Unix
  paths they're given (bare, or in a `/DName=/path` define) into Wine's `Z:\...` paths.

## Setting up

```bash
python3 launcher/scripts/setup_cross_windows.py --check     # what's there; changes nothing
python3 launcher/scripts/setup_cross_windows.py             # set up what's missing
python3 launcher/scripts/setup_cross_windows.py --accept-microsoft-license
```

It installs, and running it again only does what's missing:

| What | From | Pinned by |
| --- | --- | --- |
| llvm (clang-cl, llvm-lib), lld (lld-link), msitools | Homebrew | Homebrew |
| cargo-xwin | `cargo install --locked` | version (`CARGO_XWIN_VERSION`) |
| the `x86_64-pc-windows-msvc` target | rustup, for `rust-toolchain.toml`'s Rust | the toolchain |
| Wine (Gcenx's macOS build of wine-stable) | github.com/Gcenx/macOS_Wine_builds | URL and SHA-256 |
| fxc.exe and D3DCOMPILER_47.dll | the Windows SDK's tools installer, found through Visual Studio's manifest, only with `--accept-microsoft-license` | the SHA-256 the manifest lists (the manifest is checked against the channel's; https only) |
| Inno Setup 6, in its own Wine prefix | github.com/jrsoftware/issrc | URL and SHA-256 (`wine-prefix/inno-setup.sha256` records which installer finished) |
| Microsoft's C runtime and SDK libraries | cargo-xwin, only with `--accept-microsoft-license` | cargo-xwin |

Everything but Homebrew's formulas, cargo-xwin, the Rust target and cargo-xwin's cache goes
into one folder, `~/Library/Application Support/Lodestone/cross-windows`
(`$LODESTONE_CROSS_WINDOWS_DIR`, or `--dir`): `downloads/` (one folder per download,
each checked before use), `wine/`, `windows-sdk/`, `wine-prefix/` and `bin/`.
`publish_release.py` looks there, so nothing needs setting. With `--dir`, it doesn't:
the setup script ends by printing the `export LODESTONE_CROSS_WINDOWS_DIR=...` that
`publish_release.py` then needs. `GPUI_FXC_PATH`, `ISCC` and `XWIN_CACHE_DIR` still
override what it finds.

- **Microsoft's license.** cargo-xwin downloads the MSVC C runtime and the Windows SDK's
  libraries and headers from Microsoft's Visual Studio servers without asking. They're
  under Microsoft's license terms (https://go.microsoft.com/fwlink/?LinkId=2086102, the
  terms the `xwin` tool asks about). fxc.exe and D3DCOMPILER_47.dll come from the
  Windows SDK's tools installer, under the Windows SDK's license terms
  (https://developer.microsoft.com/windows/downloads/windows-sdk/). So the setup script
  downloads none of them without `--accept-microsoft-license`, which covers both and
  which the developer passes once they have read and accept them. `publish_release.py`
  refuses to build while they're missing, unless `XWIN_ACCEPT_LICENSE` is set (for
  cargo-xwin's part).
- **Rosetta 2.** Gcenx's Wine is an Intel build, so on Apple silicon it runs under
  Rosetta. The setup script checks for it but never installs it: that's
  `softwareupdate --install-rosetta`, for the developer.
- **Gatekeeper.** Homebrew's Wine casks are disabled because they fail Gatekeeper. The
  setup script downloads Wine itself (so it isn't quarantined) and checks its SHA-256;
  it never changes quarantine attributes or security settings.
- **Space and time.** Homebrew's llvm takes about 1.8 GB installed (lld and msitools a
  few MB), and cargo-xwin is compiled from source (a few minutes). Without the license,
  the setup script downloads about 190 MB (Wine and Inno Setup); with it, about 100 MB
  more (Visual Studio's manifest and the SDK tools) and cargo-xwin's 1.1 GB cache. The
  setup folder ends up at about 1.5 GB (Wine 0.7 GB, the SDK tools 0.2 GB, the Wine
  prefix 0.4 GB, the downloads 0.3 GB). A first setup takes about a minute after
  Homebrew's and cargo's parts; `downloads/` can be deleted afterwards (a later run
  downloads only what it needs to set up again).
- **Updating a pin** (a newer Wine, Inno Setup or cargo-xwin): change the URL and SHA-256
  (or version) at the top of `setup_cross_windows.py`, check the new release against
  OSV, RustSec and GitHub's advisories like any new dependency, and run it again. A
  changed Wine archive is unpacked again, and a changed Inno Setup installer is run
  again over the old one (`wine-prefix/inno-setup.sha256` says which one finished).

`publish_release.py` checks all of it before the long build and stops with one message
listing what's missing and naming the setup script.

## Building

- **A developer build:** `python3 launcher/scripts/publish_release.py --dev --dry-run
  --target x86_64-pc-windows-msvc` (add `--product host` for the Host). On a Mac, the Mac
  build and the Windows build of a commit can come from the same run:
  `--target aarch64-apple-darwin --target x86_64-apple-darwin --target
  x86_64-pc-windows-msvc`. Then the same command without `--dry-run` (`--dev` with the
  same `--target` options) uploads exactly what the dry run staged, once it's tested.
- **A version:** `--dry-run --tag draft --target x86_64-pc-windows-msvc`, test, then
  `--tag draft --target x86_64-pc-windows-msvc`. The draft's Windows build is then this
  one; CI leaves it alone if it's there before CI's Windows build finishes.
  - **If the draft already has `windows-x86_64`** (usually CI's untested build), the
    dry run refuses. Stop and say so; replace it with `--force` only when the developer
    asks, as for any platform a draft has.
  - **To keep CI's Windows build out of a new Launcher version,** a developer runs
    "Launcher - Release" by hand with the tag, the commit and `platforms: macos` (the
    tag it makes doesn't start another run). Pushing the tag, or `--start`, builds both.
    For the Host, "Host - Release" (by hand only) takes `platforms: macos` the same way.
- `--open` opens this machine's test copy only. A Windows-only dry run refuses it before
  building (there'd be nothing to open); with Mac targets too, it opens the Mac copy, and
  the Windows build is still tested on Windows.
- `--no-installer` skips the installer, and Inno Setup isn't needed for it.
- Only `x86_64-pc-windows-msvc` is built from a Mac; Windows on Arm users run it under
  emulation, as they do CI's build.
- A cold release build of the Launcher takes about as long as the Mac one (LTO is most
  of it); the Host adds the core. fxc and ISCC each start Wine, a few seconds a time.

## What's checked, and what isn't

Checked by the build:

- the C runtime is static (no VCRUNTIME140.dll import);
- the shaders come from Microsoft's fxc (the build fails without it, rather than
  shipping a build without shaders);
- the installer comes from the real Inno Setup compiler, from the same `.iss` files.

Checked by hand when this was set up: the programs are PE32+ x86-64 executables, and
the installer built this way installs.

Not checked: that the programs work. Only running them on Windows shows that.

## Testing

A Mac can't truly run a Windows build, so a build made on a Mac is tested on Windows
before it's uploaded, like every build ("Test a locally built release first" in
AGENTS.md, "Test before uploading" in the launcher's skill).

- **Day to day: a Windows 11 on Arm VM in VMware Fusion**, which `windows_vm.py` makes
  and drives (below). Fusion is free (Broadcom's
  download, behind a free account), and its Windows 11 on Arm guests get Direct3D 11,
  which GPUI's renderer uses. Windows' own Prism emulator runs the x64 programs at close
  to native speed. The installer, the Start Menu shortcut and Settings › Apps all behave
  as on a PC. How an agent on the Mac tests there is below. Parallels Desktop (paid) also
  gives Windows on Arm guests Direct3D 11, but `windows_vm.py` only drives Fusion; UTM has
  no 3D acceleration for Windows guests, so GPUI would draw with Windows' software
  renderer there.
- **A release's final check: a real x64 Windows PC.** Before a developer publishes a
  version whose Windows build came from a Mac, someone runs it on an x64 PC: real GPU
  drivers, no emulation, the machines most users have. The agent's report says this
  check is still to do.
- **Not worth it: an x86 VM on Apple silicon** (UTM or QEMU emulating an x64 PC). That's
  full-system emulation, with no GPU acceleration: Windows runs very slowly and Direct3D
  falls back to software (WARP). It tests less than the Arm VM, which runs the same x64
  programs through Prism on real Direct3D.
- **Wine can't run them.** Wine has no `icuuc.dll`, the ICU library Windows ships in
  System32 since Windows 10 1903 and GPUI imports, so the Launcher and Host don't even
  start under it. The setup folder's Wine is only for fxc and Inno Setup.

### Testing in the VM from the Mac

`launcher/scripts/windows_vm.py` makes the VM and runs the checklist in it, all from the
Mac and in the background: the VM has no window (Fusion's `vmrun ... nogui`), so it never
takes the developer's screen, pointer or keyboard.

**Once: make the VM.** The developer installs VMware Fusion (Broadcom's support portal,
free; "for Personal Use") and gets Microsoft's Windows 11 Arm64 ISO
(microsoft.com/software-download/windows11arm64, "Windows 11 (multi-edition ISO for
Arm64)"; the agent fetches it only with their OK, through that page). Then:

```bash
python3 launcher/scripts/windows_vm.py create --iso <the ISO> --accept-windows-license
```

`--accept-windows-license` accepts Windows' license terms in the unattended install,
which is the developer's call; without it `create` stops. It takes 20 to 40 minutes,
with no clicks:

- It makes `~/Virtual Machines.localized/Lodestone Windows Test.vmwarevm` (4 cores,
  8 GB, a 64 GB disk that grows as used; `--cpus`, `--memory-gb`, `--disk-gb`), which
  Fusion's library shows too.
- Its install disc is Microsoft's ISO rebuilt with Fusion's `mkisofs`: booting with
  `efisys_noprompt.bin` (no "Press any key to boot from CD or DVD"; Fusion's `vmcli`
  can't press keys), with `autounattend.xml` and VMware Tools' installer added.
- Windows 11 Pro installs unactivated (a watermark; the generic Pro key only picks the
  edition). The VM has no virtual TPM (Fusion only adds one to an encrypted VM), so Setup
  skips Windows 11's TPM and Secure Boot checks (`LabConfig`).
- A local account, `lodestone`, signs in by itself at every start, so a desktop session
  is always running (UI Automation needs one). Its password is made up and kept in the
  macOS Keychain ("Lodestone Windows test VM"); `LODESTONE_VM_PASSWORD`, if set, wins
  over it, for `create` and the rest alike. Nothing prints it, but `vmrun` takes it only
  on its command line (`-gp`), where other programs on the Mac can see it while a guest
  operation runs: it's a throwaway test account's, used for nothing else. The install
  disc, which holds it, is deleted afterwards (also when `create` fails).
- The first sign-in (the only step that runs elevated) installs VMware Tools (the
  network and display drivers, and the guest operations `vmrun` uses), turns off sleep,
  the screen saver and signing in on wake, Windows Update's automatic updates (so `fresh`
  stays as it was made) and Windows Firewall's "allow this app?" prompts (the core
  listens on every interface; nobody is there to answer). Then `create` snapshots the
  signed-in desktop as `fresh`, and marks the VM as its own: `reset` and `test` won't
  power off or roll back any other.

**Each build: test it.** After a Windows dry run
(`publish_release.py --dry-run --target x86_64-pc-windows-msvc`, `--product host` for the
Host):

```bash
python3 launcher/scripts/windows_vm.py test launcher --staged <cargo target dir>/release-candidate
python3 launcher/scripts/windows_vm.py test host --staged <the Host's target dir>/release-candidate
```

It goes back to `fresh` (`--keep` skips that), checks the staged files against
`stage.json`, copies them in with `ui_drive.ps1` and `windows_vm_test.ps1`, runs that in
the signed-in session as the account (not elevated, so UAC applies as on a person's PC),
and copies the results out (into the release-candidate folder, or `--out`; it only ever
empties a folder it made). A Launcher's run takes about two minutes, a Host's four:

- **standalone:** the `-standalone` exe opens its window with empty data folders of its
  own, Settings shows "Build N" (and the commit), and closing the window quits it. The
  Host's runs the dry run's core and signs in by itself; its data folders are under
  `%LOCALAPPDATA%`, as on Windows the core only lets the Host claim it when its data
  folder is inside the account's own folder. Closing the Host's window keeps it in the
  tray; its Quit is in the tray icon's menu, which UI Automation can't reach, so the test
  ends the Host instead and checks that its core stops with it, and leaves Quit to a
  person (a `check` line).
- **install:** the installer installs silently for this user (`/CURRENTUSER`; an
  administrator prompt would stall it, and the five-minute limit catches that),
  Settings › Apps lists it with "(build N)", and its Start Menu shortcut opens it, with
  Settings showing the build again (the installed Host's core on a data folder of its
  own, so nothing left in the VM's `~\.lodestone` starts).
- **uninstall:** a silent uninstall removes the program, the shortcut and the Apps entry,
  and keeps the data folder.

Each line is `ok`, `FAIL`, or `check` (read it off the screenshot: the build line wasn't
in Settings' controls). The screenshots, the controls' tree, Setup's log and
`results.json` are in the results folder, with the apps' own logs in `logs/`; look at the
screenshots before calling a build good. PowerShell runs there with `-ExecutionPolicy Bypass` for that one process; the VM's
execution policy is never changed.

Other commands: `status`, `start`, `stop`, `reset` (back to `fresh`), and
`screenshot FILE.png` (the VM's screen, without signing in).

- **Not checked there:** launching a game (it needs a signed-in Minecraft account, and
  agents never sign in), and for the Host, starting a server (never; it accepts
  Minecraft's EULA) and Quit from the tray. The report says so. A developer can check a
  game by hand in Fusion's window; `test` always goes back to `fresh`, so keep anything
  signed in out of that snapshot.
- **Fusion's quirks** the script works around: `vmcli`'s key presses don't reach the
  guest (hence the no-prompt disc), `vmrun checkToolsState` says "installed" while Tools
  runs (it waits on guest operations instead), PowerShell started straight from `vmrun
  runProgramInGuest` gets its arguments as one string and exits (it goes through
  `cmd.exe /c`), and `vmrun start` leaves the VM holding its output (read through files,
  not pipes).
- **Without a VM** (no Fusion, no ISO, or the developer would rather not), the agent
  hands the test files to a developer with Windows and waits for their go-ahead. It
  doesn't reach for computer-use tools without asking.

## The rules stay the same

- Agents never publish a version's release, and never run `--bridge`. A developer build
  (`--dev`) publishes itself, only when a developer asks for one.
- Test before uploading: a Windows build from a Mac is tested on Windows first, by the
  agent in the VM with `windows_vm.py test` ("Testing in the VM from the Mac"), or by a
  developer, who says it works.
- CI still builds macOS and Windows when a `launcher-v*` tag is pushed (and on "Host -
  Release"), and leaves a platform a developer uploaded before its build finished alone.
- Developer builds stay on developers' machines: the point of building Windows on a Mac
  is not using GitHub's runners for them.
