//! Importing a server folder: what is detected, what is copied, and who may ask. The
//! import itself downloads Java and the server, so these stop before it.

use std::io::Write;
use std::path::Path;

use reqwest::StatusCode;
use serde_json::{json, Value};

use super::*;
use crate::{
    auth::{permission::UserPermission, user::User},
    events::CausedBy,
    global_settings::GlobalSettingsData,
    test_util::{serve_api, test_app_state},
};

fn init_paths() {
    static LODESTONE_DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    let dir = LODESTONE_DIR.get_or_init(|| tempfile::tempdir().unwrap());
    crate::prelude::try_init_paths(dir.path().to_owned()).unwrap();
}

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

/// A jar with a `version.json` naming `id`, as vanilla's and Paper's server jars have.
fn write_server_jar(root: &Path, name: &str, id: &str) {
    std::fs::create_dir_all(root).unwrap();
    let mut jar = zip::ZipWriter::new(std::fs::File::create(root.join(name)).unwrap());
    jar.start_file("version.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    jar.write_all(json!({ "id": id, "name": id }).to_string().as_bytes())
        .unwrap();
    jar.finish().unwrap();
}

fn software(
    version: &str,
    flavour: Option<FlavourKind>,
    loader_version: Option<&str>,
    other_software: Option<&str>,
) -> Software {
    Software {
        version: Some(version.to_owned()),
        flavour,
        loader_version: loader_version.map(str::to_owned),
        other_software: other_software.map(str::to_owned),
    }
}

#[test]
fn a_vanilla_server_is_told_by_its_jar() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "server.properties", "server-port=25570\n");
    write_server_jar(dir.path(), "minecraft_server.jar", "1.20.4");
    assert_eq!(
        detect_software(dir.path()),
        software("1.20.4", Some(FlavourKind::Vanilla), None, None)
    );
}

#[test]
fn the_log_says_which_version_last_ran() {
    let dir = tempfile::tempdir().unwrap();
    // an old jar is still in the folder after the server moved to a newer version
    write_server_jar(dir.path(), "old.jar", "1.19.4");
    write(
        dir.path(),
        "logs/latest.log",
        "[12:00:00] [Server thread/INFO]: Starting minecraft server version 1.20.1\n[12:00:00] [Server thread/INFO]: Loading properties\n",
    );
    assert_eq!(
        detect_software(dir.path()).version.as_deref(),
        Some("1.20.1")
    );
}

#[test]
fn paper_and_its_relatives_are_imported_as_paper() {
    let paper = tempfile::tempdir().unwrap();
    write(paper.path(), "config/paper-global.yml", "");
    write(paper.path(), "bukkit.yml", "");
    write(paper.path(), "spigot.yml", "");
    write(
        paper.path(),
        "version_history.json",
        r#"{"currentVersion":"git-Paper-196 (MC: 1.20.1)"}"#,
    );
    assert_eq!(
        detect_software(paper.path()),
        software("1.20.1", Some(FlavourKind::Paper), None, None)
    );

    let purpur = tempfile::tempdir().unwrap();
    for file in [
        "purpur.yml",
        "config/paper-global.yml",
        "spigot.yml",
        "bukkit.yml",
    ] {
        write(purpur.path(), file, "");
    }
    write(
        purpur.path(),
        "version_history.json",
        r#"{"currentVersion":"1.21.1-2329-803bf62 (MC: 1.21.1)"}"#,
    );
    assert_eq!(
        detect_software(purpur.path()),
        software("1.21.1", Some(FlavourKind::Paper), None, Some("Purpur"))
    );

    let spigot = tempfile::tempdir().unwrap();
    write(spigot.path(), "spigot.yml", "");
    write(spigot.path(), "bukkit.yml", "");
    write_server_jar(spigot.path(), "spigot-1.20.4.jar", "1.20.4");
    assert_eq!(
        detect_software(spigot.path()),
        software("1.20.4", Some(FlavourKind::Paper), None, Some("Spigot"))
    );
}

