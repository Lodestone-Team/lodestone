//! Builds the deno runtime startup snapshot used by the macro executor.
//!
//! Without a snapshot, deno_runtime reads its own JavaScript sources from the cargo
//! registry path at runtime, which does not exist on machines running a shipped binary.
//! The snapshot is built for the host architecture, so core must not be cross-compiled.

use std::{collections::HashSet, fmt::Write as _, path::PathBuf};

use deno_runtime::{
    ops::bootstrap::SnapshotOptions,
    snapshot::{create_runtime_snapshot, LazyExtensionFileKind},
};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let output = create_runtime_snapshot(
        out_dir.join("RUNTIME_SNAPSHOT.bin"),
        SnapshotOptions::default(),
        // Lodestone's own extensions are registered at worker creation, not snapshotted,
        // since they carry per-worker state.
        vec![],
    );

    // Lazily loaded runtime files that the snapshot did not consume must ship in the
    // binary. The runtime does not transpile these, so transpile them here.
    let consumed: HashSet<_> = output.consumed_lazy_specifiers.iter().cloned().collect();
    let mut js = String::from("&[\n");
    let mut esm = String::from("&[\n");
    for (i, file) in output.lazy_extension_files.iter().enumerate() {
        if consumed.contains(&file.specifier) {
            continue;
        }
        println!("cargo:rerun-if-changed={}", file.path.display());
        let source = std::fs::read_to_string(&file.path).expect("failed to read lazy runtime file");
        let (code, _source_map) = deno_runtime::transpile::maybe_transpile_source(
            deno_core::ModuleName::from(file.specifier.clone()),
            source.into(),
        )
        .expect("failed to transpile lazy runtime file");
        let path = out_dir.join(format!("residual_{i}.js"));
        std::fs::write(&path, code.as_bytes()).expect("failed to write lazy runtime file");
        let entry = format!(
            "    ({:?}, include_str!({:?})),\n",
            file.specifier,
            path.display().to_string()
        );
        match file.kind {
            LazyExtensionFileKind::Js => js.push_str(&entry),
            LazyExtensionFileKind::Esm => esm.push_str(&entry),
        }
    }
    js.push(']');
    esm.push(']');

    let mut generated = String::new();
    writeln!(
        generated,
        "pub static RESIDUAL_LAZY_JS: &[(&str, &str)] = {js};"
    )
    .unwrap();
    writeln!(
        generated,
        "pub static RESIDUAL_LAZY_ESM: &[(&str, &str)] = {esm};"
    )
    .unwrap();
    std::fs::write(out_dir.join("residual_sources.rs"), generated)
        .expect("failed to write residual_sources.rs");
}
