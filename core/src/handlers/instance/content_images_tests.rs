//! Content images recorded by Lodestone's actions (`docs/instance-images.md`, "When images
//! are recorded"): the Mods tab, the Files API, settings, a new server, and the check before
//! a start. Against a core on a loopback port with real Minecraft instances restored from
//! files in a temporary folder: no server is ever started, nothing is downloaded and no
//! EULA is accepted.

use std::collections::HashSet;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;

use base64::Engine as _;
use lodestone_image::Image;
use reqwest::{Method, StatusCode};
use serde_json::{json, Value};

use crate::auth::permission::UserPermission;
use crate::auth::user::User;
use crate::events::CausedBy;
use crate::global_settings::{CodeExecutionSwitch, GlobalSettingsData};
use crate::images::{AppInstances, Recorded};
use crate::implementations::minecraft::MinecraftInstance;
use crate::modrinth::fake::{FakeModrinth, FakeVersion};
use crate::prelude::GameInstance;
use crate::test_util::{serve_api, test_app_state, wait_until};
use crate::traits::t_configurable::GameType;
use crate::types::{DotLodestoneConfig, InstanceUuid};
use crate::AppState;

fn init_paths() {
    static LODESTONE_DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    let dir = LODESTONE_DIR.get_or_init(|| tempfile::tempdir().unwrap());
    crate::prelude::try_init_paths(dir.path().to_owned()).unwrap();
}

pub(crate) fn write(root: &FsPath, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// Which server a test runs.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Kind {
    Fabric,
    Paper,
}

pub(crate) struct Core {
    pub(crate) _dir: tempfile::TempDir,
    pub(crate) state: AppState,
    pub(crate) modrinth: Arc<FakeModrinth>,
    pub(crate) root: PathBuf,
    pub(crate) base: String,
    pub(crate) client: reqwest::Client,
    pub(crate) owner: String,
    pub(crate) uuid: InstanceUuid,
}

/// Puts a server's files into `root` and returns its uuid.
pub(crate) fn server_files(root: &FsPath, kind: Kind) -> InstanceUuid {
    let uuid = InstanceUuid::default();
    std::fs::create_dir_all(root).unwrap();
    write(
        root,
        ".lodestone_config",
        serde_json::to_string(&DotLodestoneConfig::new(
            uuid.clone(),
            GameType::MinecraftJava,
        ))
        .unwrap()
        .as_bytes(),
    );
    let flavour = match kind {
        Kind::Fabric => json!({"fabric": {"loader_version": "0.16.10", "installer_version": null}}),
        Kind::Paper => json!({"paper": {"build_version": 196}}),
    };
    write(
        root,
        ".lodestone_minecraft_config.json",
        json!({
            "name": "Test SMP", "version": "1.20.1", "flavour": flavour,
            "description": "", "cmd_args": [], "java_cmd": "java",
            "port": 25570, "min_ram": 1024, "max_ram": 2048, "auto_start": false,
            "restart_on_crash": false, "backup_period": null, "jre_major_version": 17,
            "has_started": true
        })
        .to_string()
        .as_bytes(),
    );
    write(root, "server.properties", b"motd=Hello\nlevel-name=world\n");
    write(root, "eula.txt", b"eula=false\n");
    write(root, "world/level.dat", b"overworld level");
    write(root, "world/region/r.0.0.mca", b"overworld region");
    write(root, "logs/latest.log", b"log");
    match kind {
        Kind::Fabric => {
            write(root, "mods/base.jar", b"base mod");
            write(root, "config/base.toml", b"a = 1");
        }
        Kind::Paper => {
            write(root, "plugins/WorldEdit.jar", b"worldedit");
            write(root, "plugins/WorldEdit/config.yml", b"limit: 1");
            write(root, "world_nether/level.dat", b"nether level");
            write(
                root,
                "world_nether/DIM-1/region/r.0.0.mca",
                b"nether region",
            );
            write(root, "world_the_end/DIM1/region/r.0.0.mca", b"end region");
        }
    }
    uuid
}

