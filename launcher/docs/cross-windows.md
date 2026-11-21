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

- **Day to day: a Windows 11 on Arm VM in Parallels Desktop.** It has real Direct3D 11
  through Parallels' graphics driver, and Windows' own Prism emulator runs the x64
  programs at close to native speed. The installer, the Start Menu shortcut and Settings ›
  Apps all behave as on a PC. How an agent on the Mac tests there is below.
- **A release's final check: a real x64 Windows PC.** Before a developer publishes a
  version whose Windows build came from a Mac, someone runs it on an x64 PC: real GPU
  drivers, no emulation, the machines most users have. The agent's report says this
  check is still to do.
- **Not worth it: an x86 VM on Apple silicon** (UTM or QEMU emulating an x64 PC). That's
  full-system emulation, with no GPU acceleration: Windows runs very slowly and Direct3D
  falls back to software (WARP). It tests less than the Arm VM, which runs the same x64
  programs through Prism on real Direct3D.
- **Wine only smoke-tests startup.** Running the standalone exe under the setup folder's
  Wine shows it loads and starts writing its log, which catches a missing DLL or a crash
  at start. Wine's Direct3D, file dialogs and installer behaviour aren't Windows', so it
  never replaces the VM. Give it its own data folders, never the developer's.

### Testing in the VM from the Mac

The developer sets the VM up once: Parallels Desktop with a Windows 11 on Arm VM, the
Mac's home folder shared with it (Parallels' shared folders show it in Windows as
`\\Mac\Home`), and signed in to Windows so its desktop session is running. They
tell the agent the VM's name (`prlctl list -a` lists them). Then, from the Mac, with
`prlctl` (Parallels' command line tool; `--current-user` runs a command in the signed-in
user's session, without which UI Automation sees no windows):

```bash
VM="Windows 11"     # the developer's VM
# The dry run's test copy, through the shared folder (the dry run prints its Mac path;
# /Users/<name>/... is \\Mac\Home\... in the VM):
prlctl exec "$VM" --current-user powershell -NoProfile -Command \
  "New-Item -ItemType Directory -Force \$env:TEMP\lodestone-test | Out-Null;
   Copy-Item '\\Mac\Home\...\release-candidate\test\*.exe' \$env:TEMP\lodestone-test -Force"
# Start it and read its process id:
prlctl exec "$VM" --current-user powershell -NoProfile -Command \
  "(Start-Process \$env:TEMP\lodestone-test\<the exe> -PassThru).Id"
# Drive it with ui_drive.ps1 from the checkout, by that process id:
prlctl exec "$VM" --current-user powershell -NoProfile \
  -File '\\Mac\Home\...\launcher\scripts\ui_drive.ps1' --pid <PID> tree
```

`ui_drive.ps1` works there as on any Windows machine ("Testing the launcher's and Host's
UI" in AGENTS.md, `launcher/docs/ui-automation.md`): `screenshot` saves into a folder the
Mac sees through the share. If PowerShell's execution policy refuses to run the script,
that's the developer's to set in the VM; the agent never changes it. The installer is tested the same way
(`<the setup exe> /VERYSILENT /CURRENTUSER`, then the Start Menu shortcut and Settings ›
Apps).

- **The checklist's game launch** needs an instance and a signed-in Minecraft account
  in the VM, and agents never sign in or type credentials. The developer prepares them
  once in the VM (signed in, with an instance; `lodestone-launcher-cli` can make the
  instance); until then, launching a game is the developer's to check, and the agent's
  report says it wasn't checked. For the Host, never start a Minecraft server there.
- **If the agent can't reach the VM** (no Parallels, no `prlctl`, no VM name, or the VM
  isn't signed in), it doesn't improvise: it hands the test files to the developer and
  waits for their go-ahead. It doesn't reach for computer-use tools without asking.

## The rules stay the same

- Agents never publish a version's release, and never run `--bridge`. A developer build
  (`--dev`) publishes itself, only when a developer asks for one.
- Test before uploading: a Windows build from a Mac is tested on Windows first, by the
  agent in a VM with `ui_drive.ps1` ("Testing in the VM from the Mac"), or by a
  developer, who says it works.
- CI still builds macOS and Windows when a `launcher-v*` tag is pushed (and on "Host -
  Release"), and leaves a platform a developer uploaded before its build finished alone.
- Developer builds stay on developers' machines: the point of building Windows on a Mac
  is not using GitHub's runners for them.
