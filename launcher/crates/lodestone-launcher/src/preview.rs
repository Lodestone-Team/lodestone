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
        crash::{
            Cause, CrashSummary,
            suspects::{Evidence, Suspect},
        },
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
    "skin-grassblock",
    "skin-glass",
    "skin-neon",
    "skin-overrides",
    "skin-bright",
    "settings-skin-bright",
    "skin-showcase",
    "settings-skin-showcase",
    "skin-materials",
    "settings-skin-materials",
    "skin-banner",
    "background-picker-banners",
    "skin-editor",
    "skin-editor-gradient",
    "skin-editor-fonts",
    "skin-editor-banners",
    "skin-editor-readability",
    "skin-editor-material",
    "sidebar-resized",
    "sidebar-rail",
    "sidebar-rail-skin",
    "sidebar-rail-host",
    "skin-bright-host",
    "settings-skin",
    "account-menu",
    "needs-sign-in",
    "sign-in",
    "sign-in-browser",
    "sign-in-code",
    "offer-main",
    "confirm-main",
    "sign-in-stop",
    "sign-in-not-approved",
    "launcher-identity",
    "sign-in-launcher",
    "account-menu-launcher",
    "offline",
    "remove",
    "new-instance",
    "new-instance-fabric",
    "new-instance-versions",
    "new-instance-versions-failed",
    "play-progress",
    "playing",
    "launching",
    "logs",
    "logs-select",
    "logs-scroll",
    "instance-settings",
    "crash",
    "crash-mods",
    "mod-finder",
    "mod-finder-found",
    "rosetta",
    "content-mods",
    "content-browse",
    "content-search",
    "content-search-modrinth-only",
    "content-menu",
    "content-project",
    "content-resourcepacks",
    "content-shaders",
    "content-shaders-missing",
    "content-shaders-vanilla",
    "content-formatted",
    "content-formatted-shaders",
    "content-description",
    "loader-change",
    "new-instance-modpack",
    "new-instance-code",
    "new-instance-code-pack",
    "share-code",
    "share-code-pack",
    "modpack-importing",
    "modpack-settings",
    "modpack-update",
    "worlds",
    "worlds-backups",
    "screenshots",
    "screenshot-view",
    "shared-settings",
    "instance-shared-settings",
    "import",
    "import-progress",
    "import-stop",
    "instance-linked",
    "settings",
    "settings-general",
    "undo-toast",
    "narrator-offer",
    "settings-accounts",
    "settings-developer",
    "settings-developer-relay-default",
    "settings-developer-relay",
    "settings-developer-relay-error",
    "perf-monitor",
    "settings-storage",
    "settings-storage-confirm",
    "settings-storage-moved",
    "settings-storage-move",
    "play-background",
    "play-background-dark",
    "icon-picker",
    "icon-picker-emoji",
    "background-picker",
    "play-slideshow",
    "play-slideshow-fade",
    "background-picker-slideshow",
    "instance-settings-narrow",
    "instance-settings-narrow-open",
    "settings-narrow",
    "settings-narrow-open",
    "settings-unsaved",
    "settings-unsaved-prompt",
    "instance-settings-unsaved",
    "focus-dialog",
    "focus-settings-row",
    "focus-home",
    "focus-popover",
    "focus-escape",
    "focus-escape-prompt",
    "snapshots",
    "snapshot-take",
    "snapshot-revert",
    "snapshot-share",
    "snapshot-delete",
    "snapshot-import",
];

/// Keys pressed once a scene has settled, before the screenshot, to show where
/// keyboard focus goes and its ring.
pub fn keystrokes(scene: &str) -> &'static [&'static str] {
    match scene {
        // The Remove dialog takes focus when it opens; Tab reaches its ×, Remove, then
        // Cancel.
        "focus-dialog" => &["tab", "tab", "tab"],
        // Settings > Instance defaults: past Manage accounts, the seven pages and the
        // four fields, to the Fullscreen switch. (The dialog's × comes last.)
        "focus-settings-row" => &[
            "tab", "tab", "tab", "tab", "tab", "tab", "tab", "tab", "tab", "tab", "tab", "tab",
            "tab",
        ],
        // From the window's root: the logo, the account box, then the sidebar's New
        // instance.
        "focus-home" => &["tab", "tab", "tab"],
        // Shared game settings: past Manage accounts and the seven pages, the Sync
        // from list opened with Enter (it takes focus), then Tab to its second row.
        "focus-popover" => &[
            "tab", "tab", "tab", "tab", "tab", "tab", "tab", "tab", "tab", "enter", "tab", "tab",
        ],
        // The same list closed again with Escape: focus is back on its button, and the
        // Settings dialog stays open.
        "focus-escape" => &[
            "tab", "tab", "tab", "tab", "tab", "tab", "tab", "tab", "tab", "enter", "escape",
        ],
        // "Save your changes?", opened without a window, closed with Escape; Settings
        // stays open and takes focus back.
        "focus-escape-prompt" => &["escape", "tab"],
        // Settings > General with "Group instances automatically" turned off, then
        // Undo: the switch is on again and the toast offers Redo.
        "undo-toast" if cfg!(target_os = "macos") => &["cmd-z"],
        "undo-toast" => &["ctrl-z"],
        _ => &[],
    }
}