pub(crate) async fn core(kind: Kind) -> Core {
    init_paths();
    let dir = tempfile::tempdir().unwrap();
    let mut state = test_app_state(
        dir.path(),
        GlobalSettingsData {
            code_execution_enabled: false,
            safe_mode: false,
            ..Default::default()
        },
    )
    .await;
    let modrinth = Arc::new(FakeModrinth::default());
    state.mods = Arc::new(crate::mods::ModsState::new(
        modrinth.clone(),
        crate::modpack::store::PackStore::new(dir.path().join("modpacks")),
    ));
    state.images.set_instances(Arc::new(AppInstances {
        instances: state.instances.clone(),
        root: state.images.root().to_path_buf(),
    }));
    let instances = dir.path().join("instances");
    let root = instances.join("server");
    let uuid = server_files(&root, kind);
    let restored = crate::restore_instances(
        &instances,
        state.event_broadcaster.clone(),
        CodeExecutionSwitch::new(false),
    )
    .await
    .unwrap();
    for (uuid, instance) in crate::snapshot_instances(&restored) {
        state.instances.insert(uuid, instance);
    }
    assert_eq!(state.instances.len(), 1);
    let owner = add_user(&state, "owner", true, Default::default())
        .await
        .create_jwt()
        .unwrap()
        .to_string();
    let base = serve_api(state.clone()).await;
    Core {
        _dir: dir,
        state,
        modrinth,
        root,
        base,
        client: reqwest::Client::new(),
        owner,
        uuid,
    }
}

async fn add_user(
    state: &AppState,
    name: &str,
    is_owner: bool,
    permissions: UserPermission,
) -> User {
    let user = User::new(name.to_string(), "pw", is_owner, false, permissions);
    state
        .users_manager
        .write()
        .await
        .add_user(user.clone(), CausedBy::System)
        .await
        .unwrap();
    user
}

/// A Fabric mod jar: a zip with a `fabric.mod.json`.
fn fabric_jar(id: &str) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    let metadata = json!({"schemaVersion": 1, "id": id, "version": "1.0.0", "environment": "*"});
    std::io::Write::write_all(&mut zip, metadata.to_string().as_bytes()).unwrap();
    zip.finish().unwrap().into_inner()
}

fn b64(path: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(path)
}

