//! Installing Fabric and a Forge-style loader from a loopback file server: profiles,
//! Maven `.sha1` files, an installer jar made here, and its processor, run by a
//! stand-in `java` (a shell script). Nothing is fetched from the internet and no real
//! Java runs.

mod files;

use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use files::{Entry, FileServer};
use futures::executor::block_on;
use launcher_core::{
    game::{
        download::{CancelToken, Downloader, Verify, recording_sleep, sha1_hex},
        install::{InstallError, InstallProgress, Installer, JavaChoice, Prepared},
        loaders::{LoaderChoice, LoaderKind},
        mojang::MojangUrls,
        platform::{Arch, Os, Platform},
    },
    http,
    paths::DataDir,
};
use serde_json::{Value, json};

const GAME: &str = "1.20.1";

struct Fixture {
    server: FileServer,
    data: tempfile::TempDir,
}

impl Fixture {
    /// A made-up vanilla 1.20.1: a client jar and one library, no assets.
    fn new() -> Self {
        let server = FileServer::start();
        let fixture = Self {
            server,
            data: tempfile::tempdir().unwrap(),
        };
        let client = fixture.serve(
            "https://piston-data.mojang.com/v1/objects/c/client.jar",
            b"vanilla client jar",
        );
        let library = fixture.serve(
            "https://libraries.minecraft.net/com/example/lib/1.0/lib-1.0.jar",
            b"vanilla library",
        );
        let version = json!({
            "id": GAME,
            "type": "release",
            "mainClass": "net.minecraft.client.main.Main",
            "arguments": {"game": ["--version", "${version_name}"], "jvm": ["-cp", "${classpath}"]},
            "downloads": {"client": client},
            "javaVersion": {"component": "java-runtime-gamma", "majorVersion": 17},
            "libraries": [{"name": "com.example:lib:1.0", "downloads": {"artifact": {
                "path": "com/example/lib/1.0/lib-1.0.jar", "sha1": library["sha1"], "size": library["size"], "url": library["url"]}}}],
        })
        .to_string();
        let url = "https://piston-meta.mojang.com/v1/packages/v/1.20.1.json";
        fixture.server.serve(url, Entry::ok(version.as_bytes()));
        let manifest = json!({
            "latest": {"release": GAME, "snapshot": GAME},
            "versions": [{"id": GAME, "type": "release", "url": url, "sha1": sha1_hex(version.as_bytes()),
                          "time": "2023-06-12T00:00:00+00:00", "releaseTime": "2023-06-12T00:00:00+00:00"}]
        });
        fixture.server.serve(
            "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
            Entry::ok(manifest.to_string()),
        );
        fixture
    }

    fn serve(&self, url: &str, body: &[u8]) -> Value {
        self.server.serve(url, Entry::ok(body));
        json!({"url": url, "sha1": sha1_hex(body), "size": body.len()})
    }

    /// Serves `body` and its Maven `.sha1` file.
    fn serve_maven(&self, url: &str, body: &[u8]) {
        self.server.serve(url, Entry::ok(body));
        self.server.serve(
            &format!("{url}.sha1"),
            Entry::ok(format!("{}  file.jar\n", sha1_hex(body))),
        );
    }

    fn data_dir(&self) -> DataDir {
        DataDir::at(self.data.path())
    }

    fn installer(&self) -> Installer {
        let (sleep, _) = recording_sleep();
        Installer::new(
            http::default_client().unwrap(),
            Downloader::new(http::download_client().unwrap(), sleep),
            MojangUrls::mirror(&self.server.base),
            self.data_dir(),
        )
        .with_x86_64_check(|| true)
    }

    fn game_dir(&self) -> PathBuf {
        self.data.path().join("instances/test/minecraft")
    }

    fn prepare(
        &self,
        choice: &LoaderChoice,
        java: &Path,
        verify: Verify,
        progress: &InstallProgress,
    ) -> Result<Prepared, InstallError> {
        fs::create_dir_all(self.game_dir()).unwrap();
        block_on(self.installer().prepare_with(
            GAME,
            Some(choice),
            &Platform::new(Os::MacOs, Arch::Arm64, "14.6.1"),
            &JavaChoice::Override(java.to_owned()),
            &self.game_dir(),
            verify,
            progress,
            &CancelToken::new(),
        ))
    }

