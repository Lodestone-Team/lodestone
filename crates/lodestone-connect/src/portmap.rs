//! Asking the router to forward a port: UPnP IGD first, then NAT-PMP. With a mapping, the
//! router lets the other side's packets in even before this side has sent anything to
//! it, which gets through routers whose filtering defeats plain punching.
//!
//! Both are a few plain UDP and HTTP requests on the local network; no crate. Blocking.
//! A mapping lasts [`LEASE_SECS`]; [`Mapping::renew`] extends it and [`Mapping::remove`]
//! takes it away again.

use std::{
    io::{Read as _, Write as _},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, UdpSocket},
    time::Duration,
};

/// How long the router keeps a mapping unless it's renewed.
pub const LEASE_SECS: u32 = 3600;
const DESCRIPTION: &str = "Lodestone Connect";

/// A port the router forwards to this computer.
#[derive(Debug)]
pub struct Mapping {
    /// The address the internet reaches the port at.
    pub external: SocketAddr,
    /// "UPnP" or "NAT-PMP".
    pub how: &'static str,
    kind: Kind,
    port: u16,
    local_ip: Ipv4Addr,
}

#[derive(Debug)]
enum Kind {
    Upnp {
        control_url: String,
        service: String,
    },
    NatPmp {
        gateway: Ipv4Addr,
    },
}

impl Mapping {
    /// The address on the local network the router forwards to.
    pub fn local_ip(&self) -> Ipv4Addr {
        self.local_ip
    }

    /// Asks the router for another [`LEASE_SECS`].
    pub fn renew(&self) -> Result<(), String> {
        match &self.kind {
            Kind::Upnp {
                control_url,
                service,
            } => add_mapping(control_url, service, self.port, self.local_ip).map(|_| ()),
            Kind::NatPmp { gateway } => natpmp_map(*gateway, self.port, LEASE_SECS).map(|_| ()),
        }
    }

    /// Removes the forwarding from the router. Best effort.
    pub fn remove(&self) {
        match &self.kind {
            Kind::Upnp {
                control_url,
                service,
            } => {
                let _ = soap(
                    control_url,
                    service,
                    "DeletePortMapping",
                    &[
                        ("NewRemoteHost", String::new()),
                        ("NewExternalPort", self.external.port().to_string()),
                        ("NewProtocol", "UDP".into()),
                    ],
                );
            }
            Kind::NatPmp { gateway } => {
                let _ = natpmp_map(*gateway, self.port, 0);
            }
        }
    }
}

/// Maps UDP `port` on this computer (`local_ip`) to the same port on the router.
pub fn map(port: u16, local_ip: Ipv4Addr) -> Result<Mapping, String> {
    let upnp = upnp_map(port, local_ip);
    if let Ok(mapping) = upnp {
        return Ok(mapping);
    }
    let gateway = guess_gateway(local_ip);
    match natpmp_map(gateway, port, LEASE_SECS) {
        Ok(external_port) => {
            let ip = natpmp_external_ip(gateway)?;
            Ok(Mapping {
                external: SocketAddr::new(IpAddr::V4(ip), external_port),
                how: "NAT-PMP",
                kind: Kind::NatPmp { gateway },
                port,
                local_ip,
            })
        }
        Err(natpmp) => Err(format!(
            "UPnP: {}; NAT-PMP: {natpmp}",
            upnp.err().unwrap_or_default()
        )),
    }
}

/// Whether `ip` is only reachable inside a network (the router itself is behind
/// another NAT when its "external" address is one of these).
pub fn is_private(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || (a == 100 && (64..128).contains(&b))
}

/// Most home routers are the first address of the network.
fn guess_gateway(local: Ipv4Addr) -> Ipv4Addr {
    let [a, b, c, _] = local.octets();
    Ipv4Addr::new(a, b, c, 1)
}

// ---------------------------------------------------------------------------
// UPnP IGD

fn add_mapping(
    control_url: &str,
    service: &str,
    port: u16,
    local_ip: Ipv4Addr,
) -> Result<String, String> {
    soap(
        control_url,
        service,
        "AddPortMapping",
        &[
            ("NewRemoteHost", String::new()),
            ("NewExternalPort", port.to_string()),
            ("NewProtocol", "UDP".into()),
            ("NewInternalPort", port.to_string()),
            ("NewInternalClient", local_ip.to_string()),
            ("NewEnabled", "1".into()),
            ("NewPortMappingDescription", DESCRIPTION.into()),
            ("NewLeaseDuration", LEASE_SECS.to_string()),
        ],
    )
}

fn upnp_map(port: u16, local_ip: Ipv4Addr) -> Result<Mapping, String> {
    let locations = ssdp_search()?;
    let mut last = "no router answered the UPnP search".to_string();
    for location in locations {
        let (control_url, service) = match find_service(&location) {
            Ok(found) => found,
            Err(err) => {
                last = err;
                continue;
            }
        };
        if let Err(err) = add_mapping(&control_url, &service, port, local_ip) {
            last = format!("the router refused the mapping ({err})");
            continue;
        }
        let ip = soap(&control_url, &service, "GetExternalIPAddress", &[])
            .ok()
            .and_then(|body| tag(&body, "NewExternalIPAddress"))
            .and_then(|ip| ip.trim().parse::<Ipv4Addr>().ok());
        let Some(ip) = ip else {
            last = "the router didn't say its external address".into();
            continue;
        };
        return Ok(Mapping {
            external: SocketAddr::new(IpAddr::V4(ip), port),
            how: "UPnP",
            kind: Kind::Upnp {
                control_url,
                service,
            },
            port,
            local_ip,
        });
    }
    Err(last)
}

