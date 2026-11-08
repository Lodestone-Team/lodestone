//! Prepared states of the window for the offscreen screenshots (`--scene`). They
//! use made-up accounts, an in-memory secret store and a temporary data
//! directory, and never sign in or touch the network.

use std::sync::Arc;

use gpui::{App, AppContext as _, Context, Entity};
use launcher_core::{
    MemorySecretStore,
    accounts::Account,
    auth::{AuthError, MsaConfig},
    game::{
        crash::{Cause, CrashSummary},
        download::ProgressSnapshot,
        install::{InstallProgress, Stage as InstallStage},
        log4j::{Level, LogLine, Stream},
    },
    instances::InstanceStore,
    paths::DataDir,
    skin, storage,
};
use lodestone_minecraft::versions::mojang::MojangVersionManifest;

use crate::{
    accounts::{Accounts, Secrets, SessionState, sign_in::Stage},
    instances::{CrashInfo, Instances, Versions},
    shell::{InstanceTab, LauncherShell},
};

pub const SCENES: &[&str] = &[
    "signed-out",
    "home",
    "account-menu",
    "needs-sign-in",
    "sign-in",
    "sign-in-browser",
    "sign-in-code",
    "sign-in-not-approved",
    "launcher-identity",
    "sign-in-launcher",
    "account-menu-launcher",
    "offline",
    "remove",
    "new-instance",
    "play-progress",
    "playing",
    "logs",
    "instance-settings",
    "crash",
    "rosetta",
    "content-mods",
    "content-browse",
    "content-project",
    "content-resourcepacks",
    "content-shaders",
    "content-shaders-missing",
    "content-shaders-vanilla",
    "loader-change",
    "shared-settings",
    "instance-shared-settings",
    "import",
    "import-progress",
];

/// Scenes that show made-up instances.
fn has_instances(scene: &str) -> bool {
    matches!(
        scene,
        "play-progress" | "playing" | "logs" | "instance-settings" | "crash" | "rosetta"
    ) || scene.starts_with("content-")
        || scene == "loader-change"
        || shared::is_scene(scene)
        || scene.starts_with("import")
}

pub fn requested_scene() -> Option<String> {
    let mut args = std::env::args().skip_while(|arg| arg != "--scene");
    args.next()?;
    let scene = args.next()?;
    if !SCENES.contains(&scene.as_str()) {
        eprintln!("unknown scene {scene:?}; expected one of {SCENES:?}");
        std::process::exit(2);
    }
    Some(scene)
}

/// The preview's made-up Microsoft account.
fn microsoft_account() -> Account {
    Account::microsoft(
        "8667ba71-b85a-4004-af54-457a9734eed7"
            .parse()
            .expect("a valid UUID"),
        "Lodestoner",
        Some("https://textures.minecraft.net/texture/0decafc0ffee".into()),
    )
}