    /// An executable script at `name` in the data folder.
    fn script(&self, name: &str, body: &str) -> PathBuf {
        let path = self.data.path().join(name);
        fs::write(&path, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    fs::read(a).unwrap() == fs::read(b).unwrap()
}

#[test]
fn installs_fabric_with_maven_sha1_files() {
    let fixture = Fixture::new();
    let asm = b"asm jar";
    let loader = b"fabric loader jar";
    let asm_url = "https://maven.fabricmc.net/org/ow2/asm/asm/9.8/asm-9.8.jar";
    let loader_url =
        "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.16.14/fabric-loader-0.16.14.jar";
    fixture.server.serve(asm_url, Entry::ok(&asm[..]));
    fixture.serve_maven(loader_url, loader);
    let profile_url = "https://meta.fabricmc.net/v2/versions/loader/1.20.1/0.16.14/profile/json";
    let profile = json!({
        "id": "fabric-loader-0.16.14-1.20.1",
        "inheritsFrom": GAME,
        "type": "release",
        "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
        "arguments": {"game": [], "jvm": ["-DFabricMcEmu= net.minecraft.client.main.Main "]},
        "libraries": [
            {"name": "org.ow2.asm:asm:9.8", "url": "https://maven.fabricmc.net/", "sha1": sha1_hex(asm), "size": asm.len()},
            {"name": "net.fabricmc:fabric-loader:0.16.14", "url": "https://maven.fabricmc.net/"},
        ],
    });
    fixture
        .server
        .serve(profile_url, Entry::ok(profile.to_string()));
    let list = fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/loaders/fabric/loaders-1.20.1.json"),
    )
    .unwrap();
    fixture.server.serve(
        "https://meta.fabricmc.net/v2/versions/loader/1.20.1",
        Entry::ok(list),
    );

    let installer = fixture.installer();
    let versions = block_on(installer.loader_versions(LoaderKind::Fabric, GAME)).unwrap();
    assert_eq!(versions[0].id, "0.20.1");
    assert!(versions[0].recommended);

    let choice = LoaderChoice {
        kind: LoaderKind::Fabric,
        version: "0.16.14".into(),
    };
    let java = fixture.script("java", "#!/bin/sh\nexit 0\n");
    let prepared = fixture
        .prepare(
            &choice,
            &java,
            Verify::Recorded,
            &InstallProgress::default(),
        )
        .unwrap();
    let data = fixture.data_dir();
    let id = "fabric-loader-0.16.14-1.20.1";
    assert_eq!(prepared.version.id, id);
    assert_eq!(
        prepared.version.main_class,
        "net.fabricmc.loader.impl.launch.knot.KnotClient"
    );
    // The client jar is the vanilla one under the loader's name.
    assert_eq!(
        prepared.client_jar,
        data.versions_dir().join(id).join(format!("{id}.jar"))
    );
    assert!(same_file(
        &prepared.client_jar,
        &data.versions_dir().join("1.20.1/1.20.1.jar")
    ));
    let names: Vec<&str> = prepared
        .libraries
        .classpath
        .iter()
        .map(|f| f.name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "org.ow2.asm:asm:9.8",
            "net.fabricmc:fabric-loader:0.16.14",
            "com.example:lib:1.0"
        ]
    );
    assert_eq!(
        fs::read(
            data.libraries_dir()
                .join("net/fabricmc/fabric-loader/0.16.14/fabric-loader-0.16.14.jar")
        )
        .unwrap(),
        loader
    );
    // The normalised profile has the hash from the `.sha1` file.
    let saved: Value = serde_json::from_slice(
        &fs::read(data.versions_dir().join(id).join(format!("{id}.json"))).unwrap(),
    )
    .unwrap();
    assert_eq!(
        saved["libraries"][1]["downloads"]["artifact"]["sha1"],
        json!(sha1_hex(loader))
    );