/// A `multipart/form-data` body with each file as a `file` field.
fn multipart(files: &[(&str, &[u8])]) -> (String, Vec<u8>) {
    let boundary = "----lodestone-test-boundary";
    let mut body = Vec::new();
    for (name, content) in files {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(content);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

impl Core {
    pub(crate) async fn call(
        &self,
        method: Method,
        path: &str,
        token: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = self
            .client
            .request(method, format!("{}{path}", self.base))
            .bearer_auth(token);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.unwrap();
        let status = response.status();
        let text = response.text().await.unwrap();
        (
            status,
            serde_json::from_str(&text).unwrap_or(Value::String(text)),
        )
    }

    pub(crate) async fn upload(
        &self,
        path: &str,
        token: &str,
        files: &[(&str, &[u8])],
    ) -> StatusCode {
        let (content_type, body) = multipart(files);
        self.client
            .request(Method::PUT, format!("{}{path}", self.base))
            .bearer_auth(token)
            .header("content-type", content_type)
            .body(body)
            .send()
            .await
            .unwrap()
            .status()
    }

    pub(crate) async fn upload_mods(&self, token: &str, files: &[(&str, &[u8])]) -> StatusCode {
        let (content_type, body) = multipart(files);
        self.client
            .request(
                Method::POST,
                format!("{}/instance/{}/mods/upload", self.base, self.uuid),
            )
            .bearer_auth(token)
            .header("content-type", content_type)
            .body(body)
            .send()
            .await
            .unwrap()
            .status()
    }

    pub(crate) fn fs(&self, path: &str, action: &str) -> String {
        format!("/instance/{}/fs/{}/{action}", self.uuid, b64(path))
    }

    pub(crate) async fn write_file(&self, path: &str, token: &str, content: &str) -> StatusCode {
        self.client
            .request(
                Method::PUT,
                format!("{}{}", self.base, self.fs(path, "write")),
            )
            .bearer_auth(token)
            .body(content.to_string())
            .send()
            .await
            .unwrap()
            .status()
    }

    pub(crate) async fn member(&self, name: &str) -> String {
        let mut permissions = UserPermission::new();
        permissions.can_view_instance.insert(self.uuid.clone());
        permissions.can_read_instance_file.insert(self.uuid.clone());
        permissions
            .can_write_instance_file
            .insert(self.uuid.clone());
        add_user(&self.state, name, false, permissions)
            .await
            .create_jwt()
            .unwrap()
            .to_string()
    }

    pub(crate) fn instance(&self) -> MinecraftInstance {
        let instance = self
            .state
            .instances
            .get(&self.uuid)
            .unwrap()
            .value()
            .clone();
        let GameInstance::MinecraftInstance(instance) = instance;
        instance
    }

    /// Whether an operation's label waits to be recorded.
    pub(crate) fn pending(&self) -> bool {
        !self
            .state
            .images
            .slot(&self.uuid)
            .pending
            .lock()
            .unwrap()
            .is_empty()
    }

    /// Records what is queued now, as the merge window's end would.
    pub(crate) async fn flush(&self) -> Option<Recorded> {
        let ctx = crate::images::context_of(&self.state, &self.uuid)
            .await
            .unwrap();
        self.state.images.flush(ctx).await.unwrap()
    }

    /// Records the server's first image, as the startup sweep does.
    pub(crate) async fn record_initial(&self) -> Recorded {
        let ctx = crate::images::context_of(&self.state, &self.uuid)
            .await
            .unwrap();
        self.state
            .images
            .record_now(ctx, "Initial".into())
            .await
            .unwrap()
            .unwrap()
    }

    pub(crate) fn manifests(&self) -> Vec<Image> {
        self.state
            .images
            .store()
            .list_manifests(self.uuid.as_ref())
            .unwrap()
            .images
    }

    pub(crate) async fn current(&self) -> Image {
        let id = self.state.images.current_content(&self.uuid).await.unwrap();
        self.state
            .images
            .store()
            .load_manifest(self.uuid.as_ref(), &id)
            .unwrap()
    }

    /// Records what is queued and returns the new image's label; fails when nothing new
    /// was recorded.
    pub(crate) async fn recorded_label(&self) -> String {
        let recorded = self.flush().await.expect("an operation was queued");
        assert!(recorded.changed, "the content changed");
        self.current().await.label.unwrap_or_default()
    }
}

pub(crate) fn paths(image: &Image) -> Vec<&str> {
    image.entries.iter().map(|e| e.path.as_str()).collect()
}

#[tokio::test]
async fn each_mods_tab_action_records_one_labelled_image() {
    let core = core(Kind::Fabric).await;
    core.record_initial().await;
    core.modrinth
        .add(FakeVersion::new("jei", "v1", "jei-1.jar").date("2024-01-01T00:00:00Z"));
    core.modrinth
        .add(FakeVersion::new("jei", "v2", "jei-2.jar").date("2024-02-01T00:00:00Z"));
    let mods = format!("/instance/{}/mods", core.uuid);

    let (status, body) = core
        .call(
            Method::POST,
            &format!("{mods}/install"),
            &core.owner,
            Some(json!({"version_id": "v1"})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(core.recorded_label().await, "Added Title of jei");
    let image = core.current().await;
    let jar = image
        .entries
        .iter()
        .find(|e| e.path.as_str() == "mods/jei-1.jar")
        .expect("the installed jar is in the image");
    // what Modrinth said while installing gives the jar its source
    assert!(
        matches!(jar.source, lodestone_image::Source::Modrinth { .. }),
        "{:?}",
        jar.source
    );

    // an install that replaces the older version is an update
    let (status, body) = core
        .call(
            Method::POST,
            &format!("{mods}/install"),
            &core.owner,
            Some(json!({"version_id": "v2"})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(core.recorded_label().await, "Updated Title of jei");
    assert!(paths(&core.current().await).contains(&"mods/jei-2.jar"));

    let (status, body) = core
        .call(
            Method::PUT,
            &format!("{mods}/jei-2.jar/disable"),
            &core.owner,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(core.recorded_label().await, "Disabled jei-2.jar");
    assert!(paths(&core.current().await).contains(&"mods/jei-2.jar.disabled"));

    let (status, body) = core
        .call(
            Method::PUT,
            &format!("{mods}/jei-2.jar.disabled/enable"),
            &core.owner,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(core.recorded_label().await, "Enabled jei-2.jar");

    let (status, body) = core
        .call(
            Method::PUT,
            &format!("{mods}/jei-2.jar/client"),
            &core.owner,
            Some(json!({"required": true})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(core.recorded_label().await, "Marked jei-2.jar for players");
    let image = core.current().await;
    let jar = image
        .entries
        .iter()
        .find(|e| e.path.as_str() == "mods/jei-2.jar")
        .unwrap();
    assert_eq!(jar.env.client, lodestone_image::EnvValue::Required);
    // the marks file itself is no content
    assert!(!paths(&image).iter().any(|p| p.contains(".lodestone")));

    let (status, body) = core
        .call(
            Method::DELETE,
            &format!("{mods}/jei-2.jar"),
            &core.owner,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(core.recorded_label().await, "Removed jei-2.jar");
    assert!(!paths(&core.current().await).contains(&"mods/jei-2.jar"));

    let status = core
        .upload_mods(
            &core.owner,
            &[("a.jar", &fabric_jar("a")), ("b.jar", &fabric_jar("b"))],
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(core.recorded_label().await, "Uploaded 2 mods");

    // One image per action, each after the one before. An image's id is its contents', so
    // enabling the jar again is the image "Updated" recorded, and removing it the first
    // image: those are saved again under their new labels rather than kept twice.
    assert_eq!(core.manifests().len(), 6);
    assert!(!core.pending());
}

#[tokio::test]
async fn what_modrinth_said_outlives_a_restart() {
    let core = core(Kind::Fabric).await;
    core.modrinth
        .add(FakeVersion::new("jei", "v1", "jei-1.jar"));
    let (status, body) = core
        .call(
            Method::POST,
            &format!("/instance/{}/mods/install", core.uuid),
            &core.owner,
            Some(json!({"version_id": "v1"})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let result: crate::mods::install::ModInstallResult = serde_json::from_value(body).unwrap();
    core.flush().await;
    let sha512 = result.installed[0].sha512.clone();
    let url = core.state.images.modrinth_hint(&sha512).unwrap().url;
    assert!(url.is_some());

    // a core started again knows nothing in memory; listing the mods (no URLs) keeps the
    // URL the content state kept
    let restarted = test_app_state(core._dir.path(), GlobalSettingsData::default()).await;
    assert_eq!(restarted.images.modrinth_hint(&sha512), None);
    super::super::instance_mods::note_modrinth_hints(
        &restarted,
        &core.uuid,
        &result.installed,
        &Default::default(),
    )
    .await;
    let hint = restarted.images.modrinth_hint(&sha512).unwrap();
    assert_eq!(hint.url, url);
    assert_eq!(hint.server_side.as_deref(), Some("required"));
}

#[tokio::test]
async fn failed_and_refused_actions_record_nothing() {
    let core = core(Kind::Fabric).await;
    core.record_initial().await;
    let mods = format!("/instance/{}/mods", core.uuid);

    // a download that doesn't match its hash fails, and leaves nothing to record
    core.modrinth
        .add(FakeVersion::new("broken", "b1", "broken.jar").wrong_hash());
    let (status, _) = core
        .call(
            Method::POST,
            &format!("{mods}/install"),
            &core.owner,
            Some(json!({"version_id": "b1"})),
        )
        .await;
    assert!(!status.is_success());
    assert!(!core.pending());

    // an upload that isn't a mod is refused before anything is saved
    let status = core
        .upload_mods(&core.owner, &[("notamod.jar", b"x")])
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(!core.pending());

    // a member without WriteGlobalFile can't upload a jar Modrinth doesn't know
    let member = core.member("member").await;
    let status = core.upload_mods(&member, &[("evil.jar", b"evil")]).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!core.pending());
    // nor remove a mod that isn't there
    let (status, _) = core
        .call(
            Method::DELETE,
            &format!("{mods}/missing.jar"),
            &core.owner,
            None,
        )
        .await;
    assert!(!status.is_success());
    assert!(!core.pending());

    // Lodestone's own files stay protected through the Files API, and record nothing
    for path in [
        ".lodestone_config",
        ".lodestone_modpack/update.json",
        "mods/.lodestone_client_mods.json",
        "mods/new.jar",
    ] {
        let status = core.write_file(path, &member, "x").await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
    }
    assert!(!core.pending());
    assert_eq!(core.flush().await, None);
    assert_eq!(core.manifests().len(), 1);
}

#[tokio::test]
async fn files_api_changes_record_only_content() {
    let core = core(Kind::Fabric).await;
    core.record_initial().await;

    // worlds, logs and other data are no content
    for path in ["world/notes.txt", "logs/extra.log", "usercache.json"] {
        assert_eq!(
            core.write_file(path, &core.owner, "x").await,
            StatusCode::OK,
            "{path}"
        );
        assert!(!core.pending(), "{path}");
    }

    assert_eq!(
        core.write_file("config/base.toml", &core.owner, "a = 2")
            .await,
        StatusCode::OK
    );
    assert_eq!(core.recorded_label().await, "Edited config/base.toml");

    let (status, body) = core
        .call(
            Method::PUT,
            &core.fs("config/sub", "mkdir"),
            &core.owner,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = core
        .call(
            Method::PUT,
            &core.fs("config/sub/new.toml", "new"),
            &core.owner,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // merged into one image, labelled with both
    assert_eq!(
        core.recorded_label().await,
        "Created config/sub; Created config/sub/new.toml"
    );

    let (status, body) = core
        .call(
            Method::PUT,
            &format!(
                "/instance/{}/fs/{}/move/{}",
                core.uuid,
                b64("config/sub/new.toml"),
                b64("config/moved.toml")
            ),
            &core.owner,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        core.recorded_label().await,
        "Moved config/sub/new.toml to config/moved.toml"
    );

    let (status, body) = core
        .call(
            Method::DELETE,
            &core.fs("config/moved.toml", "rm"),
            &core.owner,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(core.recorded_label().await, "Deleted config/moved.toml");

    let (status, body) = core
        .call(
            Method::DELETE,
            &core.fs("config/sub", "rmdir"),
            &core.owner,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // an empty folder is no file of an image: nothing changed
    let recorded = core.flush().await.unwrap();
    assert!(!recorded.changed);

    // moving data around records nothing
    let (status, body) = core
        .call(
            Method::PUT,
            &format!(
                "/instance/{}/fs/{}/move/{}",
                core.uuid,
                b64("world/notes.txt"),
                b64("world/notes2.txt")
            ),
            &core.owner,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!core.pending());

    // an upload into config: a job that records once it succeeded
    let status = core
        .upload(
            &core.fs("config", "upload"),
            &core.owner,
            &[("up.toml", b"b = 1")],
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(core.recorded_label().await, "Uploaded config/up.toml");
    // an upload into the world records nothing
    let status = core
        .upload(
            &core.fs("world", "upload"),
            &core.owner,
            &[("data.txt", b"x")],
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!core.pending());

    // a copy job into config records when it ends
    let (status, body) = core
        .call(
            Method::PUT,
            &format!("/instance/{}/fs/cpr", core.uuid),
            &core.owner,
            Some(json!({
                "relative_paths_source": ["world/notes2.txt"],
                "relative_path_dest": "config",
            })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    wait_until("the copy is queued", || async { core.pending() }).await;
    assert_eq!(
        core.recorded_label().await,
        "Copied world/notes2.txt to config"
    );
    assert!(paths(&core.current().await).contains(&"config/notes2.txt"));

    // an archive unzipped into config
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file("zipped.toml", zip::write::SimpleFileOptions::default())
        .unwrap();
    std::io::Write::write_all(&mut zip, b"c = 1").unwrap();
    let zip = zip.finish().unwrap().into_inner();
    write(&core.root, "config/pack.zip", &zip);
    core.flush().await;
    let (status, body) = core
        .call(
            Method::PUT,
            &core.fs("config/pack.zip", "unzip"),
            &core.owner,
            Some(json!("Normal")),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    wait_until("the unzip is queued", || async { core.pending() }).await;
    assert_eq!(core.recorded_label().await, "Unzipped config/pack.zip");
    assert!(paths(&core.current().await).contains(&"config/zipped.toml"));

    // an archive zipped into config is a content change; one zipped into the world isn't
    let (status, body) = core
        .call(
            Method::PUT,
            &format!("/instance/{}/fs/zip", core.uuid),
            &core.owner,
            Some(json!({
                "target_relative_paths": ["world/notes2.txt"],
                "destination_relative_path": "config/notes.zip",
            })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    wait_until("the zip is queued", || async { core.pending() }).await;
    assert_eq!(core.recorded_label().await, "Zipped to config/notes.zip");
    let (status, body) = core
        .call(
            Method::PUT,
            &format!("/instance/{}/fs/zip", core.uuid),
            &core.owner,
            Some(json!({
                "target_relative_paths": ["world/notes2.txt"],
                "destination_relative_path": "world/notes.zip",
            })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    wait_until("the zip is written", || async {
        core.root.join("world/notes.zip").exists()
    })
    .await;
    assert!(!core.pending());

    // an upload leaves no temporary file behind, and a name taken gets a free one
    let status = core
        .upload(
            &core.fs("config", "upload"),
            &core.owner,
            &[("up.toml", b"b = 2")],
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(core.recorded_label().await, "Uploaded config/up_1.toml");
    assert!(!std::fs::read_dir(core.root.join("config"))
        .unwrap()
        .flatten()
        .any(|e| e.file_name().to_string_lossy().starts_with(".lodestone")));

    // the image never took the world or the logs
    let image = core.current().await;
    assert!(paths(&image)
        .iter()
        .all(|p| !p.starts_with("saves/") && !p.starts_with("logs/")));
}

#[tokio::test]
async fn a_success_is_recorded_after_the_merge_window_on_its_own() {
    let core = core(Kind::Fabric).await;
    core.record_initial().await;
    assert_eq!(
        core.write_file("config/base.toml", &core.owner, "a = 3")
            .await,
        StatusCode::OK
    );
    assert_eq!(
        core.write_file("config/other.toml", &core.owner, "b = 3")
            .await,
        StatusCode::OK
    );
    wait_until("the merged image is recorded", || async {
        core.manifests().len() == 2 && !core.pending()
    })
    .await;
    // the pointer moves right after the manifest is saved
    wait_until("the pointer moved", || async {
        core.current().await.label.as_deref() != Some("Initial")
    })
    .await;
    assert_eq!(
        core.current().await.label.as_deref(),
        Some("Edited config/base.toml; Edited config/other.toml")
    );
    assert!(!core.pending());
}

#[tokio::test]
async fn settings_and_the_resource_pack_record_but_local_settings_dont() {
    let core = core(Kind::Fabric).await;
    core.record_initial().await;
    let settings = format!("/instance/{}/settings", core.uuid);
    let (status, body) = core
        .call(
            Method::PUT,
            &format!("{settings}/server_properties_section/motd"),
            &core.owner,
            Some(json!({"type": "String", "value": "Changed"})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(core.recorded_label().await, "Changed motd");

    let (status, body) = core
        .call(
            Method::PUT,
            &format!("{settings}/cmd_args_section/max_ram"),
            &core.owner,
            Some(json!({"type": "UnsignedInteger", "value": 4096})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!core.pending());

    let (status, body) = core
        .call(
            Method::PUT,
            &format!("/instance/{}/resource_pack", core.uuid),
            &core.owner,
            Some(json!({"url": "https://example.com/pack.zip", "sha1": "", "required": false, "prompt": ""})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(core.recorded_label().await, "Changed the resource pack");
}

#[tokio::test]
async fn a_new_server_gets_its_first_image_once() {
    let core = core(Kind::Fabric).await;
    super::record_created(&core.state, &core.uuid).await;
    let image = core.current().await;
    assert_eq!(image.label.as_deref(), Some("Created Test SMP"));
    // a setup that recorded already (a modpack's install) keeps its image
    write(&core.root, "config/later.toml", b"x");
    super::record_created(&core.state, &core.uuid).await;
    assert_eq!(core.manifests().len(), 1);
}

#[tokio::test]
async fn the_check_before_a_start_records_hand_copied_files_once() {
    let core = core(Kind::Fabric).await;
    core.record_initial().await;
    let instance = core.instance();
    let images = core.state.images.clone();

    // nothing changed: nothing recorded
    instance.check_content_before_start(&images).await;
    assert_eq!(core.manifests().len(), 1);

    // a jar copied in by hand (over SFTP, say)
    write(&core.root, "mods/sftp.jar", b"copied by hand");
    instance.check_content_before_start(&images).await;
    assert_eq!(core.manifests().len(), 2);
    let image = core.current().await;
    assert_eq!(image.label.as_deref(), Some("Changed outside Lodestone"));
    assert!(paths(&image).contains(&"mods/sftp.jar"));

    // and only once
    instance.check_content_before_start(&images).await;
    assert_eq!(core.manifests().len(), 2);

    // a start of a server that runs already (refused right after) checks nothing
    images.server_started(&core.uuid).await;
    instance
        .set_state_for_test(crate::traits::t_server::State::Running)
        .await;
    write(&core.root, "config/written-by-the-server.toml", b"x");
    instance.check_content_before_start(&images).await;
    assert_eq!(core.manifests().len(), 2);
    let state = crate::images::state::load(images.root(), &core.uuid);
    assert!(state.running_since.is_some());
}

#[tokio::test]
async fn files_a_running_server_writes_make_no_image() {
    let core = core(Kind::Fabric).await;
    core.record_initial().await;
    let instance = core.instance();
    let images = core.state.images.clone();

    images.server_started(&core.uuid).await;
    // configs a mod rewrites at startup, and logs
    write(
        &core.root,
        "config/base.toml",
        b"a = 1 # rewritten by the mod",
    );
    write(&core.root, "config/generated.toml", b"generated");
    write(&core.root, "logs/latest.log", b"more log");
    // the stop takes the baseline again in the background; the start waits for it
    instance.content_after_stop(images.clone());
    instance.check_content_before_start(&images).await;
    assert_eq!(core.manifests().len(), 1);
    assert!(images
        .store()
        .list_manifests(core.uuid.as_ref())
        .unwrap()
        .images
        .iter()
        .all(|image| image.label.as_deref() == Some("Initial")));
}

#[tokio::test]
async fn a_start_after_the_core_died_mid_run_takes_that_run_in() {
    let core = core(Kind::Fabric).await;
    core.record_initial().await;
    let instance = core.instance();
    let images = core.state.images.clone();
    images.server_started(&core.uuid).await;
    write(&core.root, "config/generated.toml", b"written while it ran");
    // the core died here: no stop was ever seen, and this start comes before the sweep
    instance.check_content_before_start(&images).await;
    assert_eq!(core.manifests().len(), 1);
    let state = crate::images::state::load(images.root(), &core.uuid);
    assert_eq!(state.running_since, None);
    // and what it wrote is the baseline now
    instance.check_content_before_start(&images).await;
    assert_eq!(core.manifests().len(), 1);
    // a change after that is still caught
    write(&core.root, "mods/later.jar", b"copied by hand");
    instance.check_content_before_start(&images).await;
    assert_eq!(
        core.current().await.label.as_deref(),
        Some("Changed outside Lodestone")
    );
}

#[tokio::test]
async fn a_paper_servers_split_worlds_are_never_content() {
    let core = core(Kind::Paper).await;
    core.record_initial().await;
    let image = core.current().await;
    assert_eq!(
        paths(&image),
        vec![
            "plugins/WorldEdit.jar",
            "plugins/WorldEdit/config.yml",
            "server.properties"
        ]
    );
    // edits in the Nether and the End record nothing; a plugin's config does
    for path in [
        "world_nether/DIM-1/notes.txt",
        "world_the_end/DIM1/notes.txt",
        "world_nether/paper-world.yml",
    ] {
        assert_eq!(
            core.write_file(path, &core.owner, "x").await,
            StatusCode::OK,
            "{path}"
        );
        assert!(!core.pending(), "{path}");
    }
    assert_eq!(
        core.write_file("plugins/WorldEdit/config.yml", &core.owner, "limit: 2")
            .await,
        StatusCode::OK
    );
    assert_eq!(
        core.recorded_label().await,
        "Edited plugins/WorldEdit/config.yml"
    );
    let image = core.current().await;
    assert!(paths(&image)
        .iter()
        .all(|p| !p.starts_with("world") && !p.starts_with("saves/")));
}

#[tokio::test]
async fn concurrent_operations_and_records_lose_nothing() {
    let core = Arc::new(core(Kind::Fabric).await);
    core.record_initial().await;
    for (project, id) in [("alpha", "a1"), ("beta", "b1")] {
        core.modrinth
            .add(FakeVersion::new(project, id, &format!("{project}.jar")));
    }
    let install = |id: &'static str| {
        let core = core.clone();
        async move {
            let (status, body) = core
                .call(
                    Method::POST,
                    &format!("/instance/{}/mods/install", core.uuid),
                    &core.owner,
                    Some(json!({"version_id": id})),
                )
                .await;
            assert_eq!(status, StatusCode::OK, "{body}");
        }
    };
    let edit = {
        let core = core.clone();
        async move {
            assert_eq!(
                core.write_file("config/base.toml", &core.owner, "a = 9")
                    .await,
                StatusCode::OK
            );
        }
    };
    let record = {
        let core = core.clone();
        async move {
            // a record in the middle of it all
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            core.flush().await;
        }
    };
    tokio::join!(install("a1"), install("b1"), edit, record);
    core.flush().await;
    // one or two images after the first, depending on where the record fell
    let manifests = core.manifests();
    assert!(
        (2..=3).contains(&manifests.len()),
        "{} images",
        manifests.len()
    );
    let image = core.current().await;
    for path in ["mods/alpha.jar", "mods/beta.jar", "config/base.toml"] {
        assert!(paths(&image).contains(&path), "{path}");
    }
    let labels: HashSet<String> = manifests
        .iter()
        .filter_map(|image| image.label.clone())
        .flat_map(|label| {
            label
                .split("; ")
                .map(str::to_string)
                .collect::<Vec<String>>()
        })
        .collect();
    for label in [
        "Added Title of alpha",
        "Added Title of beta",
        "Edited config/base.toml",
    ] {
        assert!(labels.contains(label), "{label} in {labels:?}");
    }
    // every object every image names is in the store
    let store = core.state.images.store();
    for image in &manifests {
        for entry in &image.entries {
            assert!(
                store.object_complete(&entry.sha256, entry.size),
                "{} of {}",
                entry.path,
                image.id
            );
        }
    }
    assert!(!core.pending());
}

/// A Mods-tab install downloads for as long as the network takes, so it holds no content
/// gate: a record that comes due meanwhile, and the Files API changes behind it, never
/// wait for the download.
#[tokio::test(flavor = "multi_thread")]
async fn a_slow_install_holds_up_no_record_or_files_api_change() {
    let core = Arc::new(core(Kind::Fabric).await);
    core.record_initial().await;
    core.modrinth
        .add(FakeVersion::new("slow", "s1", "slow.jar"));
    let stall = core.modrinth.stall_downloads();
    let install = {
        let core = core.clone();
        tokio::spawn(async move {
            core.call(
                Method::POST,
                &format!("/instance/{}/mods/install", core.uuid),
                &core.owner,
                Some(json!({"version_id": "s1"})),
            )
            .await
        })
    };
    wait_until("the install is downloading", || async {
        !core.modrinth.downloaded.lock().unwrap().is_empty()
    })
    .await;

    // a label is queued and its record comes due while the download hangs
    assert_eq!(
        core.write_file("config/base.toml", &core.owner, "a = 4")
            .await,
        StatusCode::OK
    );
    let recorded = tokio::time::timeout(std::time::Duration::from_secs(10), core.flush())
        .await
        .expect("the record doesn't wait for the install");
    assert!(recorded.is_some_and(|r| r.changed));
    assert_eq!(
        core.current().await.label.as_deref(),
        Some("Edited config/base.toml")
    );
    // and a Files API change goes through at once
    let edit = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        core.write_file("config/other.toml", &core.owner, "b = 4"),
    )
    .await
    .expect("the change doesn't wait for the install");
    assert_eq!(edit, StatusCode::OK);

    // the install finishes and is recorded with its own label, whole
    stall.add_permits(1);
    let (status, body) = install.await.unwrap();
    assert_eq!(status, StatusCode::OK, "{body}");
    core.flush().await;
    let image = core.current().await;
    assert!(paths(&image).contains(&"mods/slow.jar"));
    assert!(paths(&image).contains(&"config/other.toml"));
    assert!(image
        .label
        .as_deref()
        .is_some_and(|label| label.contains("Added Title of slow")));
    assert!(!paths(&image).iter().any(|p| p.contains(".lodestone")));
    assert!(!core.pending());
}
