//! Embeds the app icon into the Windows executable. (The fonts are embedded by
//! `lodestone-ui`.)

use std::{env, fs, path::Path};

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by Cargo");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_windows_icon(&manifest_dir);
    }
}

/// Compiles `installer/windows/lodestone.ico` into the executable as icon resource 1.
/// GPUI loads that resource for the window class, so it is the icon of the window, the
/// taskbar and Alt+Tab, as well as of the .exe in Explorer; without it Windows shows a
/// blank one.
fn embed_windows_icon(manifest_dir: &str) {
    let icon = Path::new(manifest_dir).join("../../installer/windows/lodestone.ico");
    println!("cargo::rerun-if-changed={}", icon.display());
    let icon = icon
        .canonicalize()
        .expect("installer/windows/lodestone.ico exists");
    // Without the `\\?\` prefix `canonicalize` adds, which resource compilers don't read.
    let icon = icon.display().to_string();
    let icon = icon.strip_prefix(r"\\?\").unwrap_or(&icon);

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is set by Cargo");
    let rc = Path::new(&out_dir).join("lodestone-launcher.rc");
    fs::write(&rc, format!("1 ICON \"{}\"\n", icon.replace('\\', "\\\\")))
        .expect("can write to OUT_DIR");
    embed_resource::compile(&rc, embed_resource::NONE)
        .manifest_required()
        .expect("the Windows resource compiler (rc.exe from the Windows SDK, or llvm-rc) runs");
}
