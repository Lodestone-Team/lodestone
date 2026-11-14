//! Golden tests of the command line for each recorded version, on Apple Silicon and
//! Intel Macs, with the access token redacted.
//!
//! The expected command lines are in `testdata/golden/`. After an intended change,
//! regenerate them with `UPDATE_GOLDEN=1 cargo test --test launch_args` and review
//! the diff.

use std::path::{Path, PathBuf};

use launcher_core::{
    Secret,
    accounts::Account,
    game::{
        launch::{
            self, GameSettings, JoinArguments, JoinServer, LaunchAccount, LaunchError, LaunchPaths,
        },
        libraries,
        platform::{Arch, Os, Platform},
        rules::Features,
        runtime::{self, JavaSource, RuntimeIndex},
        version::VersionJson,
    },
};

const TOKEN: &str = "eyJhbGciOiJIUzI1NiJ9.SECRET-ACCESS-TOKEN.c2lnbmF0dXJl";

fn testdata(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join(path)
}

fn version(id: &str) -> VersionJson {
    VersionJson::parse(&std::fs::read(testdata(&format!("versions/{id}.json"))).unwrap()).unwrap()
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
        // Each Zulu build has a folder of its own.
        Ok(JavaSource::Zulu8) => data.join(
            "runtimes/zulu-8/mac-os-arm64/zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64/zulu-8.jre/Contents/Home/bin/java",
        ),
        Err(err) => panic!("{}: {err}", version.id),
    }
}

fn microsoft() -> LaunchAccount {
    LaunchAccount::microsoft(
        "Lodestoner",
        "8667ba71-b85a-4004-af54-457a9734eed7".parse().unwrap(),
        Secret::new(TOKEN),
        Some("2535412345678901".into()),
    )
}

/// The command line for `version` on a Mac with `machine` as its CPU, as the golden
/// file has it: the program, then one argument per line.
fn command(
    version: &VersionJson,
    machine: Arch,
    account: &LaunchAccount,
    settings: &GameSettings,
) -> (Vec<String>, String) {
    command_joining(version, machine, account, settings, None)
}

/// [`command`], joining `join` at startup.
fn command_joining(
    version: &VersionJson,
    machine: Arch,
    account: &LaunchAccount,
    settings: &GameSettings,
    join: Option<&JoinServer>,
) -> (Vec<String>, String) {
    let machine = Platform::new(Os::MacOs, machine, "14.6.1");
    let java = machine.with_arch(libraries::java_arch(version, &machine));
    let selection =
        libraries::select_on(version, &java, machine.arch, &Features::default()).unwrap();
    let data = Path::new("/data");
    let game_dir = data.join("instances/test/minecraft");
    let mut classpath: Vec<PathBuf> = selection
        .classpath
        .iter()
        .map(|file| data.join("libraries").join(&file.path))
        .collect();
    classpath.push(data.join(format!("versions/{0}/{0}.jar", version.id)));
    let legacy = match version.assets_id() {
        "pre-1.6" => game_dir.join("resources"),
        id => data.join("assets/virtual").join(id),
    };
    let paths = LaunchPaths {
        java: java_path(data, version, &java),
        game_dir: game_dir.clone(),
        natives_dir: data.join("instances/test/natives"),
        libraries_dir: data.join("libraries"),
        assets_dir: data.join("assets"),
        game_assets: legacy,
        classpath,
        logging_config: version
            .client_logging()
            .map(|logging| data.join("assets/log_configs").join(&logging.file.id)),
    };
    let command = launch::build_joining(version, &java, &paths, account, settings, join).unwrap();
    assert_eq!(command.cwd, game_dir);
    let redacted = command.redacted();
    let text = std::iter::once(command.program.display().to_string())
        .chain(redacted.iter().cloned())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    // The token is only in the real arguments, and never in the redacted ones or
    // the Debug output.
    assert!(!text.contains("SECRET"), "{text}");
    assert!(!format!("{command:?}").contains("SECRET"));
    assert!(!command.display().contains("SECRET"));
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
    let path = testdata(&format!("golden/{name}.txt"));
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &text).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("missing {}; run with UPDATE_GOLDEN=1", path.display()));
    assert_eq!(text, expected, "{name} differs from {}", path.display());
}

