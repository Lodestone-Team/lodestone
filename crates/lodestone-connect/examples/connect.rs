//! Try Lodestone Connect from a terminal.
//!
//! ```sh
//! # share a TCP port (here a test listener: `nc -l 25565`)
//! cargo run -p lodestone_connect --all-features --example connect -- host --game 127.0.0.1:25565
//! # join with the link it prints; Minecraft (or `nc 127.0.0.1 25566`) connects to the port
//! cargo run -p lodestone_connect --all-features --example connect -- join "<link>" --listen 127.0.0.1:25566
//! # or check the tunnel once and exit: send a line on a game stream and print the answer
//! cargo run -p lodestone_connect --all-features --example connect -- join "<link>" --check hello
//! ```
//!
//! Options: `--via <service URL>` (default https://connect.lodestone.cc), `--loopback`
//! (offer only 127.0.0.1: both sides on this computer, with a local service),
//! `--no-portmap` (don't ask the router to forward a port), and for the host
//! `--info <ip:port>` (an HTTP server to answer info streams) and `--link-file <path>`
//! (write the link there instead of printing it; join it with `join @<path>`).

use std::{net::SocketAddr, time::Duration};

use lodestone_connect::{
    host::{Forward, Host, HostConfig, HostIdentity},
    join::{join, JoinOptions},
    GatherOptions, Invite, Service, StreamKind,
};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

type Error = Box<dyn std::error::Error + Send + Sync>;

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn has(args: &[String], name: &str) -> bool {
    args.iter().any(|arg| arg == name)
}

fn gather(args: &[String]) -> GatherOptions {
    GatherOptions {
        portmap: !has(args, "--no-portmap"),
        loopback_only: has(args, "--loopback"),
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("host") => host(&args[1..]).await,
        Some("join") => join_cmd(&args[1..]).await,
        _ => {
            eprintln!(
                "usage:\n  connect host --game <ip:port> [--info <ip:port>] [--via <url>] [--loopback] [--no-portmap]\n  \
                 connect join <link> [--listen 127.0.0.1:25566 | --check <text>] [--loopback] [--no-portmap]"
            );
            std::process::exit(2);
        }
    };
    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

async fn host(args: &[String]) -> Result<(), Error> {
    let game: SocketAddr = flag(args, "--game")
        .ok_or("--game <ip:port> is needed")?
        .parse()?;
    let info = flag(args, "--info").map(|text| text.parse()).transpose()?;
    let service = match flag(args, "--via") {
        Some(text) => Service::parse(&text).ok_or("--via takes a service URL")?,
        None => Service::default(),
    };
    let config = HostConfig {
        service,
        gather: gather(args),
        keep: true,
    };
    println!("Finding the ways in to this computer...");
    let host = Host::start(HostIdentity::generate()?, config, Forward { game, info }).await?;
    let status = host.status();
    for candidate in &status.candidates {
        println!("  {:?}: {}", candidate.kind, candidate.addr);
    }
    match flag(args, "--link-file") {
        // for scripts: the link goes to a file rather than the terminal
        Some(path) => {
            std::fs::write(&path, host.invite().link())?;
            println!("\nWrote the link to {path}.\n");
        }
        None => println!("\nShare this link:\n\n  {}\n", host.invite().link()),
    }
    println!("Press Ctrl-C to stop sharing.");
    let mut changed = host.subscribe();
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            _ = changed.changed() => {
                let status = host.status();
                println!(
                    "{:?}: {} joined, {} connected{}",
                    status.state,
                    status.joins,
                    status.friends.len(),
                    status.problem.map(|p| format!(" ({p})")).unwrap_or_default()
                );
            }
        }
    }
    host.stop(true).await;
    println!("Stopped, and ended the link.");
    Ok(())
}

async fn join_cmd(args: &[String]) -> Result<(), Error> {
    let first = args.first().ok_or("pass the invite link, in quotes")?;
    // `@path` reads the link from a file (see `host --link-file`)
    let text = match first.strip_prefix('@') {
        Some(path) => std::fs::read_to_string(path)?,
        None => first.clone(),
    };
    let invite = Invite::parse(&text)?;
    let options = JoinOptions {
        gather: gather(args),
        ..JoinOptions::default()
    };
    println!("Connecting...");
    let joined = join(&invite, options).await?;
    println!(
        "Connected directly over {} (round trip {} ms).",
        joined.kind().describe(),
        joined.rtt().as_millis()
    );
    match joined.server_info().await {
        Ok(server) => println!(
            "Server: {} (Minecraft {}, {}{}){}",
            server.name,
            server.game_version,
            server.loader,
            server
                .loader_version
                .map(|v| format!(" {v}"))
                .unwrap_or_default(),
            if server.modpack { ", with mods" } else { "" }
        ),
        Err(err) => println!("No server info: {err}"),
    }
    if let Some(text) = flag(args, "--check") {
        let mut stream = joined.open(StreamKind::Game).await?;
        stream.write_all(format!("{text}\n").as_bytes()).await?;
        let mut answer = vec![0u8; text.len() + 1];
        tokio::time::timeout(Duration::from_secs(10), stream.read_exact(&mut answer)).await??;
        println!(
            "The game port answered: {}",
            String::from_utf8_lossy(&answer).trim_end()
        );
        joined.close().await;
        return Ok(());
    }
    let listen: SocketAddr = flag(args, "--listen")
        .unwrap_or_else(|| "127.0.0.1:25566".into())
        .parse()?;
    let forward = joined.forward(StreamKind::Game, listen).await?;
    println!(
        "In Minecraft, add the server {} and join it.",
        forward.local_addr()
    );
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        reason = joined.closed() => println!("The tunnel closed: {reason}"),
    }
    Ok(())
}
