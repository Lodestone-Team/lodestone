//! Embeds every font under `launcher/assets/fonts` into the binary.
//!
//! The list is generated so that a font file can be removed (for example, if its
//! licence does not allow it in the repository) without editing code. GPUI then
//! falls back to the system font for that family.

use std::{env, fmt::Write as _, fs, path::Path};

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by Cargo");
    let fonts_dir = Path::new(&manifest_dir).join("../../assets/fonts");
    println!("cargo::rerun-if-changed={}", fonts_dir.display());

    let mut fonts = Vec::new();
    collect_fonts(&fonts_dir, &mut fonts);
    fonts.sort();

    let mut out = String::from("&[\n");
    for path in &fonts {
        println!("cargo::rerun-if-changed={}", path.display());
        let name = path
            .strip_prefix(&fonts_dir)
            .expect("font is under the fonts directory");
        writeln!(
            out,
            "    ({:?}, include_bytes!({:?}).as_slice()),",
            name.display().to_string(),
            path.display().to_string()
        )
        .expect("writing to a String cannot fail");
    }
    out.push(']');

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is set by Cargo");
    fs::write(Path::new(&out_dir).join("fonts.rs"), out).expect("can write to OUT_DIR");
}

fn collect_fonts(dir: &Path, fonts: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            println!("cargo::rerun-if-changed={}", path.display());
            collect_fonts(&path, fonts);
        } else if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("otf" | "ttf")
        ) {
            fonts.push(path);
        }
    }
}