/// A skin that shows what skins can do: gradients (two-colour, many-colour and
/// radial), rounded and bordered panels, its own fonts and theme colours.
fn showcase_skin() -> launcher_core::ui_skin::SkinSpec {
    use launcher_core::ui_skin::{
        Fill, Gradient, GradientKind, Rgb, Shape, SkinSpec, Stop, SurfaceId, SurfaceSpec, Tokens,
    };
    let stops = |colors: &[u32]| {
        let last = colors.len() - 1;
        colors
            .iter()
            .enumerate()
            .map(|(i, c)| Stop {
                color: Rgb(*c),
                at: (i * 100 / last) as u8,
            })
            .collect::<Vec<_>>()
    };
    let linear = |angle: i16, colors: &[u32]| {
        Fill::Gradient(Gradient {
            kind: GradientKind::Linear,
            angle,
            stops: stops(colors),
        })
    };
    let panel = |fill: Fill, radius: u8| SurfaceSpec {
        fill: Some(fill),
        border: Some(Rgb(0x4A2380)),
        radius: Some(radius),
        ..SurfaceSpec::default()
    };
    let mut spec = SkinSpec::named("Showcase");
    spec.author = Some("Lodestone".into());
    spec.accent = Some(Rgb(0xF72585));
    spec.play.shape = Shape::Pill;
    spec.play.glow = true;
    spec.surfaces = [
        (
            SurfaceId::Window,
            SurfaceSpec {
                fill: Some(Fill::Gradient(Gradient {
                    kind: GradientKind::Radial,
                    angle: 180,
                    stops: stops(&[0x3A0F6B, 0x14062B, 0x09030F]),
                })),
                ..SurfaceSpec::default()
            },
        ),
        (
            SurfaceId::TopBar,
            panel(linear(90, &[0x2A0A4A, 0x5B1A8C, 0x8A1F6E]), 0),
        ),
        (
            SurfaceId::Sidebar,
            SurfaceSpec {
                fill: Some(linear(180, &[0x1E0838, 0x100422])),
                text: Some(Rgb(0xFBF2FF)),
                muted: Some(Rgb(0xD9C3EC)),
                ..SurfaceSpec::default()
            },
        ),
        (
            SurfaceId::PlayBar,
            panel(linear(0, &[0x12051F, 0x220A3C]), 0),
        ),
        (SurfaceId::Card, panel(Fill::solid(Rgb(0x1C0A33)), 16)),
        (
            SurfaceId::SettingCard,
            panel(Fill::solid(Rgb(0x1C0A33)), 16),
        ),
        (SurfaceId::Dialog, panel(Fill::solid(Rgb(0x16082A)), 20)),
        (
            SurfaceId::SettingsNav,
            panel(linear(180, &[0x1E0838, 0x100422]), 0),
        ),
        (SurfaceId::Popover, panel(Fill::solid(Rgb(0x22103E)), 12)),
        (
            SurfaceId::Input,
            SurfaceSpec {
                fill: Some(Fill::solid(Rgb(0x0E0519))),
                border: Some(Rgb(0x7B2CBF)),
                radius: Some(10),
                ..SurfaceSpec::default()
            },
        ),
        (
            SurfaceId::ButtonPrimary,
            SurfaceSpec {
                fill: Some(linear(90, &[0xC9184A, 0x7209B7])),
                radius: Some(12),
                ..SurfaceSpec::default()
            },
        ),
        (SurfaceId::Toggle, panel(Fill::solid(Rgb(0xF72585)), 0)),
        (
            SurfaceId::Pill,
            SurfaceSpec {
                fill: Some(Fill::solid(Rgb(0xB5179E))),
                text: Some(Rgb::WHITE),
                ..SurfaceSpec::default()
            },
        ),
        (
            SurfaceId::PlayButton,
            SurfaceSpec {
                fill: Some(linear(90, &[0xF72585, 0xB5179E, 0x7209B7, 0x3A0CA3])),
                text: Some(Rgb::WHITE),
                radius: Some(30),
                ..SurfaceSpec::default()
            },
        ),
        (
            SurfaceId::Progress,
            panel(linear(90, &[0xF72585, 0x4CC9F0]), 0),
        ),
        (SurfaceId::Toast, panel(Fill::solid(Rgb(0x2A1150)), 12)),
    ]
    .into_iter()
    .collect();
    spec.tokens = Tokens {
        text: Some(Rgb(0xF6EEFF)),
        secondary: Some(Rgb(0xD9C8F0)),
        muted: Some(Rgb(0xC0A9DE)),
        faint: Some(Rgb(0xA98CCB)),
        border: Some(Rgb(0x3D1D66)),
    };
    spec.fonts.play = Some("Lodestone Pixel".into());
    spec.fonts.button = Some("Clash Grotesk".into());
    spec.fonts.tab = Some("Clash Grotesk".into());
    spec
}

/// A skin of see-through panels over a busy picture: frosted, blurred and
/// translucent, for the material scenes.
fn materials_skin() -> launcher_core::ui_skin::SkinSpec {
    use launcher_core::ui_skin::{
        Fill, ImageFill, Material, MaterialKind, Rgb, SkinSpec, SurfaceId, SurfaceSpec, Tokens,
    };
    let glass = |kind: MaterialKind, color: u32, radius: u8| SurfaceSpec {
        fill: Some(Fill::solid(Rgb(color))),
        border: Some(Rgb(0x5A6A8C)),
        radius: Some(radius),
        material: Some(Material::of(kind)),
        ..SurfaceSpec::default()
    };
    let bar = |material: Material, color: u32| SurfaceSpec {
        fill: Some(Fill::solid(Rgb(color))),
        material: Some(material),
        ..SurfaceSpec::default()
    };
    let mut spec = SkinSpec::named("Materials");
    spec.author = Some("Lodestone".into());
    spec.accent = Some(Rgb(0x9FD6FF));
    spec.surfaces = [
        (
            SurfaceId::Window,
            SurfaceSpec {
                fill: Some(Fill::Image(ImageFill {
                    image: "builtin:aurora".into(),
                    fit: launcher_core::ui_skin::Fit::Cover,
                    tint: None,
                    tint_opacity: 0,
                    blur: 0,
                })),
                ..SurfaceSpec::default()
            },
        ),
        (
            SurfaceId::TopBar,
            bar(Material::of(MaterialKind::Frost), 0x0C1222),
        ),
        (
            SurfaceId::Sidebar,
            SurfaceSpec {
                text: Some(Rgb(0xF4F7FF)),
                muted: Some(Rgb(0xD0D8EA)),
                ..bar(
                    Material {
                        blur: Some(28),
                        opacity: Some(50),
                        ..Material::of(MaterialKind::Blur)
                    },
                    0x080C18,
                )
            },
        ),
        (
            SurfaceId::PlayBar,
            bar(Material::of(MaterialKind::Blur), 0x080C18),
        ),
        (SurfaceId::Card, glass(MaterialKind::Frost, 0x141C30, 14)),
        (
            SurfaceId::Row,
            glass(MaterialKind::Translucent, 0x0C1222, 10),
        ),
        (
            SurfaceId::Input,
            glass(MaterialKind::Translucent, 0x0A0F1C, 8),
        ),
        (SurfaceId::Dialog, glass(MaterialKind::Frost, 0x101828, 16)),
        (
            SurfaceId::SettingsNav,
            bar(Material::of(MaterialKind::Blur), 0x0A101E),
        ),
        (
            SurfaceId::SettingCard,
            glass(MaterialKind::Translucent, 0x0C1222, 12),
        ),
        (SurfaceId::Popover, glass(MaterialKind::Frost, 0x121A2C, 12)),
        (SurfaceId::Toast, glass(MaterialKind::Frost, 0x121A2C, 12)),
    ]
    .into_iter()
    .collect();
    spec.tokens = Tokens {
        text: Some(Rgb(0xF4F7FF)),
        secondary: Some(Rgb(0xDCE3F2)),
        muted: Some(Rgb(0xC4CEE2)),
        faint: Some(Rgb(0xAEB9D0)),
        border: Some(Rgb(0x4A5878)),
    };
    spec
}