    // The second launch needs neither the profile nor the `.sha1` files, and the
    // version list comes from the cache.
    fixture
        .server
        .state()
        .entries
        .remove("/meta.fabricmc.net/v2/versions/loader/1.20.1");
    fixture
        .prepare(
            &choice,
            &java,
            Verify::Recorded,
            &InstallProgress::default(),
        )
        .unwrap();
    assert_eq!(fixture.server.hits(profile_url), 1);
    assert_eq!(fixture.server.hits(&format!("{loader_url}.sha1")), 1);
    assert_eq!(fixture.server.hits(loader_url), 1);
    let cached = block_on(installer.loader_versions(LoaderKind::Fabric, GAME)).unwrap();
    assert_eq!(cached, versions);

    // A loader version for another Minecraft version is refused before any download.
    let wrong = LoaderChoice {
        kind: LoaderKind::Forge,
        version: "1.21.1-52.1.21".into(),
    };
    let err = fixture
        .prepare(&wrong, &java, Verify::Recorded, &InstallProgress::default())
        .unwrap_err();
    assert!(err.to_string().contains("is for Minecraft 1.21.1"), "{err}");
}

const FORGE: &str = "1.20.1-47.4.28";
const FORGE_ID: &str = "1.20.1-forge-47.4.28";
const BINPATCH: &[u8] = b"binary patches";

fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut bytes);
        for (name, body) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(body).unwrap();
        }
        writer.finish().unwrap();
    }
    bytes.into_inner()
}

/// What the processor makes from the patches.
fn patched() -> Vec<u8> {
    [b"patched:".as_slice(), BINPATCH].concat()
}

/// Serves a Forge-style installer with one client processor (and one server-only
/// processor that must not run), its jars, and the installer's `.sha1`.
fn serve_forge(fixture: &Fixture) {
    serve_forge_with(fixture, false);
}

