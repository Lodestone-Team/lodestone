//! Installing a made-up version from a loopback file server, then starting it with a
//! stand-in `java` (a shell script) and reading its output.
//!
//! The version, its libraries, assets, logging config and Java runtime are small
//! files made here; their URLs are Mojang's, served from paths on the mock server
//! through [`MojangUrls::mirror`]. Nothing is fetched from the internet and no real
//! Java or Minecraft runs.

mod files;

use std::{fs, io::Write as _, path::Path, time::Duration};

use files::{Entry, FileServer};
use futures::executor::block_on;
// Starting the game runs a shell-script `java`, so those tests are Unix only.
#[cfg(unix)]
use futures::StreamExt as _;
#[cfg(unix)]
use launcher_core::{
    Secret,
    game::{
        crash::{self, Outcome},
        launch::{self, GameSettings, LaunchAccount},
        log4j::{Level, Redactor},
        process::{GameEvent, GameProcess},
    },
};
use launcher_core::{
    game::{
        download::{CancelToken, Downloader, Verify, recording_sleep, sha1_hex},
        install::{InstallError, InstallProgress, Installer, JavaChoice, Rosetta, Stage},
        mojang::MojangUrls,
        platform::{Arch, Os, Platform},
    },
    http,
    paths::DataDir,
};
use serde_json::{Value, json};

#[cfg(unix)]
const TOKEN: &str = "eyJhbGciOiJIUzI1NiJ9.SECRET-TOKEN-FOR-TESTS.c2ln";

/// What the made-up `java` does: prints two Log4j events (one echoing the access
/// token, as old versions did), a plain stderr line, writes a crash report, and
/// exits with code 1. It never reads anything else.
const FAKE_JAVA: &str = r#"#!/bin/sh
token=""
while [ $# -gt 0 ]; do
  if [ "$1" = "--accessToken" ]; then token="$2"; fi
  shift
done
echo '<log4j:Event logger="net.minecraft.client.Minecraft" timestamp="1700000000000" level="INFO" thread="Render thread">'
echo '  <log4j:Message><![CDATA[Setting user: Lodestoner]]></log4j:Message>'
echo '</log4j:Event>'
echo "<log4j:Event logger=\"auth\" timestamp=\"1700000000001\" level=\"WARN\" thread=\"main\"><log4j:Message><![CDATA[Session token: $token]]></log4j:Message></log4j:Event>"
echo 'Picked up a plain stderr line' >&2
mkdir -p crash-reports
printf -- '---- Minecraft Crash Report ----\nDescription: Initializing game\n\njava.lang.OutOfMemoryError: Java heap space\n' > crash-reports/crash-2026-11-07_12.00.00-client.txt
exit 1
"#;

struct Fixture {
    server: FileServer,
    data: tempfile::TempDir,
    version_sha1: String,
}

fn mojang(path: &str) -> String {
    format!("https://piston-data.mojang.com/{path}")
}

/// A jar with a native library and a META-INF entry to exclude.
fn natives_jar() -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut bytes);
        writer
            .start_file(
                "META-INF/MANIFEST.MF",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(b"Manifest-Version: 1.0").unwrap();
        writer
            .start_file("liblwjgl.dylib", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"native code").unwrap();
        writer.finish().unwrap();
    }
    bytes.into_inner()
}

