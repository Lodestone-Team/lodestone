//! The Servers page with made-up cores, for the offscreen screenshots. Nothing is
//! fetched and no key is read or written.

use gpui::Context;
use launcher_core::cores::{
    CoreError, ServerInstance, ServerList, ServerLoader, ServerState,
    store::SavedCore,
    sync::PackChanges,
    tls::{CertificatePin, format_fingerprint},
    trust::{Advisory, ModrinthMatch},
};

use launcher_core::cores::{
    connect::{ConnectionKind, Invite, JoinError, SharedServer},
    friends::FriendServer,
};

use crate::{
    servers::{
        Busy, CoreStatus, Servers,
        friends::{FriendStatus, PreviewJoin},
    },
    shell::{LauncherShell, Page},
};

pub const SCENES: &[&str] = &[
    "servers-empty",
    "servers",
    "servers-ready",
    "servers-add",
    "servers-update",
    "servers-signed-out",
    "servers-lan",
    "servers-lan-empty",
    "servers-lan-join",
    "servers-host",
    "servers-host-owner",
    "servers-host-progress",
    "servers-host-done",
    "servers-certificate",
    "servers-certificate-changed",
    "servers-certificate-new",
    "servers-trust",
    "servers-advisory",
    "servers-local",
    "servers-shared-local",
    "servers-shared-local-gone",
    "servers-friends",
    "servers-friends-relay",
    "servers-join-paste",
    "servers-join-connecting",
    "servers-join-ready",
    "servers-join-ready-mods",
    "servers-join-port-taken",
    "servers-join-failed",
    "servers-join-no-path",
    "servers-friend-list-empty",
    "servers-friend-list",
    "servers-friend-list-reply",
    "servers-friend-list-add",
    "servers-friend-list-add-link",
    "servers-friend-list-verify",
    "servers-friend-list-remove",
    "servers-friend-list-retire",
    "servers-friend-list-retire-core",
];

/// Made-up fingerprints.
const FINGERPRINT: &str = "3F9A0C11D2E84B7756A1C0FFEE42B19D8E7A6C5B4D3E2F1A0B9C8D7E6F5A4B3C";
const NEW_FINGERPRINT: &str = "A1B2C3D4E5F60718293A4B5C6D7E8F90112233445566778899AABBCCDDEEFF00";

/// The scenes that show made-up instances: to host, and to join a LAN world with.
pub fn has_instances(scene: &str) -> bool {
    matches!(
        scene,
        "servers-lan-join"
            | "servers-host"
            | "servers-host-owner"
            | "servers-host-progress"
            | "servers-host-done"
    )
}

fn server(
    uuid: &str,
    name: &str,
    version: &str,
    loader: ServerLoader,
    loader_version: Option<&str>,
    state: ServerState,
    players: Option<(u32, u32)>,
) -> ServerInstance {
    ServerInstance {
        uuid: uuid.into(),
        name: name.into(),
        game_version: version.into(),
        loader,
        loader_version: loader_version.map(str::to_string),
        port: Some(25565),
        state,
        player_count: players.map(|p| p.0),
        max_player_count: players.map(|p| p.1),
        has_client_pack: Some(loader == ServerLoader::Fabric),
        listed: true,
    }
}

fn saved(uuid: &str, url: &str, name: &str, username: Option<&str>) -> SavedCore {
    SavedCore {
        uuid: uuid.into(),
        url: url.into(),
        name: name.into(),
        username: username.map(str::to_string),
        key_id: username.map(|_| "k3y1d0000000".to_string()),
        key_expires_at: None,
        ..SavedCore::default()
    }
}