/// [`serve_forge`]; with `universal_in_profile`, the universal jar is listed in the
/// installer's `install_profile.json` instead of its `version.json`, as Forge 1.13 to
/// 1.16 list it.
fn serve_forge_with(fixture: &Fixture, universal_in_profile: bool) {
    let patcher = zip(&[(
        "META-INF/MANIFEST.MF",
        b"Manifest-Version: 1.0\r\nMain-Class: net.minecraftforge.binarypatcher.Con\r\n soleTool\r\n",
    )]);
    let patcher_url = "https://maven.minecraftforge.net/net/minecraftforge/binarypatcher/1.1.1/binarypatcher-1.1.1.jar";
    let patcher_ref = fixture.serve(patcher_url, &patcher);
    let jopt_url = "https://libraries.minecraft.net/net/sf/jopt-simple/jopt-simple/5.0.4/jopt-simple-5.0.4.jar";
    let jopt_ref = fixture.serve(jopt_url, b"jopt");
    let modlauncher = fixture.serve(
        "https://maven.minecraftforge.net/cpw/mods/modlauncher/10.0.9/modlauncher-10.0.9.jar",
        b"modlauncher",
    );
    let universal = b"forge universal jar";
    let universal_library = json!({"name": format!("net.minecraftforge:forge:{FORGE}:universal"), "downloads": {"artifact": {
        "path": format!("net/minecraftforge/forge/{FORGE}/forge-{FORGE}-universal.jar"),
        "url": "", "sha1": sha1_hex(universal), "size": universal.len()}}});
    let mut install_profile = json!({
        "spec": 1,
        "minecraft": GAME,
        "json": "/version.json",
        "data": {
            "PATCHED": {"client": format!("[net.minecraftforge:forge:{FORGE}:client]"), "server": "[x:y:1]"},
            "PATCHED_SHA": {"client": format!("'{}'", sha1_hex(&patched())), "server": "''"},
            "BINPATCH": {"client": "/data/client.lzma", "server": "/data/server.lzma"},
        },
        "processors": [
            {"sides": ["server"], "jar": "net.minecraftforge:installertools:1.4.1", "args": ["--task", "BUNDLER_EXTRACT"]},
            {"jar": "net.minecraftforge:binarypatcher:1.1.1",
             "classpath": ["net.sf.jopt-simple:jopt-simple:5.0.4"],
             "args": ["--clean", "{MINECRAFT_JAR}", "--output", "{PATCHED}", "--apply", "{BINPATCH}"],
             "outputs": {"{PATCHED}": "{PATCHED_SHA}"}},
        ],
        "libraries": [
            {"name": "net.minecraftforge:binarypatcher:1.1.1", "downloads": {"artifact": {
                "path": "net/minecraftforge/binarypatcher/1.1.1/binarypatcher-1.1.1.jar",
                "url": patcher_url, "sha1": patcher_ref["sha1"], "size": patcher_ref["size"]}}},
            {"name": "net.sf.jopt-simple:jopt-simple:5.0.4", "downloads": {"artifact": {
                "path": "net/sf/jopt-simple/jopt-simple/5.0.4/jopt-simple-5.0.4.jar",
                "url": jopt_url, "sha1": jopt_ref["sha1"], "size": jopt_ref["size"]}}},
        ],
    });
    let mut version = json!({
        "id": FORGE_ID,
        "inheritsFrom": GAME,
        "type": "release",
        "mainClass": "cpw.mods.bootstraplauncher.BootstrapLauncher",
        "logging": {},
        "arguments": {"game": ["--launchTarget", "forgeclient"], "jvm": ["-DignoreList=${version_name}.jar"]},
        "libraries": [
            {"name": format!("net.minecraftforge:forge:{FORGE}:client"), "downloads": {"artifact": {
                "path": format!("net/minecraftforge/forge/{FORGE}/forge-{FORGE}-client.jar"),
                "url": "", "sha1": sha1_hex(&patched()), "size": patched().len()}}},
            {"name": "cpw.mods:modlauncher:10.0.9", "downloads": {"artifact": {
                "path": "cpw/mods/modlauncher/10.0.9/modlauncher-10.0.9.jar",
                "url": modlauncher["url"], "sha1": modlauncher["sha1"], "size": modlauncher["size"]}}},
        ],
    });
    let listed_in = if universal_in_profile {
        &mut install_profile
    } else {
        &mut version
    };
    listed_in["libraries"]
        .as_array_mut()
        .unwrap()
        .insert(0, universal_library);
    let install_profile = install_profile.to_string();
    let version = version.to_string();
    let universal_entry =
        format!("maven/net/minecraftforge/forge/{FORGE}/forge-{FORGE}-universal.jar");
    let installer = zip(&[
        ("install_profile.json", install_profile.as_bytes()),
        ("version.json", version.as_bytes()),
        ("data/client.lzma", BINPATCH),
        ("data/server.lzma", b"server patches"),
        (universal_entry.as_str(), universal),
    ]);
    fixture.serve_maven(
        &format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{FORGE}/forge-{FORGE}-installer.jar"),
        &installer,
    );
}

/// A `java` that acts as the patcher: it records each run in `runs.txt`, prints a
/// line, and writes `patched:` and the patches to `--output`. With `wrong`, it
/// writes something else.
fn patcher_java(fixture: &Fixture, wrong: bool) -> PathBuf {
    let runs = fixture.data.path().join("runs.txt");
    let content = if wrong {
        "printf 'garbage' > \"$out\"".to_string()
    } else {
        "{ printf 'patched:'; cat \"$apply\"; } > \"$out\"".to_string()
    };
    fixture.script(
        "java",
        &format!(
            r#"#!/bin/sh
echo "$@" >> '{runs}'
out=""; apply=""
while [ $# -gt 0 ]; do
  case "$1" in
    --output) out="$2"; shift ;;
    --apply) apply="$2"; shift ;;
  esac
  shift
done
echo "Applying patches"
mkdir -p "$(dirname "$out")"
{content}
"#,
            runs = runs.display()
        ),
    )
}