/// Serves a version `test-1.0` that needs `jre-legacy`, which, as with Mojang's
/// real manifest, only has an x86_64 build for macOS.
fn fixture() -> Fixture {
    let server = FileServer::start();
    let serve = |url: &str, body: &[u8]| {
        server.serve(url, Entry::ok(body));
        json!({"url": url, "sha1": sha1_hex(body), "size": body.len()})
    };

    let client = serve(&mojang("v1/objects/client/client.jar"), b"client jar");
    let library = serve(
        "https://libraries.minecraft.net/com/example/lib/1.0/lib-1.0.jar",
        b"library jar",
    );
    let natives = serve(
        "https://libraries.minecraft.net/org/lwjgl/lwjgl/lwjgl-platform/2.9.2/lwjgl-platform-2.9.2-natives-osx.jar",
        &natives_jar(),
    );
    let logging = serve(
        &mojang("v1/objects/log/client-1.12.xml"),
        b"<Configuration/>",
    );
    let sound = b"ogg vorbis bytes";
    let icon = b"png bytes";
    let asset_url = |body: &[u8]| {
        let hash = sha1_hex(body);
        let url = format!(
            "https://resources.download.minecraft.net/{}/{hash}",
            &hash[..2]
        );
        server.serve(&url, Entry::ok(body));
        hash
    };
    let index = json!({
        "virtual": true,
        "objects": {
            "sound/step/grass1.ogg": {"hash": asset_url(sound), "size": sound.len()},
            "icons/icon_16x16.png": {"hash": asset_url(icon), "size": icon.len()},
        }
    })
    .to_string();
    let index_ref = serve(&mojang("v1/packages/legacy.json"), index.as_bytes());

    let version = json!({
        "id": "test-1.0",
        "type": "release",
        "mainClass": "net.minecraft.client.main.Main",
        "minecraftArguments": "--username ${auth_player_name} --version ${version_name} --gameDir ${game_directory} --assetsDir ${game_assets} --uuid ${auth_uuid} --accessToken ${auth_access_token} --userType ${user_type}",
        "assets": "legacy",
        "assetIndex": {"id": "legacy", "sha1": index_ref["sha1"], "size": index_ref["size"], "totalSize": 25, "url": index_ref["url"]},
        "downloads": {"client": client},
        "javaVersion": {"component": "jre-legacy", "majorVersion": 8},
        "logging": {"client": {
            "argument": "-Dlog4j.configurationFile=${path}",
            "file": {"id": "client-1.12.xml", "sha1": logging["sha1"], "size": logging["size"], "url": logging["url"]},
            "type": "log4j2-xml"
        }},
        "libraries": [
            {"name": "org.apache.logging.log4j:log4j-core:2.17.1", "downloads": {"artifact": {
                "path": "com/example/lib/1.0/lib-1.0.jar", "sha1": library["sha1"], "size": library["size"], "url": library["url"]}}},
            {"name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.2",
             "natives": {"osx": "natives-osx", "linux": "natives-linux"},
             "extract": {"exclude": ["META-INF/"]},
             "downloads": {"classifiers": {"natives-osx": {
                "path": "org/lwjgl/lwjgl/lwjgl-platform/2.9.2/lwjgl-platform-2.9.2-natives-osx.jar",
                "sha1": natives["sha1"], "size": natives["size"], "url": natives["url"]}}}},
            {"name": "com.example:windows-only:1.0", "rules": [{"action": "allow", "os": {"name": "windows"}}],
             "downloads": {"artifact": {"path": "com/example/windows-only/1.0/windows-only-1.0.jar", "sha1": "0000000000000000000000000000000000000000", "size": 1,
                "url": "https://libraries.minecraft.net/com/example/windows-only/1.0/windows-only-1.0.jar"}}}
        ]
    })
    .to_string();
    let version_url = "https://piston-meta.mojang.com/v1/packages/abc/test-1.0.json";
    server.serve(version_url, Entry::ok(version.as_bytes()));
    let version_sha1 = sha1_hex(version.as_bytes());
    // `test-arm64`: the same, with arm64 natives, so on Apple silicon it runs in an
    // arm64 Java 8, which Mojang doesn't make: Azul's Zulu 8.
    let mut arm64_version: Value = serde_json::from_str(&version).unwrap();
    arm64_version["id"] = "test-arm64".into();
    arm64_version["libraries"].as_array_mut().unwrap().push(json!(
        {"name": "com.example:natives:1.0:natives-macos-arm64", "rules": [{"action": "allow", "os": {"name": "osx"}}],
         "downloads": {"artifact": {"path": "com/example/natives/1.0/natives-1.0-natives-macos-arm64.jar",
            "sha1": library["sha1"], "size": library["size"], "url": library["url"]}}}
    ));
    // `test-old-forge`: the same, with ModLauncher 8.0 as Forge for 1.16.5 had it
    // before 36.2.26, which can't start in Java 8u321 or newer.
    let mut old_forge: Value = arm64_version.clone();
    old_forge["id"] = "test-old-forge".into();
    old_forge["libraries"].as_array_mut().unwrap().push(json!(
        {"name": "cpw.mods:modlauncher:8.0.9",
         "downloads": {"artifact": {"path": "cpw/mods/modlauncher/8.0.9/modlauncher-8.0.9.jar",
            "sha1": library["sha1"], "size": library["size"], "url": library["url"]}}}
    ));
    let old_forge = old_forge.to_string();
    let old_forge_url = "https://piston-meta.mojang.com/v1/packages/fed/test-old-forge.json";
    server.serve(old_forge_url, Entry::ok(old_forge.as_bytes()));
    let arm64_version = arm64_version.to_string();
    let arm64_url = "https://piston-meta.mojang.com/v1/packages/def/test-arm64.json";
    server.serve(arm64_url, Entry::ok(arm64_version.as_bytes()));
    let manifest = json!({
        "latest": {"release": "test-1.0", "snapshot": "test-1.0"},
        "versions": [{"id": "test-1.0", "type": "release", "url": version_url, "sha1": version_sha1,
                      "time": "2026-01-01T00:00:00+00:00", "releaseTime": "2026-01-01T00:00:00+00:00"},
                     {"id": "test-arm64", "type": "release", "url": arm64_url, "sha1": sha1_hex(arm64_version.as_bytes()),
                      "time": "2026-01-01T00:00:00+00:00", "releaseTime": "2026-01-01T00:00:00+00:00"},
                     {"id": "test-old-forge", "type": "release", "url": old_forge_url, "sha1": sha1_hex(old_forge.as_bytes()),
                      "time": "2026-01-01T00:00:00+00:00", "releaseTime": "2026-01-01T00:00:00+00:00"}]
    });
    server.serve(
        "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
        Entry::ok(manifest.to_string()),
    );

    // The Java runtime: a folder, the stand-in java, a license and a link to it.
    let java = serve(&mojang("v1/objects/rt/java"), FAKE_JAVA.as_bytes());
    let license = serve(
        &mojang("v1/objects/rt/LICENSE"),
        b"GPLv2 with the Classpath Exception",
    );
    let runtime_manifest = json!({"files": {
        "jre.bundle": {"type": "directory"},
        "jre.bundle/Contents/Home/bin": {"type": "directory"},
        "jre.bundle/Contents/Home/bin/java": {"type": "file", "executable": true, "downloads": {
            "raw": java, "lzma": {"url": mojang("v1/objects/rt/java.lzma"), "sha1": "x", "size": 1}}},
        "jre.bundle/Contents/Home/legal/java.base/LICENSE": {"type": "file", "executable": false, "downloads": {"raw": license}},
        "jre.bundle/Contents/Home/legal/java.desktop/LICENSE": {"type": "link", "target": "../java.base/LICENSE"}
    }})
    .to_string();
    let runtime_manifest_ref = serve(
        &mojang("v1/packages/rt/manifest.json"),
        runtime_manifest.as_bytes(),
    );
    let entry = json!([{"manifest": runtime_manifest_ref, "version": {"name": "8u74", "released": "2016-01-01T00:00:00+00:00"}}]);
    let all = json!({
        "mac-os": {"jre-legacy": entry, "java-runtime-gamma": []},
        "mac-os-arm64": {"jre-legacy": [], "java-runtime-gamma": []}
    });
    server.serve(
        "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json",
        Entry::ok(all.to_string()),
    );

    // Azul's list of Zulu 8 builds for Apple silicon, and the build: a zip with the
    // Java home inside `zulu-8.jre` and a link to its `bin` at the top, as Azul's are.
    let zulu = zulu_zip();
    server.serve(ZULU_URL, Entry::ok(zulu.clone()));
    server.serve(ZULU_LIST, Entry::ok(zulu_list(&zulu)));

    Fixture {
        server,
        data: tempfile::tempdir().unwrap(),
        version_sha1,
    }
}