fn arch_name(arch: Arch) -> &'static str {
    match arch {
        Arch::Arm64 => "arm64",
        _ => "x64",
    }
}

#[test]
fn golden_command_lines() {
    let settings = GameSettings {
        min_ram_mb: 512,
        max_ram_mb: 4096,
        ..GameSettings::default()
    };
    for id in [
        "1.8.9", "1.12.2", "1.13.2", "1.16.5", "1.17.1", "1.18.2", "1.20.1", "26.3",
    ] {
        let version = version(id);
        for arch in [Arch::Arm64, Arch::X86_64] {
            let (args, text) = command(&version, arch, &microsoft(), &settings);
            check_golden(&format!("{id}-{}", arch_name(arch)), &text);
            assert_eq!(args.iter().filter(|a| a.contains(TOKEN)).count(), 1, "{id}");
        }
    }
}

#[test]
fn macos_first_thread_follows_lwjgl() {
    let settings = GameSettings::default();
    for (id, wanted) in [
        ("1.8.9", false),
        ("1.12.2", false),
        ("1.16.5", true),
        ("1.20.1", true),
        ("26.3", true),
    ] {
        let (args, _) = command(&version(id), Arch::Arm64, &microsoft(), &settings);
        assert_eq!(
            args.iter().filter(|a| *a == "-XstartOnFirstThread").count(),
            usize::from(wanted),
            "{id}"
        );
    }
    // A file without it gets it, if it uses LWJGL 3.
    let mut stripped = version("1.20.1");
    if let Some(arguments) = &mut stripped.arguments {
        arguments.jvm.retain(|arg| !matches!(arg, launcher_core::game::version::Argument::Conditional { value, .. } if value.values().contains(&"-XstartOnFirstThread".to_string())));
    }
    let (args, _) = command(&stripped, Arch::Arm64, &microsoft(), &settings);
    assert_eq!(args[0], "-XstartOnFirstThread");
}

#[test]
fn custom_settings_and_an_offline_account() {
    let offline = LaunchAccount::offline(&Account::offline("Dev_Steve").unwrap());
    let settings = GameSettings {
        min_ram_mb: 1024,
        max_ram_mb: 3072,
        jvm_args: vec!["-XX:+UseG1GC".into()],
        game_args: vec!["--server".into(), "localhost".into()],
        resolution: Some((1280, 720)),
        fullscreen: true,
        env: vec![("MESA_DEBUG".into(), "1".into())],
    };
    let (_, text) = command(&version("1.20.1"), Arch::Arm64, &offline, &settings);
    check_golden("1.20.1-arm64-offline-custom", &text);
    let (args, _) = command(&version("1.8.9"), Arch::Arm64, &offline, &settings);
    let joined = args.join(" ");
    assert!(
        joined.contains("--width 1280 --height 720 --fullscreen"),
        "{joined}"
    );
    assert!(joined.contains("--userType legacy"));
    assert!(joined.contains("--accessToken offline"));
    assert!(joined.ends_with("--server localhost"));
    // The user's JVM arguments replace Mojang's suggested ones on 26.3.
    let (args, _) = command(&version("26.3"), Arch::Arm64, &offline, &settings);
    assert!(!args.contains(&"-XX:+UseZGC".to_string()));
    let (args, _) = command(
        &version("26.3"),
        Arch::Arm64,
        &offline,
        &GameSettings::default(),
    );
    assert!(args.contains(&"-XX:+UseZGC".to_string()));
    assert!(!args.iter().any(|a| a == "-Xmx4G" || a == "-Xms2G"));
}

#[test]
fn pre_1_6_versions_use_the_session_and_resources() {
    let (args, text) = command(
        &version("1.5.2"),
        Arch::Arm64,
        &microsoft(),
        &GameSettings::default(),
    );
    check_golden("1.5.2-arm64", &text);
    let joined = args.join(" ");
    assert!(joined.contains(&format!("token:{TOKEN}:8667ba71b85a4004af54457a9734eed7")));
    assert!(joined.contains("--assetsDir /data/instances/test/minecraft/resources"));
}

