//! Golden tests of the command line with each mod loader, on Apple Silicon and Intel
//! Macs, from the recorded loader profiles merged onto the recorded vanilla versions.
//!
//! The expected command lines are in `testdata/golden/loaders/`. After an intended
//! change, regenerate them with `UPDATE_GOLDEN=1 cargo test --test loader_args` and
//! review the diff.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use launcher_core::{
    Secret,
    game::{
        launch::{self, GameSettings, LaunchAccount, LaunchPaths},
        libraries,
        loaders::{
            self, LoaderChoice, LoaderKind,
            forge::{self, LegacyInstall},
            meta,
        },
        platform::{Arch, Os, Platform},
        rules::Features,
        runtime::{self, JavaSource, RuntimeIndex},
        version::VersionJson,
    },
    instances::{Instance, Memory},
    settings::InstanceDefaults,
};
use serde_json::Value;

const TOKEN: &str = "eyJhbGciOiJIUzI1NiJ9.SECRET-ACCESS-TOKEN.c2lnbmF0dXJl";

fn testdata(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join(path)
}

fn json(path: &str) -> Value {
    serde_json::from_slice(&std::fs::read(testdata(path)).unwrap()).unwrap()
}

fn vanilla(id: &str) -> VersionJson {
    VersionJson::parse(&std::fs::read(testdata(&format!("versions/{id}.json"))).unwrap()).unwrap()
}

/// Made-up SHA-1s for the jars a profile has none for; they are not on the command
/// line.
fn fake_hashes(urls: Vec<String>) -> HashMap<String, String> {
    urls.into_iter().map(|url| (url, "0".repeat(40))).collect()
}

/// The normalised profile of `kind` `version` for `game`, from the recorded files.
fn profile(kind: LoaderKind, version: &str, game: &str) -> VersionJson {
    let choice = LoaderChoice {
        kind,
        version: version.into(),
    };
    let id = loaders::profile_id(&choice, game).unwrap();
    let normalized = match kind {
        LoaderKind::Fabric | LoaderKind::Quilt => {
            let name = kind.label().to_lowercase();
            let raw = json(&format!("loaders/{name}/profile-{version}-{game}.json"));
            let hashes = fake_hashes(meta::missing_hashes(&raw).unwrap());
            meta::normalize(&raw, &id, &hashes).unwrap()
        }
        LoaderKind::Forge | LoaderKind::NeoForge => {
            let dir = match kind {
                LoaderKind::NeoForge => format!("loaders/neoforge/{version}"),
                _ => format!("loaders/forge/{version}"),
            };
            let install_profile = json(&format!("{dir}/install_profile.json"));
            let mut normalized = match (
                install_profile.get("install"),
                install_profile.get("versionInfo"),
            ) {
                (Some(install), Some(info)) => {
                    let install: LegacyInstall = serde_json::from_value(install.clone()).unwrap();
                    let mut info = info.clone();
                    for library in info["libraries"].as_array_mut().unwrap() {
                        library.as_object_mut().unwrap().remove("checksums");
                    }
                    let hashes = fake_hashes(forge::legacy_downloads(&info, &install).unwrap());
                    forge::normalize_legacy(&info, &install, &id, &hashes, b"universal").unwrap()
                }
                _ => forge::normalize_modern(&json(&format!("{dir}/version.json")), &id).unwrap(),
            };
            normalized["inheritsFrom"] = game.into();
            normalized
        }
    };
    VersionJson::parse(&serde_json::to_vec(&normalized).unwrap()).unwrap()
}

/// Where the installer puts the Java for `version` on `java`'s platform: Mojang's
/// runtime for its component, which on Apple silicon is its arm64 Java 17 for Java 16
/// and 17, or Azul's Zulu 8 for Java 8.
fn java_path(data: &Path, version: &VersionJson, java: &Platform) -> PathBuf {
    let index: RuntimeIndex =
        serde_json::from_slice(&std::fs::read(testdata("runtime/all.json")).unwrap()).unwrap();
    match runtime::select(&index, version.runtime_component(), java) {
        Ok(JavaSource::Mojang(choice)) => runtime::java_binary(
            &runtime::install_dir(&data.join("runtimes"), &choice).unwrap(),
            java.os,
        ),
        Ok(JavaSource::Zulu8) => {
            data.join("runtimes/zulu-8/mac-os-arm64/zulu-8.jre/Contents/Home/bin/java")
        }
        Err(err) => panic!("{}: {err}", version.id),
    }
}

