//! A server's page (live state, console, chat, players, account link) and the sign-in
//! dialog's console choices, with made-up contents, for the offscreen screenshots.
//! Nothing is fetched and no key is read or written.

use gpui::{AppContext as _, Context};
use launcher_core::cores::{
    ServerInstance, ServerList, ServerLoader, ServerState,
    live::{LauncherAccess, LauncherScope, MinecraftLink, Player},
    store::SavedCore,
};

use crate::{
    servers::{CoreStatus, detail::ServerDetail},
    shell::{LauncherShell, Page},
};

pub const SCENES: &[&str] = &[
    "server-detail",
    "server-detail-linked",
    "server-detail-no-console",
    "server-sign-in-console",
];

pub fn is_scene(scene: &str) -> bool {
    SCENES.contains(&scene)
}

const E: &str = "\x1b";

fn smp() -> ServerInstance {
    ServerInstance {
        uuid: "smp".into(),
        name: "Survival SMP".into(),
        game_version: "1.20.1".into(),
        loader: ServerLoader::Fabric,
        loader_version: Some("0.16.14".into()),
        port: 25565,
        state: ServerState::Running,
        player_count: Some(3),
        max_player_count: Some(20),
        has_client_pack: true,
        playit_address: None,
        listed: true,
    }
}

fn console() -> Vec<String> {
    vec![
        "[12:00:01] [main/INFO]: Loading Minecraft 1.20.1 with Fabric Loader 0.16.14".into(),
        format!("[12:00:02] [main/INFO]: {E}[32mLoaded 42 mods{E}[0m"),
        "[12:00:04] [Server thread/INFO]: Starting minecraft server version 1.20.1".into(),
        "[12:00:04] [Server thread/INFO]: Preparing level \"world\"".into(),
        "[12:00:09] [Server thread/WARN]: Can't keep up! Is the server overloaded? Running 2048ms or 40 ticks behind".into(),
        "[12:00:10] [Server thread/INFO]: Done (6.214s)! For help, type \"help\"".into(),
        "[12:01:12] [Server thread/INFO]: Steve joined the game".into(),
        "[12:01:30] [Server thread/INFO]: <Steve> hi everyone!".into(),
        "[12:02:05] [Server thread/INFO]: Alex joined the game".into(),
        "[12:02:11] [Server thread/INFO]: §e[Server] §fWelcome to §a§lSurvival SMP§r, Alex".into(),
        format!("[12:02:40] [Server thread/INFO]: {E}[1;38;2;255;128;0m[Shop]{E}[0m Alex bought {E}[93m16x Oak Planks{E}[0m"),
        format!("[12:03:02] [Server thread/INFO]: {E}[30;43m NOTICE {E}[0m Backups run at 04:00"),
        "[12:03:20] [Server thread/ERROR]: Couldn't save chunk [12, -4]: disk is full".into(),
        "[12:04:00] [Server thread/INFO]: Lodestoner joined the game".into(),
        "[12:04:03] [Server thread/INFO]: <Lodestoner> hello from the launcher".into(),
    ]
}

fn players() -> Vec<Player> {
    ["Alex", "Lodestoner", "Steve"]
        .into_iter()
        .map(|name| Player {
            name: name.into(),
            uuid: None,
        })
        .collect()
}

fn core(servers: &mut crate::servers::Servers) {
    servers.preview(
        vec![(
            SavedCore {
                uuid: "home".into(),
                url: "http://localhost:16662".into(),
                name: "Peter's Lodestone Core".into(),
                username: Some("peter".into()),
                key_id: Some("k3y1d0000000".into()),
                key_expires_at: None,
            },
            CoreStatus::Online(ServerList {
                core_name: "Peter's Lodestone Core".into(),
                core_uuid: "home".into(),
                listing_enabled: true,
                instances: vec![smp()],
            }),
            true,
        )],
        Vec::new(),
    );
}

pub fn apply(scene: &str, shell: &mut LauncherShell, cx: &mut Context<LauncherShell>) {
    let servers = shell.servers().clone();
    let accounts = servers.read(cx).accounts_entity().clone();
    servers.update(cx, |servers, cx| {
        core(servers);
        if scene == "server-sign-in-console" {
            servers.open_add(Some(0), cx);
            if let Some(dialog) = servers.dialog().cloned() {
                dialog.update(cx, |dialog, cx| {
                    dialog.preview("http://localhost:16662", "peter", "correct horse", cx);
                    dialog.preview_console(LauncherScope {
                        console_read: true,
                        console_write: false,
                    });
                });
            }
            return;
        }
        let access = |read: bool, write: bool| LauncherAccess {
            view: true,
            start: true,
            stop: true,
            console_read: read,
            console_write: write,
        };
        let (access, links) = match scene {
            "server-detail-linked" => (
                access(true, false),
                vec![MinecraftLink {
                    minecraft_uuid: "8667ba71-b85a-4004-af54-457a9734eed7".into(),
                    minecraft_name: "Lodestoner".into(),
                    verified: true,
                }],
            ),
            "server-detail-no-console" => (access(false, false), Vec::new()),
            _ => (access(true, true), Vec::new()),
        };
        let lines = console();
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let detail = cx.new(|cx| {
            ServerDetail::preview(
                "Peter's Lodestone Core",
                "peter",
                smp(),
                access,
                players(),
                &lines,
                links,
                accounts,
                cx,
            )
        });
        servers.show_detail(detail, cx);
    });
    shell.show(Page::Servers, cx);
}