#[test]
fn log4j_versions_need_their_logging_config() {
    let version = version("1.12.2");
    let machine = Platform::new(Os::MacOs, Arch::X86_64, "14.6.1");
    let paths = LaunchPaths {
        java: "/java".into(),
        game_dir: "/game".into(),
        natives_dir: "/natives".into(),
        libraries_dir: "/libraries".into(),
        assets_dir: "/assets".into(),
        game_assets: "/assets/virtual/1.12".into(),
        classpath: vec![],
        logging_config: None,
    };
    let err = launch::build(
        &version,
        &machine,
        &paths,
        &microsoft(),
        &GameSettings::default(),
    )
    .unwrap_err();
    assert_eq!(err, LaunchError::NoLoggingConfig("1.12.2".into()));
}

/// The position of `needle` (a run of arguments) in `args`.
fn find_run(args: &[String], needle: &[&str]) -> Option<usize> {
    args.windows(needle.len())
        .position(|window| window.iter().zip(needle).all(|(a, b)| a == b))
}

#[test]
fn joins_a_server_with_the_arguments_each_version_reads() {
    let server = JoinServer {
        host: "play.example.com".into(),
        port: 25570,
    };
    let settings = GameSettings::default();
    for (id, expected) in [
        ("1.5.2", JoinArguments::Unsupported),
        ("1.7.10", JoinArguments::ServerAndPort),
        ("1.8.9", JoinArguments::ServerAndPort),
        ("1.12.2", JoinArguments::ServerAndPort),
        ("1.16.5", JoinArguments::ServerAndPort),
        ("1.20.1", JoinArguments::QuickPlay),
        ("1.21.1", JoinArguments::QuickPlay),
        ("26.3", JoinArguments::QuickPlay),
    ] {
        let version = version(id);
        assert_eq!(launch::join_arguments(&version), expected, "{id}");
        let (plain, _) = command(&version, Arch::Arm64, &microsoft(), &settings);
        let (args, _) = command_joining(
            &version,
            Arch::Arm64,
            &microsoft(),
            &settings,
            Some(&server),
        );
        let quick_play = find_run(&args, &["--quickPlayMultiplayer", "play.example.com:25570"]);
        let server_port = find_run(&args, &["--server", "play.example.com", "--port", "25570"]);
        match expected {
            JoinArguments::QuickPlay => {
                assert!(quick_play.is_some(), "{id}: {args:?}");
                assert!(server_port.is_none(), "{id}");
                // only the multiplayer Quick Play argument, nothing else of the feature set
                assert!(!args.iter().any(|a| a == "--quickPlaySingleplayer"), "{id}");
                assert!(!args.iter().any(|a| a == "--quickPlayRealms"), "{id}");
                assert_eq!(args.len(), plain.len() + 2, "{id}");
            }
            JoinArguments::ServerAndPort => {
                assert!(server_port.is_some(), "{id}: {args:?}");
                assert!(quick_play.is_none(), "{id}");
                // after the game's own arguments, before the user's
                assert_eq!(args[..plain.len()], plain[..], "{id}");
                assert_eq!(args.len(), plain.len() + 4, "{id}");
            }
            JoinArguments::Unsupported => assert_eq!(args, plain, "{id}"),
        }
        // the token is still in exactly one argument
        assert_eq!(args.iter().filter(|a| a.contains(TOKEN)).count(), 1, "{id}");
    }

    // the user's own game arguments still come last
    let custom = GameSettings {
        game_args: vec!["--demo".into()],
        ..GameSettings::default()
    };
    let (args, _) = command_joining(
        &version("1.16.5"),
        Arch::Arm64,
        &microsoft(),
        &custom,
        Some(&server),
    );
    assert_eq!(args.last().map(String::as_str), Some("--demo"));
}

#[test]
fn join_addresses_bracket_ipv6() {
    let v4 = JoinServer {
        host: "192.168.1.20".into(),
        port: 25565,
    };
    assert_eq!(v4.address(), "192.168.1.20:25565");
    let v6 = JoinServer {
        host: "fe80::1".into(),
        port: 25565,
    };
    assert_eq!(v6.address(), "[fe80::1]:25565");
    let (args, _) = command_joining(
        &version("1.21.1"),
        Arch::Arm64,
        &microsoft(),
        &GameSettings::default(),
        Some(&v6),
    );
    assert!(find_run(&args, &["--quickPlayMultiplayer", "[fe80::1]:25565"]).is_some());
}
