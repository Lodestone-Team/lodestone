//! Imports from other launchers, against made-up data folders in `testdata/import/`
//! (written by `make_fixtures.py`) and a loopback Modrinth mock: reading each launcher's
//! format, detection in the default places, the copy (caches and logs left out, links
//! and unsafe names), cancel and failure leaving nothing, and Modrinth matching.

mod modrinth;

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use futures::executor::block_on;
use launcher_core::{
    content::{ContentKind, provenance::Provenance},
    game::{download::CancelToken, loaders::LoaderKind, platform::Os},
    import::{
        self, CopyOptions, Found, IconSource, ImportError, ImportProgress, ImportRequest,
        Locations, Resolved, Source, WantedLoader, WantedVersion, detect,
    },
    instances::{CUSTOM_ICON, InstanceStore, Loader, Memory},
};
use modrinth::{FakeVersion, ModrinthMock};
// Only the Unix-only test below copies with symbolic links.
#[cfg(unix)]
use launcher_core::import::copy;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/import")
}

fn fixture(path: &str) -> PathBuf {
    fixtures().join(path)
}

fn by_name<'a>(launcher: &'a detect::Launcher, name: &str) -> &'a Found {
    launcher
        .instances
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| {
            panic!(
                "no {name} in {:?}",
                launcher
                    .instances
                    .iter()
                    .map(|f| &f.name)
                    .collect::<Vec<_>>()
            )
        })
}

fn loader(kind: LoaderKind, version: &str) -> Option<WantedLoader> {
    Some(WantedLoader::new(kind, Some(version)))
}

fn exact(version: &str) -> WantedVersion {
    WantedVersion::Exact(version.into())
}

/// Every file under `dir` with its size and modification time, to check it is unchanged.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, (u64, Option<SystemTime>)> {
    let mut map = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d).unwrap().flatten() {
            let meta = fs::symlink_metadata(entry.path()).unwrap();
            if meta.is_dir() {
                stack.push(entry.path());
            }
            map.insert(entry.path(), (meta.len(), meta.modified().ok()));
        }
    }
    map
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn reads_the_official_launcher() {
    let root = fixture("vanilla/minecraft");
    let launcher = detect::read_root(Source::Vanilla, &root);
    assert!(launcher.problems.is_empty(), "{:?}", launcher.problems);
    let names: Vec<&str> = launcher.instances.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        ["Latest release", "Fabric 1.20.1", "Old Forge"],
        "last used first"
    );
    let latest = &launcher.instances[0];
    assert_eq!(latest.game_version, WantedVersion::LatestRelease);
    assert_eq!(latest.game_dir, root);
    assert_eq!(latest.icon, None, "built-in icons have no image");
    let fabric = by_name(&launcher, "Fabric 1.20.1");
    assert_eq!(fabric.game_version, exact("1.20.1"));
    assert_eq!(fabric.loader, loader(LoaderKind::Fabric, "0.16.14"));
    assert_eq!(fabric.game_dir, root.join("profiles/fabric"));
    assert_eq!(
        fabric.memory,
        Some(Memory {
            min: 1024,
            max: 4096
        })
    );
    assert_eq!(fabric.jvm_args.as_deref(), Some("-XX:+UseG1GC"));
    assert!(matches!(&fabric.icon, Some(IconSource::Bytes(b)) if b.starts_with(b"\x89PNG")));
    let forge = by_name(&launcher, "Old Forge");
    assert_eq!(forge.game_version, exact("1.7.10"));
    assert_eq!(forge.loader, loader(LoaderKind::Forge, "10.13.4.1614"));
    assert!(forge.notes.iter().any(|n| n.contains("Java")));
    assert_eq!(fabric.version_label(), "1.20.1 · Fabric 0.16.14");
}

#[test]
fn reads_prism_and_multimc() {
    let root = fixture("prism/PrismLauncher");
    let prism = detect::read_root(Source::Prism, &root);
    assert!(prism.problems.is_empty(), "{:?}", prism.problems);
    assert_eq!(
        prism.instances.len(),
        3,
        "the folder without instance.cfg is skipped"
    );
    let fabric = by_name(&prism, "Fabric Pack, Deluxe");
    assert_eq!(fabric.game_version, exact("1.20.1"));
    assert_eq!(fabric.loader, loader(LoaderKind::Fabric, "0.16.14"));
    assert_eq!(
        fabric.game_dir,
        root.join("instances/Fabric Pack/minecraft")
    );
    assert_eq!(
        fabric.memory,
        Some(Memory {
            min: 1024,
            max: 6144
        })
    );
    assert_eq!(fabric.jvm_args.as_deref(), Some("-XX:+UseZGC -Dfoo=bar"));
    assert!(matches!(&fabric.icon, Some(IconSource::File(p)) if p.ends_with("mypack.png")));
    let forge = by_name(&prism, "Forge Old");
    assert_eq!(forge.loader, loader(LoaderKind::Forge, "10.13.4.1614"));
    assert_eq!(
        forge.game_dir,
        root.join("instances/Forge Old/.minecraft"),
        "Prism uses .minecraft only when minecraft doesn't exist"
    );
    assert_eq!(forge.memory, None, "OverrideMemory is off");
    assert_eq!(forge.icon, None, "built-in icon");
    let neo = by_name(&prism, "NeoForge 1.20.1");
    assert_eq!(neo.loader, loader(LoaderKind::Forge, "47.1.106"));
    assert!(neo.notes[0].contains("NeoForge"));

    let multimc = detect::read_root(Source::MultiMc, &fixture("multimc/MultiMC.app/Data"));
    let legacy = by_name(&multimc, "Legacy#1");
    assert_eq!(legacy.game_version, exact("1.8.9"));
    assert_eq!(legacy.loader, loader(LoaderKind::Forge, "11.15.1.2318"));
    assert!(legacy.game_dir.ends_with("Legacy/.minecraft"));
    assert_eq!(
        legacy.memory,
        Some(Memory {
            min: 512,
            max: 2048
        })
    );
    // Picking the app bundle finds its data.
    let picked = detect::read_folder(&fixture("multimc/MultiMC.app")).unwrap();
    assert_eq!(picked[0].source, Source::MultiMc);
}