#[test]
fn mod_loaders_are_told_by_their_libraries() {
    let forge = tempfile::tempdir().unwrap();
    write(
        forge.path(),
        "libraries/net/minecraftforge/forge/1.20.1-47.2.0/unix_args.txt",
        "",
    );
    write(
        forge.path(),
        "libraries/net/minecraftforge/forge/1.20.1-47.1.3/unix_args.txt",
        "",
    );
    assert_eq!(
        detect_software(forge.path()),
        software(
            "1.20.1",
            Some(FlavourKind::Forge),
            Some("1.20.1-47.2.0"),
            None
        )
    );

    let neoforge = tempfile::tempdir().unwrap();
    write(
        neoforge.path(),
        "libraries/net/neoforged/neoforge/21.1.77/unix_args.txt",
        "",
    );
    assert_eq!(
        detect_software(neoforge.path()),
        software("1.21.1", Some(FlavourKind::NeoForge), Some("21.1.77"), None)
    );

    let fabric = tempfile::tempdir().unwrap();
    write(
        fabric.path(),
        "libraries/net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar",
        "",
    );
    write(fabric.path(), "versions/1.21.1/server-1.21.1.jar", "");
    assert_eq!(
        detect_software(fabric.path()),
        software("1.21.1", Some(FlavourKind::Fabric), Some("0.16.9"), None)
    );

    // Quilt isn't offered, and its mods don't all run on Fabric: nothing is guessed
    let quilt = tempfile::tempdir().unwrap();
    write(
        quilt.path(),
        "libraries/org/quiltmc/quilt-loader/0.26.0/quilt-loader.jar",
        "",
    );
    write(quilt.path(), "versions/1.20.1/server-1.20.1.jar", "");
    assert_eq!(
        detect_software(quilt.path()),
        software("1.20.1", None, None, Some("Quilt"))
    );

    // mods, but no sign of what loads them
    let unknown = tempfile::tempdir().unwrap();
    write(unknown.path(), "mods/some-mod.jar", "");
    assert_eq!(detect_software(unknown.path()).flavour, None);
}

#[test]
fn versions_are_read_strictly() {
    assert_eq!(
        version_after("git-Paper-196 (MC: 1.20.1)", "(MC: "),
        Some("1.20.1")
    );
    assert_eq!(
        version_after("Starting minecraft server version 24w14a\n", "version "),
        Some("24w14a")
    );
    // a version goes into download URLs and messages
    assert_eq!(version_after("(MC: ../../x)", "(MC: "), None);
    assert_eq!(version_after("no marker", "(MC: "), None);
    assert!(looks_like_version("1.21.11"));
    assert!(!looks_like_version(""));
    assert!(!looks_like_version("latest"));
    assert!(!looks_like_version("1.20/../x"));
}

/// A Paper server folder with a world, a plugin, a script, and the files an import
/// leaves behind.
fn paper_server(root: &Path) {
    write(
        root,
        "server.properties",
        "server-port=25599\nlevel-name=survival\nmotd=Hello\n",
    );
    write(root, "eula.txt", "eula=true\n");
    write(root, "config/paper-global.yml", "");
    write(
        root,
        "version_history.json",
        r#"{"currentVersion":"git-Paper-196 (MC: 1.20.1)"}"#,
    );
    write(root, "survival/level.dat", "level");
    write(root, "survival/region/r.0.0.mca", "region data");
    write(root, "survival/session.lock", "x");
    write(root, "plugins/Essentials.jar", "plugin");
    write(root, "plugins/Skript/scripts/hello.js", "script");
    write(root, "paper-1.20.1.jar", "old server");
    write(root, "libraries/com/example/lib.jar", "library");
    write(root, "versions/1.20.1/paper-1.20.1.jar", "patched");
    write(root, "cache/mojang_1.20.1.jar", "cache");
    write(root, ".lodestone_config", "{}");
}

