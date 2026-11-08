//! The Servers page with made-up cores, for the offscreen screenshots. Nothing is
//! fetched and no key is read or written.

use gpui::Context;
use launcher_core::cores::{
    CoreError, ServerInstance, ServerList, ServerLoader, ServerState, pack::PackChanges,
    store::SavedCore,
};

use crate::{
    servers::{Busy, CoreStatus, Servers},
    shell::{LauncherShell, Page},
};

pub const SCENES: &[&str] = &[
    "servers-empty",
    "servers",
    "servers-add",
    "servers-update",
    "servers-signed-out",
];

#[allow(clippy::too_many_arguments)]
fn server(
    uuid: &str,
    name: &str,
    version: &str,
    loader: ServerLoader,
    loader_version: Option<&str>,
    state: ServerState,
    players: Option<(u32, u32)>,
    playit: Option<&str>,
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
        playit_address: playit.map(str::to_string),
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
    }
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
        Some("smp-lodestone.ply.gg:41234"),
    )
}

fn home(signed_in: bool) -> (SavedCore, CoreStatus, bool) {
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
            None,
        ),
    ];
    (
        saved(
            "home",
            "http://localhost:16662",
            "Peter's Lodestone Core",
            signed_in.then_some("peter"),
        ),
        CoreStatus::Online(ServerList {
            core_name: "Peter's Lodestone Core".into(),
            core_uuid: "home".into(),
            listing_enabled: true,
            instances,
        }),
        signed_in,
    )
}

fn friend() -> (SavedCore, CoreStatus, bool) {
    (
        saved(
            "friend",
            "https://192.168.1.42:16662",
            "Alex's server box",
            None,
        ),
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
                None,
            )],
        }),
        false,
    )
}

fn apply_servers(scene: &str, servers: &mut Servers, cx: &mut Context<Servers>) {
    match scene {
        "servers" => {
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
                        added: vec!["create-fabric-0.5.1.jar".into()],
                        updated: vec!["sodium-fabric-0.5.13.jar".into()],
                        removed: vec!["old-minimap-1.2.jar".into()],
                    },
                    cx,
                );
            }
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
        _ => servers.preview(Vec::new(), Vec::new()),
    }
}

fn saved_hidden() -> SavedCore {
    saved(
        "club",
        "https://club.example.org:16662",
        "School club",
        None,
    )
}

pub fn apply(scene: &str, shell: &mut LauncherShell, cx: &mut Context<LauncherShell>) {
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