fn ssdp_search() -> Result<Vec<String>, String> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).map_err(|e| e.to_string())?;
    socket
        .set_read_timeout(Some(Duration::from_millis(1500)))
        .map_err(|e| e.to_string())?;
    for st in [
        "urn:schemas-upnp-org:device:InternetGatewayDevice:1",
        "urn:schemas-upnp-org:device:InternetGatewayDevice:2",
    ] {
        let search = format!(
            "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 1\r\nST: {st}\r\n\r\n"
        );
        let _ = socket.send_to(search.as_bytes(), "239.255.255.250:1900");
    }
    let mut locations = Vec::new();
    let mut buf = [0u8; 2048];
    while let Ok((len, _)) = socket.recv_from(&mut buf) {
        let text = String::from_utf8_lossy(&buf[..len]);
        for line in text.lines() {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            if !name.trim().eq_ignore_ascii_case("location") {
                continue;
            }
            let value = value.trim().to_string();
            // only routers on the local network, over plain HTTP
            if value.starts_with("http://") && !locations.contains(&value) {
                locations.push(value);
            }
        }
    }
    if locations.is_empty() {
        Err("no router answered the UPnP search (UPnP may be off)".into())
    } else {
        Ok(locations)
    }
}

/// The WAN connection service's control URL and type, from the device description.
fn find_service(location: &str) -> Result<(String, String), String> {
    let body = http(location, "GET", &[], "")?;
    let (base_host, _) = split_url(location)?;
    let base = tag(&body, "URLBase")
        .map(|url| url.trim().trim_end_matches('/').to_string())
        .unwrap_or_else(|| format!("http://{base_host}"));
    for service in [
        "urn:schemas-upnp-org:service:WANIPConnection:2",
        "urn:schemas-upnp-org:service:WANIPConnection:1",
        "urn:schemas-upnp-org:service:WANPPPConnection:1",
    ] {
        let Some(at) = body.find(&format!("<serviceType>{service}</serviceType>")) else {
            continue;
        };
        let rest = &body[at..];
        let end = rest.find("</service>").unwrap_or(rest.len());
        let Some(control) = tag(&rest[..end], "controlURL") else {
            continue;
        };
        let control = control.trim();
        let url = if control.starts_with("http") {
            control.to_string()
        } else {
            format!("{base}/{}", control.trim_start_matches('/'))
        };
        return Ok((url, service.to_string()));
    }
    Err("the router has no WAN connection service".into())
}

fn soap(
    control_url: &str,
    service: &str,
    action: &str,
    args: &[(&str, String)],
) -> Result<String, String> {
    let args: String = args
        .iter()
        .map(|(name, value)| format!("<{name}>{value}</{name}>"))
        .collect();
    let body = format!(
        "<?xml version=\"1.0\"?>\r\n<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" \
         s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body>\
         <u:{action} xmlns:u=\"{service}\">{args}</u:{action}></s:Body></s:Envelope>"
    );
    let soap_action = format!("\"{service}#{action}\"");
    http(
        control_url,
        "POST",
        &[
            ("Content-Type", "text/xml; charset=\"utf-8\""),
            ("SOAPAction", &soap_action),
        ],
        &body,
    )
}

fn split_url(url: &str) -> Result<(String, String), String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| format!("not an http URL: {url}"))?;
    // The URL comes from whatever answered on the network: it goes into the request
    // line, so nothing may break out of it.
    if rest.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("a router address with spaces or line breaks".into());
    }
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    Ok((host.to_string(), path.to_string()))
}