#[test]
fn a_folder_is_read_without_its_software() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("My Paper Server");
    paper_server(&root);
    let info = inspect_folder(&root, true).unwrap();
    assert_eq!(
        info,
        ServerFolderInfo {
            path: root.to_string_lossy().into_owned(),
            name: "My Paper Server".to_owned(),
            version: Some("1.20.1".to_owned()),
            flavour: Some(FlavourKind::Paper),
            port: Some(25599),
            eula_accepted: true,
            has_world: true,
            // the properties, eula, Paper's two files, the world's two, the plugin and
            // its script
            file_count: 8,
            size_bytes: info.size_bytes,
            ..Default::default()
        }
    );
    // without the right to add scripts, the script isn't counted
    assert_eq!(inspect_folder(&root, false).unwrap().file_count, 7);

    // a folder of something else isn't a server
    let other = dir.path().join("photos");
    write(&other, "holiday.png", "");
    let error = inspect_folder(&other, true).unwrap_err();
    assert!(matches!(error.kind, ErrorKind::BadRequest));
}

#[test]
fn a_server_lodestone_made_says_what_it_runs() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        ".lodestone_minecraft_config.json",
        &json!({
            "name": "Survival", "version": "1.21.1",
            "flavour": { "fabric": { "loader_version": "0.16.9", "installer_version": "1.0.1" } },
            "description": "", "cmd_args": [], "java_cmd": "java",
            "port": 25571, "min_ram": 1024, "max_ram": 6144, "auto_start": false,
            "restart_on_crash": false, "backup_period": null, "jre_major_version": 21,
            "has_started": true
        })
        .to_string(),
    );
    write(dir.path(), "server.properties", "server-port=25571\n");
    let info = inspect_folder(dir.path(), true).unwrap();
    assert_eq!(info.name, "Survival");
    assert_eq!(info.version.as_deref(), Some("1.21.1"));
    assert_eq!(info.flavour, Some(FlavourKind::Fabric));
    assert_eq!(info.loader_version.as_deref(), Some("0.16.9"));
    assert_eq!((info.min_ram, info.max_ram), (Some(1024), Some(6144)));
    // its own config stays behind
    assert_eq!(info.file_count, 1);
}

#[test]
fn the_copy_keeps_the_server_and_leaves_its_software() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("old");
    paper_server(&source);
    // what a new instance has before the copy
    let target = dir.path().join("new");
    write(&target, "server.properties", "server-port=25565\n");
    write(&target, "eula.txt", "eula=false\n");
    write(&target, "server.jar", "new server");
    write(&target, ".lodestone_config", r#"{"uuid":"new"}"#);

    let plan = plan_copy(&source, false).unwrap();
    assert_eq!((plan.skipped_scripts, plan.skipped_links), (1, 0));
    let copied = std::cell::Cell::new(0);
    copy_plan(&plan, &source, &target, &|bytes| {
        copied.set(copied.get() + bytes)
    })
    .unwrap();
    assert_eq!(copied.get(), plan.size_bytes);

    let read = |relative: &str| std::fs::read_to_string(target.join(relative)).ok();
    // the server's own settings, world and plugins replace the new instance's
    assert_eq!(
        read("server.properties").as_deref(),
        Some("server-port=25599\nlevel-name=survival\nmotd=Hello\n")
    );
    assert_eq!(read("eula.txt").as_deref(), Some("eula=true\n"));
    assert_eq!(
        read("survival/region/r.0.0.mca").as_deref(),
        Some("region data")
    );
    assert_eq!(read("plugins/Essentials.jar").as_deref(), Some("plugin"));
    // the new instance keeps its own server and identity
    assert_eq!(read("server.jar").as_deref(), Some("new server"));
    assert_eq!(
        read(".lodestone_config").as_deref(),
        Some(r#"{"uuid":"new"}"#)
    );
    for left_out in [
        "paper-1.20.1.jar",
        "libraries",
        "versions",
        "cache",
        "survival/session.lock",
        "plugins/Skript/scripts/hello.js",
    ] {
        assert!(!target.join(left_out).exists(), "{left_out} was copied");
    }
    // the old server is untouched
    assert!(source.join("paper-1.20.1.jar").exists());
    assert!(source.join("survival/session.lock").exists());

    assert_eq!(
        describe_skipped(&plan).as_deref(),
        Some("1 script file(s) were not copied: only the owner can import scripts, while code execution is on")
    );
    let with_scripts = plan_copy(&source, true).unwrap();
    assert_eq!(with_scripts.skipped_scripts, 0);
    assert_eq!(describe_skipped(&with_scripts), None);
}

#[cfg(unix)]
#[test]
fn symbolic_links_are_not_followed() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("server");
    write(&source, "server.properties", "");
    write(dir.path(), "secret/key.txt", "secret");
    std::os::unix::fs::symlink(dir.path().join("secret"), source.join("linked")).unwrap();
    let plan = plan_copy(&source, true).unwrap();
    assert_eq!(plan.skipped_links, 1);
    assert_eq!(plan.files.len(), 1);
}