fn runs(fixture: &Fixture) -> Vec<String> {
    fs::read_to_string(fixture.data.path().join("runs.txt"))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

/// Forge 1.13 to 1.16 list their universal jar only in the installer's
/// `install_profile.json`; it comes out of the installer all the same, or FML says
/// "Missing forge!".
#[test]
#[cfg_attr(windows, ignore = "the made-up java is a shell script")]
fn restores_a_universal_jar_listed_only_in_the_install_profile() {
    let fixture = Fixture::new();
    serve_forge_with(&fixture, true);
    let java = patcher_java(&fixture, false);
    let choice = LoaderChoice {
        kind: LoaderKind::Forge,
        version: FORGE.into(),
    };
    let universal = fixture.data_dir().libraries_dir().join(format!(
        "net/minecraftforge/forge/{FORGE}/forge-{FORGE}-universal.jar"
    ));
    fixture
        .prepare(
            &choice,
            &java,
            Verify::Recorded,
            &InstallProgress::default(),
        )
        .unwrap();
    assert_eq!(fs::read(&universal).unwrap(), b"forge universal jar");
    // Gone (deleted by hand): back from the installer on the next launch.
    fs::remove_file(&universal).unwrap();
    fixture
        .prepare(
            &choice,
            &java,
            Verify::Recorded,
            &InstallProgress::default(),
        )
        .unwrap();
    assert_eq!(fs::read(&universal).unwrap(), b"forge universal jar");
}

#[test]
#[cfg_attr(windows, ignore = "the made-up java is a shell script")]
fn installs_forge_and_runs_its_processor_once() {
    let fixture = Fixture::new();
    serve_forge(&fixture);
    let java = patcher_java(&fixture, false);
    let choice = LoaderChoice {
        kind: LoaderKind::Forge,
        version: FORGE.into(),
    };
    let data = fixture.data_dir();
    let libraries = data.libraries_dir();
    let client = libraries.join(format!(
        "net/minecraftforge/forge/{FORGE}/forge-{FORGE}-client.jar"
    ));
    let universal = libraries.join(format!(
        "net/minecraftforge/forge/{FORGE}/forge-{FORGE}-universal.jar"
    ));

    let progress = InstallProgress::default();
    let prepared = fixture
        .prepare(&choice, &java, Verify::Recorded, &progress)
        .unwrap();
    // Modern Forge has no splash screen to turn off.
    assert!(!fixture.game_dir().join("config/splash.properties").exists());
    let runs_now = runs(&fixture);
    assert_eq!(runs_now.len(), 1, "only the client processor runs");
    // `java -cp <patcher>:<jopt> <Main-Class> <args>`.
    let run = &runs_now[0];
    assert!(
        run.starts_with(&format!(
            "-cp {}:{} net.minecraftforge.binarypatcher.ConsoleTool --clean {} --output {}",
            libraries
                .join("net/minecraftforge/binarypatcher/1.1.1/binarypatcher-1.1.1.jar")
                .display(),
            libraries
                .join("net/sf/jopt-simple/jopt-simple/5.0.4/jopt-simple-5.0.4.jar")
                .display(),
            data.versions_dir().join("1.20.1/1.20.1.jar").display(),
            client.display(),
        )),
        "{run}"
    );
    assert_eq!(fs::read(&client).unwrap(), patched());
    assert_eq!(fs::read(&universal).unwrap(), b"forge universal jar");
    let notes = progress.take_notes();
    assert!(notes.iter().any(|n| n == "Applying patches"), "{notes:?}");
    assert!(
        notes.iter().any(|n| n.contains("processor 1/1")),
        "{notes:?}"
    );
    // The merged version: Forge's libraries first, Mojang's logging kept away.
    assert_eq!(prepared.version.id, FORGE_ID);
    assert_eq!(
        prepared.libraries.classpath[0].name,
        format!("net.minecraftforge:forge:{FORGE}:universal")
    );
    assert!(
        prepared
            .client_jar
            .ends_with(format!("{FORGE_ID}/{FORGE_ID}.jar"))
    );
    // The extracted installer data is cleaned up.
    assert!(
        !data
            .versions_dir()
            .join(FORGE_ID)
            .join("installer-data")
            .exists()
    );

    // The next launch trusts the processor's record.
    fixture
        .prepare(
            &choice,
            &java,
            Verify::Recorded,
            &InstallProgress::default(),
        )
        .unwrap();
    assert_eq!(runs(&fixture).len(), 1);

    // A missing output makes it run again; a missing installer file is put back.
    fs::remove_file(&client).unwrap();
    fs::remove_file(&universal).unwrap();
    fixture
        .prepare(
            &choice,
            &java,
            Verify::Recorded,
            &InstallProgress::default(),
        )
        .unwrap();
    assert_eq!(runs(&fixture).len(), 2);
    assert!(universal.is_file() && client.is_file());

    // "Verify game files" hashes the outputs: a changed one (same size, same
    // modification time) is caught.
    let mtime = fs::metadata(&client).unwrap().modified().unwrap();
    let mut damaged = patched();
    damaged[0] ^= 0xff;
    fs::write(&client, &damaged).unwrap();
    fs::File::options()
        .write(true)
        .open(&client)
        .unwrap()
        .set_modified(mtime)
        .unwrap();
    fixture
        .prepare(
            &choice,
            &java,
            Verify::Recorded,
            &InstallProgress::default(),
        )
        .unwrap();
    assert_eq!(runs(&fixture).len(), 2, "the record trusts it");
    fixture
        .prepare(&choice, &java, Verify::Full, &InstallProgress::default())
        .unwrap();
    assert_eq!(runs(&fixture).len(), 3);
    assert_eq!(fs::read(&client).unwrap(), patched());
}

#[test]
#[cfg_attr(windows, ignore = "the made-up java is a shell script")]
fn a_deleted_libraries_folder_is_repaired_with_the_installer_downloaded_again() {
    let fixture = Fixture::new();
    serve_forge(&fixture);
    let java = patcher_java(&fixture, false);
    let choice = LoaderChoice {
        kind: LoaderKind::Forge,
        version: FORGE.into(),
    };
    let libraries = fixture.data_dir().libraries_dir();
    let installer_url = format!(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/{FORGE}/forge-{FORGE}-installer.jar"
    );
    fixture
        .prepare(
            &choice,
            &java,
            Verify::Recorded,
            &InstallProgress::default(),
        )
        .unwrap();
    assert_eq!(fixture.server.hits(&installer_url), 1);

    // Only the installer gone: nothing needs it, so it isn't fetched.
    let installer = libraries.join(format!(
        "net/minecraftforge/forge/{FORGE}/forge-{FORGE}-installer.jar"
    ));
    fs::remove_file(&installer).unwrap();
    fixture
        .prepare(
            &choice,
            &java,
            Verify::Recorded,
            &InstallProgress::default(),
        )
        .unwrap();
    assert_eq!(fixture.server.hits(&installer_url), 1);

    // The whole folder gone, the installer with it, while the saved profile stays.
    fs::remove_dir_all(&libraries).unwrap();
    for verify in [Verify::Recorded, Verify::Full] {
        fixture
            .prepare(&choice, &java, verify, &InstallProgress::default())
            .unwrap();
        fs::remove_dir_all(&libraries).unwrap();
    }
    assert_eq!(fixture.server.hits(&installer_url), 3);
    assert_eq!(runs(&fixture).len(), 3);
}

#[test]
#[cfg_attr(windows, ignore = "the made-up java is a shell script")]
fn a_processor_output_with_the_wrong_sha1_fails() {
    let fixture = Fixture::new();
    serve_forge(&fixture);
    let java = patcher_java(&fixture, true);
    let choice = LoaderChoice {
        kind: LoaderKind::Forge,
        version: FORGE.into(),
    };
    let err = fixture
        .prepare(
            &choice,
            &java,
            Verify::Recorded,
            &InstallProgress::default(),
        )
        .unwrap_err();
    assert!(err.to_string().contains("wrong SHA-1"), "{err}");
    // Nothing is recorded, so the next launch tries again.
    assert!(
        !fixture
            .data_dir()
            .versions_dir()
            .join(FORGE_ID)
            .join(format!("{FORGE_ID}.processors.json"))
            .exists()
    );
}

#[test]
fn a_wrong_installer_sha1_is_refused() {
    let fixture = Fixture::new();
    serve_forge(&fixture);
    let url = format!(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/{FORGE}/forge-{FORGE}-installer.jar.sha1"
    );
    fixture
        .server
        .serve(&url, Entry::ok(format!("{}\n", "0".repeat(40))));
    let java = patcher_java(&fixture, false);
    let choice = LoaderChoice {
        kind: LoaderKind::Forge,
        version: FORGE.into(),
    };
    let err = fixture
        .prepare(
            &choice,
            &java,
            Verify::Recorded,
            &InstallProgress::default(),
        )
        .unwrap_err();
    assert!(err.to_string().contains("SHA-1"), "{err}");
    assert!(runs(&fixture).is_empty());
}

const LEGACY: &str = "1.20.1-11.0.0.1";

/// Serves an old-format (`install` + `versionInfo`) Forge installer whose Scala
/// library has no `.sha1` file next to it, like Forge 1.8.9's. `checksums` lists the
/// jar's SHA-1 when `listed`, and only an unrelated one otherwise.
fn serve_legacy_forge(fixture: &Fixture, listed: bool) -> &'static [u8] {
    let scala: &'static [u8] = b"scala library jar";
    serve_legacy_forge_with(fixture, listed, scala);
    scala
}