const ZULU_NAME: &str = "zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64.zip";
const ZULU_URL: &str =
    "https://cdn.azul.com/zulu/bin/zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64.zip";
const ZULU_LIST: &str = "https://api.azul.com/metadata/v1/zulu/packages/";

fn zulu_zip() -> Vec<u8> {
    zulu_zip_named("zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64")
}

fn zulu_zip_named(top: &str) -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut bytes);
        let options = |mode| zip::write::SimpleFileOptions::default().unix_permissions(mode);
        writer
            .start_file(
                format!("{top}/zulu-8.jre/Contents/Home/bin/java"),
                options(0o755),
            )
            .unwrap();
        writer.write_all(FAKE_JAVA.as_bytes()).unwrap();
        writer
            .start_file(
                format!("{top}/zulu-8.jre/Contents/Home/lib/rt.jar"),
                options(0o644),
            )
            .unwrap();
        writer.write_all(b"rt").unwrap();
        writer
            .add_symlink(
                format!("{top}/bin"),
                "zulu-8.jre/Contents/Home/bin",
                options(0o777),
            )
            .unwrap();
        writer.finish().unwrap();
    }
    bytes.into_inner()
}

fn zulu_list(zip: &[u8]) -> String {
    zulu_list_for(ZULU_NAME, [8, 98, 0, 111], zip)
}

/// Azul's answer listing one build, `name`, with Azul's build number `distro`.
fn zulu_list_for(name: &str, distro: [u64; 4], zip: &[u8]) -> String {
    json!([zulu_entry(name, distro, [8, 0, 512], zip)]).to_string()
}

/// A build in Azul's answer: `name`, of Java `java`.
fn zulu_entry(name: &str, distro: [u64; 4], java: [u64; 3], zip: &[u8]) -> Value {
    use sha2::Digest as _;
    json!({
        "availability_type": "CA", "distro_version": distro,
        "download_url": format!("https://cdn.azul.com/zulu/bin/{name}"), "java_version": java, "latest": true,
        "name": name, "product": "zulu",
        "sha256_hash": format!("{:x}", sha2::Sha256::digest(zip)), "size": zip.len()
    })
}

impl Fixture {
    fn data_dir(&self) -> DataDir {
        DataDir::at(self.data.path())
    }

    fn installer(&self, rosetta: fn() -> bool) -> Installer {
        let (sleep, _) = recording_sleep();
        Installer::new(
            http::default_client().unwrap(),
            Downloader::new(http::download_client().unwrap(), sleep),
            MojangUrls::mirror(&self.server.base),
            self.data_dir(),
        )
        .with_x86_64_check(rosetta)
    }
}

fn apple_silicon() -> Platform {
    Platform::new(Os::MacOs, Arch::Arm64, "14.6.1")
}

fn yes() -> bool {
    true
}

fn no() -> bool {
    false
}