#[test]
fn only_the_top_of_the_folder_is_treated_as_software() {
    assert!(is_left_out(Path::new("server.jar"), false));
    assert!(is_left_out(Path::new("Forge-Installer.JAR"), false));
    assert!(is_left_out(Path::new("libraries"), true));
    assert!(is_left_out(
        Path::new(".lodestone_minecraft_config.json"),
        false
    ));
    assert!(is_left_out(Path::new("world/session.lock"), false));
    // mods, plugins and a world's own folders are the server's
    assert!(!is_left_out(Path::new("mods/sodium.jar"), false));
    assert!(!is_left_out(Path::new("world/versions"), true));
    assert!(!is_left_out(Path::new("config/cache"), true));
    assert!(!is_left_out(Path::new("libraries.txt"), false));
}

#[test]
fn the_request_is_checked_before_anything_is_made() {
    let request = |change: fn(&mut ImportServerRequest)| {
        let mut request = ImportServerRequest {
            path: "/srv/minecraft".to_owned(),
            name: "Survival".to_owned(),
            version: "1.20.1".to_owned(),
            flavour: FlavourKind::Paper,
            loader_version: None,
            port: 25565,
            min_ram: Some(1024),
            max_ram: Some(4096),
        };
        change(&mut request);
        check_request(&request)
    };
    assert!(request(|_| {}).is_ok());
    assert!(request(|r| r.name = "a/b".to_owned()).is_err());
    assert!(request(|r| r.version = String::new()).is_err());
    assert!(request(|r| r.version = "../1.20".to_owned()).is_err());
    assert!(request(|r| r.port = 0).is_err());
    assert!(request(|r| r.port = 70000).is_err());
    assert!(request(|r| r.min_ram = Some(8192)).is_err());

    assert_eq!(
        flavour_with_loader(FlavourKind::Forge, Some("1.20.1-47.2.0")),
        Flavour::Forge {
            build_version: Some(ForgeBuildVersion::new("1.20.1-47.2.0".to_owned()))
        }
    );
    assert_eq!(
        flavour_with_loader(FlavourKind::Paper, Some("196")),
        Flavour::Paper {
            build_version: None
        }
    );
}

struct Core {
    dir: tempfile::TempDir,
    state: AppState,
    base: String,
    client: reqwest::Client,
}

async fn core() -> Core {
    init_paths();
    let dir = tempfile::tempdir().unwrap();
    // safe mode keeps reading the machine's files to the owner
    let state = test_app_state(
        dir.path(),
        GlobalSettingsData {
            safe_mode: false,
            ..Default::default()
        },
    )
    .await;
    let base = serve_api(state.clone()).await;
    Core {
        dir,
        state,
        base,
        client: reqwest::Client::new(),
    }
}

impl Core {
    async fn user(&self, name: &str, is_owner: bool, permissions: UserPermission) -> String {
        let user = User::new(name.to_string(), "pw", is_owner, false, permissions);
        self.state
            .users_manager
            .write()
            .await
            .add_user(user.clone(), CausedBy::System)
            .await
            .unwrap();
        user.create_jwt().unwrap().to_string()
    }

    async fn post(&self, route: &str, token: &str, body: Value) -> (StatusCode, Value) {
        let response = self
            .client
            .post(format!("{}{route}", self.base))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = response.status();
        let text = response.text().await.unwrap();
        (
            status,
            serde_json::from_str(&text).unwrap_or(Value::String(text)),
        )
    }
}