/// [`serve_legacy_forge`], with `scala` as the Scala library jar.
fn serve_legacy_forge_with(fixture: &Fixture, listed: bool, scala: &[u8]) {
    let scala_url = "https://maven.minecraftforge.net/org/scala-lang/scala-library/2.11.1/scala-library-2.11.1.jar";
    fixture.server.serve(scala_url, Entry::ok(scala.to_vec()));
    let other_url = "https://maven.minecraftforge.net/com/example/other/1.0/other-1.0.jar";
    fixture.serve_maven(other_url, b"other library");
    let checksums = if listed {
        json!([sha1_hex(b"pack.xz of the scala library"), sha1_hex(scala)])
    } else {
        json!([sha1_hex(b"something else")])
    };
    let profile = json!({
        "install": {
            "path": format!("net.minecraftforge:forge:{LEGACY}"),
            "filePath": "forge-universal.jar",
            "minecraft": GAME,
        },
        "versionInfo": {
            "id": format!("{GAME}-Forge"),
            "mainClass": "net.minecraft.launchwrapper.Launch",
            "minecraftArguments": "--username ${auth_player_name} --tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker",
            "libraries": [
                {"name": format!("net.minecraftforge:forge:{LEGACY}")},
                {"name": "org.scala-lang:scala-library:2.11.1", "url": "https://maven.minecraftforge.net/",
                 "checksums": checksums, "clientreq": true},
                {"name": "com.example:other:1.0", "url": "https://maven.minecraftforge.net/"},
                {"name": "jline:jline:2.13", "clientreq": false},
            ],
        },
    })
    .to_string();
    let installer = zip(&[
        ("install_profile.json", profile.as_bytes()),
        ("forge-universal.jar", b"legacy universal jar"),
    ]);
    fixture.serve_maven(
        &format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{LEGACY}/forge-{LEGACY}-installer.jar"),
        &installer,
    );
}