#[test]
fn installs_a_version_with_the_x64_runtime_under_rosetta() {
    let fixture = fixture();
    let data = fixture.data_dir();
    let game_dir = fixture.data.path().join("instances/test/minecraft");
    let progress = InstallProgress::default();
    let prepared = block_on(fixture.installer(yes).prepare(
        "test-1.0",
        None,
        &apple_silicon(),
        &JavaChoice::Mojang,
        &game_dir,
        &progress,
        &CancelToken::new(),
    ))
    .unwrap();

    // Java 8 comes from the x86_64 runtime, and the version is set up for it.
    assert_eq!(prepared.rosetta, Some(Rosetta::Version));
    assert_eq!(prepared.java_platform.arch, Arch::X86_64);
    let runtime = data.runtimes_dir().join("jre-legacy/mac-os");
    assert_eq!(
        prepared.java,
        runtime.join("jre.bundle/Contents/Home/bin/java")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = fs::metadata(&prepared.java).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111);
        let link = runtime.join("jre.bundle/Contents/Home/legal/java.desktop/LICENSE");
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::read(&link).unwrap(),
            b"GPLv2 with the Classpath Exception"
        );
    }

    // The version, client, libraries (not the Windows-only one) and logging config.
    assert_eq!(
        sha1_hex(&fs::read(data.versions_dir().join("test-1.0/test-1.0.json")).unwrap()),
        fixture.version_sha1
    );
    assert_eq!(fs::read(&prepared.client_jar).unwrap(), b"client jar");
    assert!(
        data.libraries_dir()
            .join("com/example/lib/1.0/lib-1.0.jar")
            .is_file()
    );
    assert!(
        !data
            .libraries_dir()
            .join("com/example/windows-only")
            .exists()
    );
    assert_eq!(
        prepared.logging_config.as_deref(),
        Some(
            data.assets_dir()
                .join("log_configs/client-1.12.xml")
                .as_path()
        )
    );

    // Assets by hash, and the by-name copies of a virtual index.
    assert_eq!(
        fs::read(
            data.assets_dir()
                .join("virtual/legacy/sound/step/grass1.ogg")
        )
        .unwrap(),
        b"ogg vorbis bytes"
    );
    let hash = sha1_hex(b"png bytes");
    assert!(
        data.assets_dir()
            .join(format!("objects/{}/{hash}", &hash[..2]))
            .is_file()
    );
    assert_eq!(progress.stage(), Stage::Assets);

    // Natives are extracted per launch, without META-INF.
    let natives = fixture.data.path().join("instances/test/natives");
    fs::create_dir_all(&natives).unwrap();
    fs::write(natives.join("stale.dylib"), b"from another version").unwrap();
    prepared.extract_natives(&data, &natives).unwrap();
    assert_eq!(
        fs::read(natives.join("liblwjgl.dylib")).unwrap(),
        b"native code"
    );
    assert!(!natives.join("META-INF").exists());
    assert!(!natives.join("stale.dylib").exists());

    // A second install downloads nothing it already has.
    let hits_before = fixture.server.total_hits();
    let progress = InstallProgress::default();
    block_on(fixture.installer(yes).prepare(
        "test-1.0",
        None,
        &apple_silicon(),
        &JavaChoice::Mojang,
        &game_dir,
        &progress,
        &CancelToken::new(),
    ))
    .unwrap();
    // Only the version list and the runtime list are fetched again.
    assert_eq!(fixture.server.total_hits() - hits_before, 2);
    assert_eq!(
        fixture.server.hits(&mojang("v1/objects/client/client.jar")),
        1
    );
}