/// Alex's core over HTTPS with its own certificate, pinned.
fn pinned(mut core: SavedCore) -> SavedCore {
    let host = url::Url::parse(&core.url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();
    core.pin_certificate(CertificatePin::new(&host, FINGERPRINT));
    core
}

fn smp() -> ServerInstance {
    server(
        "smp",
        "Survival SMP",
        "1.20.1",
        ServerLoader::Fabric,
        Some("0.16.14"),
        ServerState::Running,
        Some((3, 20)),
    )
}

fn home(signed_in: bool) -> (SavedCore, CoreStatus, bool) {
    let mut core = saved(
        "home",
        "http://localhost:16662",
        "Peter's Lodestone Core",
        signed_in.then_some("peter"),
    );
    core.set_trusted(true);
    let instances = vec![
        smp(),
        server(
            "creative",
            "Creative world",
            "1.21.1",
            ServerLoader::Vanilla,
            None,
            ServerState::Stopped,
            None,
        ),
        server(
            "atm",
            "All the Mods",
            "1.20.1",
            ServerLoader::Forge,
            Some("1.20.1-47.4.28"),
            ServerState::Stopped,
            None,
        ),
    ];
    (
        core,
        CoreStatus::Online(ServerList {
            core_name: "Peter's Lodestone Core".into(),
            core_uuid: "home".into(),
            listing_enabled: true,
            instances,
        }),
        signed_in,
    )
}

fn friend_core() -> SavedCore {
    pinned(saved(
        "friend",
        "https://192.168.1.42:16662",
        "Alex's server box",
        None,
    ))
}

fn friend() -> (SavedCore, CoreStatus, bool) {
    (
        friend_core(),
        CoreStatus::Online(ServerList {
            core_name: "Alex's server box".into(),
            core_uuid: "friend".into(),
            listing_enabled: true,
            instances: vec![server(
                "skyblock",
                "Skyblock",
                "1.21.1",
                ServerLoader::Fabric,
                Some("0.16.14"),
                ServerState::Running,
                Some((7, 16)),
            )],
        }),
        false,
    )
}

fn apply_servers(scene: &str, servers: &mut Servers, cx: &mut Context<Servers>) {
    match scene {
        "servers" | "servers-ready" => {
            servers.preview(
                vec![
                    home(true),
                    friend(),
                    (
                        saved(
                            "laptop",
                            "https://old-laptop.local:16662",
                            "Old laptop",
                            None,
                        ),
                        CoreStatus::Failed(CoreError::Unreachable("connection refused".into())),
                        false,
                    ),
                ],
                vec![("home".into(), "smp".into(), "survival-smp".into())],
            );
            servers.preview_busy("home", "atm", Busy::Starting);
            if scene == "servers-ready" {
                servers.preview_ready("Cobblemon Official Modpack [Fabric]");
            }
        }
        "servers-add" | "servers-update" => {
            servers.preview(
                vec![home(true)],
                vec![("home".into(), "smp".into(), "survival-smp".into())],
            );
            if scene == "servers-update" {
                servers.preview_prompt(
                    "home",
                    smp(),
                    "survival-smp",
                    PackChanges {
                        added: vec!["mods/create-fabric-0.5.1.jar".into()],
                        updated: vec!["mods/sodium-fabric-0.5.13.jar".into()],
                        removed: vec!["mods/old-minimap-1.2.jar".into()],
                        other: vec!["config/create-common.toml".into()],
                    },
                    cx,
                );
            }
        }
        "servers-local" => {
            use launcher_core::cores::{
                CoreProduct, LaunchedBy,
                local_claim::{ClaimError, LocalCredentials},
            };
            let (mut host, status, _) = home(true);
            host.product = CoreProduct::Host;
            host.launched_by = LaunchedBy::Host;
            host.username = Some("owner_k3m9x".into());
            let mut direct = saved("lab", "http://127.0.0.1:16700", "Test core", None);
            direct.launched_by = LaunchedBy::Direct;
            servers.preview(
                vec![
                    (host, status, true),
                    (
                        direct,
                        CoreStatus::Online(ServerList {
                            core_uuid: "lab".into(),
                            core_name: "Test core".into(),
                            listing_enabled: false,
                            instances: Vec::new(),
                        }),
                        false,
                    ),
                ],
                Vec::new(),
            );
            servers.preview_local(
                vec![
                    (
                        "home",
                        "Peter's Lodestone Core",
                        CoreProduct::Host,
                        LaunchedBy::Host,
                    ),
                    (
                        "docker",
                        "Docker core",
                        CoreProduct::Core,
                        LaunchedBy::Docker,
                    ),
                ],
                vec![(
                    "home",
                    LocalCredentials {
                        url: "https://127.0.0.1:16662".into(),
                        api_key: Some("lsk_preview_key".into()),
                        owner_username: Some("owner_k3m9x".into()),
                        owner_password: Some("preview-password".into()),
                        ..Default::default()
                    },
                )],
                Some(("docker", ClaimError::FolderNotFound)),
            );
        }
        "servers-signed-out" => {
            let (saved, status, _) = home(false);
            let public = match status {
                CoreStatus::Online(mut list) => {
                    list.instances.truncate(1);
                    CoreStatus::Online(list)
                }
                other => other,
            };
            servers.preview(
                vec![
                    (saved, public, false),
                    (
                        saved_hidden(),
                        CoreStatus::Online(ServerList {
                            core_name: "School club".into(),
                            core_uuid: "club".into(),
                            listing_enabled: false,
                            instances: Vec::new(),
                        }),
                        false,
                    ),
                ],
                Vec::new(),
            );
            servers.preview_notice(
                "Signed out of Peter's Lodestone Core",
                &CoreError::SignedOut.to_string(),
            );
        }
        "servers-certificate" => {
            servers.preview(
                vec![
                    home(true),
                    (
                        saved(
                            "friend",
                            "https://192.168.1.42:16662",
                            "Alex's server box",
                            None,
                        ),
                        CoreStatus::Failed(CoreError::Untrusted {
                            host: "192.168.1.42".into(),
                            fingerprint: Some(format_fingerprint(FINGERPRINT)),
                        }),
                        false,
                    ),
                ],
                Vec::new(),
            );
            servers.preview_certificate(1, &format_fingerprint(FINGERPRINT), None, cx);
        }
        "servers-certificate-changed" | "servers-certificate-new" => {
            servers.preview(
                vec![
                    home(true),
                    (
                        friend_core(),
                        CoreStatus::Failed(CoreError::CertificateChanged {
                            host: "192.168.1.42".into(),
                            pinned: format_fingerprint(FINGERPRINT),
                            presented: format_fingerprint(NEW_FINGERPRINT),
                        }),
                        false,
                    ),
                ],
                Vec::new(),
            );
            if scene == "servers-certificate-new" {
                servers.preview_certificate(
                    1,
                    &format_fingerprint(NEW_FINGERPRINT),
                    Some(&format_fingerprint(FINGERPRINT)),
                    cx,
                );
            }
        }
        "servers-trust" => {
            servers.preview(vec![home(true), friend()], Vec::new());
            servers.preview_trust(1, cx);
        }
        "servers-advisory" => {
            servers.preview(vec![home(true), friend()], Vec::new());
            let found = |path: &str, project: &str, version: &str| ModrinthMatch {
                path: path.into(),
                project: project.into(),
                version: version.into(),
            };
            servers.preview_advisory(
                "Alex's server box",
                "Skyblock",
                Advisory {
                    on_modrinth: vec![
                        found("mods/fabric-api-0.92.2.jar", "Fabric API", "0.92.2+1.20.1"),
                        found("mods/sodium-fabric-0.5.13.jar", "Sodium", "mc1.20.1-0.5.13"),
                        found(
                            "mods/lithium-fabric-0.11.2.jar",
                            "Lithium",
                            "mc1.20.1-0.11.2",
                        ),
                    ],
                    uploaded: vec![
                        "mods/skyblock-islands-1.4.jar".into(),
                        "mods/alex-utils-dev.jar".into(),
                    ],
                    configs: 14,
                    scripts: 2,
                    other: 1,
                    unchecked: false,
                },
                cx,
            );
        }
        scene if scene.starts_with("servers-friend-list") => {
            servers.preview(Vec::new(), Vec::new());
            super::friend_list::apply(scene, servers, cx);
        }
        "servers-shared-local" | "servers-shared-local-gone" => {
            apply_shared_local(scene == "servers-shared-local", servers)
        }
        scene if scene.starts_with("servers-friends") || scene.starts_with("servers-join") => {
            if scene == "servers-friends-relay" {
                // a tester pointed at the dev service: the section says so
                launcher_core::cores::connect_service::set_setting(
                    launcher_core::cores::connect_service::Service::from_setting(
                        "https://connect-dev.lodestone.cc",
                    )
                    .ok(),
                );
            }
            servers.preview(vec![home(true)], Vec::new());
            servers.preview_friends(made_up_friends());
            apply_join(scene, servers, cx);
        }
        _ => servers.preview(Vec::new(), Vec::new()),
    }
}

/// The instance page of an instance made for a server: the link under its name, and
/// "Play and join".
/// A server a friend shares, as the host describes it.
fn shared(name: &str, loader: &str, version: Option<&str>, modpack: bool) -> SharedServer {
    SharedServer {
        core_name: "Alex's Lodestone".into(),
        core_uuid: "alex".into(),
        instance_uuid: name.to_lowercase().replace(' ', "-"),
        name: name.into(),
        game_version: "1.21.4".into(),
        loader: loader.into(),
        loader_version: version.map(str::to_string),
        modpack,
        state: Some("Running".into()),
        player_count: Some(3),
        max_player_count: Some(20),
    }
}

/// A made-up saved friend's server, joined with the invite link `code`.
fn friend_entry(
    code: &str,
    server: &SharedServer,
    trusted: bool,
    port: Option<u16>,
) -> FriendServer {
    let invite = Invite::parse(&format!(
        "https://connect.lodestone.cc/j/{code}#{}.{}",
        "ab".repeat(32),
        "01".repeat(16)
    ))
    .expect("a made-up invite");
    let mut saved = FriendServer::new(&invite, server, 1_700_000_000);
    saved.set_trusted(trusted);
    saved.game_port = port;
    saved
}

/// Servers the user joined with invite links shared from cores on this computer (with
/// `here`), or the same once those cores stopped: their own Lodestone Host (connected
/// as owner, so its card lists the server, which shows once there), a second Host they
/// aren't the owner of here (under it in "On this computer"), and a friend's server.
fn apply_shared_local(here: bool, servers: &mut Servers) {
    use launcher_core::cores::{
        CoreProduct, LaunchedBy,
        local_claim::{ClaimError, LocalCredentials},
    };
    let (mut host, status, _) = home(true);
    host.url = "https://127.0.0.1:16662".into();
    host.product = CoreProduct::Host;
    host.launched_by = LaunchedBy::Host;
    host.local_claim = true;
    host.username = Some("owner_k3m9x".into());
    let status = if here {
        status
    } else {
        CoreStatus::Failed(CoreError::Unreachable("connection refused".into()))
    };
    // the user's own server, joined with its invite link: an instance was made for it
    let mut creative = shared("Creative world", "vanilla", None, false);
    creative.core_name = "Peter's Lodestone Core".into();
    creative.core_uuid = "home".into();
    creative.instance_uuid = "creative".into();
    let creative = friend_entry("HMCRTV2345", &creative, true, Some(34711));
    let mut lab = shared("Modded test", "fabric", Some("0.16.14"), true);
    lab.core_name = "Lab Host".into();
    lab.core_uuid = "lab".into();
    lab.instance_uuid = "modded-test".into();
    let lab = friend_entry("LBMDTS6789", &lab, false, Some(38250));
    let alex = shared("Alex's SMP", "fabric", Some("0.16.14"), true);
    let alex = friend_entry("ABCDEFGH23", &alex, true, Some(31234));
    servers.preview(
        vec![(host, status, true)],
        vec![
            ("home".into(), "smp".into(), "survival-smp".into()),
            (
                creative.id.clone(),
                "creative".into(),
                "creative-world".into(),
            ),
        ],
    );
    servers.preview_friends(vec![
        (creative, FriendStatus::Unknown),
        (lab, FriendStatus::Unknown),
        (
            alex.clone(),
            FriendStatus::Online {
                server: Box::new(shared("Alex's SMP", "fabric", Some("0.16.14"), true)),
                kind: ConnectionKind::Internet,
                rtt: std::time::Duration::from_millis(34),
            },
        ),
    ]);
    if !here {
        servers.preview_local(Vec::new(), Vec::new(), None);
        return;
    }
    servers.preview_local(
        vec![
            (
                "home",
                "Peter's Lodestone Core",
                CoreProduct::Host,
                LaunchedBy::Host,
            ),
            ("lab", "Lab Host", CoreProduct::Host, LaunchedBy::Host),
        ],
        vec![(
            "home",
            LocalCredentials {
                url: "https://127.0.0.1:16662".into(),
                api_key: Some("lsk_preview_key".into()),
                owner_username: Some("owner_k3m9x".into()),
                owner_password: Some("preview-password".into()),
                ..Default::default()
            },
        )],
        None::<(&str, ClaimError)>,
    );
}

/// Made-up friends' servers: one checked and online, one offline, one not asked yet.
fn made_up_friends() -> Vec<(FriendServer, FriendStatus)> {
    let entry = friend_entry;
    let smp = shared("Alex's SMP", "fabric", Some("0.16.14"), true);
    let creative = shared("Creative build", "vanilla", None, false);
    let mut sky = shared("Sam's Skyblock", "paper", Some("196"), false);
    sky.core_name = "Sam's PC".into();
    vec![
        (
            entry("ABCDEFGH23", &smp, true, Some(31234)),
            FriendStatus::Online {
                server: Box::new(smp.clone()),
                kind: ConnectionKind::Internet,
                rtt: std::time::Duration::from_millis(34),
            },
        ),
        (
            entry("JKMNPQRS45", &creative, false, Some(36017)),
            FriendStatus::Unknown,
        ),
        (
            entry("TVWXYZ0167", &sky, false, None),
            FriendStatus::Offline(JoinError::HostOffline),
        ),
    ]
}

fn apply_join(scene: &str, servers: &mut Servers, cx: &mut Context<Servers>) {
    let stage = match scene {
        "servers-join-paste" => PreviewJoin::Paste {
            text: "https://connect.lodestone.cc/j/ABCDEF".into(),
            error: true,
        },
        "servers-join-connecting" => PreviewJoin::Connecting {
            name: Some("Alex's SMP".into()),
            reading: false,
        },
        "servers-join-ready" => PreviewJoin::Ready {
            server: shared("Creative build", "vanilla", None, false),
            kind: ConnectionKind::LocalNetwork,
            port: 36017,
            taken_port: None,
        },
        "servers-join-ready-mods" => PreviewJoin::Ready {
            server: shared("Alex's SMP", "fabric", Some("0.16.14"), true),
            kind: ConnectionKind::Internet,
            port: 31234,
            taken_port: None,
        },
        "servers-join-port-taken" => PreviewJoin::Ready {
            server: shared("Creative build", "vanilla", None, false),
            kind: ConnectionKind::LocalNetwork,
            port: 36017,
            taken_port: Some(52841),
        },
        "servers-join-failed" => PreviewJoin::Failed {
            error: JoinError::WrongHost,
            name: None,
        },
        "servers-join-no-path" => PreviewJoin::Failed {
            error: JoinError::NoPath,
            name: Some("Alex's SMP".into()),
        },
        _ => return,
    };
    servers.preview_join(stage, cx);
}

pub fn apply_linked_instance(shell: &mut LauncherShell, cx: &mut Context<LauncherShell>) {
    let Some(id) = shell.instances().read(cx).selected().map(|i| i.id.clone()) else {
        return;
    };
    let servers = shell.servers().clone();
    servers.update(cx, |servers, _| {
        servers.preview(vec![home(true)], vec![("home".into(), "smp".into(), id)])
    });
    shell.show(crate::shell::Page::Instance, cx);
}

/// Links the made-up instances `(instance id, core, server)` to the made-up cores'
/// servers: `home`'s `smp`, `creative` and `atm`, and `friend`'s `skyblock`.
pub fn link_instances(
    shell: &mut LauncherShell,
    links: Vec<(String, &str, &str)>,
    cx: &mut Context<LauncherShell>,
) {
    let links = links
        .into_iter()
        .map(|(id, core, server)| (core.to_string(), server.to_string(), id))
        .collect();
    let servers = shell.servers().clone();
    servers.update(cx, |servers, cx| {
        servers.preview(vec![home(true), friend()], links);
        // the shell hands the links to the instance list
        cx.notify();
    });
}

fn saved_hidden() -> SavedCore {
    saved(
        "club",
        "https://club.example.org:16662",
        "School club",
        None,
    )
}

/// The id of the made-up instance called `name`.
fn instance_id(
    shell: &LauncherShell,
    name: &str,
    cx: &mut Context<LauncherShell>,
) -> Option<String> {
    shell
        .servers()
        .read(cx)
        .instances()
        .read(cx)
        .list()
        .iter()
        .find(|i| i.name == name)
        .map(|i| i.id.clone())
}

fn apply_extras(scene: &str, shell: &mut LauncherShell, cx: &mut Context<LauncherShell>) -> bool {
    let servers = shell.servers().clone();
    let survival = instance_id(shell, "Survival 1.20.1", cx).unwrap_or_default();
    match scene {
        "servers-lan" | "servers-lan-empty" | "servers-lan-join" => {
            servers.update(cx, |servers, cx| {
                servers.preview(vec![home(true)], Vec::new());
                if scene == "servers-lan-empty" {
                    servers.preview_lan(&[], true);
                } else {
                    servers.preview_lan(
                        &[
                            ("Steve - Survival Island", "192.168.1.31", 52144),
                            ("Alex - Creative build", "192.168.1.47", 61021),
                        ],
                        false,
                    );
                }
                if scene == "servers-lan-join" {
                    let world = servers.preview_lan_world(0);
                    if let Some(world) = world {
                        servers.open_lan_join_preview(world, cx);
                    }
                }
            });
            shell.show(Page::Servers, cx);
            true
        }
        "servers-host" | "servers-host-owner" | "servers-host-progress" | "servers-host-done" => {
            servers.update(cx, |servers, cx| {
                let mut home = home(true);
                // connected as owner on this computer: the owner key creates the server
                home.0.local_claim = scene == "servers-host-owner";
                servers.preview(vec![home, friend()], Vec::new());
                let (stage, done) = match scene {
                    "servers-host-progress" => (Some("Uploading the modpack (3.2 MB)…"), None),
                    "servers-host-done" => (None, Some("Survival 1.20.1 is on Peter's Lodestone Core, on port 25566. The core is setting it up now: it installs Fabric and downloads 24 mod file(s) from Modrinth.\n\nThe server is stopped. Start it from the Lodestone Host page or the dashboard when you're ready; this instance is ready to join it.")),
                    _ => (None, None),
                };
                servers.preview_host(&survival, stage, done, cx);
            });
            shell.select_instance(&survival, cx);
            shell.select_tab(crate::shell::InstanceTab::Settings, cx);
            true
        }
        _ => false,
    }
}

pub fn apply(scene: &str, shell: &mut LauncherShell, cx: &mut Context<LauncherShell>) {
    if apply_extras(scene, shell, cx) {
        return;
    }
    let servers = shell.servers().clone();
    servers.update(cx, |servers, cx| apply_servers(scene, servers, cx));
    shell.show(Page::Servers, cx);
    if scene == "servers-add" {
        servers.update(cx, |servers, cx| {
            servers.open_add(None, cx);
            if let Some(dialog) = servers.dialog().cloned() {
                dialog.update(cx, |dialog, cx| {
                    dialog.preview("192.168.1.42", "peter", "correct horse", cx)
                });
            }
        });
    }
}