/// The 0.14 schema, with rows only in the WAL (one with an overflowing JSONB column) and
/// columns added after rows were written.
#[test]
fn reads_the_modrinth_app_database_with_its_wal() {
    let root = fixture("modrinth-app/ModrinthApp");
    let app = detect::read_root(Source::ModrinthApp, &root);
    assert!(app.problems.is_empty(), "{:?}", app.problems);
    let mut names: Vec<&str> = app.instances.iter().map(|f| f.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Forge Pack", "Plain", "Sodium Pack (renamed)"]);
    let sodium = by_name(&app, "Sodium Pack (renamed)");
    assert_eq!(sodium.loader, loader(LoaderKind::Fabric, "0.16.14"));
    assert_eq!(sodium.game_dir, root.join("profiles/Sodium Pack"));
    assert_eq!(sodium.memory, None);
    let forge = by_name(&app, "Forge Pack");
    assert_eq!(forge.game_version, exact("1.20.1"));
    assert_eq!(forge.loader, loader(LoaderKind::Forge, "47.4.28"));
    assert_eq!(
        forge.memory,
        Some(Memory {
            min: 512,
            max: 6144
        })
    );
    let args = forge.jvm_args.as_deref().unwrap();
    assert!(args.starts_with("-Dlong=xxxx") && args.ends_with(" -XX:+UseG1GC"));
    assert_eq!(args.len(), "-Dlong=".len() + 9000 + " -XX:+UseG1GC".len());
    assert!(matches!(&forge.icon, Some(IconSource::File(p)) if p.ends_with("forge.png")));
    assert!(forge.notes.iter().any(|n| n.contains("Java")));
    let plain = by_name(&app, "Plain");
    assert_eq!(plain.loader, None);
    assert!(plain.notes[0].contains("hadn't finished"));

    // Without the WAL, only what was checkpointed: the first profile, unrenamed.
    let db = launcher_core::import::sqlite::Database::from_bytes(
        fs::read(root.join("app.db")).unwrap(),
        None,
    )
    .unwrap();
    let rows = db.rows("profiles").unwrap().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["name"].as_str(), Some("Sodium Pack"));
    assert_eq!(
        rows[0]["override_mc_memory_max"],
        launcher_core::import::sqlite::Value::Null
    );
    // A WAL cut in the middle of a transaction counts only the committed ones.
    let wal = fs::read(root.join("app.db-wal")).unwrap();
    let cut = launcher_core::import::sqlite::Database::from_bytes(
        fs::read(root.join("app.db")).unwrap(),
        Some(wal[..wal.len() - 100].to_vec()),
    )
    .unwrap();
    assert!(cut.rows("profiles").unwrap().unwrap().len() < 3);
    assert!(db.rows("nope").unwrap().is_none());
}

#[test]
fn reads_the_newer_modrinth_app_and_its_json_predecessor() {
    let root = fixture("modrinth-app-0.15/ModrinthApp");
    let app = detect::read_root(Source::ModrinthApp, &root);
    assert!(app.problems.is_empty(), "{:?}", app.problems);
    assert_eq!(
        app.instances.len(),
        60,
        "interior b-tree pages are followed"
    );
    let neo = by_name(&app, "NeoForge 21");
    assert_eq!(neo.game_version, exact("1.21.1"));
    assert_eq!(neo.loader, loader(LoaderKind::NeoForge, "21.1.209"));
    assert_eq!(
        neo.memory,
        Some(Memory {
            min: 512,
            max: 5120
        })
    );
    assert_eq!(neo.jvm_args.as_deref(), Some("-XX:+UseZGC"));
    assert_eq!(neo.game_dir, root.join("profiles/Instance 00"));

    let legacy = detect::read_root(
        Source::ModrinthApp,
        &fixture("modrinth-app-legacy/com.modrinth.theseus"),
    );
    let quilt = by_name(&legacy, "Quilt Fun");
    assert_eq!(quilt.loader, loader(LoaderKind::Quilt, "0.26.0"));
    assert_eq!(
        quilt.memory,
        Some(Memory {
            min: 512,
            max: 3000
        })
    );
    assert_eq!(quilt.jvm_args.as_deref(), Some("-Dq=1"));
}