#[test]
fn explains_when_rosetta_is_missing() {
    let fixture = fixture();
    let err = block_on(fixture.installer(no).prepare(
        "test-1.0",
        None,
        &apple_silicon(),
        &JavaChoice::Mojang,
        &fixture.data.path().join("game"),
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap_err();
    assert!(matches!(err, InstallError::RosettaMissing), "{err}");
    assert!(err.to_string().contains("softwareupdate --install-rosetta"));
    // Nothing of the runtime was downloaded.
    assert_eq!(fixture.server.hits(&mojang("v1/objects/rt/java")), 0);
}

#[test]
fn a_java_override_skips_the_runtime() {
    let fixture = fixture();
    let prepared = block_on(fixture.installer(no).prepare(
        "test-1.0",
        None,
        &Platform::new(Os::MacOs, Arch::X86_64, "14.6.1"),
        &JavaChoice::Override("/opt/java/bin/java".into()),
        &fixture.data.path().join("game"),
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    assert_eq!(prepared.java, Path::new("/opt/java/bin/java"));
    assert_eq!(prepared.rosetta, None);
    assert_eq!(
        fixture.server.hits(&mojang("v1/packages/rt/manifest.json")),
        0
    );
}

#[test]
fn works_offline_from_what_is_installed() {
    let fixture = fixture();
    let game_dir = fixture.data.path().join("game");
    let installer = fixture.installer(yes);
    block_on(installer.prepare(
        "test-1.0",
        None,
        &apple_silicon(),
        &JavaChoice::Mojang,
        &game_dir,
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    // The manifests now fail; the cached copies and installed files are used.
    for url in [
        "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
        "https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json",
    ] {
        fixture.server.serve(
            url,
            Entry {
                status: 503,
                ..Entry::ok("down")
            },
        );
    }
    let prepared = block_on(installer.prepare(
        "test-1.0",
        None,
        &apple_silicon(),
        &JavaChoice::Mojang,
        &game_dir,
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    assert_eq!(prepared.version.id, "test-1.0");
    // An unknown version is still an error.
    let err = block_on(installer.prepare(
        "9.9.9",
        None,
        &apple_silicon(),
        &JavaChoice::Mojang,
        &game_dir,
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap_err();
    assert!(matches!(err, InstallError::UnknownVersion(_)), "{err}");
}

#[test]
fn reports_a_tampered_version_file() {
    let fixture = fixture();
    let manifest = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
    let body =
        fixture.server.state().entries["/piston-meta.mojang.com/mc/game/version_manifest_v2.json"]
            .body
            .clone();
    let mut value: Value = serde_json::from_slice(&body).unwrap();
    value["versions"][0]["sha1"] = json!(sha1_hex(b"something else"));
    fixture.server.serve(manifest, Entry::ok(value.to_string()));
    let err = block_on(fixture.installer(yes).prepare(
        "test-1.0",
        None,
        &apple_silicon(),
        &JavaChoice::Mojang,
        &fixture.data.path().join("game"),
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap_err();
    assert!(err.to_string().contains("SHA-1"), "{err}");
}

/// Starts the installed version with the stand-in java, as the app does, and checks
/// the console, the redaction and the crash detection.
#[cfg(unix)]
#[test]
fn runs_the_game_and_detects_the_crash() {
    let fixture = fixture();
    let data = fixture.data_dir();
    let game_dir = fixture.data.path().join("instances/test/minecraft");
    fs::create_dir_all(&game_dir).unwrap();
    let natives = fixture.data.path().join("instances/test/natives");
    let prepared = block_on(fixture.installer(yes).prepare(
        "test-1.0",
        None,
        &apple_silicon(),
        &JavaChoice::Mojang,
        &game_dir,
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    prepared.extract_natives(&data, &natives).unwrap();
    let paths = prepared.launch_paths(&data, &game_dir, &natives).unwrap();
    let account = LaunchAccount::microsoft(
        "Lodestoner",
        "8667ba71-b85a-4004-af54-457a9734eed7".parse().unwrap(),
        Secret::new(TOKEN),
        None,
    );
    let command = launch::build(
        &prepared.version,
        &prepared.java_platform,
        &paths,
        &account,
        &GameSettings::default(),
    )
    .unwrap();
    assert!(command.args.contains(&TOKEN.to_string()));
    assert!(!command.display().contains("SECRET"));

    let before = crash::crash_reports(&game_dir);
    let (process, events) = GameProcess::spawn(&command, Redactor::new(command.secrets())).unwrap();
    let events: Vec<GameEvent> = block_on(events.collect());
    let lines: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::Line(line) => Some(line.clone()),
            GameEvent::Exited { .. } => None,
        })
        .collect();
    let Some(GameEvent::Exited { code, stopped }) = events.last() else {
        panic!("the last event is the exit: {events:?}");
    };
    assert_eq!((*code, *stopped), (Some(1), false));
    process.stop(); // after the exit, harmless

    let info = lines
        .iter()
        .find(|l| l.message == "Setting user: Lodestoner")
        .unwrap();
    assert_eq!(info.level, Level::Info);
    assert_eq!(info.thread.as_deref(), Some("Render thread"));
    let echoed = lines
        .iter()
        .find(|l| l.message.starts_with("Session token"))
        .unwrap();
    assert_eq!(echoed.message, "Session token: <redacted>");
    assert!(
        lines
            .iter()
            .any(|l| l.message == "Picked up a plain stderr line" && l.level == Level::Warn)
    );
    assert!(lines.iter().all(|l| !l.message.contains("SECRET")));

    let report = crash::new_crash_report(&game_dir, &before);
    assert!(report.is_some());
    let outcome = crash::outcome(*code, *stopped, report.clone());
    assert!(matches!(
        outcome,
        Outcome::Crashed {
            exit_code: Some(1),
            ..
        }
    ));
    let text = fs::read_to_string(report.unwrap()).unwrap();
    let summary = crash::summarize(Some(&text), &lines);
    assert_eq!(
        summary.exception.as_deref(),
        Some("java.lang.OutOfMemoryError: Java heap space")
    );
    assert_eq!(summary.causes, [crash::Cause::OutOfMemory]);
}

/// Stop kills a game that would otherwise run until its input closes.
#[cfg(unix)]
#[test]
fn stop_kills_the_game() {
    let dir = tempfile::tempdir().unwrap();
    let java = dir.path().join("java");
    // Prints a line, then blocks on a pipe nothing writes to.
    fs::write(&java, "#!/bin/sh\necho started\nexec tail -f /dev/null\n").unwrap();
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&java, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let version = launcher_core::game::version::VersionJson::parse(
        br#"{"id": "x", "mainClass": "Main", "minecraftArguments": "--username ${auth_player_name}"}"#,
    )
    .unwrap();
    let paths = launch::LaunchPaths {
        java: java.clone(),
        game_dir: dir.path().to_owned(),
        natives_dir: dir.path().join("natives"),
        libraries_dir: dir.path().join("libraries"),
        assets_dir: dir.path().join("assets"),
        game_assets: dir.path().join("assets"),
        classpath: vec![],
        logging_config: None,
    };
    let account =
        LaunchAccount::offline(&launcher_core::accounts::Account::offline("Dev_Steve").unwrap());
    let command = launch::build(
        &version,
        &apple_silicon(),
        &paths,
        &account,
        &GameSettings::default(),
    )
    .unwrap();
    let (process, mut events) = GameProcess::spawn(&command, Redactor::default()).unwrap();
    let first = block_on(events.next()).unwrap();
    assert!(
        matches!(&first, GameEvent::Line(line) if line.message == "started"),
        "{first:?}"
    );
    process.stop();
    let rest: Vec<GameEvent> = block_on(events.collect());
    let Some(GameEvent::Exited {
        code,
        stopped: true,
    }) = rest.last()
    else {
        panic!("expected a stopped exit, got {rest:?}");
    };
    // Killed by a signal: no exit code, and not a crash.
    assert_eq!(*code, None);
    assert_eq!(crash::outcome(*code, true, None), Outcome::Exited);
}

/// Installs `test-1.0` with `verify` and returns (trusted, hashed, downloaded) for
/// the recorded files: the client, libraries, logging config and assets.
fn install_counts(fixture: &Fixture, verify: Verify) -> (u64, u64, u64) {
    let progress = InstallProgress::default();
    block_on(fixture.installer(yes).prepare_with(
        "test-1.0",
        None,
        &apple_silicon(),
        &JavaChoice::Mojang,
        &fixture.data.path().join("game"),
        verify,
        &progress,
        &CancelToken::new(),
    ))
    .unwrap();
    progress.stats.counts()
}

/// The library jar of the fixture, which is in the version's install record.
fn library(fixture: &Fixture) -> std::path::PathBuf {
    fixture
        .data_dir()
        .libraries_dir()
        .join("com/example/lib/1.0/lib-1.0.jar")
}

/// Sets a file's modification time.
fn set_mtime(path: &Path, time: std::time::SystemTime) {
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(time)
        .unwrap();
}

#[test]
fn a_warm_launch_hashes_nothing_the_records_vouch_for() {
    let fixture = fixture();
    // 4 version files (client, library, natives jar, logging config) and 2 assets.
    assert_eq!(install_counts(&fixture, Verify::Recorded), (0, 0, 6));
    assert_eq!(install_counts(&fixture, Verify::Recorded), (6, 0, 0));
    let data = fixture.data_dir();
    assert!(
        data.versions_dir()
            .join("test-1.0/test-1.0.install.json")
            .is_file()
    );
    assert!(
        data.assets_dir()
            .join("indexes/legacy.install.json")
            .is_file()
    );
    // Files the records do not know are hashed once and then recorded.
    fs::remove_file(data.versions_dir().join("test-1.0/test-1.0.install.json")).unwrap();
    assert_eq!(install_counts(&fixture, Verify::Recorded), (2, 4, 0));
    assert_eq!(install_counts(&fixture, Verify::Recorded), (6, 0, 0));
}

#[test]
fn a_changed_size_or_time_is_hashed_again() {
    let fixture = fixture();
    install_counts(&fixture, Verify::Recorded);
    let jar = library(&fixture);

    // Touched but unchanged: hashed, found good, not downloaded.
    set_mtime(&jar, std::time::SystemTime::now() + Duration::from_secs(60));
    assert_eq!(install_counts(&fixture, Verify::Recorded), (5, 1, 0));
    assert_eq!(install_counts(&fixture, Verify::Recorded), (6, 0, 0));

    // A different size: hashed, found bad, downloaded again.
    let mut file = fs::File::options().append(true).open(&jar).unwrap();
    file.write_all(b"!").unwrap();
    drop(file);
    assert_eq!(install_counts(&fixture, Verify::Recorded), (5, 1, 1));
    assert_eq!(fs::read(&jar).unwrap(), b"library jar");
}

#[test]
fn verify_game_files_catches_corruption_the_record_cannot_see() {
    let fixture = fixture();
    install_counts(&fixture, Verify::Recorded);
    let jar = library(&fixture);
    // Corrupt the jar without changing its size or modification time.
    let mtime = fs::metadata(&jar).unwrap().modified().unwrap();
    fs::write(&jar, b"LIBRARY JAR").unwrap();
    set_mtime(&jar, mtime);
    // A launch trusts the record, as designed...
    assert_eq!(install_counts(&fixture, Verify::Recorded), (6, 0, 0));
    assert_eq!(fs::read(&jar).unwrap(), b"LIBRARY JAR");
    // ...and "Verify game files" hashes everything and repairs it.
    assert_eq!(install_counts(&fixture, Verify::Full), (0, 6, 1));
    assert_eq!(fs::read(&jar).unwrap(), b"library jar");
    // The rewritten records vouch for the repaired file.
    assert_eq!(install_counts(&fixture, Verify::Recorded), (6, 0, 0));
}

#[test]
fn a_new_version_file_invalidates_its_record() {
    let fixture = fixture();
    install_counts(&fixture, Verify::Recorded);
    // Mojang republishes the version file (here, with a trailing newline): its SHA-1
    // in the manifest changes, so the old record no longer applies.
    let version_path = "/piston-meta.mojang.com/v1/packages/abc/test-1.0.json";
    let manifest_path = "/piston-meta.mojang.com/mc/game/version_manifest_v2.json";
    let mut version = fixture.server.state().entries[version_path].body.clone();
    version.push(b'\n');
    let mut manifest: Value =
        serde_json::from_slice(&fixture.server.state().entries[manifest_path].body).unwrap();
    manifest["versions"][0]["sha1"] = json!(sha1_hex(&version));
    fixture.server.serve(
        "https://piston-meta.mojang.com/v1/packages/abc/test-1.0.json",
        Entry::ok(version),
    );
    fixture.server.serve(
        "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
        Entry::ok(manifest.to_string()),
    );
    // The 4 version files are hashed again; the assets' record still holds.
    assert_eq!(install_counts(&fixture, Verify::Recorded), (2, 4, 0));
    assert_eq!(install_counts(&fixture, Verify::Recorded), (6, 0, 0));
}

/// Minecraft 1.14 to 1.16.5 need Java 8, which Mojang only makes for Intel Macs. On
/// Apple silicon they run in Azul's Zulu 8 instead, natively: found through Azul's
/// list, checked against its SHA-256 and unpacked beside Mojang's runtimes, each build
/// in a folder of its own.
#[test]
fn installs_zulu_8_for_java_8_on_apple_silicon() {
    let fixture = fixture();
    let data = fixture.data_dir();
    let game_dir = fixture.data.path().join("instances/test/minecraft");
    let prepare_with = |verify| {
        block_on(fixture.installer(no).prepare_with(
            "test-arm64",
            None,
            &apple_silicon(),
            &JavaChoice::Mojang,
            &game_dir,
            verify,
            &InstallProgress::default(),
            &CancelToken::new(),
        ))
    };
    let prepare = || prepare_with(Verify::Recorded);
    // Rosetta isn't needed (the check says it's missing).
    let prepared = prepare().unwrap();
    assert_eq!(prepared.rosetta, None);
    assert_eq!(prepared.java_platform.arch, Arch::Arm64);
    let builds = data.runtimes_dir().join("zulu-8/mac-os-arm64");
    let runtime = builds.join("zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64");
    assert_eq!(
        prepared.java,
        runtime.join("zulu-8.jre/Contents/Home/bin/java")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = fs::metadata(&prepared.java).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111);
        assert!(runtime.join("bin/java").is_file());
    }
    // Mojang's Intel Java 8 was never fetched; the archive is kept for "Verify game
    // files"; no folder of the install is left behind.
    assert_eq!(
        fixture.server.hits(&mojang("v1/packages/rt/manifest.json")),
        0
    );
    assert!(data.cache_dir().join("java").join(ZULU_NAME).is_file());
    assert_eq!(fixture.server.hits(ZULU_URL), 1);
    assert_eq!(fs::read_dir(&builds).unwrap().count(), 1);

    // Installed: the list is read again, the build isn't downloaded again.
    prepare().unwrap();
    assert_eq!(fixture.server.hits(ZULU_URL), 1);
    assert_eq!(fixture.server.hits(ZULU_LIST), 2);

    // "Verify game files" unpacks it again from the kept archive, without a download.
    // The old copy stays beside it, unused, for the storage cleanup: a game another
    // instance started may still run from it.
    fs::remove_file(runtime.join("zulu-8.jre/Contents/Home/lib/rt.jar")).unwrap();
    assert_eq!(prepare_with(Verify::Full).unwrap().java, prepared.java);
    assert!(
        runtime
            .join("zulu-8.jre/Contents/Home/lib/rt.jar")
            .is_file()
    );
    assert_eq!(fixture.server.hits(ZULU_URL), 1);
    let kept: Vec<String> = fs::read_dir(&builds)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(kept.len(), 2, "{kept:?}");
    assert!(
        kept.iter()
            .any(|name| name.starts_with(".zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64.old-")),
        "{kept:?}"
    );
    assert_eq!(
        launcher_core::game::zulu::older_builds(&builds).len(),
        1,
        "the old copy is removable"
    );

    // Azul unreachable, or listing nothing, or answering something else (a captive
    // portal's page), or its build not downloadable: the build installed before.
    for (status, body) in [
        (503, ""),
        (404, ""),
        (200, "[]"),
        (200, "<html>Sign in to the Wi-Fi</html>"),
    ] {
        fixture.server.serve(
            ZULU_LIST,
            Entry {
                status,
                ..Entry::ok(body)
            },
        );
        assert_eq!(prepare().unwrap().java, prepared.java, "{status} {body}");
    }
    let newer = "zulu8.99.0.7-ca-jre8.0.522-macosx_aarch64";
    let newer_zip = zulu_zip_named(newer);
    fixture.server.serve(
        ZULU_LIST,
        Entry::ok(zulu_list_for(
            &format!("{newer}.zip"),
            [8, 99, 0, 7],
            &newer_zip,
        )),
    );
    let newer_url = format!("https://cdn.azul.com/zulu/bin/{newer}.zip");
    fixture.server.serve(
        &newer_url,
        Entry {
            status: 500,
            ..Entry::ok("")
        },
    );
    assert_eq!(prepare().unwrap().java, prepared.java);

    // A newer build goes into a folder of its own, and the one a running game may
    // use stays as it was, for the storage cleanup to remove later.
    fixture.server.serve(&newer_url, Entry::ok(newer_zip));
    let updated = prepare().unwrap();
    assert_eq!(
        updated.java,
        builds.join(newer).join("zulu-8.jre/Contents/Home/bin/java")
    );
    assert!(prepared.java.is_file());
    assert!(
        runtime
            .join("zulu-8.jre/Contents/Home/lib/rt.jar")
            .is_file()
    );
    // The older archive goes: no game uses it.
    assert!(!data.cache_dir().join("java").join(ZULU_NAME).exists());
}

/// Forge with ModLauncher 8.0 (1.16.3 to 1.16.5 before January 2022) can't start in
/// Java 8u321 or newer: it gets Azul's newest build before that, from Azul's whole
/// list, in a folder of its own, while other Java 8 versions keep the newest build.
#[test]
fn modlauncher_8_0_gets_a_zulu_8_before_8u321() {
    let fixture = fixture();
    let data = fixture.data_dir();
    let old = "zulu8.58.0.13-ca-jre8.0.312-macosx_aarch64";
    let old_zip = zulu_zip_named(old);
    let old_url = format!("https://cdn.azul.com/zulu/bin/{old}.zip");
    fixture.server.serve(&old_url, Entry::ok(old_zip.clone()));
    let new_zip = zulu_zip();
    fixture.server.serve(
        ZULU_LIST,
        Entry::ok(
            json!([
                zulu_entry(ZULU_NAME, [8, 98, 0, 111], [8, 0, 512], &new_zip),
                zulu_entry(&format!("{old}.zip"), [8, 58, 0, 13], [8, 0, 312], &old_zip),
            ])
            .to_string(),
        ),
    );
    let prepare = |id: &str| {
        block_on(fixture.installer(no).prepare(
            id,
            None,
            &apple_silicon(),
            &JavaChoice::Mojang,
            &fixture.data.path().join("instances").join(id),
            &InstallProgress::default(),
            &CancelToken::new(),
        ))
        .unwrap()
    };
    let forge = prepare("test-old-forge");
    assert_eq!(forge.rosetta, None);
    assert_eq!(forge.java_platform.arch, Arch::Arm64);
    assert_eq!(
        forge.java,
        data.runtimes_dir()
            .join("zulu-8-before-8u321/mac-os-arm64")
            .join(old)
            .join("zulu-8.jre/Contents/Home/bin/java")
    );
    assert_eq!(fixture.server.hits(ZULU_URL), 0);
    let other = prepare("test-arm64");
    assert_eq!(
        other.java,
        data.runtimes_dir()
            .join("zulu-8/mac-os-arm64/zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64")
            .join("zulu-8.jre/Contents/Home/bin/java")
    );
    // Each keeps its archive for "Verify game files".
    let archives = data.cache_dir().join("java");
    assert!(archives.join(ZULU_NAME).is_file());
    assert!(archives.join(format!("{old}.zip")).is_file());
    assert_eq!(prepare("test-old-forge").java, forge.java);
    assert_eq!(fixture.server.hits(&old_url), 1);

    // Azul lists nothing that old: said plainly, nothing newer or Intel is used.
    let fresh = self::fixture();
    let err = block_on(fresh.installer(yes).prepare(
        "test-old-forge",
        None,
        &apple_silicon(),
        &JavaChoice::Mojang,
        &fresh.data.path().join("game"),
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap_err();
    assert!(matches!(err, InstallError::NoNativeJava8), "{err}");
    assert_eq!(fresh.server.hits(ZULU_URL), 0);
}

/// With nothing installed, a 404 or an empty list from Azul means it has no Zulu 8 for
/// Apple silicon; any other failure is an error, never taken for "none". Either way
/// nothing Intel is used.
#[test]
fn no_zulu_8_is_said_plainly_and_outages_are_errors() {
    let prepare = |fixture: &Fixture| {
        block_on(fixture.installer(yes).prepare(
            "test-arm64",
            None,
            &apple_silicon(),
            &JavaChoice::Mojang,
            &fixture.data.path().join("game"),
            &InstallProgress::default(),
            &CancelToken::new(),
        ))
    };
    for (status, body) in [(200, "[]"), (404, "")] {
        let answered = fixture();
        answered.server.serve(
            ZULU_LIST,
            Entry {
                status,
                ..Entry::ok(body)
            },
        );
        let err = prepare(&answered).unwrap_err();
        assert!(
            matches!(err, InstallError::NoNativeJava8),
            "{status}: {err}"
        );
        assert_eq!(answered.server.hits(&mojang("v1/objects/rt/java")), 0);
    }
    let outage = fixture();
    outage.server.serve(
        ZULU_LIST,
        Entry {
            status: 500,
            ..Entry::ok("")
        },
    );
    let err = prepare(&outage).unwrap_err();
    assert!(matches!(err, InstallError::Network(_)), "{err}");

    // A build whose bytes don't match Azul's SHA-256 is refused.
    let tampered = fixture();
    tampered
        .server
        .serve(ZULU_URL, Entry::ok(b"not the zip".to_vec()));
    let err = prepare(&tampered).unwrap_err();
    assert!(err.to_string().contains("SHA-256"), "{err}");
    let builds = tampered
        .data_dir()
        .runtimes_dir()
        .join("zulu-8/mac-os-arm64");
    assert!(fs::read_dir(&builds).map_or(true, |mut entries| entries.next().is_none()));
}

/// A Mac program header: a thin x86_64 or arm64 Mach-O file.
fn mach_o(arm64: bool) -> Vec<u8> {
    let cpu: u32 = if arm64 { 0x0100_000c } else { 0x0100_0007 };
    let mut bytes = vec![0xcf, 0xfa, 0xed, 0xfe];
    bytes.extend(cpu.to_le_bytes());
    bytes.extend([0u8; 56]);
    bytes
}

/// A Java override is run as what it is. On Apple silicon an Intel one runs a version
/// that could run natively under Rosetta, with the version's Intel libraries, and the
/// player is told why; it needs Rosetta like Mojang's Intel Java. One made for Apple
/// silicon can't run a version that needs an Intel Java, and says so before anything
/// starts.
#[test]
fn a_java_override_runs_as_the_processor_it_is_made_for() {
    let fixture = fixture();
    let intel = fixture.data.path().join("intel-java");
    let arm64 = fixture.data.path().join("arm64-java");
    fs::write(&intel, mach_o(false)).unwrap();
    fs::write(&arm64, mach_o(true)).unwrap();
    let prepare = |id: &str, java: &Path, rosetta: fn() -> bool| {
        block_on(fixture.installer(rosetta).prepare(
            id,
            None,
            &apple_silicon(),
            &JavaChoice::Override(java.to_path_buf()),
            &fixture.data.path().join("game"),
            &InstallProgress::default(),
            &CancelToken::new(),
        ))
    };
    let prepared = prepare("test-arm64", &intel, yes).unwrap();
    assert_eq!(prepared.rosetta, Some(Rosetta::JavaOverride));
    assert_eq!(prepared.java_platform.arch, Arch::X86_64);
    assert_eq!(prepared.java, intel);
    let err = prepare("test-arm64", &intel, no).unwrap_err();
    assert!(matches!(err, InstallError::RosettaMissing), "{err}");

    let prepared = prepare("test-arm64", &arm64, no).unwrap();
    assert_eq!(prepared.rosetta, None);
    assert_eq!(prepared.java_platform.arch, Arch::Arm64);

    // test-1.0 has only Intel natives (LWJGL 2).
    let prepared = prepare("test-1.0", &intel, yes).unwrap();
    assert_eq!(prepared.rosetta, Some(Rosetta::Version));
    let err = prepare("test-1.0", &arm64, yes).unwrap_err();
    assert!(
        matches!(&err, InstallError::NeedsIntelJava(id) if id == "test-1.0"),
        "{err}"
    );
    // Neither override's Java was needed from Mojang or Azul.
    assert_eq!(fixture.server.hits(ZULU_LIST), 0);
}