/// The "Bright" skin: a bright sky behind the sidebar and no tint. The launcher
/// leaves its text alone, and the editor suggests fixes.
fn use_bright_skin(shell: &mut LauncherShell, cx: &mut gpui::Context<LauncherShell>) {
    let instances = shell.instances().clone();
    let store = instances.read(cx).skins();
    let mut spec = launcher_core::ui_skin::SkinSpec::named("Bright");
    spec.sidebar.image = Some("builtin:sky".into());
    spec.banner.in_sidebar = true;
    if store.save("bright", &spec).is_ok() {
        instances.update(cx, |instances, cx| {
            instances.set_skin(Some("bright".into()), cx)
        });
    }
}

/// The Lodestone Host page with no cores, and nothing found on this computer: the
/// sidebar's Lodestone Host selected, and the page offering Lodestone Host.
fn show_empty_host_page(shell: &mut LauncherShell, cx: &mut gpui::Context<LauncherShell>) {
    shell
        .servers()
        .clone()
        .update(cx, |servers, _| servers.preview(Vec::new(), Vec::new()));
    shell.show(crate::shell::Page::Servers, cx);
}

/// Scenes that show instance icons and Play backgrounds.
fn is_appearance_scene(scene: &str) -> bool {
    scene.starts_with("play-background")
        || scene.starts_with("icon-picker")
        || scene.starts_with("background-picker")
        || scene == "skin-banner"
        || scene.starts_with("play-slideshow")
}

/// A made-up landscape for the background scenes: a pale sky over a snowfield (bright
/// under the text), or a night sky over dark hills.
fn background_image(bright: bool) -> image::RgbaImage {
    image::RgbaImage::from_fn(1600, 900, |x, y| {
        let (fx, fy) = (x as f32 / 1600.0, y as f32 / 900.0);
        let hill = 0.55 + 0.08 * (fx * 7.0).sin() + 0.05 * (fx * 17.0 + 1.0).sin();
        let ground = fy > hill;
        let rgb = match (bright, ground) {
            (true, true) => [236, 242, 248],
            (true, false) => [(150.0 + 90.0 * fy) as u8, (200.0 + 50.0 * fy) as u8, 255],
            (false, true) => [18, 30, 26],
            (false, false) => [
                (14.0 + 20.0 * fy) as u8,
                (18.0 + 26.0 * fy) as u8,
                (40.0 + 50.0 * fy) as u8,
            ],
        };
        image::Rgba([rgb[0], rgb[1], rgb[2], 255])
    })
}

/// Scenes of the Worlds tab.
fn is_worlds_scene(scene: &str) -> bool {
    scene.starts_with("worlds") || scene.starts_with("screenshot")
}

/// Scenes that show made-up instances.
fn has_instances(scene: &str) -> bool {
    matches!(
        scene,
        "play-progress"
            | "playing"
            | "launching"
            | "offer-main"
            | "confirm-main"
            | "logs"
            | "logs-select"
            | "logs-scroll"
            | "instance-settings"
            | "crash"
            | "crash-mods"
            | "mod-finder"
            | "mod-finder-found"
            | "rosetta"
    ) || scene.starts_with("content-")
        || scene.starts_with("modpack-")
        || scene == "new-instance-modpack"
        || scene == "share-code"
        || scene == "share-code-pack"
        || is_worlds_scene(scene)
        || snapshots::is_scene(scene)
        || scene == "loader-change"
        || shared::is_scene(scene)
        || scene.starts_with("instance-settings-narrow")
        || scene == "instance-settings-unsaved"
        || scene.starts_with("import")
        || servers::has_instances(scene)
        || scene == "instance-linked"
        || is_appearance_scene(scene)
        || scene.starts_with("skin-")
        || matches!(
            scene,
            "sidebar-resized" | "sidebar-rail" | "sidebar-rail-skin" | "sidebar-rail-host"
        )
}

/// `--lang <tag>`: shows the scene in that language (`en`, `fr`, `zh-Hans`,
/// `zh-Hant`) whatever the system's languages are. Without it, scenes follow the
/// Language setting's default, System.
pub fn requested_locale() -> Option<crate::i18n::Locale> {
    let mut args = std::env::args().skip_while(|arg| arg != "--lang");
    args.next()?;
    let tag = args.next()?;
    match crate::i18n::Locale::from_tag(&tag) {
        Some(locale) => Some(locale),
        None => {
            eprintln!("unknown language {tag:?}; expected en, fr, zh-Hans or zh-Hant");
            std::process::exit(2);
        }
    }
}

/// `--zoom 1.5`: the interface's zoom for the screenshot.
pub fn requested_zoom() -> Option<f32> {
    let mut args = std::env::args().skip_while(|arg| arg != "--zoom");
    args.next()?;
    args.next()?.parse().ok()
}