#[test]
fn an_old_forge_library_without_a_sha1_file_is_hashed_whatever_its_size() {
    // Larger than the 2 MiB limit on API responses, as Scala's compiler (13 MB) is.
    let scala: Vec<u8> = (0..3 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
    let fixture = Fixture::new();
    serve_legacy_forge_with(&fixture, true, &scala);
    let java = fixture.script("java", "#!/bin/sh\n");
    let choice = LoaderChoice {
        kind: LoaderKind::Forge,
        version: LEGACY.into(),
    };
    fixture
        .prepare(
            &choice,
            &java,
            Verify::Recorded,
            &InstallProgress::default(),
        )
        .unwrap();
    let installed = fixture
        .data
        .path()
        .join("libraries/org/scala-lang/scala-library/2.11.1/scala-library-2.11.1.jar");
    assert!(fs::read(installed).unwrap() == scala);
}

#[test]
fn an_old_forge_library_without_a_sha1_file_is_hashed_and_checked_against_checksums() {
    let fixture = Fixture::new();
    let scala = serve_legacy_forge(&fixture, true);
    let java = fixture.script("java", "#!/bin/sh\n");
    let choice = LoaderChoice {
        kind: LoaderKind::Forge,
        version: LEGACY.into(),
    };
    fixture
        .prepare(
            &choice,
            &java,
            Verify::Recorded,
            &InstallProgress::default(),
        )
        .unwrap();
    let installed = fixture
        .data
        .path()
        .join("libraries/org/scala-lang/scala-library/2.11.1/scala-library-2.11.1.jar");
    assert_eq!(fs::read(installed).unwrap(), scala);
    // The `.sha1` file was asked for once, found missing, and the jar hashed instead.
    let scala_url = "https://maven.minecraftforge.net/org/scala-lang/scala-library/2.11.1/scala-library-2.11.1.jar";
    assert_eq!(fixture.server.hits(&format!("{scala_url}.sha1")), 1);

    // Old Forge's splash screen is turned off on macOS, once; a file the player
    // changed afterwards is left alone.
    let splash = fixture.game_dir().join("config/splash.properties");
    assert_eq!(fs::read_to_string(&splash).unwrap(), "enabled=false\n");
    fs::write(&splash, "enabled=true\n").unwrap();
    fixture
        .prepare(
            &choice,
            &java,
            Verify::Recorded,
            &InstallProgress::default(),
        )
        .unwrap();
    assert_eq!(fs::read_to_string(&splash).unwrap(), "enabled=true\n");
}

#[test]
fn an_old_forge_library_without_a_sha1_file_must_match_the_listed_checksums() {
    let fixture = Fixture::new();
    serve_legacy_forge(&fixture, false);
    let java = fixture.script("java", "#!/bin/sh\n");
    let choice = LoaderChoice {
        kind: LoaderKind::Forge,
        version: LEGACY.into(),
    };
    let err = fixture
        .prepare(
            &choice,
            &java,
            Verify::Recorded,
            &InstallProgress::default(),
        )
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("is not one the Forge profile lists"),
        "{err}"
    );
}

