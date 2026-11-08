//! A timing of the "warm launch": everything is installed, and the launcher checks it
//! before starting Java. Ignored by default because it writes about 560 MB; run it with
//!
//! ```sh
//! cargo test --release -p launcher-core --test warm_launch -- --ignored --nocapture
//! ```
//!
//! The made-up version has the shape of a current release: a 25 MB client jar, 90
//! libraries (about 55 MB) and 4,000 asset objects (about 480 MB), served from a
//! loopback server. The time measured is from the start of `prepare` to a built
//! command line, which is what Play waits for before Java is spawned. It also times
//! "Verify game files", which hashes everything.

mod files;

use std::time::{Duration, Instant};

use files::{Entry, FileServer};
use futures::executor::block_on;
use launcher_core::{
    Secret,
    game::{
        download::{CancelToken, Downloader, Verify, recording_sleep, sha1_hex},
        install::{InstallProgress, Installer, JavaChoice},
        launch::{self, GameSettings, LaunchAccount},
        mojang::MojangUrls,
        platform::{Arch, Os, Platform},
    },
    http,
    paths::DataDir,
};
use serde_json::json;

/// Deterministic bytes, different for each seed.
fn bytes(seed: u64, len: usize) -> Vec<u8> {
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut out = Vec::with_capacity(len + 8);
    while out.len() < len {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.extend_from_slice(&state.to_le_bytes());
    }
    out.truncate(len);
    out
}

fn serve(server: &FileServer, url: &str, body: Vec<u8>) -> serde_json::Value {
    let value = json!({"url": url, "sha1": sha1_hex(&body), "size": body.len()});
    server.serve(url, Entry::ok(body));
    value
}

fn fixture(server: &FileServer) {
    let client = serve(
        server,
        "https://piston-data.mojang.com/v1/objects/c/client.jar",
        bytes(1, 25 << 20),
    );
    let libraries: Vec<_> = (0..90u64)
        .map(|n| {
            let path = format!("com/example/lib{n}/1.0/lib{n}-1.0.jar");
            let size = 64 * 1024 + (n as usize * 13_000);
            let artifact = serve(server, &format!("https://libraries.minecraft.net/{path}"), bytes(100 + n, size));
            json!({"name": format!("com.example:lib{n}:1.0"), "downloads": {"artifact": {
                "path": path, "url": artifact["url"], "sha1": artifact["sha1"], "size": artifact["size"]}}})
        })
        .collect();
    let mut objects = serde_json::Map::new();
    for n in 0..4000u64 {
        let body = bytes(10_000 + n, 2_000 + (n as usize * 7_919) % 240_000);
        let hash = sha1_hex(&body);
        let size = body.len();
        server.serve(
            &format!(
                "https://resources.download.minecraft.net/{}/{hash}",
                &hash[..2]
            ),
            Entry::ok(body),
        );
        objects.insert(
            format!("minecraft/sounds/{n}.ogg"),
            json!({"hash": hash, "size": size}),
        );
    }
    let index = serve(
        server,
        "https://piston-meta.mojang.com/v1/packages/i/bench.json",
        json!({"objects": objects}).to_string().into_bytes(),
    );
    let version = json!({
        "id": "bench", "type": "release", "mainClass": "net.minecraft.client.main.Main",
        "minecraftArguments": "--username ${auth_player_name} --accessToken ${auth_access_token}",
        "assetIndex": {"id": "bench", "sha1": index["sha1"], "size": index["size"], "url": index["url"]},
        "downloads": {"client": client},
        "libraries": libraries,
    })
    .to_string();
    let version_url = "https://piston-meta.mojang.com/v1/packages/v/bench.json";
    let version_sha1 = sha1_hex(version.as_bytes());
    server.serve(version_url, Entry::ok(version.into_bytes()));
    server.serve(
        "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json",
        Entry::ok(
            json!({"versions": [{"id": "bench", "type": "release", "url": version_url, "sha1": version_sha1}]})
                .to_string(),
        ),
    );
}

#[test]
#[ignore = "writes about 560 MB; run by hand to time warm launches"]
fn time_warm_launches() {
    let server = FileServer::start();
    fixture(&server);
    let data = tempfile::tempdir().unwrap();
    let (sleep, _) = recording_sleep();
    let installer = Installer::new(
        http::default_client().unwrap(),
        Downloader::new(http::download_client().unwrap(), sleep),
        MojangUrls::mirror(&server.base),
        DataDir::at(data.path()),
    );
    let machine = Platform::new(Os::MacOs, Arch::Arm64, "14.6.1");
    let game_dir = data.path().join("instances/bench/minecraft");
    let java = JavaChoice::Override("/usr/bin/true".into());
    let account = LaunchAccount::microsoft(
        "Bench",
        uuid::Uuid::nil(),
        Secret::new("token-for-bench"),
        None,
    );
    let launch = || {
        let start = Instant::now();
        let prepared = block_on(installer.prepare(
            "bench",
            &machine,
            &java,
            &game_dir,
            &InstallProgress::default(),
            &CancelToken::new(),
        ))
        .unwrap();
        let natives = data.path().join("instances/bench/natives");
        prepared
            .extract_natives(&DataDir::at(data.path()), &natives)
            .unwrap();
        let paths = prepared
            .launch_paths(&DataDir::at(data.path()), &game_dir, &natives)
            .unwrap();
        launch::build(
            &prepared.version,
            &prepared.java_platform,
            &paths,
            &account,
            &GameSettings::default(),
        )
        .unwrap();
        start.elapsed()
    };
    let cold = launch();
    eprintln!("cold install (downloads from loopback): {cold:?}");
    let mut warm: Vec<Duration> = (0..5).map(|_| launch()).collect();
    warm.sort();
    eprintln!("warm launches: {warm:?}");
    eprintln!("warm median: {:?}", warm[warm.len() / 2]);

    // "Verify game files": every file hashed, in parallel.
    let start = Instant::now();
    let progress = InstallProgress::default();
    block_on(installer.prepare_with(
        "bench",
        &machine,
        &java,
        &game_dir,
        Verify::Full,
        &progress,
        &CancelToken::new(),
    ))
    .unwrap();
    eprintln!(
        "verify game files: {:?}, (trusted, hashed, downloaded) = {:?}",
        start.elapsed(),
        progress.stats.counts()
    );
}
