//! Where to join a server: its playit.gg address when it has one, otherwise the core's
//! host with the server's game port.

use url::Url;

use super::models::ServerInstance;
use crate::game::launch::JoinServer;

/// Reads `host:port` (or `[v6]:port`, or a host alone, port 25565).
fn parse_host_port(address: &str) -> Option<JoinServer> {
    let address = address.trim();
    if address.is_empty() {
        return None;
    }
    if let Some(rest) = address.strip_prefix('[') {
        let (host, after) = rest.split_once(']')?;
        let port = match after.strip_prefix(':') {
            Some(port) => port.parse().ok()?,
            None if after.is_empty() => 25565,
            None => return None,
        };
        return Some(JoinServer {
            host: host.to_string(),
            port,
        });
    }
    match address.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => Some(JoinServer {
            host: host.to_string(),
            port: port.parse().ok()?,
        }),
        Some(_) => None,
        None => Some(JoinServer {
            host: address.to_string(),
            port: 25565,
        }),
    }
}

/// The address to join `server` at, on the core at `core_url`.
pub fn join_target(core_url: &Url, server: &ServerInstance) -> Option<JoinServer> {
    if let Some(target) = server.playit_address.as_deref().and_then(parse_host_port) {
        return Some(target);
    }
    let host = match core_url.host()? {
        url::Host::Domain(domain) => domain.to_string(),
        url::Host::Ipv4(ip) => ip.to_string(),
        url::Host::Ipv6(ip) => ip.to_string(),
    };
    Some(JoinServer {
        host,
        port: u16::try_from(server.port).ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cores::models::{ServerLoader, ServerState};

    fn server(port: u32, playit: Option<&str>) -> ServerInstance {
        ServerInstance {
            uuid: "i".into(),
            name: "SMP".into(),
            game_version: "1.20.1".into(),
            loader: ServerLoader::Fabric,
            loader_version: None,
            port,
            state: ServerState::Running,
            player_count: None,
            max_player_count: None,
            has_client_pack: false,
            playit_address: playit.map(str::to_string),
            listed: true,
        }
    }

    #[test]
    fn prefers_playit_then_the_core_host() {
        let core = Url::parse("https://192.168.1.5:16662").unwrap();
        assert_eq!(
            join_target(&core, &server(25570, None)),
            Some(JoinServer {
                host: "192.168.1.5".into(),
                port: 25570
            })
        );
        assert_eq!(
            join_target(&core, &server(25570, Some("abc.ply.gg:41234"))),
            Some(JoinServer {
                host: "abc.ply.gg".into(),
                port: 41234
            })
        );
        // an unreadable playit address falls back to the core
        assert_eq!(
            join_target(&core, &server(25570, Some("a:b:c")))
                .unwrap()
                .host,
            "192.168.1.5"
        );
        let v6 = Url::parse("http://[::1]:16662").unwrap();
        assert_eq!(
            join_target(&v6, &server(25565, None)).unwrap().address(),
            "[::1]:25565"
        );
        let named = Url::parse("http://localhost:16662").unwrap();
        assert_eq!(
            join_target(&named, &server(25565, None)).unwrap().address(),
            "localhost:25565"
        );
        // a port that doesn't fit
        assert_eq!(join_target(&named, &server(70000, None)), None);
    }

    #[test]
    fn reads_host_and_port() {
        assert_eq!(parse_host_port("mc.example.com").unwrap().port, 25565);
        assert_eq!(parse_host_port("[fe80::1]:2").unwrap().host, "fe80::1");
        assert!(parse_host_port("[fe80::1]x").is_none());
        assert!(parse_host_port("").is_none());
        assert!(parse_host_port("host:notaport").is_none());
    }
}