/// NeoForge for 1.20.1 comes in as Forge 47.1 from every launcher, not only Prism and
/// CurseForge: NeoForge's own list has no 1.20.1 builds, so it would be vanilla (LS-42).
#[test]
fn neoforge_for_1_20_1_is_forge_from_every_launcher() {
    let temp = tempfile::tempdir().unwrap();
    let write = |folder: &str, file: &str, json: serde_json::Value| {
        let dir = temp.path().join(folder);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(file), json.to_string()).unwrap();
        dir
    };
    let instances = [
        (
            Source::ModrinthApp,
            write(
                "modrinth",
                "profile.json",
                serde_json::json!({
                    "path": "modrinth",
                    "metadata": {
                        "name": "Neo", "game_version": "1.20.1", "loader": "neoforge",
                        "loader_version": { "id": "47.1.106" },
                    },
                }),
            ),
        ),
        (
            Source::AtLauncher,
            write(
                "atlauncher",
                "instance.json",
                serde_json::json!({
                    "id": "1.20.1",
                    "launcher": {
                        "name": "Neo",
                        "loaderVersion": { "version": "47.1.106", "type": "NeoForge" },
                    },
                }),
            ),
        ),
        (
            Source::GdLauncher,
            write(
                "gdlauncher",
                "config.json",
                serde_json::json!({
                    "loader": {
                        "loaderType": "neoforge", "loaderVersion": "47.1.106",
                        "mcVersion": "1.20.1",
                    },
                }),
            ),
        ),
        (
            Source::GdLauncherCarbon,
            write(
                "carbon",
                "instance.json",
                serde_json::json!({
                    "name": "Neo",
                    "game_configuration": { "version": {
                        "release": "1.20.1",
                        "modloaders": [{ "type": "NeoForge", "version": "47.1.106" }],
                    }},
                }),
            ),
        ),
    ];
    for (source, dir) in instances {
        let read = detect::read_folder(&dir).unwrap();
        assert_eq!(read[0].source, source);
        let found = &read[0].instances[0];
        assert_eq!(found.game_version, exact("1.20.1"), "{source:?}");
        assert_eq!(
            found.loader,
            loader(LoaderKind::Forge, "47.1.106"),
            "{source:?}"
        );
        assert!(
            found.notes.iter().any(|n| n.contains("runs as Forge 47.1")),
            "{source:?}: {:?}",
            found.notes
        );
    }
    // NeoForge for other versions stays NeoForge
    let newer = write(
        "newer",
        "profile.json",
        serde_json::json!({
            "path": "newer",
            "metadata": {
                "name": "Neo 21", "game_version": "1.21.1", "loader": "neoforge",
                "loader_version": { "id": "21.1.209" },
            },
        }),
    );
    let read = detect::read_folder(&newer).unwrap();
    assert_eq!(
        read[0].instances[0].loader,
        loader(LoaderKind::NeoForge, "21.1.209")
    );
    assert!(read[0].instances[0].notes.is_empty());
}