pub fn requested_scene() -> Option<String> {
    let mut args = std::env::args().skip_while(|arg| arg != "--scene");
    args.next()?;
    let scene = args.next()?;
    if !SCENES.contains(&scene.as_str())
        && !groups::is_scene(&scene)
        && !servers::SCENES.contains(&scene.as_str())
        && !server_detail::is_scene(&scene)
    {
        eprintln!(
            "unknown scene {scene:?}; expected one of {SCENES:?}, {:?}, {:?} or {:?}",
            groups::SCENES,
            servers::SCENES,
            server_detail::SCENES
        );
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
    // A folder of its own each time, so tests running side by side don't remove
    // each other's.
    static MADE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let made = MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "lodestone-launcher-preview-{}-{made}",
        std::process::id()
    ));
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
        // The scenes show Lodestone's own sign-in, apart from the "-launcher" ones.
        accounts.preview_launcher_identity(false);
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
                if is_appearance_scene(scene) {
                    let icon = match name {
                        "Latest release" => Some("emoji:1D8EB2:🚀"),
                        "All the mods" => Some("builtin:gold-ore"),
                        "Old school" => Some("builtin:cobblestone"),
                        _ => None,
                    };
                    let _ = store.set_icon_name(&instance.id, icon);
                    if name == "Survival 1.20.1" {
                        worlds::fill(&store.dir(&instance.id));
                        if scene.starts_with("play-background") {
                            let file = data_dir.root().join("background-preview.png");
                            let bright = scene == "play-background";
                            if background_image(bright).save(&file).is_ok() {
                                let _ = store.set_background(&instance.id, &file);
                            }
                        }
                        // A slideshow of its own two images: the bright one, then the dark.
                        if scene.starts_with("play-slideshow")
                            || scene == "background-picker-slideshow"
                        {
                            for bright in [true, false] {
                                let file = data_dir.root().join(format!("slide-{bright}.png"));
                                if background_image(bright).save(&file).is_ok() {
                                    let _ = store.add_slideshow_image(&instance.id, &file);
                                }
                            }
                            let _ = store.set_slideshow(
                                &instance.id,
                                Some(launcher_core::instances::SlideshowSource::Custom),
                            );
                        }
                    }
                }
                if name == "Survival 1.20.1" && snapshots::is_scene(scene) {
                    snapshots::fill(&data_dir, &store, &instance);
                }
                if name == "Survival 1.20.1" && is_worlds_scene(scene) {
                    worlds::fill(&store.dir(&instance.id));
                }
                if name == "Survival 1.20.1" && scene.starts_with("modpack-") {
                    modpacks::fill(&store.dir(&instance.id));
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
    if groups::is_scene(scene) {
        let store = InstanceStore::new(data_dir.instances_dir());
        groups::fill(&data_dir, &store, scene);
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
            "instance-shared-settings" | "instance-linked" => Some("Survival 1.20.1"),
            scene if groups::is_scene(scene) => Some(groups::SELECTED),
            scene if is_appearance_scene(scene) => Some("Survival 1.20.1"),
            scene
                if scene.starts_with("content-")
                    || scene.starts_with("modpack-")
                    || is_worlds_scene(scene)
                    || scene == "loader-change" =>
            {
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
            "logs" | "logs-select" | "crash" | "crash-mods" => {
                instances.preview_log(&first.id, sample_log())
            }
            "logs-scroll" => instances.preview_log(&first.id, long_log()),
            "playing" => instances.preview_play_state(crate::instances::PlayState::Playing),
            "launching" => instances.preview_play_state(crate::instances::PlayState::Preparing {
                label: crate::i18n::t(crate::i18n::InstancesMsg::StageLaunching).into(),
                detail: None,
                fraction: None,
            }),
            "modpack-importing" => modpacks::importing(&mut instances, cx),
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

/// The made-up console with many more lines than fit, so it scrolls.
fn long_log() -> Vec<LogLine> {
    let mut lines = sample_log();
    let template = lines[2].clone();
    let base = template.timestamp.unwrap_or_default();
    let more = (0..80).map(|i| LogLine {
        timestamp: Some(base + 10 * i),
        thread: Some(format!("Worker-Main-{}", i % 4 + 1)),
        message: format!("Preparing spawn area: {}%", i * 5 / 4),
        ..template.clone()
    });
    lines.splice(3..3, more);
    lines
}

mod content;
mod friend_list;
mod groups;
mod import;
mod modpacks;
mod server_detail;
mod servers;
mod shared;
mod snapshots;
mod worlds;

pub use shared::{window_height, window_width};

pub fn apply(scene: &str, shell: &mut LauncherShell, cx: &mut Context<LauncherShell>) {
    match scene {
        "new-instance" => shell.open_new_instance_with(Some(("Survival", "")), None, cx),
        "new-instance-fabric" | "new-instance-versions" => {
            let search = if scene == "new-instance-versions" {
                "26"
            } else {
                ""
            };
            shell.open_new_instance_with(Some(("", search)), None, cx);
            if let Some(dialog) = shell.new_instance_dialog().cloned() {
                dialog.update(cx, |dialog, cx| {
                    if scene == "new-instance-fabric" {
                        dialog.choose_loader(launcher_core::game::loaders::LoaderKind::Fabric, cx);
                    } else {
                        dialog.open_versions(cx);
                    }
                });
            }
        }
        "new-instance-versions-failed" => {
            shell.open_new_instance_with(Some(("Crash Test", "")), None, cx);
            // as Windows words a refused connection: long, with a URL that can't wrap
            let error = "could not connect to https://piston-meta.mojang.com/mc/game/version_manifest_v2.json: could not connect: error sending request for url (https://piston-meta.mojang.com/mc/game/version_manifest_v2.json): client error (Connect): tcp connect error: An attempt was made to access a socket in a way forbidden by its access permissions. (os error 10013)";
            shell.instances().clone().update(cx, |instances, cx| {
                instances.set_versions(Versions::Failed(error.into()), cx)
            });
            if let Some(dialog) = shell.new_instance_dialog().cloned() {
                dialog.update(cx, |dialog, cx| dialog.open_versions(cx));
            }
        }
        "new-instance-code" => {
            shell.open_new_instance_with(None, None, cx);
            let code = sample_share_plan().code.encode();
            if let Some(dialog) = shell.new_instance_dialog().cloned() {
                dialog.update(cx, |dialog, cx| dialog.paste_code(&code, cx));
            }
        }
        "new-instance-code-pack" => {
            shell.open_new_instance_with(None, None, cx);
            let code = sample_pack_share_plan().code.encode();
            if let Some(dialog) = shell.new_instance_dialog().cloned() {
                dialog.update(cx, |dialog, cx| dialog.paste_code(&code, cx));
            }
        }
        "share-code" | "share-code-pack" => {
            shell.select_tab(InstanceTab::Settings, cx);
            let (name, plan) = if scene == "share-code" {
                ("Survival".to_string(), sample_share_plan())
            } else {
                pack_share_plan()
            };
            let dialog =
                cx.new(|cx| crate::instances::share_dialog::ShareDialog::preview(&name, plan, cx));
            shell.show_share(dialog, cx);
        }
        "logs" | "logs-scroll" => shell.select_tab(InstanceTab::Logs, cx),
        "logs-select" => {
            shell.select_tab(InstanceTab::Logs, cx);
            let console = shell.console().clone();
            console.update(cx, |console, cx| console.preview_select(cx));
        }
        "sidebar-groups-settings" => shell.select_tab(InstanceTab::Settings, cx),
        scene if scene.starts_with("sidebar-play-") || scene.starts_with("sidebar-linked") => {
            groups::apply(scene, shell, cx)
        }
        "instance-settings" => {
            shell.select_tab(InstanceTab::Settings, cx);
            if let Some(settings) = shell.settings_view() {
                settings.update(cx, |settings, _| settings.preview_java("17.0.15", 17));
            }
        }
        "crash" => shell.show_crash(
            CrashInfo {
                instance_id: "preview".into(),
                instance_name: "Survival 1.20.1".into(),
                summary: CrashSummary {
                    exception: Some("java.lang.OutOfMemoryError: Java heap space".into()),
                    description: Some("Initializing game".into()),
                    causes: vec![Cause::OutOfMemory],
                },
                suspects: Vec::new(),
                exit_code: Some(-1),
                report: Some("crash-reports/crash-2026-11-07_12.00.00-client.txt".into()),
                log_text: String::new(),
                at_startup: true,
                mods_refused: false,
            },
            cx,
        ),
        "crash-mods" => {
            // A modded instance, so the dialog offers "Find it for me".
            let (instance_id, instance_name) = shell
                .instances()
                .read(cx)
                .list()
                .iter()
                .find(|i| i.loader != launcher_core::instances::Loader::None)
                .map(|i| (i.id.clone(), i.name.clone()))
                .unwrap_or_default();
            shell.show_crash(
            CrashInfo {
                instance_id,
                instance_name,
                summary: CrashSummary {
                    exception: Some(
                        "java.lang.NullPointerException: Cannot invoke \"net.minecraft.class_1799.method_7909()\" because \"stack\" is null".into(),
                    ),
                    description: Some("Rendering screen".into()),
                    causes: Vec::new(),
                },
                suspects: [
                    ("zenith-1.2.3.jar", "Zenith", Evidence::PatchedCrashingCode),
                    ("fancymenu-3.5.0.jar", "FancyMenu", Evidence::PatchedCrashingCode),
                    ("sodium-0.6.0.jar", "Sodium", Evidence::InStack),
                ]
                .into_iter()
                .map(|(file_name, name, evidence)| Suspect {
                    file_name: file_name.into(),
                    name: name.into(),
                    evidence,
                })
                .collect(),
                exit_code: Some(-1),
                report: Some("crash-reports/crash-2026-11-07_12.00.00-client.txt".into()),
                log_text: String::new(),
                at_startup: false,
                mods_refused: false,
            },
            cx,
        );
        }
        "mod-finder" | "mod-finder-found" => {
            let instances = shell.instances().clone();
            let id = instances
                .read(cx)
                .list()
                .iter()
                .find(|i| i.loader != launcher_core::instances::Loader::None)
                .map(|i| i.id.clone())
                .unwrap_or_default();
            let found = scene == "mod-finder-found";
            let finder = cx.new(|cx| {
                crate::instances::mod_finder::ModFinder::preview(instances, &id, found, cx)
            });
            shell.show_mod_finder(finder, cx);
        }
        "rosetta" => shell.preview_launch_failed(
            crate::i18n::t(crate::i18n::InstancesMsg::RosettaNeeded),
            crate::instances::rosetta_missing(),
            cx,
        ),
        scene if snapshots::is_scene(scene) => snapshots::apply(scene, shell, cx),
        scene if is_worlds_scene(scene) => {
            shell.select_tab(InstanceTab::Worlds, cx);
            let view = shell.worlds().clone();
            view.update(cx, |view, cx| worlds::apply(scene, view, cx));
        }
        "new-instance-modpack" | "modpack-settings" | "modpack-update" => {
            modpacks::apply(scene, shell, cx)
        }
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
        "narrator-offer" => shell.preview_narrator_offer(cx),
        "shared-settings" => {
            shell.open_settings(Some(crate::shell::SettingsPage::Shared), None, cx);
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
        "import" | "import-progress" | "import-stop" => import::apply(scene, shell, cx),
        "play-slideshow-fade" => {
            // Once the first image is up, hold the next one halfway through its fade.
            let slideshow = shell.slideshow().clone();
            cx.spawn(async move |_, cx| {
                for _ in 0..40 {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(25))
                        .await;
                    let ready = slideshow.read_with(cx, |s, _| s.is_ready());
                    if ready {
                        slideshow.update(cx, |s, cx| s.preview_fade(cx));
                        break;
                    }
                }
            })
            .detach();
        }
        "icon-picker"
        | "icon-picker-emoji"
        | "background-picker"
        | "background-picker-slideshow" => {
            shell.select_tab(InstanceTab::Settings, cx);
            if let Some(settings) = shell.settings_view().cloned() {
                settings.update(cx, |settings, cx| match scene {
                    "background-picker" | "background-picker-slideshow" => {
                        settings.preview_background_picker(cx)
                    }
                    _ => settings.preview_icon_picker(scene == "icon-picker-emoji"),
                });
            }
        }
        "instance-settings-narrow" | "instance-settings-narrow-open" => {
            shell.select_tab(InstanceTab::Settings, cx);
            if scene.ends_with("-open")
                && let Some(settings) = shell.settings_view().cloned()
            {
                settings.update(cx, |settings, _| settings.preview_nav_open());
            }
        }
        "settings-unsaved" | "settings-unsaved-prompt" => {
            shell.open_settings(Some(crate::shell::SettingsPage::Defaults), None, cx);
            shell.preview_unsaved_defaults(cx);
            if scene.ends_with("-prompt") {
                shell.preview_unsaved_prompt(cx);
            }
        }
        "instance-settings-unsaved" => {
            shell.select_tab(InstanceTab::Settings, cx);
            if let Some(settings) = shell.settings_view().cloned() {
                settings.update(cx, |settings, cx| settings.preview_change(cx));
            }
        }
        "settings-narrow" | "settings-narrow-open" => {
            shell.open_settings(Some(crate::shell::SettingsPage::Defaults), None, cx);
            if scene.ends_with("-open") {
                shell.preview_settings_nav_open();
            }
        }
        "settings" | "settings-general" | "settings-accounts" | "settings-developer" => {
            let page = match scene {
                "settings-general" => crate::shell::SettingsPage::General,
                "settings-accounts" => crate::shell::SettingsPage::Accounts,
                "settings-developer" => crate::shell::SettingsPage::Developer,
                _ => crate::shell::SettingsPage::Defaults,
            };
            if scene == "settings-developer" {
                preview_control(cx);
            }
            shell.open_settings(Some(page), None, cx);
        }
        // Settings > Developer, tall enough for "Connect service (relay)": as it is,
        // pointed at the dev service, and with a URL it refuses.
        "settings-developer-relay-default"
        | "settings-developer-relay"
        | "settings-developer-relay-error" => {
            use launcher_core::cores::connect_service::{self, Service};
            preview_control(cx);
            let dev = "https://connect-dev.lodestone.cc";
            let (setting, typed) = match scene {
                "settings-developer-relay" => (Service::from_setting(dev).ok(), dev),
                "settings-developer-relay-error" => (None, "http://connect-dev.lodestone.cc"),
                _ => (None, ""),
            };
            // set for this process only: the screenshots save nothing
            connect_service::set_setting(setting);
            shell
                .connect_service_setting()
                .update(cx, |setting, cx| setting.preview_type(typed, cx));
            shell.open_settings(Some(crate::shell::SettingsPage::Developer), None, cx);
        }
        "skin-bright" | "settings-skin-bright" | "skin-bright-host" => {
            use_bright_skin(shell, cx);
            if scene == "settings-skin-bright" {
                shell.open_settings(Some(crate::shell::SettingsPage::Skin), None, cx);
            }
            if scene == "skin-bright-host" {
                show_empty_host_page(shell, cx);
            }
        }
        "skin-showcase" | "settings-skin-showcase" => {
            let instances = shell.instances().clone();
            let store = instances.read(cx).skins();
            let mut spec = showcase_skin();
            // a font shipped in the pack: one of the system's, for the screenshot
            let font = std::path::Path::new("/System/Library/Fonts/Supplemental/Georgia.ttf");
            if font.is_file()
                && store.save("showcase", &spec).is_ok()
                && let Ok((file, _family)) = store.add_font("showcase", font)
            {
                spec.fonts.heading = Some(file);
            }
            if store.save("showcase", &spec).is_ok() {
                instances.update(cx, |instances, cx| {
                    instances.set_skin(Some("showcase".into()), cx)
                });
            }
            if scene == "settings-skin-showcase" {
                shell.open_settings(Some(crate::shell::SettingsPage::Skin), None, cx);
            }
        }
        "skin-materials" | "settings-skin-materials" => {
            let instances = shell.instances().clone();
            let store = instances.read(cx).skins();
            if store.save("materials", &materials_skin()).is_ok() {
                instances.update(cx, |instances, cx| {
                    instances.set_skin(Some("materials".into()), cx)
                });
            }
            if scene == "settings-skin-materials" {
                shell.open_settings(Some(crate::shell::SettingsPage::Skin), None, cx);
            }
        }
        "skin-editor"
        | "skin-editor-gradient"
        | "skin-editor-fonts"
        | "skin-editor-banners"
        | "skin-editor-readability"
        | "skin-editor-material" => {
            use crate::shell::skin_editor::Pane;
            use launcher_core::ui_skin::SurfaceId;
            let instances = shell.instances().clone();
            let store = instances.read(cx).skins();
            let (skin, pane) = match scene {
                "skin-editor-gradient" => ("showcase", Pane::Surface(SurfaceId::PlayButton)),
                "skin-editor-fonts" => ("showcase", Pane::Fonts),
                "skin-editor-banners" => ("grassblock", Pane::Banners),
                "skin-editor-readability" => ("bright", Pane::Readability),
                "skin-editor-material" => ("materials", Pane::Surface(SurfaceId::Card)),
                _ => ("grassblock", Pane::Surface(SurfaceId::Sidebar)),
            };
            match skin {
                "showcase" => {
                    let mut spec = showcase_skin();
                    let font =
                        std::path::Path::new("/System/Library/Fonts/Supplemental/Georgia.ttf");
                    if font.is_file()
                        && store.save("showcase", &spec).is_ok()
                        && let Ok((file, _)) = store.add_font("showcase", font)
                    {
                        spec.fonts.heading = Some(file);
                    }
                    let _ = store.save("showcase", &spec);
                }
                "materials" => {
                    let _ = store.save("materials", &materials_skin());
                }
                "bright" => {
                    let mut spec = launcher_core::ui_skin::SkinSpec::named("Bright");
                    spec.sidebar.image = Some("builtin:sky".into());
                    let _ = store.save("bright", &spec);
                }
                _ => {}
            }
            instances.update(cx, |instances, cx| {
                instances.set_skin(Some(skin.into()), cx)
            });
            let shell_handle = cx.entity().downgrade();
            cx.defer(move |cx| {
                if let Some(editor) = crate::shell::skin_editor::open(instances, shell_handle, cx) {
                    editor
                        .update(cx, |editor, _, cx| editor.show(pane, cx))
                        .ok();
                }
            });
        }
        "perf-monitor" => {
            preview_control(cx);
            shell.open_settings(Some(crate::shell::SettingsPage::Developer), None, cx);
            let sources = shell.perf_sources();
            cx.defer(move |cx| {
                if let Some(monitor) = crate::shell::perf_monitor::open(sources, cx) {
                    monitor
                        .update(cx, |monitor, _, cx| {
                            monitor.preview_history(perf_history());
                            cx.notify();
                        })
                        .ok();
                }
            });
        }
        "skin-banner" | "background-picker-banners" => {
            let instances = shell.instances().clone();
            instances.update(cx, |instances, cx| {
                instances.set_skin(Some("grassblock".into()), cx);
                if let Some(id) = instances.selected().map(|i| i.id.clone()) {
                    let _ = instances.set_skin_banner(&id, "grassblock", "sunset", cx);
                }
            });
            if scene == "background-picker-banners" {
                shell.select_tab(InstanceTab::Settings, cx);
                if let Some(settings) = shell.settings_view().cloned() {
                    settings.update(cx, |settings, cx| settings.preview_background_picker(cx));
                }
            }
        }
        "sidebar-resized" | "sidebar-rail" | "sidebar-rail-skin" | "sidebar-rail-host" => {
            let instances = shell.instances().clone();
            instances.update(cx, |instances, cx| {
                if scene == "sidebar-resized" {
                    instances.set_sidebar(320.0, false, cx);
                } else {
                    instances.set_sidebar(crate::theme::layout::SIDEBAR_DEFAULT, true, cx);
                }
                if scene == "sidebar-rail-skin" {
                    instances.set_skin(Some("grassblock".into()), cx);
                }
            });
            if scene == "sidebar-rail-host" {
                show_empty_host_page(shell, cx);
            }
        }
        "skin-grassblock" | "skin-glass" | "skin-neon" | "skin-overrides" | "settings-skin" => {
            let id = match scene {
                "skin-glass" => "glass",
                "skin-neon" | "skin-overrides" => "neon",
                _ => "grassblock",
            };
            let instances = shell.instances().clone();
            instances.update(cx, |instances, cx| instances.set_skin(Some(id.into()), cx));
            if scene == "skin-overrides" {
                instances.update(cx, |instances, cx| {
                    if let Some(selected) = instances.selected().map(|i| i.id.clone()) {
                        let _ = instances.set_instance_colors(
                            &selected,
                            Some(launcher_core::ui_skin::Rgb(0xEFB440)),
                            Some(launcher_core::ui_skin::Rgb(0x6DD277)),
                            cx,
                        );
                    }
                });
            }
            if scene == "settings-skin" {
                shell.open_settings(Some(crate::shell::SettingsPage::Skin), None, cx);
            }
        }
        "settings-storage" | "settings-storage-confirm" => {
            shell.open_settings(Some(crate::shell::SettingsPage::Storage), None, cx);
            let confirm = (scene == "settings-storage-confirm")
                .then_some(launcher_core::disk::Clean::UnusedGameFiles);
            shell.preview_storage(storage_report(), confirm, cx);
            shell.preview_data_folder(false, false, cx);
        }
        // The data folder moved to another drive, and "Move your data?".
        "settings-storage-moved" | "settings-storage-move" => {
            shell.open_settings(Some(crate::shell::SettingsPage::Storage), None, cx);
            shell.preview_storage(storage_report(), None, cx);
            shell.preview_data_folder(
                scene == "settings-storage-moved",
                scene == "settings-storage-move",
                cx,
            );
        }
        "undo-toast" => {
            shell.open_settings(Some(crate::shell::SettingsPage::General), None, cx);
            shell
                .instances()
                .clone()
                .update(cx, |instances, cx| instances.set_auto_groups(false, cx));
        }
        "instance-linked" => servers::apply_linked_instance(shell, cx),
        "account-menu" | "needs-sign-in" | "account-menu-launcher" => shell.open_account_menu(cx),
        "launcher-identity" => shell.preview_enable_launcher_identity(cx),
        "sign-in-launcher" => shell.open_sign_in_at(Stage::Choose, None, cx),
        "sign-in" => shell.open_sign_in_at(Stage::Choose, None, cx),
        "sign-in-browser" => shell.open_sign_in_at(Stage::Browser, None, cx),
        "sign-in-stop" => {
            shell.open_sign_in_at(
                Stage::DeviceCode {
                    user_code: "F7K2-9QXM".into(),
                    verification_uri: launcher_core::auth::DEVICE_LOGIN_URL.into(),
                    opened: true,
                },
                None,
                cx,
            );
            shell.preview_ask_to_stop_signing_in(cx);
        }
        "offer-main" => shell.preview_main_dialog(false, cx),
        "confirm-main" => shell.preview_main_dialog(true, cx),
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
        scene if scene.starts_with("servers") => servers::apply(scene, shell, cx),
        scene if server_detail::is_scene(scene) => server_detail::apply(scene, shell, cx),
        "remove" | "focus-dialog" => {
            let id = microsoft_account().id;
            shell.confirm_remove(&id, None, cx);
        }
        "focus-settings-row" => {
            shell.open_settings(Some(crate::shell::SettingsPage::Defaults), None, cx)
        }
        "focus-popover" | "focus-escape" => {
            shell.open_settings(Some(crate::shell::SettingsPage::Shared), None, cx)
        }
        "focus-escape-prompt" => {
            shell.open_settings(Some(crate::shell::SettingsPage::Defaults), None, cx);
            shell.preview_unsaved_defaults(cx);
            shell.preview_unsaved_prompt(cx);
        }
        _ => {}
    }
}

/// A made-up Settings > Storage count, like a data folder after some months of play.
fn storage_report() -> launcher_core::disk::Report {
    use launcher_core::disk::{GameFiles, InstanceUsage, Logs, Report, RuntimeUsage, Unused};
    const MB: u64 = 1_000_000;
    let instance = |id: &str, name: &str, worlds: u64, content: u64, other: u64| InstanceUsage {
        id: id.into(),
        name: name.into(),
        worlds: worlds * MB,
        content: content * MB,
        logs: 3 * MB,
        screenshots: 12 * MB,
        other: other * MB,
    };
    let runtime = |component: &str, java: Option<u32>, mb: u64, used_by: &[&str]| RuntimeUsage {
        component: component.into(),
        java,
        bytes: mb * MB,
        used_by: used_by.iter().map(|v| v.to_string()).collect(),
    };
    Report {
        instances: vec![
            instance("all-the-mods", "All the mods", 412, 520, 96),
            instance("survival-1-20-1", "Survival 1.20.1", 238, 84, 31),
            instance("old-school", "Old school", 46, 0, 8),
            instance("latest-release", "Latest release", 12, 0, 4),
        ],
        runtimes: vec![
            runtime("java-runtime-delta", Some(21), 104, &["26.3"]),
            runtime("java-runtime-gamma", Some(17), 92, &["1.20.1"]),
            runtime("jre-legacy", Some(8), 164, &["1.8.9"]),
            runtime("java-runtime-beta", None, 92, &[]),
        ],
        game_files: GameFiles {
            versions: 153 * MB,
            libraries: 252 * MB,
            assets: 1_100 * MB,
            unused: Unused {
                versions: vec!["1.18.2".into(), "26.1.1".into()],
                versions_bytes: 48 * MB,
                libraries: 31 * MB,
                assets: 210 * MB,
            },
        },
        cache: 6 * MB + 300_000,
        leftovers: launcher_core::disk::Leftovers {
            bytes: 228 * MB,
            items: vec![".lodestone-pack-cb8aa247.mrpack".into()],
        },
        logs: Logs {
            bytes: 12 * MB,
            files: 84,
            old_bytes: 9 * MB,
            old_files: 61,
        },
        snapshots: launcher_core::snapshots::Usage {
            bytes: 186 * MB,
            snapshots: 7,
            removable_snapshots: 2,
            removable_bytes: 41 * MB,
        },
        other: 400_000,
    }
}

/// A Fabric instance's share code with a few hundred mods, and the files it leaves out.
fn sample_share_plan() -> launcher_core::content::share::SharePlan {
    use launcher_core::content::{
        ContentKind,
        share::{LeftOut, LeftOutReason, ShareCode, SharePlan, SharedItem},
    };
    let id = |seed: usize| {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let mut n = seed.wrapping_mul(2654435761).wrapping_add(97);
        (0..8)
            .map(|_| {
                n = n
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ALPHABET[(n >> 33) % ALPHABET.len()] as char
            })
            .collect::<String>()
    };
    let item = |kind, seed: usize, enabled| SharedItem {
        kind,
        project: id(seed),
        version: id(seed + 10_000),
        enabled,
    };
    let mut items: Vec<SharedItem> = (0..186)
        .map(|i| item(ContentKind::Mod, i, i % 23 != 0))
        .collect();
    items.extend((0..7).map(|i| item(ContentKind::ResourcePack, 500 + i, i < 4)));
    items.extend((0..3).map(|i| item(ContentKind::ShaderPack, 600 + i, i == 0)));
    let left_out = |kind, file_name: &str, reason| LeftOut {
        kind,
        file_name: file_name.into(),
        reason,
    };
    SharePlan {
        code: ShareCode {
            name: Some("Survival".into()),
            game_version: "1.21.1".into(),
            loader: launcher_core::instances::Loader::Fabric,
            loader_version: Some("0.16.9".into()),
            pack: None,
            items,
            changes: Default::default(),
        },
        shared: [
            (ContentKind::Mod, 186),
            (ContentKind::ResourcePack, 7),
            (ContentKind::ShaderPack, 3),
        ]
        .into_iter()
        .collect(),
        changes: None,
        left_out: vec![
            left_out(
                ContentKind::Mod,
                "my-server-tweaks-1.2.jar",
                LeftOutReason::NotOnModrinth,
            ),
            left_out(
                ContentKind::Mod,
                "jei-1.21.1-fabric-19.21.0.247.jar",
                LeftOutReason::CurseForge,
            ),
            left_out(
                ContentKind::Mod,
                "OptiFine_1.21.1_HD_U_J1.jar",
                LeftOutReason::NotOnModrinth,
            ),
            left_out(
                ContentKind::ResourcePack,
                "Friends Textures.zip",
                LeftOutReason::NotOnModrinth,
            ),
        ],
    }
}

/// The share plan of the instance folder in `LODESTONE_SHARE_PLAN_DIR` (read as the
/// dialog reads it, offline), else [`sample_pack_share_plan`]: to see the dialog for a
/// real instance.
fn pack_share_plan() -> (String, launcher_core::content::share::SharePlan) {
    use launcher_core::content::{InstanceDirs, Target, share};
    let real = std::env::var_os("LODESTONE_SHARE_PLAN_DIR").and_then(|dir| {
        let dirs = InstanceDirs::new(std::path::PathBuf::from(dir));
        let instance: launcher_core::instances::Instance =
            serde_json::from_slice(&std::fs::read(dirs.instance.join("instance.json")).ok()?)
                .ok()?;
        let target = Target::new(instance.game_version.clone(), instance.loader);
        let plan = futures::executor::block_on(share::collect(
            None,
            &dirs,
            &target,
            Some(instance.name.clone()),
            instance.loader,
            instance.loader_version.clone(),
        ))
        .ok()?;
        Some((instance.name, plan))
    });
    real.unwrap_or_else(|| ("Gaycraft".into(), sample_pack_share_plan()))
}

/// An instance from a Modrinth modpack with two of its mods turned off and one mod
/// added by hand.
fn sample_pack_share_plan() -> launcher_core::content::share::SharePlan {
    use launcher_core::content::{
        ContentKind,
        share::{
            ChangeCounts, FileRef, LeftOut, LeftOutReason, PackChanges, ShareCode, SharePlan,
            SharedPack,
        },
    };
    SharePlan {
        code: ShareCode {
            name: Some("Gaycraft".into()),
            game_version: "1.20.1".into(),
            loader: launcher_core::instances::Loader::Fabric,
            loader_version: Some("0.16.10".into()),
            pack: Some(SharedPack {
                project: "a0TMbfWJ".into(),
                version: "xcEwT3IK".into(),
                title: "Gaycraft".into(),
                version_number: "5.0".into(),
            }),
            items: Vec::new(),
            changes: PackChanges {
                turned_off: vec![
                    "mods/[1.20.1] Hybrid Aquatic 1.4.2.jar".into(),
                    "mods/[Fabric]backported_wolves-1.1.0-1.19.3-1.20.4.jar".into(),
                ],
                resource_packs: vec![FileRef::Pack("3D Chiseled Bookshelves".into())],
                ..PackChanges::default()
            },
        },
        shared: [
            (ContentKind::Mod, 308),
            (ContentKind::ResourcePack, 37),
            (ContentKind::ShaderPack, 5),
        ]
        .into_iter()
        .collect(),
        changes: Some(ChangeCounts {
            toggled: 2,
            ..ChangeCounts::default()
        }),
        left_out: vec![LeftOut {
            kind: ContentKind::Mod,
            file_name: "my-tweaks.jar".into(),
            reason: LeftOutReason::AddedByHand,
        }],
    }
}

/// Settings > Developer's socket, with two programs connected.
fn preview_control(cx: &mut App) {
    use std::time::{Duration, SystemTime};
    let now = SystemTime::now();
    crate::control::preview(
        crate::control::ControlStatus {
            socket: if cfg!(windows) {
                r"C:\Users\you\AppData\Roaming\Lodestone Launcher\control.sock".into()
            } else {
                "/Users/you/Library/Application Support/Lodestone Launcher/control.sock".into()
            },
            running: true,
            error: None,
            clients: vec![
                crate::control::ClientInfo {
                    connection: (1 << 32) | 3,
                    name: Some(format!(
                        "lodestone-launcher-cli {}",
                        env!("CARGO_PKG_VERSION")
                    )),
                    pid: Some(48213),
                    since: now - Duration::from_secs(95),
                    subscriptions: 1,
                },
                crate::control::ClientInfo {
                    connection: (1 << 32) | 4,
                    name: None,
                    pid: None,
                    since: now - Duration::from_secs(12),
                    subscriptions: 0,
                },
            ],
        },
        cx,
    );
}

/// A made-up minute of readings for the performance monitor's screenshot: the
/// launcher settling after a download, with a game running.
fn perf_history() -> Vec<crate::perf::Sample> {
    use crate::perf::{FrameStats, GameReading, ImageTally, ProcessReading, Sample, TraceReading};
    const MB: u64 = 1024 * 1024;
    (0..60u64)
        .map(|i| {
            let wave = ((i as f64) / 6.0).sin();
            let busy = i > 40 && i < 50;
            Sample {
                at: std::time::SystemTime::now(),
                launcher: ProcessReading {
                    pid: 51234,
                    memory: Some(212 * MB + i * MB / 2 + (wave * 3.0 * MB as f64) as u64),
                    cpu_percent: Some(if busy {
                        38.0 + wave * 8.0
                    } else {
                        2.5 + wave.abs() * 2.0
                    }),
                    threads: Some(41),
                    open_files: Some(57),
                },
                cores: 10,
                frames: FrameStats {
                    fps: if busy { 58.4 } else { 4.0 },
                    avg_ms: Some(if busy { 4.1 } else { 2.6 }),
                    p95_ms: Some(if busy { 6.5 + wave * 1.5 } else { 3.4 }),
                    max_ms: Some(if busy { 11.0 } else { 4.8 }),
                    idle: false,
                    background: false,
                },
                games: vec![GameReading {
                    instance: "a1".into(),
                    name: "Survival".into(),
                    process: ProcessReading {
                        pid: 51302,
                        memory: Some(2_340 * MB),
                        cpu_percent: Some(142.0 + wave * 20.0),
                        threads: Some(88),
                        open_files: Some(310),
                    },
                }],
                images: ImageTally {
                    count: 23,
                    bytes: 41 * MB,
                },
                content_cache: Some((48, 3 * MB)),
                jobs: 1,
                downloads: 1,
                download_rate: Some(6.2 * MB as f64),
                clients: Some(2),
                trace: Some(TraceReading {
                    subscribers: 1,
                    events_per_sec: Some(12.5),
                }),
            }
        })
        .collect()
}