fn account() -> LaunchAccount {
    LaunchAccount::microsoft(
        "Lodestoner",
        "8667ba71-b85a-4004-af54-457a9734eed7".parse().unwrap(),
        Secret::new(TOKEN),
        Some("2535412345678901".into()),
    )
}

/// The command line of the merged version, as the golden file has it: the program,
/// then one argument per line.
fn command(version: &VersionJson, machine: Arch, settings: &GameSettings) -> (Vec<String>, String) {
    let machine = Platform::new(Os::MacOs, machine, "14.6.1");
    let java = machine.with_arch(libraries::java_arch(version, &machine));
    let selection = libraries::select(version, &java, &Features::default()).unwrap();
    let data = Path::new("/data");
    let game_dir = data.join("instances/test/minecraft");
    let mut classpath: Vec<PathBuf> = selection
        .classpath
        .iter()
        .map(|file| data.join("libraries").join(&file.path))
        .collect();
    // A loader instance's client jar is under the loader's name.
    classpath.push(data.join(format!("versions/{0}/{0}.jar", version.id)));
    let paths = LaunchPaths {
        java: java_path(data, version, &java),
        game_dir: game_dir.clone(),
        natives_dir: data.join("instances/test/natives"),
        libraries_dir: data.join("libraries"),
        assets_dir: data.join("assets"),
        game_assets: data.join("assets/virtual").join(version.assets_id()),
        classpath,
        logging_config: version
            .client_logging()
            .map(|logging| data.join("assets/log_configs").join(&logging.file.id)),
    };
    let command = launch::build(version, &java, &paths, &account(), settings).unwrap();
    let text = std::iter::once(command.program.display().to_string())
        .chain(command.redacted())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    assert!(!text.contains("SECRET"), "{text}");
    // The command lines are for other systems, under a made-up /data; on Windows
    // its paths are joined with `\`.
    let args = command
        .args
        .iter()
        .map(|arg| arg.replace('\\', "/"))
        .collect();
    (args, text)
}

fn check_golden(name: &str, text: &str) {
    // The command lines are for other systems, under a made-up /data; on Windows
    // its paths are joined with `\`.
    let text = text.replace('\\', "/");
    let path = testdata(&format!("golden/loaders/{name}.txt"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &text).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("missing {}; run with UPDATE_GOLDEN=1", path.display()));
    assert_eq!(text, expected, "{name} differs from {}", path.display());
}

const CASES: &[(LoaderKind, &str, &str, &str)] = &[
    (LoaderKind::Fabric, "0.16.14", "1.20.1", "fabric-1.20.1"),
    (LoaderKind::Quilt, "0.29.2", "1.20.1", "quilt-1.20.1"),
    (
        LoaderKind::Forge,
        "1.20.1-47.4.28",
        "1.20.1",
        "forge-1.20.1",
    ),
    (
        LoaderKind::Forge,
        "1.21.1-52.1.21",
        "1.21.1",
        "forge-1.21.1",
    ),
    (
        LoaderKind::NeoForge,
        "21.1.209",
        "1.21.1",
        "neoforge-1.21.1",
    ),
    (
        LoaderKind::Forge,
        "1.16.5-36.2.42",
        "1.16.5",
        "forge-1.16.5",
    ),
    (
        LoaderKind::Forge,
        "1.12.2-14.23.5.2864",
        "1.12.2",
        "forge-1.12.2",
    ),
    (
        LoaderKind::Forge,
        "1.8.9-11.15.1.2318-1.8.9",
        "1.8.9",
        "forge-1.8.9",
    ),
    (
        LoaderKind::Forge,
        "1.7.10-10.13.4.1614-1.7.10",
        "1.7.10",
        "forge-1.7.10",
    ),
];