#[test]
fn reads_atlauncher_curseforge_and_gdlauncher() {
    let at = detect::read_root(Source::AtLauncher, &fixture("atlauncher"));
    let fabric = by_name(&at, "Vanilla Plus (Fabric)");
    assert_eq!(fabric.game_version, exact("1.21.1"));
    assert_eq!(fabric.loader, loader(LoaderKind::Fabric, "0.16.5"));
    assert_eq!(
        fabric.memory,
        Some(Memory {
            min: 512,
            max: 8192
        })
    );
    assert!(matches!(&fabric.icon, Some(IconSource::File(p)) if p.ends_with("instance.png")));
    assert_eq!(
        fabric.game_dir,
        fixture("atlauncher/instances/VanillaPlusFabric")
    );
    let forge = by_name(&at, "Forge One");
    assert_eq!(forge.loader, loader(LoaderKind::Forge, "1.20.1-47.4.0"));

    let cf = detect::read_root(
        Source::CurseForge,
        &fixture("curseforge/minecraft/Instances"),
    );
    let atm = by_name(&cf, "All the Mods");
    assert_eq!(atm.loader, loader(LoaderKind::Forge, "forge-47.4.0"));
    assert_eq!(
        atm.memory,
        Some(Memory {
            min: 512,
            max: 10240
        })
    );
    assert_eq!(atm.jvm_args.as_deref(), Some("-XX:+UseG1GC"));
    let fabulously = by_name(&cf, "Fabulously Fabric");
    assert_eq!(
        fabulously.loader,
        loader(LoaderKind::Fabric, "fabric-0.16.9-1.21.1")
    );
    assert_eq!(fabulously.memory, None, "not overridden");
    // Picking CurseForge's minecraft folder finds the instances.
    let picked = detect::read_folder(&fixture("curseforge/minecraft")).unwrap();
    assert_eq!(picked[0].instances.len(), 2);

    let gd = detect::read_root(Source::GdLauncher, &fixture("gdlauncher/gdlauncher_next"));
    let forge = by_name(&gd, "Forge 1.16");
    assert_eq!(forge.game_version, exact("1.16.5"));
    assert_eq!(forge.loader, loader(LoaderKind::Forge, "1.16.5-36.2.39"));
    assert_eq!(
        forge.memory,
        Some(Memory {
            min: 512,
            max: 6144
        })
    );
    assert!(matches!(&forge.icon, Some(IconSource::File(p)) if p.ends_with("background.png")));

    let carbon = detect::read_root(
        Source::GdLauncherCarbon,
        &fixture("gdlauncher-carbon/gdlauncher_carbon"),
    );
    let quilt = by_name(&carbon, "Quilt Test");
    assert_eq!(quilt.game_version, exact("1.20.4"));
    assert_eq!(quilt.loader, loader(LoaderKind::Quilt, "0.26.0"));
    assert_eq!(
        quilt.memory,
        Some(Memory {
            min: 1024,
            max: 3072
        })
    );
    assert_eq!(quilt.jvm_args.as_deref(), Some("-Dx=y"));
    assert!(quilt.game_dir.ends_with("Quilt Test/instance"));
    // A single instance folder can be picked too.
    let single = detect::read_folder(&fixture(
        "gdlauncher-carbon/gdlauncher_carbon/data/instances/Quilt Test",
    ))
    .unwrap();
    assert_eq!(single[0].source, Source::GdLauncherCarbon);
    let single = detect::read_folder(&fixture("atlauncher/instances/ForgeOne")).unwrap();
    assert_eq!(single[0].source, Source::AtLauncher);
    assert!(
        detect::read_folder(&fixture("prism/PrismLauncher/instances/not an instance")).is_err()
    );
}

/// Each launcher in its default place under a made-up home folder.
#[test]
fn detects_launchers_in_their_default_places() {
    let home = tempfile::tempdir().unwrap();
    let support = home.path().join("Library/Application Support");
    copy_dir(&fixture("vanilla/minecraft"), &support.join("minecraft"));
    copy_dir(
        &fixture("prism/PrismLauncher"),
        &support.join("PrismLauncher"),
    );
    copy_dir(
        &fixture("modrinth-app/ModrinthApp"),
        &support.join("ModrinthApp"),
    );
    copy_dir(
        &fixture("curseforge/minecraft"),
        &home.path().join("Documents/curseforge/minecraft"),
    );
    copy_dir(
        &fixture("gdlauncher/gdlauncher_next"),
        &support.join("gdlauncher_next"),
    );
    copy_dir(
        &fixture("gdlauncher-carbon/gdlauncher_carbon"),
        &support.join("gdlauncher_carbon"),
    );
    let apps = home.path().join("Applications");
    copy_dir(&fixture("multimc/MultiMC.app"), &apps.join("MultiMC.app"));
    copy_dir(
        &fixture("atlauncher"),
        &apps.join("ATLauncher.app/Contents/Java"),
    );
    // Something that isn't a launcher's folder is not listed.
    fs::create_dir_all(support.join("PolyMC")).unwrap();
    let locations = Locations {
        os: Os::MacOs,
        home: home.path().to_path_buf(),
        config: support.clone(),
        app_data: support,
        documents: home.path().join("Documents"),
        applications: vec![apps],
    };
    let found = detect::detect(&locations);
    let sources: Vec<Source> = found.iter().map(|l| l.source).collect();
    assert_eq!(
        sources,
        [
            Source::Vanilla,
            Source::Prism,
            Source::ModrinthApp,
            Source::CurseForge,
            Source::GdLauncher,
            Source::GdLauncherCarbon,
            Source::MultiMc,
            Source::AtLauncher,
        ]
    );
    assert!(found.iter().all(|l| !l.instances.is_empty()));
    let empty = tempfile::tempdir().unwrap();
    assert!(
        detect::detect(&Locations {
            os: Os::MacOs,
            home: empty.path().to_path_buf(),
            app_data: empty.path().join("Library/Application Support"),
            config: empty.path().join("Library/Application Support"),
            documents: empty.path().join("Documents"),
            applications: Vec::new(),
        })
        .is_empty()
    );
}

