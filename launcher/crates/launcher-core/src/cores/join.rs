//! Where to join a server: the core's host with the server's game port.

use url::Url;

use super::models::ServerInstance;
use crate::game::launch::JoinServer;

/// The address to join `server` at, on the core at `core_url`. `None` when the address
/// isn't shared or isn't one Minecraft can join.
pub fn join_target(core_url: &Url, server: &ServerInstance) -> Option<JoinServer> {
    let host = match core_url.host()? {
        url::Host::Domain(domain) => domain.to_string(),
        url::Host::Ipv4(ip) => ip.to_string(),
        url::Host::Ipv6(ip) => ip.to_string(),
    };
    Some(JoinServer {
        host,
        port: u16::try_from(server.port?).ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cores::models::{ServerLoader, ServerState};

    fn server(port: impl Into<Option<u32>>) -> ServerInstance {
        ServerInstance {
            uuid: "i".into(),
            name: "SMP".into(),
            game_version: "1.20.1".into(),
            loader: ServerLoader::Fabric,
            loader_version: None,
            port: port.into(),
            state: ServerState::Running,
            player_count: None,
            max_player_count: None,
            has_client_pack: None,
            listed: true,
        }
    }

    #[test]
    fn joins_the_core_host_at_the_game_port() {
        let core = Url::parse("https://192.168.1.5:16662").unwrap();
        assert_eq!(
            join_target(&core, &server(25570)),
            Some(JoinServer {
                host: "192.168.1.5".into(),
                port: 25570
            })
        );
        let v6 = Url::parse("http://[::1]:16662").unwrap();
        assert_eq!(
            join_target(&v6, &server(25565)).unwrap().address(),
            "[::1]:25565"
        );
        let named = Url::parse("http://localhost:16662").unwrap();
        assert_eq!(
            join_target(&named, &server(25565)).unwrap().address(),
            "localhost:25565"
        );
        // a port that doesn't fit
        assert_eq!(join_target(&named, &server(70000)), None);
        // the address isn't shared
        assert_eq!(join_target(&named, &server(None)), None);
    }
}