pub fn accounts(scene: &str, cx: &mut App) -> Entity<Accounts> {
    let dir =
        std::env::temp_dir().join(format!("lodestone-launcher-preview-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    // Remove the made-up data when the screenshot has been taken.
    cx.on_app_quit({
        let dir = dir.clone();
        move |_| {
            let _ = std::fs::remove_dir_all(&dir);
            async {}
        }
    })
    .detach();
    let data_dir = DataDir::at(dir);
    let http = launcher_core::http::default_client().expect("could not start the HTTP client");
    let scene = scene.to_string();
    cx.new(|cx| {
        let mut accounts = Accounts::new(
            data_dir.clone(),
            http,
            MsaConfig::from_env(),
            Secrets::Memory(MemorySecretStore::new()),
            true,
            cx,
        );
        if scene == "signed-out" {
            return accounts;
        }
        let mut microsoft = microsoft_account();
        if matches!(scene.as_str(), "sign-in-launcher" | "account-menu-launcher") {
            accounts.preview_launcher_identity(true);
            microsoft =
                microsoft.with_client(launcher_core::auth::ClientIdentity::MinecraftLauncher);
        }
        accounts.insert_for_preview(Account::offline("Dev_Steve").expect("a valid name"));
        accounts.insert_for_preview(microsoft.clone());
        // A head rendered from a made-up skin, through the same code as real ones.
        let head = (|| {
            let skin_png = skin::encode_png(&skin::sample_skin(true)).ok()?;
            let head = skin::render_head(&skin_png, skin::HEAD_SCALE).ok()?;
            let hash = skin::texture_hash(microsoft.skin_url.as_deref()?)?;
            let path = skin::head_path(&data_dir.avatar_cache_dir(), &hash);
            storage::write_atomic(&path, &skin::encode_png(&head).ok()?).ok()?;
            Some(path)
        })();
        if let Some(path) = head {
            accounts.set_avatar(&microsoft.id, path);
        }
        if scene == "needs-sign-in" {
            accounts.set_session_state(&microsoft.id, SessionState::NeedsSignIn);
        }
        accounts
    })
}

/// The instances, with made-up ones for the instance scenes. The version list is
/// Mojang's recorded one from the tests, so nothing is downloaded.
pub fn instances(scene: &str, accounts: &Entity<Accounts>, cx: &mut App) -> Entity<Instances> {
    let data_dir = accounts.read(cx).data_dir().clone();
    if has_instances(scene) {
        let store = InstanceStore::new(data_dir.instances_dir());
        let loader = |kind, version: &str| {
            Some(launcher_core::game::loaders::LoaderChoice {
                kind,
                version: version.to_string(),
            })
        };
        use launcher_core::game::loaders::LoaderKind;
        for (name, version, choice, group, played) in [
            (
                "Survival 1.20.1",
                "1.20.1",
                loader(LoaderKind::Fabric, "0.16.14"),
                Some("Survival"),
                0,
            ),
            ("Latest release", "26.3", None, None, 2 * 3600 + 25 * 60),
            (
                "All the mods",
                "1.20.1",
                loader(LoaderKind::Forge, "1.20.1-47.4.28"),
                Some("Modded"),
                5 * 3600,
            ),
            ("Old school", "1.8.9", None, Some("Survival"), 40 * 60),
        ] {
            if let Ok(mut instance) = store.create(name, version, choice.as_ref()) {
                instance.group = group.map(str::to_string);
                let _ = store.save(&instance);
                if name == "Survival 1.20.1"
                    && (scene.starts_with("content-") || scene == "loader-change")
                {
                    content::fill(&store.dir(&instance.id), scene);
                }
                if played > 0 {
                    let _ = store.record_play(&instance.id, 1_762_500_000, played);
                }
            }
        }
        if shared::is_scene(scene) {
            shared::fill(&data_dir, &store);
        }
    }
    let accounts = accounts.clone();
    let scene = scene.to_string();
    cx.new(|cx| {
        let mut instances = Instances::new(data_dir, accounts, cx);
        // The screenshots never touch the network.
        instances.preview_offline();
        let manifest: MojangVersionManifest = serde_json::from_str(include_str!(
            "../../launcher-core/testdata/version_manifest_v2.json"
        ))
        .expect("the recorded manifest parses");
        instances.set_versions(Versions::Ready(Arc::new(manifest.versions)), cx);
        let fabric = launcher_core::game::loaders::meta::parse_fabric_versions(
            include_bytes!("../../launcher-core/testdata/loaders/fabric/loaders-1.20.1.json"),
            "1.20.1",
        )
        .expect("the recorded Fabric list parses");
        instances.set_loader_versions(
            launcher_core::game::loaders::LoaderKind::Fabric,
            "1.20.1",
            crate::instances::LoaderVersions::Ready(Arc::new(fabric)),
        );
        let wanted = match scene.as_str() {
            "content-shaders-vanilla" => Some("Latest release"),
            "instance-shared-settings" => Some("Survival 1.20.1"),
            scene if scene.starts_with("content-") || scene == "loader-change" => {
                Some("Survival 1.20.1")
            }
            _ => None,
        };
        let Some(first) = instances
            .list()
            .iter()
            .find(|i| match wanted {
                Some(name) => i.name == name,
                None => i.game_version == "1.20.1",
            })
            .cloned()
        else {
            return instances;
        };
        instances.select(&first.id, cx);
        match scene.as_str() {
            "play-progress" => {
                let progress = Arc::new(InstallProgress::default());
                progress.set_stage(InstallStage::Assets);
                progress.downloads.set(ProgressSnapshot {
                    files_done: 1234,
                    files_total: 4567,
                    bytes_done: 210 * 1024 * 1024,
                    bytes_total: 380 * 1024 * 1024,
                });
                instances.preview_preparing(&first.id, progress);
            }
            "logs" | "crash" => instances.preview_log(&first.id, sample_log()),
            "playing" => instances.preview_play_state(crate::instances::PlayState::Playing),
            _ => {}
        }
        instances
    })
}

/// A made-up console: launcher lines, Log4j events at each level, and a stack trace.
fn sample_log() -> Vec<LogLine> {
    let base = 1_762_500_000_000u64;
    let event = |offset: u64, level: Level, thread: &str, message: &str| LogLine {
        level,
        timestamp: Some(base + offset),
        logger: None,
        thread: Some(thread.into()),
        message: message.into(),
        throwable: None,
        stream: Stream::Stdout,
    };
    let launcher = |offset: u64, message: &str| LogLine {
        stream: Stream::Launcher,
        ..event(offset, Level::Info, "Launcher", message)
    };
    let mut lines = vec![
        launcher(0, "Preparing Minecraft 1.20.1"),
        launcher(
            900,
            "Starting Java: /…/runtimes/java-runtime-gamma/mac-os-arm64/jre.bundle/Contents/Home/bin/java -XstartOnFirstThread … --accessToken <redacted> …",
        ),
        event(2_100, Level::Info, "main", "Setting user: Lodestoner"),
        event(
            2_400,
            Level::Info,
            "main",
            "Backend library: LWJGL version 3.3.1 SNAPSHOT",
        ),
        event(
            3_000,
            Level::Warn,
            "Render thread",
            "Missing sound for event: minecraft:item.goat_horn.play",
        ),
        event(
            3_200,
            Level::Info,
            "Render thread",
            "Reloading ResourceManager: vanilla",
        ),
        event(
            4_100,
            Level::Info,
            "Worker-Main-2",
            "Found file library path: /…/instances/survival-1-20-1/natives",
        ),
        event(5_000, Level::Info, "Render thread", "Sound engine started"),
        event(
            5_200,
            Level::Info,
            "Render thread",
            "Created: 1024x1024x4 minecraft:textures/atlas/blocks.png-atlas",
        ),
        event(
            6_000,
            Level::Debug,
            "Render thread",
            "Narrator library for arm64 successfully loaded",
        ),
        event(
            7_300,
            Level::Warn,
            "Server thread",
            "Can't keep up! Is the server overloaded? Running 2034ms or 40 ticks behind",
        ),
    ];
    let mut error = event(
        8_000,
        Level::Error,
        "Render thread",
        "Unreported exception thrown!",
    );
    error.throwable = Some(
        "java.lang.IllegalStateException: Lock is no longer valid
	at net.minecraft.util.ThreadingDetector.checkAndLock(ThreadingDetector.java:32)
	at net.minecraft.client.Minecraft.run(Minecraft.java:735)"
            .into(),
    );
    lines.push(error);
    lines.push(event(8_500, Level::Info, "Render thread", "Stopping!"));
    lines
}

mod content;
mod import;
mod shared;

pub use shared::window_height;

pub fn apply(scene: &str, shell: &mut LauncherShell, cx: &mut Context<LauncherShell>) {
    match scene {
        "new-instance" => shell.open_new_instance_with(Some(("Survival", "1.")), None, cx),
        "logs" => shell.select_tab(InstanceTab::Logs, cx),
        "instance-settings" => {
            shell.select_tab(InstanceTab::Settings, cx);
            if let Some(settings) = shell.settings_view() {
                settings.update(cx, |settings, _| settings.preview_java("17.0.15", 17));
            }
        }
        "crash" => shell.show_crash(
            CrashInfo {
                instance_name: "Survival 1.20.1".into(),
                summary: CrashSummary {
                    exception: Some("java.lang.OutOfMemoryError: Java heap space".into()),
                    description: Some("Initializing game".into()),
                    causes: vec![Cause::OutOfMemory],
                },
                exit_code: Some(-1),
                report: Some("crash-reports/crash-2026-11-07_12.00.00-client.txt".into()),
                log_text: String::new(),
            },
            cx,
        ),
        "rosetta" => shell.preview_launch_failed(
            "Rosetta 2 is needed",
            launcher_core::game::platform::ROSETTA_MISSING,
            cx,
        ),
        scene if scene.starts_with("content-") => {
            shell.select_tab(InstanceTab::Content, cx);
            let view = shell.content().clone();
            view.update(cx, |view, cx| content::apply(scene, view, cx));
        }
        "loader-change" => {
            shell.select_tab(InstanceTab::Settings, cx);
            let prompt = shell.settings_view().and_then(|settings| {
                settings.read(cx).preview_loader_change(
                    launcher_core::instances::Loader::Forge,
                    "1.20.1-47.4.28",
                    cx,
                )
            });
            if let Some(prompt) = prompt {
                shell.show_loader_change(prompt, cx);
            }
        }
        "shared-settings" => {
            shell.show(crate::shell::Page::Settings, cx);
            shell.shared_settings().clone().update(cx, |shared, cx| {
                shared.preview_expand(&[launcher_core::shared_options::Category::Video], cx)
            });
        }
        "instance-shared-settings" => {
            shell.select_tab(InstanceTab::Settings, cx);
            if let Some(settings) = shell.settings_view() {
                let shared = settings.read(cx).shared_settings().clone();
                shared.update(cx, |shared, cx| {
                    shared.preview_expand(&[launcher_core::shared_options::Category::Video], cx)
                });
            }
        }
        "import" | "import-progress" => import::apply(scene, shell, cx),
        "account-menu" | "needs-sign-in" | "account-menu-launcher" => shell.open_account_menu(cx),
        "launcher-identity" => shell.preview_enable_launcher_identity(cx),
        "sign-in-launcher" => shell.open_sign_in_at(Stage::Choose, None, cx),
        "sign-in" => shell.open_sign_in_at(Stage::Choose, None, cx),
        "sign-in-browser" => shell.open_sign_in_at(Stage::Browser, None, cx),
        "sign-in-code" => shell.open_sign_in_at(
            Stage::DeviceCode {
                user_code: "F7K2-9QXM".into(),
                verification_uri: launcher_core::auth::DEVICE_LOGIN_URL.into(),
                opened: false,
            },
            None,
            cx,
        ),
        "sign-in-not-approved" => {
            shell.open_sign_in_at(Stage::Failed(AuthError::AppNotApproved), None, cx)
        }
        "offline" => shell.open_offline(Some("Alex_Dev"), None, cx),
        "remove" => {
            let id = microsoft_account().id;
            shell.confirm_remove(&id, None, cx);
        }
        _ => {}
    }
}