#[test]
fn a_minecraft_version_a_loader_does_not_support_has_no_loader_versions() {
    let fixture = Fixture::new();
    let installer = fixture.installer();
    // Fabric's meta answers 400 (with `[]`) and Quilt's 404 for versions they don't
    // support, such as 1.8.9: that's "no versions", not a download failure.
    let fabric_url = "https://meta.fabricmc.net/v2/versions/loader/1.8.9";
    fixture.server.serve(
        fabric_url,
        Entry {
            status: 400,
            ..Entry::ok("[]")
        },
    );
    let versions = block_on(installer.loader_versions(LoaderKind::Fabric, "1.8.9")).unwrap();
    assert!(versions.is_empty());
    // The Quilt URL isn't served at all, so the test server answers 404.
    let versions = block_on(installer.loader_versions(LoaderKind::Quilt, "1.8.9")).unwrap();
    assert!(versions.is_empty());

    // A list cached from before is dropped, not shown, once the answer is "unsupported".
    let cache = fixture.data.path().join("cache/loaders/fabric-1.8.9.json");
    fs::create_dir_all(cache.parent().unwrap()).unwrap();
    fs::write(
        &cache,
        br#"[{"loader": {"version": "0.1.0", "stable": true}}]"#,
    )
    .unwrap();
    let versions = block_on(installer.loader_versions(LoaderKind::Fabric, "1.8.9")).unwrap();
    assert!(versions.is_empty());
    assert!(!cache.exists());

    // Any other error is still an error, and says which address failed.
    fixture.server.serve(
        "https://meta.fabricmc.net/v2/versions/loader/1.12.2",
        Entry {
            status: 503,
            ..Entry::ok("busy")
        },
    );
    let err = block_on(installer.loader_versions(LoaderKind::Fabric, "1.12.2")).unwrap_err();
    let message = err.to_string();
    assert!(message.contains("HTTP 503"), "{message}");
    assert!(message.contains("/v2/versions/loader/1.12.2"), "{message}");
}