fn merged(kind: LoaderKind, version: &str, game: &str) -> VersionJson {
    loaders::merge(&profile(kind, version, game), &vanilla(game))
}

#[test]
fn golden_command_lines() {
    let settings = GameSettings {
        min_ram_mb: 512,
        max_ram_mb: 4096,
        ..GameSettings::default()
    };
    for (kind, version, game, name) in CASES {
        let merged = merged(*kind, version, game);
        for (arch, arch_name) in [(Arch::Arm64, "arm64"), (Arch::X86_64, "x64")] {
            let (args, text) = command(&merged, arch, &settings);
            check_golden(&format!("{name}-{arch_name}"), &text);
            assert_eq!(
                args.iter().filter(|a| a.contains(TOKEN)).count(),
                1,
                "{name}"
            );
        }
    }
}

#[test]
fn loader_specifics() {
    let settings = GameSettings::default();
    let arg_after = |args: &[String], flag: &str| -> String {
        let at = args
            .iter()
            .position(|a| a == flag)
            .unwrap_or_else(|| panic!("no {flag}"));
        args[at + 1].clone()
    };

    // Fabric: Knot as the main class, its JVM argument, vanilla's LWJGL 3 natives.
    let (args, _) = command(
        &merged(LoaderKind::Fabric, "0.16.14", "1.20.1"),
        Arch::Arm64,
        &settings,
    );
    assert!(args.contains(&"net.fabricmc.loader.impl.launch.knot.KnotClient".to_string()));
    assert!(args.contains(&"-DFabricMcEmu= net.minecraft.client.main.Main ".to_string()));
    assert!(arg_after(&args, "-cp").contains("fabric-loader-0.16.14.jar"));
    assert_eq!(
        args.iter().filter(|a| *a == "-XstartOnFirstThread").count(),
        1
    );

    // Forge 1.20.1: the module path, and the client jar named so the ignore list
    // matches it.
    let (args, _) = command(
        &merged(LoaderKind::Forge, "1.20.1-47.4.28", "1.20.1"),
        Arch::Arm64,
        &settings,
    );
    let ignore = args
        .iter()
        .find(|a| a.starts_with("-DignoreList="))
        .unwrap();
    assert!(ignore.ends_with(",1.20.1-forge-47.4.28.jar"), "{ignore}");
    assert!(
        arg_after(&args, "-cp")
            .ends_with("/data/versions/1.20.1-forge-47.4.28/1.20.1-forge-47.4.28.jar")
    );
    assert!(
        arg_after(&args, "-p").starts_with("/data/libraries/cpw/mods/bootstraplauncher/1.1.2/")
    );
    assert!(args.contains(&"cpw.mods.bootstraplauncher.BootstrapLauncher".to_string()));
    assert_eq!(arg_after(&args, "--launchTarget"), "forgeclient");
    // Mojang's patched Log4j config stays.
    assert!(
        args.iter()
            .any(|a| a == "-Dlog4j.configurationFile=/data/assets/log_configs/client-1.12.xml")
    );

    // NeoForge on 1.21.1.
    let (args, _) = command(
        &merged(LoaderKind::NeoForge, "21.1.209", "1.21.1"),
        Arch::Arm64,
        &settings,
    );
    assert_eq!(arg_after(&args, "--fml.neoForgeVersion"), "21.1.209");
    assert!(
        args.iter()
            .any(|a| a == "-DignoreList=client-extra,neoforge-21.1.209.jar")
    );

    // Forge 1.16.5 on Apple silicon runs natively: Azul's Zulu 8, and Mojang's arm64
    // LWJGL 3.3.1 in place of vanilla's Intel-only 3.2.1, swapped after the merge. On an
    // Intel Mac it keeps Mojang's Java 8 and LWJGL 3.2.1.
    let forge = merged(LoaderKind::Forge, "1.16.5-36.2.42", "1.16.5");
    let (args, text) = command(&forge, Arch::Arm64, &settings);
    assert_eq!(arg_after(&args, "--launchTarget"), "fmlclient");
    assert!(
        text.starts_with("/data/runtimes/zulu-8/mac-os-arm64/zulu-8.jre/Contents/Home/bin/java\n"),
        "{text}"
    );
    let classpath = arg_after(&args, "-cp");
    assert!(
        classpath.contains("/org/lwjgl/lwjgl-glfw/3.3.1/lwjgl-glfw-3.3.1-natives-macos-arm64.jar")
    );
    assert!(!classpath.contains("/3.2.1/"));
    assert!(classpath.contains("/java-objc-bridge-1.1.jar"));
    assert!(classpath.contains("/fmlloader-1.16.5-36.2.42.jar") || classpath.contains("forge"));
    let (args, text) = command(&forge, Arch::X86_64, &settings);
    assert!(
        text.starts_with("/data/runtimes/jre-legacy/mac-os/"),
        "{text}"
    );
    assert!(arg_after(&args, "-cp").contains("/org/lwjgl/lwjgl-glfw/3.2.1/"));
    assert!(!arg_after(&args, "-cp").contains("3.3.1"));

    // Forge 1.12.2: launchwrapper with FML's tweaker, Log4j 2.15 instead of
    // vanilla's 2.8.1, and an Intel Java on Apple Silicon.
    let forge = merged(LoaderKind::Forge, "1.12.2-14.23.5.2864", "1.12.2");
    let (args, text) = command(&forge, Arch::Arm64, &settings);
    assert_eq!(
        arg_after(&args, "--tweakClass"),
        "net.minecraftforge.fml.common.launcher.FMLTweaker"
    );
    assert!(text.contains("log4j-core-2.15.0.jar") && !text.contains("log4j-core-2.8.1.jar"));
    assert!(text.contains("/mac-os/"), "x86_64 runtime under Rosetta");
    assert!(
        !args.contains(&"-XstartOnFirstThread".to_string()),
        "LWJGL 2"
    );

    // Forge 1.8.9 and 1.7.10: the universal jar first on the classpath.
    for (version, game) in [
        ("1.8.9-11.15.1.2318-1.8.9", "1.8.9"),
        ("1.7.10-10.13.4.1614-1.7.10", "1.7.10"),
    ] {
        let (args, _) = command(
            &merged(LoaderKind::Forge, version, game),
            Arch::X86_64,
            &settings,
        );
        assert!(arg_after(&args, "-cp").starts_with(&format!(
            "/data/libraries/net/minecraftforge/forge/{version}/forge-{version}.jar:"
        )));
        assert_eq!(arg_after(&args, "--assetIndex"), vanilla(game).assets_id());
    }
}