/// The same on Windows: data in the roaming `AppData`, the official launcher's in
/// `.minecraft`, ATLauncher installed there, CurseForge's in the home folder, and the
/// Documents folder moved into OneDrive.
#[test]
fn detects_launchers_in_their_windows_places() {
    let home = tempfile::tempdir().unwrap();
    let roaming = home.path().join("AppData").join("Roaming");
    copy_dir(&fixture("vanilla/minecraft"), &roaming.join(".minecraft"));
    copy_dir(
        &fixture("prism/PrismLauncher"),
        &roaming.join("PrismLauncher"),
    );
    copy_dir(
        &fixture("modrinth-app/ModrinthApp"),
        &roaming.join("ModrinthApp"),
    );
    copy_dir(&fixture("atlauncher"), &roaming.join("ATLauncher"));
    copy_dir(
        &fixture("gdlauncher/gdlauncher_next"),
        &roaming.join("gdlauncher_next"),
    );
    let documents = home.path().join("OneDrive").join("Documents");
    copy_dir(
        &fixture("curseforge/minecraft"),
        &documents.join("curseforge").join("minecraft"),
    );
    let found = detect::detect(&Locations {
        os: Os::Windows,
        home: home.path().to_path_buf(),
        app_data: roaming.clone(),
        config: roaming.clone(),
        documents,
        applications: Vec::new(),
    });
    let sources: Vec<Source> = found.iter().map(|l| l.source).collect();
    assert_eq!(
        sources,
        [
            Source::Vanilla,
            Source::Prism,
            Source::ModrinthApp,
            Source::AtLauncher,
            Source::CurseForge,
            Source::GdLauncher,
        ]
    );
    assert!(found.iter().all(|l| !l.instances.is_empty()));
    assert_eq!(found[0].root, roaming.join(".minecraft"));
    // CurseForge's default on Windows, in the home folder.
    let home = tempfile::tempdir().unwrap();
    let instances = home.path().join("curseforge").join("minecraft");
    copy_dir(&fixture("curseforge/minecraft"), &instances);
    let found = detect::detect(&Locations {
        os: Os::Windows,
        home: home.path().to_path_buf(),
        app_data: home.path().join("AppData").join("Roaming"),
        config: home.path().join("AppData").join("Roaming"),
        documents: home.path().join("Documents"),
        applications: Vec::new(),
    });
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].root, instances.join("Instances"));
}

/// Linux's places, in the XDG folders and the home folder: the official launcher's
/// `~/.minecraft`, GDLauncher's in `~/.config` (Electron's `appData`), and the lowercase
/// folders the .deb packages use.
#[test]
fn detects_launchers_in_their_linux_places() {
    let home = tempfile::tempdir().unwrap();
    let share = home.path().join(".local/share");
    let config = home.path().join(".config");
    copy_dir(
        &fixture("vanilla/minecraft"),
        &home.path().join(".minecraft"),
    );
    copy_dir(
        &fixture("prism/PrismLauncher"),
        &share.join("PrismLauncher"),
    );
    copy_dir(&fixture("multimc/MultiMC.app/Data"), &share.join("multimc"));
    copy_dir(
        &fixture("modrinth-app/ModrinthApp"),
        &share.join("ModrinthApp"),
    );
    copy_dir(&fixture("atlauncher"), &share.join("atlauncher"));
    copy_dir(
        &fixture("curseforge/minecraft"),
        &home.path().join("curseforge/minecraft"),
    );
    copy_dir(
        &fixture("gdlauncher/gdlauncher_next"),
        &config.join("gdlauncher_next"),
    );
    copy_dir(
        &fixture("gdlauncher-carbon/gdlauncher_carbon"),
        &share.join("gdlauncher_carbon"),
    );
    // The macOS and Windows names don't count on Linux.
    copy_dir(&fixture("atlauncher"), &share.join("ATLauncher"));
    copy_dir(&fixture("vanilla/minecraft"), &share.join("minecraft"));
    let linux = |home: &Path| Locations {
        os: Os::Linux,
        home: home.to_path_buf(),
        app_data: home.join(".local/share"),
        config: home.join(".config"),
        documents: home.join("Documents"),
        applications: Vec::new(),
    };
    let found = detect::detect(&linux(home.path()));
    let roots: Vec<(Source, PathBuf)> = found
        .iter()
        .map(|l| (l.source, l.root.strip_prefix(home.path()).unwrap().into()))
        .collect();
    assert_eq!(
        roots,
        [
            (Source::Vanilla, ".minecraft".into()),
            (Source::Prism, ".local/share/PrismLauncher".into()),
            (Source::MultiMc, ".local/share/multimc".into()),
            (Source::ModrinthApp, ".local/share/ModrinthApp".into()),
            (Source::AtLauncher, ".local/share/atlauncher".into()),
            (Source::CurseForge, "curseforge/minecraft/Instances".into()),
            (Source::GdLauncher, ".config/gdlauncher_next".into()),
            (
                Source::GdLauncherCarbon,
                ".local/share/gdlauncher_carbon".into()
            ),
        ]
    );
    assert!(found.iter().all(|l| !l.instances.is_empty()));
    assert!(detect::detect(&linux(tempfile::tempdir().unwrap().path())).is_empty());
}

