//! Lodestone cores: servers the launcher can list, join, start and stop.
//!
//! - [`client`]: the core's API (`/info`, the instance lists, signing in, start/stop,
//!   client packs);
//! - [`store`]: the cores the launcher knows (`servers.json`) and their device keys;
//! - [`pack`]: a server's client mods in a launcher instance;
//! - [`join`]: where to join a server.

pub mod client;
pub mod join;
pub mod models;
pub mod pack;
pub mod store;

pub use client::{CoreClient, CoreError, PackFetch, parse_address};
pub use models::{CoreInfo, ServerInstance, ServerList, ServerLoader, ServerState};

use crate::game::loaders::{LoaderChoice, LoaderKind};

/// The loader a launcher instance needs to join `server`: Fabric or Forge at the
/// server's version, or none (vanilla) for vanilla, Paper and Spigot servers.
pub fn matching_loader(server: &ServerInstance) -> Result<Option<LoaderChoice>, String> {
    let kind = match server.loader {
        ServerLoader::Fabric => LoaderKind::Fabric,
        ServerLoader::Forge => LoaderKind::Forge,
        ServerLoader::Vanilla | ServerLoader::Paper | ServerLoader::Spigot => return Ok(None),
        ServerLoader::Other => {
            return Err(format!(
                "{} runs a mod loader Lodestone Launcher doesn't know.",
                server.name
            ));
        }
    };
    let Some(version) = server.loader_version.clone().filter(|v| !v.is_empty()) else {
        return Err(format!(
            "The core doesn't say which {} version {} runs.",
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
            playit_address: None,
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