#[tokio::test]
async fn importing_needs_both_creating_servers_and_reading_files() {
    let core = core().await;
    let source = core.dir.path().join("Old Server");
    paper_server(&source);
    let body = json!({ "path": source.to_string_lossy() });

    let owner = core.user("owner", true, Default::default()).await;
    let creator = core
        .user(
            "creator",
            false,
            UserPermission {
                can_create_instance: true,
                ..Default::default()
            },
        )
        .await;
    let reader = core
        .user(
            "reader",
            false,
            UserPermission {
                can_read_global_file: true,
                ..Default::default()
            },
        )
        .await;
    let both = core
        .user(
            "both",
            false,
            UserPermission {
                can_create_instance: true,
                can_read_global_file: true,
                ..Default::default()
            },
        )
        .await;

    for token in [&creator, &reader] {
        let (status, _) = core
            .post("/instance/import/inspect", token, body.clone())
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = core
            .post(
                "/instance/import",
                token,
                json!({
                    "path": source.to_string_lossy(), "name": "Imported",
                    "version": "1.20.1", "flavour": "Paper", "port": 25565
                }),
            )
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    let (status, info) = core
        .post("/instance/import/inspect", &owner, body.clone())
        .await;
    assert_eq!(status, StatusCode::OK, "{info}");
    assert_eq!(info["name"], "Old Server");
    assert_eq!(info["version"], "1.20.1");
    assert_eq!(info["flavour"], "Paper");
    assert_eq!(info["port"], 25599);
    assert_eq!(info["file_count"], 8);

    // only the owner brings scripts along
    let (status, info) = core.post("/instance/import/inspect", &both, body).await;
    assert_eq!(status, StatusCode::OK, "{info}");
    assert_eq!(info["file_count"], 7);
}

#[tokio::test]
async fn folders_that_cannot_be_imported_are_refused() {
    let core = core().await;
    let owner = core.user("owner", true, Default::default()).await;
    let inspect = |path: String| {
        let core = &core;
        let owner = owner.clone();
        async move {
            core.post("/instance/import/inspect", &owner, json!({ "path": path }))
                .await
        }
    };

    let (status, _) = inspect(String::new()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let missing = core.dir.path().join("missing");
    let (status, _) = inspect(missing.to_string_lossy().into_owned()).await;
    assert_ne!(status, StatusCode::OK);

    // a file, and a folder that isn't a server
    let file = core.dir.path().join("server.properties");
    std::fs::write(&file, "").unwrap();
    let (status, _) = inspect(file.to_string_lossy().into_owned()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let empty = core.dir.path().join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    let (status, error) = inspect(empty.to_string_lossy().into_owned()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");

    // a folder among this core's own servers, and the folder that holds them. Other
    // tests share that folder, so the one made here is empty and removed at once.
    let instances = crate::prelude::path_to_instances();
    let existing = instances.join(format!(
        "import-test-{}",
        InstanceUuid::default().no_prefix()
    ));
    std::fs::create_dir_all(&existing).unwrap();
    let inside = inspect(existing.to_string_lossy().into_owned()).await;
    std::fs::remove_dir(&existing).unwrap();
    let holder = inspect(instances.to_string_lossy().into_owned()).await;
    for (status, error) in [inside, holder] {
        assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
        assert!(error.to_string().contains("already part of"), "{error}");
    }
    // Lodestone's data folder, which holds its secrets
    let (status, _) = inspect(
        crate::prelude::lodestone_path()
            .to_string_lossy()
            .into_owned(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // a bad request is refused before the folder is read or anything is downloaded
    let source = core.dir.path().join("Old Server");
    paper_server(&source);
    let (status, error) = core
        .post(
            "/instance/import",
            &owner,
            json!({
                "path": source.to_string_lossy(), "name": "a/b",
                "version": "1.20.1", "flavour": "Paper", "port": 25565
            }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    assert_eq!(core.state.instances.len(), 0);
}