/// Flatpak installs, each in its own `~/.var/app/<app id>` folder.
#[test]
fn detects_flatpak_launchers_on_linux() {
    let home = tempfile::tempdir().unwrap();
    let app = |id: &str, path: &str| home.path().join(".var/app").join(id).join(path);
    copy_dir(
        &fixture("vanilla/minecraft"),
        &app("com.mojang.Minecraft", ".minecraft"),
    );
    copy_dir(
        &fixture("prism/PrismLauncher"),
        &app("org.prismlauncher.PrismLauncher", "data/PrismLauncher"),
    );
    copy_dir(
        &fixture("prism/PrismLauncher"),
        &app("org.polymc.PolyMC", "data/PolyMC"),
    );
    copy_dir(
        &fixture("modrinth-app/ModrinthApp"),
        &app("com.modrinth.ModrinthApp", "data/ModrinthApp"),
    );
    copy_dir(
        &fixture("atlauncher"),
        &app("com.atlauncher.ATLauncher", "data"),
    );
    copy_dir(
        &fixture("gdlauncher/gdlauncher_next"),
        &app("io.gdevs.GDLauncher", "config/gdlauncher_next"),
    );
    let found = detect::detect(&Locations {
        os: Os::Linux,
        home: home.path().to_path_buf(),
        app_data: home.path().join(".local/share"),
        config: home.path().join(".config"),
        documents: home.path().join("Documents"),
        applications: Vec::new(),
    });
    let roots: Vec<(Source, PathBuf)> = found
        .iter()
        .map(|l| {
            let root = l.root.strip_prefix(home.path().join(".var/app")).unwrap();
            (l.source, root.into())
        })
        .collect();
    assert_eq!(
        roots,
        [
            (Source::Vanilla, "com.mojang.Minecraft/.minecraft".into()),
            (
                Source::Prism,
                "org.prismlauncher.PrismLauncher/data/PrismLauncher".into()
            ),
            (Source::Prism, "org.polymc.PolyMC/data/PolyMC".into()),
            (
                Source::ModrinthApp,
                "com.modrinth.ModrinthApp/data/ModrinthApp".into()
            ),
            (Source::AtLauncher, "com.atlauncher.ATLauncher/data".into()),
            (
                Source::GdLauncher,
                "io.gdevs.GDLauncher/config/gdlauncher_next".into()
            ),
        ]
    );
    assert!(found.iter().all(|l| !l.instances.is_empty()));
}

fn request(found: &Found, game_version: &str, loader: Option<(LoaderKind, &str)>) -> ImportRequest {
    ImportRequest {
        found: found.clone(),
        resolved: Resolved {
            game_version: game_version.into(),
            loader: loader.map(
                |(kind, version)| launcher_core::game::loaders::LoaderChoice {
                    kind,
                    version: version.into(),
                },
            ),
            notes: vec!["a resolution note".into()],
        },
        options: CopyOptions::default(),
    }
}

fn files_under(dir: &Path) -> Vec<String> {
    let mut list: Vec<String> = snapshot(dir)
        .keys()
        // with `/` between the parts on every system
        .map(|p| {
            let parts: Vec<_> = p
                .strip_prefix(dir)
                .unwrap()
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect();
            parts.join("/")
        })
        .collect();
    list.sort();
    list
}

