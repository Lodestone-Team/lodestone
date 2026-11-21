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
`--dev`, or `--tag ...`, from the same checkout without changes.

## How it works

- **Compiling and linking.** `cargo-xwin` gives cargo what it needs to build for
  `x86_64-pc-windows-msvc` on a Mac: clang-cl as the C compiler, lld-link as the linker,
  and Microsoft's C runtime and Windows SDK libraries and headers, which it downloads
  into its cache. `publish_release.py` runs `cargo xwin env --target ...` and builds with
  the same `cargo build` command as on Windows, with that environment added to the
  Windows builds only. Each Mac build in the same run (`--target aarch64-apple-darwin
  --target x86_64-pc-windows-msvc` works) gets none of it.
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
| fxc.exe and D3DCOMPILER_47.dll | the Windows SDK's tools installer, found through Visual Studio's manifest | the SHA-256 the manifest lists |
| Inno Setup 6, in its own Wine prefix | github.com/jrsoftware/issrc | URL and SHA-256 |
| Microsoft's C runtime and SDK libraries | cargo-xwin, only with `--accept-microsoft-license` | cargo-xwin |

Everything but Homebrew's formulas, cargo-xwin, the Rust target and cargo-xwin's cache goes
into one folder, `~/Library/Application Support/Lodestone/cross-windows`
(`$LODESTONE_CROSS_WINDOWS_DIR`, or `--dir`): `downloads/` (one folder per download,
each checked before use), `wine/`, `windows-sdk/`, `wine-prefix/` and `bin/`.
`publish_release.py` looks there, so nothing needs setting. `GPUI_FXC_PATH`, `ISCC` and
`XWIN_CACHE_DIR` still override what it finds.

- **Microsoft's license.** cargo-xwin downloads the MSVC C runtime and the Windows SDK's
  libraries and headers from Microsoft's Visual Studio servers without asking. They're
  under Microsoft's license terms (https://go.microsoft.com/fwlink/?LinkId=2086102, the
  terms the `xwin` tool asks about). So the setup script only downloads them with
  `--accept-microsoft-license`, which the developer passes once they have read and accept
  them, and `publish_release.py` refuses to build while they're missing, unless
  `XWIN_ACCEPT_LICENSE` is set. fxc.exe also comes from the Windows SDK.
- **Rosetta 2.** Gcenx's Wine is an Intel build, so on Apple silicon it runs under
  Rosetta. The setup script checks for it but never installs it: that's
  `softwareupdate --install-rosetta`, for the developer.
- **Gatekeeper.** Homebrew's Wine casks are disabled because they fail Gatekeeper. The
  setup script downloads Wine itself (so it isn't quarantined) and checks its SHA-256;
  it never changes quarantine attributes or security settings.
- **Space and time.** The setup folder takes about 1.5 GB (Wine 0.7 GB, the SDK tools
  0.2 GB, the Wine prefix 0.4 GB, the downloads 0.3 GB) and cargo-xwin's cache about
  1.1 GB, plus Homebrew's llvm. A first setup takes about a minute after Homebrew's
  part; `downloads/` can be deleted afterwards (a later run downloads only what it
  needs to set up again).
- **Updating a pin** (a newer Wine, Inno Setup or cargo-xwin): change the URL and SHA-256
  (or version) at the top of `setup_cross_windows.py`, check the new release against
  OSV, RustSec and GitHub's advisories like any new dependency, and run it again. A
  changed Wine archive is unpacked again; delete `wine-prefix/` to reinstall Inno Setup.

`publish_release.py` checks all of it before the long build and stops with one message
listing what's missing and naming the setup script.

## Building

- **A developer build:** `python3 launcher/scripts/publish_release.py --dev --dry-run
  --target x86_64-pc-windows-msvc` (add `--product host` for the Host). On a Mac, the Mac
  build and the Windows build of a commit can come from the same run:
  `--target aarch64-apple-darwin --target x86_64-apple-darwin --target
  x86_64-pc-windows-msvc`. Then `--dev` alone uploads exactly what the dry run staged,
  once it's tested.
- **A version:** `--dry-run --tag draft --target x86_64-pc-windows-msvc`, test, then
  `--tag draft --target x86_64-pc-windows-msvc`. The draft's Windows build is then this
  one; CI leaves a platform a developer uploaded alone.
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
before it's uploaded, like every build ("Test before uploading" in AGENTS.md).

- **Day to day: a Windows 11 on Arm VM in Parallels Desktop.** It has real Direct3D 11
  through Parallels' graphics driver, and Windows' own Prism emulator runs the x64
  programs at close to native speed. Copy the dry run's test files into the VM (a shared
  folder works) and drive them there with `launcher/scripts/ui_drive.ps1`, which runs in
  the background ("Testing the launcher's and Host's UI" in AGENTS.md,
  `launcher/docs/ui-automation.md`). The installer, the Start Menu shortcut and Settings ›
  Apps all behave as on a PC.
- **A release's final check: a real x64 Windows PC.** Before a developer publishes a
  version whose Windows build came from a Mac, someone runs it on an x64 PC: real GPU
  drivers, no emulation, the machines most users have.
- **Not worth it: an x86 VM on Apple silicon** (UTM or QEMU emulating an x64 PC). That's
  full-system emulation, with no GPU acceleration: Windows runs very slowly and Direct3D
  falls back to software (WARP). It tests less than the Arm VM, which runs the same x64
  programs through Prism on real Direct3D.
- **Wine only smoke-tests startup.** Running the standalone exe under the setup folder's
  Wine shows it loads and starts writing its log, which catches a missing DLL or a crash
  at start. Wine's Direct3D, file dialogs and installer behaviour aren't Windows', so it
  never replaces the VM. Give it its own data folders, never the developer's.

## The rules stay the same

- Agents never publish a version's release, and never run `--bridge`. A developer build
  (`--dev`) publishes itself, only when a developer asks for one.
- Test before uploading: a Windows build from a Mac is tested on Windows first, by the
  agent in a VM with `ui_drive.ps1`, or by a developer, who says it works.
- CI still builds macOS and Windows when a `launcher-v*` tag is pushed (and on "Host -
  Release"), and leaves a platform a developer already uploaded alone.
- Developer builds stay on developers' machines: the point of building Windows on a Mac
  is not using GitHub's runners for them.