/// `-Xms`/`-Xmx` come from the global settings until the instance overrides its
/// memory.
#[test]
fn memory_is_inherited_or_overridden() {
    let defaults = InstanceDefaults {
        min_ram: Some(1024),
        max_ram: Some(6144),
        ..InstanceDefaults::default()
    };
    let version = merged(LoaderKind::Fabric, "0.16.14", "1.20.1");
    let memory = |instance: &Instance| {
        let settings = instance.game_settings(&defaults, Some(16 << 30));
        let (args, _) = command(&version, Arch::Arm64, &settings);
        args.into_iter()
            .filter(|a| a.starts_with("-Xms") || a.starts_with("-Xmx"))
            .collect::<Vec<_>>()
    };
    assert_eq!(memory(&Instance::default()), ["-Xms1024M", "-Xmx6144M"]);
    let custom = Instance {
        memory: Some(Memory {
            min: 2048,
            max: 3072,
        }),
        ..Instance::default()
    };
    assert_eq!(memory(&custom), ["-Xms2048M", "-Xmx3072M"]);
    // Without global memory: 512 MB and half the Mac's memory, at most 4 GB.
    let settings = Instance::default().game_settings(&InstanceDefaults::default(), Some(6 << 30));
    let (args, _) = command(&version, Arch::Arm64, &settings);
    assert!(args.contains(&"-Xms512M".to_string()) && args.contains(&"-Xmx3072M".to_string()));
}