#[test]
fn imports_a_copy_without_caches_logs_or_launcher_files() {
    let root = fixture("vanilla/minecraft");
    let before = snapshot(&fixtures());
    let launcher = detect::read_root(Source::Vanilla, &root);
    let data = tempfile::tempdir().unwrap();
    let store = InstanceStore::new(data.path().join("instances"));
    let progress = ImportProgress::default();

    // The launcher's own folder: worlds and options, but no accounts, assets or caches.
    let latest = by_name(&launcher, "Latest release");
    let imported = block_on(import::import(
        &store,
        &request(latest, "26.3", None),
        None,
        &progress,
        &CancelToken::new(),
    ))
    .unwrap();
    let game = store.game_dir(&imported.instance.id);
    let files = files_under(&game);
    for kept in [
        "options.txt",
        "servers.dat",
        "saves/My World/level.dat",
        "config/example.toml",
        "resourcepacks/Faithful.zip",
        "screenshots/2026-01-01_12.00.00.png",
    ] {
        assert!(files.contains(&kept.to_string()), "{kept} in {files:?}");
    }
    for left_out in [
        "launcher_accounts.json",
        "launcher_msa_credentials.bin",
        "launcher_profiles.json",
        "launcher_log.txt",
        "logs",
        "crash-reports",
        ".cache",
        "Thumbs.db",
        "assets",
        "libraries",
        "versions",
        "webcache2",
    ] {
        assert!(
            !files
                .iter()
                .any(|f| f == left_out || f.starts_with(&format!("{left_out}/"))),
            "{left_out}"
        );
    }
    assert!(
        imported
            .skipped
            .iter()
            .any(|(p, _)| p == "launcher_accounts.json")
    );
    assert_eq!(imported.instance.name, "Latest release");
    assert_eq!(imported.instance.game_version, "26.3");
    // Marked as imported, so the sidebar lists it under Imported, and saved so.
    assert_eq!(
        imported.instance.imported_from.as_deref(),
        Some("Minecraft Launcher")
    );
    assert_eq!(
        store.get(&imported.instance.id).unwrap().imported_from,
        imported.instance.imported_from
    );
    assert!(imported.notes.iter().any(|n| n == "a resolution note"));
    assert_eq!(progress.stage(), import::Stage::Done);
    assert_eq!(progress.fraction(), Some(1.0));

    // A modded profile: loader, memory, arguments and icon carry over.
    let fabric = by_name(&launcher, "Fabric 1.20.1");
    let imported = block_on(import::import(
        &store,
        &request(fabric, "1.20.1", Some((LoaderKind::Fabric, "0.16.14"))),
        None,
        &ImportProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    let instance = store.get(&imported.instance.id).unwrap();
    assert_eq!(instance.loader, Loader::Fabric);
    assert_eq!(instance.loader_version.as_deref(), Some("0.16.14"));
    assert_eq!(
        instance.memory,
        Some(Memory {
            min: 1024,
            max: 4096
        })
    );
    assert_eq!(instance.jvm_args.as_deref(), Some("-XX:+UseG1GC"));
    assert_eq!(instance.icon.as_deref(), Some(CUSTOM_ICON));
    assert!(store.icon_path(&instance).is_some());
    assert!(
        store
            .game_dir(&instance.id)
            .join("mods/sodium-fabric-0.5.13.jar")
            .is_file()
    );
    // Without Modrinth the files are hashed but not matched yet.
    let provenance = Provenance::load(&store.dir(&instance.id));
    let entry = provenance
        .get(ContentKind::Mod, "sodium-fabric-0.5.13.jar")
        .unwrap();
    assert!(entry.modrinth.is_none() && !entry.looked_up);

    // Without worlds.
    let mut without = request(latest, "26.3", None);
    without.options.without_worlds = true;
    let imported = block_on(import::import(
        &store,
        &without,
        None,
        &ImportProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    assert!(!store.game_dir(&imported.instance.id).join("saves").exists());
    assert!(
        store
            .game_dir(&imported.instance.id)
            .join("options.txt")
            .exists()
    );

    assert_eq!(store.list().0.len(), 3);
    // The source is untouched.
    assert_eq!(snapshot(&fixtures()), before);
}

#[cfg(unix)]
#[test]
fn follows_links_only_inside_the_game_folder() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("game");
    let outside = dir.path().join("outside");
    fs::create_dir_all(game.join("shared/packs")).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(game.join("shared/packs/a.zip"), b"pack").unwrap();
    fs::write(outside.join("secret.txt"), b"secret").unwrap();
    fs::write(game.join("options.txt"), b"fov:1").unwrap();
    symlink(game.join("shared/packs"), game.join("resourcepacks")).unwrap();
    symlink(game.join("options.txt"), game.join("optionsof.txt")).unwrap();
    symlink(&outside, game.join("escape")).unwrap();
    symlink(outside.join("secret.txt"), game.join("secret.txt")).unwrap();
    symlink(&game, game.join("shared/loop")).unwrap();
    symlink("/etc/hosts", game.join("hosts")).unwrap();
    fs::write(game.join("back\\slash.txt"), b"x").unwrap();

    let plan = copy::plan(&game, &CopyOptions::default()).unwrap();
    let relative: Vec<String> = plan
        .files
        .iter()
        .map(|f| f.relative.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        relative,
        [
            "options.txt",
            "optionsof.txt",
            "resourcepacks/a.zip",
            "shared/packs/a.zip"
        ]
    );
    let skipped: Vec<&str> = plan.skipped.iter().map(|(p, _)| p.as_str()).collect();
    for path in [
        "escape",
        "secret.txt",
        "hosts",
        "shared/loop",
        "back\\slash.txt",
    ] {
        assert!(skipped.contains(&path), "{path} in {skipped:?}");
    }
    assert_eq!(plan.total_bytes, 4 + 5 + 4 + 5);

    let dest = dir.path().join("dest");
    copy::copy(
        &plan,
        &dest,
        &ImportProgress::default(),
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(fs::read(dest.join("resourcepacks/a.zip")).unwrap(), b"pack");
    assert!(
        !fs::symlink_metadata(dest.join("resourcepacks"))
            .unwrap()
            .is_symlink()
    );
    assert!(!dest.join("secret.txt").exists());
    assert!(!dest.join("escape").exists());
    // A planned file that would land on a link in the destination is refused.
    let dest2 = dir.path().join("dest2");
    fs::create_dir_all(&dest2).unwrap();
    symlink(&outside, dest2.join("resourcepacks")).unwrap();
    let err = copy::copy(
        &plan,
        &dest2,
        &ImportProgress::default(),
        &CancelToken::new(),
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("outside the new instance"),
        "{err}"
    );
    assert!(!outside.join("a.zip").exists());
}

fn leftovers(store: &InstanceStore) -> Vec<String> {
    fs::read_dir(store.root())
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_cancelled_or_failed_import_leaves_nothing() {
    let launcher = detect::read_root(Source::Prism, &fixture("prism/PrismLauncher"));
    let fabric = by_name(&launcher, "Fabric Pack, Deluxe");
    let data = tempfile::tempdir().unwrap();
    let store = InstanceStore::new(data.path().join("instances"));
    let cancel = CancelToken::new();
    cancel.cancel();
    let err = block_on(import::import(
        &store,
        &request(fabric, "1.20.1", None),
        None,
        &ImportProgress::default(),
        &cancel,
    ))
    .unwrap_err();
    assert!(matches!(err, ImportError::Cancelled));
    assert!(leftovers(&store).is_empty(), "{:?}", leftovers(&store));

    // A game folder that disappeared.
    let mut gone = fabric.clone();
    gone.game_dir = data.path().join("nowhere");
    assert!(
        block_on(import::import(
            &store,
            &request(&gone, "1.20.1", None),
            None,
            &ImportProgress::default(),
            &CancelToken::new(),
        ))
        .is_err()
    );
    assert!(leftovers(&store).is_empty());

    // A file that can't be read, in the middle of the copy.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let game = data.path().join("game");
        copy_dir(&fabric.game_dir, &game);
        let locked = game.join("config/example.toml");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        if fs::read(&locked).is_err() {
            let mut broken = fabric.clone();
            broken.game_dir = game.clone();
            let err = block_on(import::import(
                &store,
                &request(&broken, "1.20.1", None),
                None,
                &ImportProgress::default(),
                &CancelToken::new(),
            ))
            .unwrap_err();
            assert!(err.to_string().contains("example.toml"), "{err}");
            assert!(leftovers(&store).is_empty(), "{:?}", leftovers(&store));
        }
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).unwrap();
    }
    assert!(store.list().0.is_empty());
}

/// The copied mods, resource packs and shader packs are matched to Modrinth by SHA-1, so
/// the Content tab shows them with updates.
#[test]
fn matches_the_copied_content_to_modrinth() {
    let launcher = detect::read_root(Source::Prism, &fixture("prism/PrismLauncher"));
    let fabric = by_name(&launcher, "Fabric Pack, Deluxe");
    let sodium_bytes = fs::read(fabric.game_dir.join("mods/sodium-fabric-0.5.13.jar")).unwrap();
    let pack_bytes = fs::read(fabric.game_dir.join("resourcepacks/Faithful.zip")).unwrap();
    let mock = ModrinthMock::start();
    mock.project("AANobbMI", "sodium", "mod")
        .project("faithful", "faithful-32x", "resourcepack")
        .version(
            FakeVersion::new(
                "sodium-0513",
                "AANobbMI",
                "0.5.13",
                &["fabric"],
                &["1.20.1"],
                "2024-01-01T00:00:00Z",
            )
            .file("sodium-fabric-0.5.13.jar", &sodium_bytes),
        )
        .version(
            FakeVersion::new(
                "faithful-1",
                "faithful",
                "1.0",
                &["minecraft"],
                &["1.20.1"],
                "2024-01-01T00:00:00Z",
            )
            .file("Faithful.zip", &pack_bytes),
        );
    let api = mock.client();
    let data = tempfile::tempdir().unwrap();
    let store = InstanceStore::new(data.path().join("instances"));
    let imported = block_on(import::import(
        &store,
        &request(fabric, "1.20.1", Some((LoaderKind::Fabric, "0.16.14"))),
        Some(&api),
        &ImportProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    let provenance = Provenance::load(&store.dir(&imported.instance.id));
    let sodium = provenance
        .get(ContentKind::Mod, "sodium-fabric-0.5.13.jar")
        .unwrap();
    let source = sodium.modrinth.as_ref().unwrap();
    assert_eq!(source.project_id, "AANobbMI");
    assert_eq!(source.version_id, "sodium-0513");
    assert_eq!(source.title, "sodium title");
    let unknown = provenance.get(ContentKind::Mod, "unknown-mod.jar").unwrap();
    assert!(unknown.modrinth.is_none() && unknown.looked_up);
    let pack = provenance
        .get(ContentKind::ResourcePack, "Faithful.zip")
        .unwrap();
    assert_eq!(pack.modrinth.as_ref().unwrap().project_id, "faithful");
    assert!(
        provenance
            .get(ContentKind::ShaderPack, "BSL.zip")
            .unwrap()
            .looked_up
    );
    // One lookup per kind, by SHA-1.
    assert_eq!(mock.api_requests("POST", "/v2/version_files").len(), 3);

    // Offline: the import still works, and says the matching waits.
    let offline = launcher_core::content::Modrinth::with_urls(
        launcher_core::http::default_client().unwrap(),
        url::Url::parse("http://127.0.0.1:9/v2").unwrap(),
        None,
    );
    let imported = block_on(import::import(
        &store,
        &request(fabric, "1.20.1", Some((LoaderKind::Fabric, "0.16.14"))),
        Some(&offline),
        &ImportProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    assert!(imported.notes.iter().any(|n| n.contains("weren't matched")));
    assert_eq!(imported.instance.id, "fabric-pack-deluxe-2");
}
