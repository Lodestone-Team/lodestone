//! Lodestone cores: servers the launcher can list, join, start and stop.
//!
//! - [`client`]: the core's API (`/info`, the instance lists, signing in, start/stop,
//!   client packs);
//! - [`tls`]: pinned certificates for cores macOS doesn't trust;
//! - [`store`]: the cores the launcher knows (`servers.json`), their device keys, pins
//!   and whether the user trusts them for mods;
//! - [`trust`]: the advisory before installing code from a core that isn't trusted;
//! - [`link`] and [`sync`]: an instance made for a server, and its mods kept in step
//!   with the server's client pack through the modpack import and update;
//! - [`join`]: where to join a server;
//! - [`live`]: a server's live state, players and console, and linking Minecraft
//!   accounts;
//! - [`hosting`]: "Host this modpack", a launcher instance as a new server;
//! - [`lan`]: worlds opened to LAN on this network;
//! - [`lan_open`]: the worlds this computer has open to LAN, for friends;
//! - [`local_claim`]: "Connect as owner" for a core on this computer;
//! - [`connect`] and [`friends`]: servers friends share with Lodestone Connect invite
//!   links, joined through a direct tunnel, and the ones the user joined;
//! - [`connect_service`]: which Lodestone Connect introduction service the apps use;
//! - [`admin`]: the core's full API, as the dashboard and Lodestone Host use it.

pub mod admin;
pub mod client;
pub mod connect;
pub mod connect_service;
pub mod download;
pub mod friends;
pub mod hosting;
pub mod join;
pub mod lan;
pub mod lan_open;
pub mod link;
pub mod live;
pub mod local_claim;
pub mod models;
pub mod server_list;
pub mod store;
pub mod sync;
pub mod tls;
pub mod trust;

pub use client::{CoreClient, CoreError, PackFetch, parse_address};
pub use models::{
    CoreInfo, CoreProduct, LaunchedBy, ServerInstance, ServerList, ServerLoader, ServerState,
};

use crate::game::loaders::{LoaderChoice, LoaderKind};

/// The loader a launcher instance needs to join `server`: Fabric, Forge or NeoForge at the
/// server's version, or none (vanilla) for vanilla, Paper and Spigot servers.
pub fn matching_loader(server: &ServerInstance) -> Result<Option<LoaderChoice>, String> {
    let kind = match server.loader {
        ServerLoader::Fabric => LoaderKind::Fabric,
        ServerLoader::Forge => LoaderKind::Forge,
        ServerLoader::NeoForge => LoaderKind::NeoForge,
        ServerLoader::Vanilla | ServerLoader::Paper | ServerLoader::Spigot => return Ok(None),
        ServerLoader::Other => {
            return Err(format!(
                "{} runs a mod loader {} doesn't know.",
                server.name,
                crate::wording::app_name()
            ));
        }
    };
    let Some(version) = server.loader_version.clone().filter(|v| !v.is_empty()) else {
        return Err(format!(
            "The host doesn't say which {} version {} runs.",
            kind.label(),
            server.name
        ));
    };
    // Forge versions are full Maven versions: `1.20.1-47.4.28`
    let version = match kind {
        LoaderKind::Forge if !version.starts_with(&format!("{}-", server.game_version)) => {
            format!("{}-{version}", server.game_version)
        }
        _ => version,
    };
    Ok(Some(LoaderChoice { kind, version }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(loader: ServerLoader, version: Option<&str>) -> ServerInstance {
        ServerInstance {
            uuid: "i".into(),
            name: "SMP".into(),
            game_version: "1.20.1".into(),
            loader,
            loader_version: version.map(str::to_string),
            port: Some(25565),
            state: ServerState::Stopped,
            player_count: None,
            max_player_count: None,
            has_client_pack: Some(false),
            listed: false,
        }
    }

    #[test]
    fn matches_the_servers_loader() {
        assert_eq!(
            matching_loader(&server(ServerLoader::Fabric, Some("0.16.14"))).unwrap(),
            Some(LoaderChoice {
                kind: LoaderKind::Fabric,
                version: "0.16.14".into()
            })
        );
        assert_eq!(
            matching_loader(&server(ServerLoader::Forge, Some("1.20.1-47.4.28")))
                .unwrap()
                .unwrap()
                .version,
            "1.20.1-47.4.28"
        );
        assert_eq!(
            matching_loader(&server(ServerLoader::Forge, Some("47.4.28")))
                .unwrap()
                .unwrap()
                .version,
            "1.20.1-47.4.28"
        );
        for plain in [
            ServerLoader::Vanilla,
            ServerLoader::Paper,
            ServerLoader::Spigot,
        ] {
            assert_eq!(matching_loader(&server(plain, Some("196"))).unwrap(), None);
        }
        assert!(matching_loader(&server(ServerLoader::Fabric, None)).is_err());
        assert!(matching_loader(&server(ServerLoader::Other, Some("1"))).is_err());
    }
}