/// A minimal HTTP/1.1 request to the router; returns the body of a 200 answer.
fn http(url: &str, method: &str, headers: &[(&str, &str)], body: &str) -> Result<String, String> {
    let (host, path) = split_url(url)?;
    let addr: SocketAddr = if host.contains(':') {
        host.parse()
            .map_err(|_| format!("bad router address {host}"))?
    } else {
        format!("{host}:80")
            .parse()
            .map_err(|_| format!("bad router address {host}"))?
    };
    // A router is on the local network: never follow a description elsewhere, and
    // never to this computer's own services.
    match addr.ip() {
        IpAddr::V4(ip) if is_private(ip) && !ip.is_loopback() => {}
        _ => return Err(format!("{host} isn't on the local network")),
    }
    let mut stream =
        TcpStream::connect_timeout(&addr, Duration::from_secs(2)).map_err(|e| e.to_string())?;
    stream.set_read_timeout(Some(Duration::from_secs(3))).ok();
    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (name, value) in headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(body);
    stream
        .write_all(request.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut response = Vec::new();
    let _ = stream.take(256 * 1024).read_to_end(&mut response);
    read_answer(&response)
}

/// The body of a 200 answer, as text. The body is de-chunked as bytes and only then
/// turned into text: chunk sizes count bytes on the wire, and whatever answered on the
/// network may send Latin-1 or split a character across chunks, which would put a cut
/// inside a character of the decoded text.
fn read_answer(response: &[u8]) -> Result<String, String> {
    let status_line = || {
        let end = response
            .windows(2)
            .position(|w| w == b"\r\n")
            .unwrap_or(response.len());
        String::from_utf8_lossy(&response[..end]).into_owned()
    };
    match crate::info::parse_response(response) {
        Some((200, body)) => Ok(String::from_utf8_lossy(&body).into_owned()),
        _ => Err(status_line()),
    }
}

/// The text inside the first `<name>…</name>` (with or without a namespace prefix).
fn tag(xml: &str, name: &str) -> Option<String> {
    let open = xml.find(&format!("{name}>"))?;
    let start = open + name.len() + 1;
    let end = xml[start..].find("</")? + start;
    Some(xml[start..end].to_string())
}

// ---------------------------------------------------------------------------
// NAT-PMP (RFC 6886)

fn natpmp_socket(gateway: Ipv4Addr) -> Result<UdpSocket, String> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).map_err(|e| e.to_string())?;
    socket
        .set_read_timeout(Some(Duration::from_millis(800)))
        .map_err(|e| e.to_string())?;
    socket.connect((gateway, 5351)).map_err(|e| e.to_string())?;
    Ok(socket)
}

/// Maps UDP `port`; returns the external port. A lifetime of 0 removes the mapping.
fn natpmp_map(gateway: Ipv4Addr, port: u16, lifetime: u32) -> Result<u16, String> {
    let socket = natpmp_socket(gateway)?;
    let mut request = vec![0u8, 1, 0, 0];
    request.extend_from_slice(&port.to_be_bytes());
    request.extend_from_slice(&(if lifetime == 0 { 0 } else { port }).to_be_bytes());
    request.extend_from_slice(&lifetime.to_be_bytes());
    let mut buf = [0u8; 16];
    for _ in 0..2 {
        let _ = socket.send(&request);
        if let Ok(16) = socket.recv(&mut buf) {
            if buf[1] != 129 {
                return Err("unexpected answer".into());
            }
            let result = u16::from_be_bytes([buf[2], buf[3]]);
            if result != 0 {
                return Err(format!("the router refused (code {result})"));
            }
            return Ok(u16::from_be_bytes([buf[10], buf[11]]));
        }
    }
    Err(format!("no answer from {gateway}"))
}

fn natpmp_external_ip(gateway: Ipv4Addr) -> Result<Ipv4Addr, String> {
    let socket = natpmp_socket(gateway)?;
    let mut buf = [0u8; 12];
    let _ = socket.send(&[0, 0]);
    match socket.recv(&mut buf) {
        Ok(12) if buf[1] == 128 && buf[2..4] == [0, 0] => {
            Ok(Ipv4Addr::new(buf[8], buf[9], buf[10], buf[11]))
        }
        _ => Err("the router didn't say its external address".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_tags_and_chunks() {
        assert_eq!(
            tag(
                "<s:Body><NewExternalIPAddress>1.2.3.4</NewExternalIPAddress>",
                "NewExternalIPAddress"
            ),
            Some("1.2.3.4".into())
        );
        assert_eq!(
            read_answer(
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n"
            ),
            Ok("hello world".into())
        );
        assert!(is_private("100.72.1.1".parse().unwrap()));
        assert!(!is_private("99.228.45.79".parse().unwrap()));
    }

    #[test]
    fn reads_chunks_that_split_a_character() {
        // Chunk sizes count bytes: a chunk that ends inside a character, or a Latin-1
        // byte, must not cut the decoded text inside a character (LS-36).
        assert_eq!(
            read_answer(
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nab\xc3\r\n1\r\n\xa9\r\n0\r\n\r\n"
            ),
            Ok("ab\u{e9}".into())
        );
        assert_eq!(
            read_answer(
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nGer\xe4\r\n2\r\nt!\r\n0\r\n\r\n"
            ),
            Ok("Ger\u{fffd}t!".into())
        );
        assert_eq!(
            read_answer(b"HTTP/1.1 500 Internal Server Error\r\n\r\noops"),
            Err("HTTP/1.1 500 Internal Server Error".into())
        );
    }

    #[test]
    fn never_asks_a_router_outside_the_network() {
        assert!(http("http://203.0.113.5:5000/desc.xml", "GET", &[], "")
            .unwrap_err()
            .contains("local network"));
        // Anything on the network can answer the UPnP search: it can't point this
        // computer at its own services, or add lines to the request.
        assert!(http("http://127.0.0.1:16662/api/v1/x", "GET", &[], "")
            .unwrap_err()
            .contains("local network"));
        assert!(
            http("http://192.0.2.1/ctl\r\nX-Injected: 1", "POST", &[], "")
                .unwrap_err()
                .contains("line breaks")
        );
    }
}
