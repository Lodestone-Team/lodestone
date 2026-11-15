//! Milestone 3 end to end: a test client does what a player's Launcher will (milestone 4):
//! it downloads a server's client view, fetches the files only the server has into a store
//! of its own, places them into an empty folder with `lodestone_image`, and keeps up with
//! the server's changes through deltas, all over the HTTP API. Every core lives in a
//! temporary folder; no server is started, nothing is downloaded and no EULA is accepted.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use lodestone_image::hex::sha256_hex;
use lodestone_image::store::materialize::{materialize, missing_objects, MaterializeOptions};
use lodestone_image::store::{Encoding, Store};
use lodestone_image::view::check_received;
use lodestone_image::{apply, Delta, DiskLayout, EnvFrom, EnvValue, Image, Side, Source};
use reqwest::{Method, StatusCode};
use serde_json::json;
use sha2::Digest;

use crate::handlers::instance::content_images_tests::{self as content, write, Kind};
use crate::images::end_to_end_tests::age_store;
use crate::images::ModrinthHint;
use crate::mods::metadata::tests::make_zip;

/// A Fabric mod jar with `environment`.
fn fabric_jar(path: &Path, id: &str, environment: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    make_zip(
        path,
        &[(
            "fabric.mod.json",
            &format!(
                r#"{{"schemaVersion":1,"id":"{id}","version":"1","environment":"{environment}"}}"#
            ),
        )],
    );
}

/// A delta applied as a player must: to the view its `from` names, then checked as a whole
/// view would be (`apply` checks only the result's id), and its id against
/// `X-Lodestone-View`.
fn apply_received(from: &Image, delta: &Delta, current: &str) -> Image {
    let next = apply(from, delta).unwrap();
    check_received(&next, Side::Client).unwrap();
    assert_eq!(next.id, current);
    next
}

/// What a player's client does with one server.
struct Player<'a> {
    core: &'a content::Core,
    http: reqwest::Client,
    /// The player's own store: raw files, like the Launcher's.
    store: Store,
    _dir: tempfile::TempDir,
    /// Files the test stands in for Modrinth's CDN, the base pack's `.mrpack` and
    /// CurseForge with, found by SHA-256.
    elsewhere: Vec<Vec<u8>>,
}

/// An answer to `GET …/image`.
enum Answer {
    Current(String),
    View(Box<Image>),
    Delta(Delta, String),
}

impl<'a> Player<'a> {
    fn new(core: &'a content::Core, elsewhere: Vec<Vec<u8>>) -> Player<'a> {
        let dir = tempfile::tempdir().unwrap();
        Player {
            core,
            http: reqwest::Client::new(),
            store: Store::new(dir.path().join("store"), Encoding::Raw),
            _dir: dir,
            elsewhere,
        }
    }

    fn url(&self, rest: &str) -> String {
        format!(
            "{}/launcher/instances/{}/{rest}",
            self.core.base, self.core.uuid
        )
    }

    /// `GET …/image?side=client&have=…`, through 503s while the first image is recorded,
    /// checking the answer as a player must.
    async fn image(&self, have: &[&str]) -> Answer {
        let mut url = self.url(&format!(
            "image?side=client&rules={}",
            lodestone_image::RULES
        ));
        if !have.is_empty() {
            url.push_str(&format!("&have={}", have.join(",")));
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            let response = self
                .http
                .get(&url)
                .bearer_auth(&self.core.owner)
                .send()
                .await
                .unwrap();
            if response.status() == StatusCode::SERVICE_UNAVAILABLE {
                assert!(std::time::Instant::now() < deadline);
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                continue;
            }
            return self.answer(response).await;
        }
    }

    async fn answer(&self, response: reqwest::Response) -> Answer {
        let status = response.status();
        let current = response.headers()["x-lodestone-view"]
            .to_str()
            .unwrap()
            .to_string();
        if status == StatusCode::NO_CONTENT {
            return Answer::Current(current);
        }
        assert_eq!(status, StatusCode::OK);
        let kind = response.headers()["x-lodestone-answer"]
            .to_str()
            .unwrap()
            .to_string();
        let body = response.bytes().await.unwrap();
        match kind.as_str() {
            "view" => {
                let view = Image::from_json(&body).unwrap();
                check_received(&view, Side::Client).unwrap();
                assert_eq!(view.id, current);
                Answer::View(Box::new(view))
            }
            "delta" => Answer::Delta(Delta::from_json(&body).unwrap(), current),
            other => panic!("unknown answer {other}"),
        }
    }

    /// `GET …/objects/{sha256}`: the status, and the bytes when they check out. Asks again,
    /// as a player must, while the server says it is busy (`TooManyRequests`: a collection
    /// runs, or too many downloads do), sooner than its `Retry-After` to keep tests quick.
    async fn object(&self, sha256: &str) -> (StatusCode, Vec<u8>) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            let response = self
                .http
                .get(self.url(&format!("objects/{sha256}")))
                .bearer_auth(&self.core.owner)
                .send()
                .await
                .unwrap();
            let status = response.status();
            let bytes = response
                .bytes()
                .await
                .map(|b| b.to_vec())
                .unwrap_or_default();
            if status == StatusCode::SERVICE_UNAVAILABLE {
                let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                if body["kind"] == "TooManyRequests" {
                    assert!(std::time::Instant::now() < deadline, "always busy");
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                    continue;
                }
            }
            if status == StatusCode::OK {
                assert_eq!(sha256_hex(&bytes), sha256, "the bytes the server sent");
            }
            return (status, bytes);
        }
    }

    /// Fetches what the player's store lacks of `view`: objects (and the client settings'
    /// icon and background) from the server, Modrinth, base pack and CurseForge files from
    /// the stand-in for those (which the server must refuse). False when an object is no
    /// longer served (the view was superseded meanwhile).
    async fn fetch(&self, view: &Image) -> bool {
        let stored = view
            .settings
            .client
            .iter()
            .flat_map(|client| client.icon.iter().chain(client.background.iter()));
        for file in stored {
            if self.store.object_complete(&file.sha256, file.size) {
                continue;
            }
            let (status, bytes) = self.object(&file.sha256).await;
            if status == StatusCode::NOT_FOUND {
                return false;
            }
            assert_eq!(status, StatusCode::OK, "a stored setting");
            self.store.put_bytes(&bytes).unwrap();
        }
        for entry in missing_objects(
            &self.store,
            view,
            lodestone_image::store::materialize::Selection::All,
        ) {
            match &entry.source {
                Source::Object {} => {
                    let (status, bytes) = self.object(&entry.sha256).await;
                    if status == StatusCode::NOT_FOUND {
                        return false;
                    }
                    assert_eq!(status, StatusCode::OK, "{}", entry.path);
                    self.store.put_bytes(&bytes).unwrap();
                }
                Source::Modrinth { .. } | Source::Base {} | Source::Curseforge { .. } => {
                    if let Source::Modrinth { url, .. } = &entry.source {
                        assert!(url.starts_with("https://cdn.modrinth.com/"));
                    }
                    assert_eq!(
                        self.object(&entry.sha256).await.0,
                        StatusCode::NOT_FOUND,
                        "{} comes from where its source says",
                        entry.path
                    );
                    let bytes = self
                        .elsewhere
                        .iter()
                        .find(|b| sha256_hex(b) == entry.sha256)
                        .expect("its source has it");
                    self.store.put_bytes(bytes).unwrap();
                }
            }
        }
        true
    }

    /// Places `view` into a new empty folder; returns the files there.
    fn place(&self, view: &Image) -> BTreeSet<String> {
        let staging = tempfile::tempdir_in(self._dir.path()).unwrap().keep();
        materialize(
            &self.store,
            view,
            &DiskLayout::Client,
            &staging,
            &MaterializeOptions {
                durable: false,
                ..Default::default()
            },
        )
        .unwrap();
        walkdir::WalkDir::new(&staging)
            .into_iter()
            .flatten()
            .filter(|e| e.file_type().is_file())
            .map(|e| {
                e.path()
                    .strip_prefix(&staging)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect()
    }
}

impl Player<'_> {
    /// What a player ends up with from a view of an image built on a pack, as M4 does it:
    /// the pack's files for clients (`share`, by canonical path, as read from the
    /// `.mrpack`: index files whose client side isn't unsupported, `overrides/` with
    /// `client-overrides/` over them), less the view's `removed`, with the view's entries
    /// placed over them by path. Placed into a new folder and read back: each file with its
    /// bytes' SHA-256.
    fn install(&self, view: &Image, share: &[(&str, &[u8])]) -> BTreeMap<String, String> {
        let staging = tempfile::tempdir_in(self._dir.path()).unwrap().keep();
        materialize(
            &self.store,
            view,
            &DiskLayout::Client,
            &staging,
            &MaterializeOptions {
                durable: false,
                ..Default::default()
            },
        )
        .unwrap();
        for (path, bytes) in share {
            let taken_out = view.removed.iter().any(|p| p.as_str() == *path);
            let listed = view.entries.iter().any(|e| e.path.as_str() == *path);
            if !taken_out && !listed {
                write(&staging, &format!("minecraft/{path}"), bytes);
            }
        }
        walkdir::WalkDir::new(&staging)
            .into_iter()
            .flatten()
            .filter(|e| e.file_type().is_file())
            .map(|e| {
                let path = e
                    .path()
                    .strip_prefix(&staging)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                (path, sha256_hex(&std::fs::read(e.path()).unwrap()))
            })
            .collect()
    }
}

fn set(files: &[&str]) -> BTreeSet<String> {
    files.iter().map(|f| f.to_string()).collect()
}

#[tokio::test]
async fn a_player_downloads_the_client_view_and_keeps_up_with_deltas() {
    let core = content::core(Kind::Fabric).await;
    let mods = core.root.join("mods");
    // JEI: on Modrinth (the core learnt so while listing mods), optional on both sides
    fabric_jar(&mods.join("jei.jar"), "jei", "*");
    let jei = std::fs::read(mods.join("jei.jar")).unwrap();
    let jei_sha512 = format!("{:x}", sha2::Sha512::digest(&jei));
    core.state.images().note_modrinth(
        &jei_sha512,
        ModrinthHint {
            url: Some("https://cdn.modrinth.com/data/u6dRKJwZ/versions/abcdefgh/jei.jar".into()),
            environment: Some("client_or_server".into()),
            client_side: Some("optional".into()),
            server_side: Some("optional".into()),
        },
    );
    // the jar says it is server only; the owner left another out of the client pack
    fabric_jar(&mods.join("server.jar"), "serverthing", "server");
    fabric_jar(&mods.join("hidden.jar"), "hidden", "*");
    write(
        &core.root,
        "mods/.lodestone_client_mods.json",
        br#"{"required":[],"excluded":["hidden.jar"]}"#,
    );
    // nothing known about it (not even a jar): players get it
    write(&core.root, "mods/plain.jar", b"plain");
    write(&core.root, "mods/x.jar.disabled", b"disabled");
    write(&core.root, "config/a.toml", b"password = 1");

    let player = Player::new(&core, vec![jei.clone()]);
    let Answer::View(first) = player.image(&[]).await else {
        panic!("the whole view");
    };
    let content = core.current().await;
    assert_eq!(
        content.label.as_deref(),
        Some(crate::images::recorder::INITIAL_LABEL)
    );
    let entry = |image: &Image, path: &str| {
        image
            .entries
            .iter()
            .find(|e| e.path.as_str() == path)
            .unwrap_or_else(|| panic!("{path}"))
            .clone()
    };
    let jei_entry = entry(&first, "mods/jei.jar");
    assert!(matches!(jei_entry.source, Source::Modrinth { .. }));
    assert_eq!(jei_entry.env.client, EnvValue::Optional);
    assert_eq!(
        entry(&content, "mods/server.jar").env.client,
        EnvValue::Unsupported
    );
    assert_eq!(
        entry(&content, "mods/server.jar").env_from,
        vec![EnvFrom::Jar]
    );
    assert_eq!(
        entry(&content, "mods/hidden.jar").env.client,
        EnvValue::Unsupported
    );
    assert!(player.fetch(&first).await);
    assert_eq!(
        player.place(&first),
        set(&[
            "minecraft/mods/base.jar",
            "minecraft/mods/jei.jar",
            "minecraft/mods/plain.jar",
        ])
    );

    // nothing the view doesn't name is served: server-only and hidden mods, configs,
    // server.properties, the marks file, a disabled jar
    for path in [
        "mods/server.jar",
        "mods/hidden.jar",
        "config/a.toml",
        "config/base.toml",
        "server.properties",
        "mods/x.jar.disabled",
    ] {
        let sha256 = entry(&content, path).sha256;
        assert_eq!(
            player.object(&sha256).await.0,
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
    // Lodestone's own files are never content at all
    assert!(!content
        .entries
        .iter()
        .any(|e| e.path.as_str().contains(".lodestone")));
    // nor a world's files, which a backup stored
    let (status, body) = core
        .call(
            Method::POST,
            &format!("/instance/{}/backups", core.uuid),
            &core.owner,
            Some(json!({ "name": "with the world" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    crate::test_util::wait_until("the backup ends", || async {
        let (_, list) = core
            .call(
                Method::GET,
                &format!("/instance/{}/backups", core.uuid),
                &core.owner,
                None,
            )
            .await;
        list["running"].is_null() && !list["backups"].as_array().unwrap().is_empty()
    })
    .await;
    let region = sha256_hex(b"overworld region");
    assert!(core.state.images().store().has_object(&region, 16));
    assert_eq!(player.object(&region).await.0, StatusCode::NOT_FOUND);

    // in step
    assert!(matches!(
        player.image(&[&first.id]).await,
        Answer::Current(id) if id == first.id
    ));

    // the server changes: a mod uploaded, one removed
    fabric_jar(&core._dir.path().join("added.jar"), "added", "*");
    let added = std::fs::read(core._dir.path().join("added.jar")).unwrap();
    assert_eq!(
        core.upload_mods(&core.owner, &[("added.jar", &added)])
            .await,
        StatusCode::OK
    );
    let (status, body) = core
        .call(
            Method::DELETE,
            &core.fs("mods/plain.jar", "rm"),
            &core.owner,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(core.flush().await.unwrap().changed);

    let Answer::Delta(delta, current) = player.image(&["f".repeat(64).as_str(), &first.id]).await
    else {
        panic!("a delta");
    };
    let second = apply_received(&first, &delta, &current);
    assert_eq!(
        delta
            .new_contents(&first)
            .map(|e| e.path.as_str())
            .collect::<Vec<_>>(),
        ["mods/added.jar"]
    );
    assert!(player.fetch(&second).await);
    assert_eq!(
        player.place(&second),
        set(&[
            "minecraft/mods/added.jar",
            "minecraft/mods/base.jar",
            "minecraft/mods/jei.jar",
        ])
    );
    // the removed mod's object is no longer served
    assert_eq!(
        player
            .object(&entry(&first, "mods/plain.jar").sha256)
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    // in step again; a view the core doesn't know gets the whole view
    assert!(matches!(
        player.image(&[&second.id]).await,
        Answer::Current(id) if id == second.id
    ));
    let Answer::View(whole) = player.image(&["e".repeat(64).as_str()]).await else {
        panic!("the whole view");
    };
    assert_eq!(whole.id, second.id);

    // which is how a player whose views were all pruned catches up: it diffs the whole
    // view against the view it applied, as the three-way merge needs
    let changes = lodestone_image::delta::diff(&first, &whole);
    apply_received(&first, &changes, &second.id);
}

/// A server built on a Modrinth pack, with a CurseForge file and client settings: the
/// player gets the base's files from the pack and the CurseForge file from CurseForge (the
/// server refuses both), the icon and background from the server, and ends with exactly
/// the pack's client files plus what the server added.
#[tokio::test]
async fn a_player_of_a_modpack_server_gets_the_base_from_the_pack() {
    let core = content::core(Kind::Fabric).await;
    let store = core.state.images().store();
    let elsewhere: Vec<Vec<u8>> = vec![
        b"pack mod".to_vec(),
        b"pack server mod".to_vec(),
        b"curseforge mod".to_vec(),
    ];
    let referenced = |path: &str, bytes: &[u8], source: Source, client: EnvValue| {
        let mut entry = lodestone_image::Entry::new(
            lodestone_image::CanonicalPath::new(path).unwrap(),
            bytes.len() as u64,
            sha256_hex(bytes),
            source,
        );
        entry.env = lodestone_image::Env {
            client,
            server: EnvValue::Required,
        };
        entry.env_from = vec![EnvFrom::BasePack];
        entry
    };
    let own = store.put_bytes(b"the server's own mod").unwrap();
    let shared_config = store.put_bytes(b"shared = true").unwrap();
    let icon = store.put_bytes(b"an icon").unwrap();
    let background = store.put_bytes(&vec![7; 3 << 20]).unwrap();
    let object = |path: &str, stored: &lodestone_image::store::Stored| {
        lodestone_image::Entry::new(
            lodestone_image::CanonicalPath::new(path).unwrap(),
            stored.size,
            stored.sha256.clone(),
            Source::Object {},
        )
    };
    let mut config = object("config/shared.toml", &shared_config);
    config.env.client = EnvValue::Required;
    config.env_from = vec![EnvFrom::OwnerMark];
    let mut image = Image::new(
        lodestone_image::Kind::Content,
        lodestone_image::Game {
            minecraft: "1.20.1".into(),
            loader: lodestone_image::Loader::Fabric,
            loader_version: Some("0.16.10".into()),
        },
    );
    image.base = Some(lodestone_image::Base::ModrinthPack {
        project: "pack".into(),
        version: "1.0.0".into(),
        sha512: "c".repeat(128),
    });
    image.entries = vec![
        config,
        referenced(
            "mods/cf.jar",
            &elsewhere[2],
            Source::Curseforge {
                project: 1,
                file: 2,
                downloadable: true,
            },
            EnvValue::Required,
        ),
        object("mods/own.jar", &own),
        referenced(
            "mods/pack.jar",
            &elsewhere[0],
            Source::Base {},
            EnvValue::Required,
        ),
        referenced(
            "mods/pack-server.jar",
            &elsewhere[1],
            Source::Base {},
            EnvValue::Unsupported,
        ),
    ];
    image.entries.sort_by(|a, b| a.path.cmp(&b.path));
    image.settings.client = Some(lodestone_image::settings::ClientSettings {
        icon: Some(lodestone_image::settings::StoredFile {
            sha256: icon.sha256.clone(),
            size: icon.size,
        }),
        background: Some(lodestone_image::settings::StoredFile {
            sha256: background.sha256.clone(),
            size: background.size,
        }),
        ..Default::default()
    });
    image.seal();
    store.save_manifest(core.uuid.as_ref(), &image).unwrap();
    crate::images::state::save(
        core.state.images().root(),
        &core.uuid,
        &crate::images::state::ContentState {
            current: Some(image.id.clone()),
            ..Default::default()
        },
    )
    .unwrap();

    let player = Player::new(&core, elsewhere);
    let Answer::View(view) = player.image(&[]).await else {
        panic!("the whole view");
    };
    assert_eq!(view.base, image.base);
    assert!(player.fetch(&view).await);
    assert!(player.store.object_complete(&icon.sha256, icon.size));
    assert!(player
        .store
        .object_complete(&background.sha256, background.size));
    assert_eq!(
        player.place(&view),
        set(&[
            "minecraft/config/shared.toml",
            "minecraft/mods/cf.jar",
            "minecraft/mods/own.jar",
            "minecraft/mods/pack.jar",
        ])
    );
    // the background is a large file, sent whole and in ranges from its chunks
    let response = player
        .http
        .get(player.url(&format!("objects/{}", background.sha256)))
        .bearer_auth(&core.owner)
        .header("range", "bytes=1048570-1048585")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.bytes().await.unwrap().to_vec(), vec![7; 16]);
    let past = player
        .http
        .get(player.url(&format!("objects/{}", background.sha256)))
        .bearer_auth(&core.owner)
        .header("range", format!("bytes={}-", background.size))
        .send()
        .await
        .unwrap();
    assert_eq!(past.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(
        past.headers()["content-range"],
        format!("bytes */{}", background.size).as_str()
    );
}

/// A server installed from a Modrinth pack, recorded by the core: its content image lists
/// what the server has, names the pack as its base and takes out (`removed`) the pack's
/// files the owner deleted, disabled or replaced under another name. A player that
/// installs the pack's client files, takes those out and places the entries over them ends
/// with exactly what the server's players need: the pack's client-only mods and
/// `client-overrides/`, the pack's configs, the server's own mods, and none of what the
/// owner took out. When the owner then removes a pack mod, the delta takes it out too.
#[tokio::test]
async fn a_player_of_a_pack_server_installs_the_pack_less_what_the_server_took_out() {
    use crate::images::base::{save_base, BaseFile, BaseInfo};
    let core = content::core(Kind::Fabric).await;
    let sha1 = |bytes: &[u8]| {
        use sha1::Digest;
        sha1::Sha1::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let both = lodestone_image::Env::new(EnvValue::Required, EnvValue::Required);
    // what the pack wrote on the server (`pack.json`)
    let pack_file = |bytes: &[u8], env: lodestone_image::Env, download: bool| BaseFile {
        sha1: sha1(bytes),
        env: Some(env),
        modrinth_url: download.then(|| {
            format!(
                "https://cdn.modrinth.com/data/AAAAAAAA/versions/BBBBBBBB/{}.jar",
                sha1(bytes)
            )
        }),
        sha512: download.then(|| "e".repeat(128)),
        ..Default::default()
    };
    let info = BaseInfo {
        base: Some(lodestone_image::Base::ModrinthPack {
            project: "pack".into(),
            version: "1.0.0".into(),
            sha512: "c".repeat(128),
        }),
        files: BTreeMap::from([
            ("mods/pack.jar".into(), pack_file(b"pack mod", both, true)),
            (
                "mods/fabric-api-0.92.0.jar".into(),
                pack_file(b"old api", both, true),
            ),
            (
                "mods/crashes-clients.jar".into(),
                pack_file(b"crashes clients", both, true),
            ),
            (
                "mods/disabled.jar".into(),
                pack_file(b"disabled pack mod", both, true),
            ),
            (
                "config/pack.toml".into(),
                pack_file(b"pack config", both, false),
            ),
            // replaced by `client-overrides/config/x.toml` for clients
            (
                "config/x.toml".into(),
                pack_file(
                    b"server x",
                    lodestone_image::Env::new(EnvValue::Unsupported, EnvValue::Required),
                    false,
                ),
            ),
        ]),
        removed: Vec::new(),
        paths: Vec::new(),
    };
    save_base(core.state.images().root(), &core.uuid, &info).unwrap();
    // the server's folder: the owner updated Fabric API (a new file name), deleted one pack
    // mod and disabled another
    write(&core.root, "mods/pack.jar", b"pack mod");
    write(&core.root, "mods/fabric-api-0.92.2.jar", b"new api");
    write(
        &core.root,
        "mods/disabled.jar.disabled",
        b"disabled pack mod",
    );
    write(&core.root, "config/pack.toml", b"pack config");
    write(&core.root, "config/x.toml", b"server x");
    // the pack's files for clients, as the player reads them from the `.mrpack`
    let share: Vec<(&str, &[u8])> = vec![
        ("mods/pack.jar", b"pack mod"),
        ("mods/fabric-api-0.92.0.jar", b"old api"),
        ("mods/crashes-clients.jar", b"crashes clients"),
        ("mods/disabled.jar", b"disabled pack mod"),
        // a client-only index file: never on the server
        ("mods/sodium.jar", b"sodium"),
        ("config/pack.toml", b"pack config"),
        ("config/x.toml", b"client x"),
        ("options.txt", b"client options"),
    ];

    let player = Player::new(&core, vec![b"pack mod".to_vec()]);
    let Answer::View(view) = player.image(&[]).await else {
        panic!("the whole view");
    };
    assert_eq!(view.base, info.base);
    let removed: Vec<&str> = view.removed.iter().map(|p| p.as_str()).collect();
    assert_eq!(
        removed,
        [
            "mods/crashes-clients.jar",
            "mods/disabled.jar",
            "mods/fabric-api-0.92.0.jar"
        ]
    );
    assert!(player.fetch(&view).await);
    let installed = player.install(&view, &share);
    let hash = |bytes: &[u8]| sha256_hex(bytes);
    let expected = BTreeMap::from([
        (
            "minecraft/config/pack.toml".to_string(),
            hash(b"pack config"),
        ),
        ("minecraft/config/x.toml".to_string(), hash(b"client x")),
        ("minecraft/mods/base.jar".to_string(), hash(b"base mod")),
        (
            "minecraft/mods/fabric-api-0.92.2.jar".to_string(),
            hash(b"new api"),
        ),
        ("minecraft/mods/pack.jar".to_string(), hash(b"pack mod")),
        ("minecraft/mods/sodium.jar".to_string(), hash(b"sodium")),
        ("minecraft/options.txt".to_string(), hash(b"client options")),
    ]);
    assert_eq!(installed, expected);

    // the owner removes a pack mod: the delta takes it out of the player's install too
    std::fs::remove_file(core.root.join("mods/pack.jar")).unwrap();
    let ctx = crate::images::context_of(&core.state, &core.uuid)
        .await
        .unwrap();
    core.state
        .images
        .record_now(ctx, "Removed Pack Mod".into())
        .await
        .unwrap();
    let next = match player.image(&[&view.id]).await {
        Answer::Delta(delta, current) => apply_received(&view, &delta, &current),
        // the core sends the view when the delta would be no smaller
        Answer::View(next) => *next,
        Answer::Current(_) => panic!("the view changed"),
    };
    assert!(next.removed.iter().any(|p| p.as_str() == "mods/pack.jar"));
    assert!(player.fetch(&next).await);
    let mut after_removal = expected.clone();
    after_removal.remove("minecraft/mods/pack.jar");
    assert_eq!(player.install(&next, &share), after_removal);

    // The pack is updated to another version, then the first image is restored (a backup,
    // or the modpack history when the old version's `.mrpack` can't be read): `pack.json`
    // names the old base without its file list, and keeps what that image took out, so
    // players still don't get those pack files back.
    let first = core
        .state
        .images()
        .store()
        .load_manifest(core.uuid.as_ref(), view.projected_from.as_deref().unwrap())
        .unwrap();
    let updated = BaseInfo {
        base: Some(lodestone_image::Base::ModrinthPack {
            project: "pack".into(),
            version: "2.0.0".into(),
            sha512: "d".repeat(128),
        }),
        files: BTreeMap::from([("mods/pack.jar".into(), pack_file(b"pack mod", both, true))]),
        removed: Vec::new(),
        paths: Vec::new(),
    };
    save_base(core.state.images().root(), &core.uuid, &updated).unwrap();
    write(&core.root, "mods/pack.jar", b"pack mod");
    crate::modpack::pack_info::follow_base(core.state.images(), &core.uuid, &first).await;
    let followed = crate::images::base::load_base(core.state.images().root(), &core.uuid).unwrap();
    assert_eq!(followed.base, info.base);
    assert!(followed.files.is_empty());
    assert_eq!(followed.removed, first.removed);
    assert!(followed
        .paths
        .iter()
        .any(|path| path.as_str() == "mods/pack.jar"));
    let ctx = crate::images::context_of(&core.state, &core.uuid)
        .await
        .unwrap();
    core.state
        .images
        .record_now(ctx, "Restored".into())
        .await
        .unwrap();
    let Answer::View(restored) = player.image(&[]).await else {
        panic!("the whole view");
    };
    assert_eq!(restored.base, info.base);
    assert_eq!(restored.removed, view.removed);
    assert!(player.fetch(&restored).await);
    assert_eq!(player.install(&restored, &share), expected);

    // After the restore, `pack.json` still has no file list, yet a pack mod the owner now
    // deletes is taken out of players' installs too: the restored image's paths stand in
    // for the pack's files.
    std::fs::remove_file(core.root.join("mods/pack.jar")).unwrap();
    let ctx = crate::images::context_of(&core.state, &core.uuid)
        .await
        .unwrap();
    core.state
        .images
        .record_now(ctx, "Removed Pack Mod Again".into())
        .await
        .unwrap();
    let Answer::View(after) = player.image(&[]).await else {
        panic!("the whole view");
    };
    assert!(after.removed.iter().any(|p| p.as_str() == "mods/pack.jar"));
    assert!(player.fetch(&after).await);
    assert_eq!(
        player.install(&after, &share),
        after_removal,
        "the pack mod the owner deleted after the restore came back"
    );
}

/// A pack's configs come from the pack, not the server: the owner deleting one the pack
/// put on both sides (`overrides/config/xaerominimap.txt`, a client-only mod's settings)
/// is in the server's image (`removed`), so a restore keeps it deleted, but not in the
/// client view, whose id doesn't change: players keep the pack's copy, as they do when
/// the owner edits it. A pack mod the owner deletes is still taken out.
#[tokio::test]
async fn a_pack_config_the_server_deleted_stays_with_players() {
    use crate::images::base::{save_base, BaseFile, BaseInfo};
    let core = content::core(Kind::Fabric).await;
    let sha1 = |bytes: &[u8]| {
        use sha1::Digest;
        sha1::Sha1::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let both = lodestone_image::Env::new(EnvValue::Required, EnvValue::Required);
    let pack_file = |bytes: &[u8], download: bool| BaseFile {
        sha1: sha1(bytes),
        env: Some(both),
        modrinth_url: download.then(|| {
            format!(
                "https://cdn.modrinth.com/data/AAAAAAAA/versions/BBBBBBBB/{}.jar",
                sha1(bytes)
            )
        }),
        sha512: download.then(|| "e".repeat(128)),
        ..Default::default()
    };
    let info = BaseInfo {
        base: Some(lodestone_image::Base::ModrinthPack {
            project: "pack".into(),
            version: "1.0.0".into(),
            sha512: "c".repeat(128),
        }),
        files: BTreeMap::from([
            ("mods/pack.jar".into(), pack_file(b"pack mod", true)),
            (
                "config/xaerominimap.txt".into(),
                pack_file(b"minimap settings", false),
            ),
            (
                "defaultconfigs/keys.toml".into(),
                pack_file(b"pack keys", false),
            ),
        ]),
        removed: Vec::new(),
        paths: Vec::new(),
    };
    save_base(core.state.images().root(), &core.uuid, &info).unwrap();
    write(&core.root, "mods/pack.jar", b"pack mod");
    write(&core.root, "config/xaerominimap.txt", b"minimap settings");
    write(&core.root, "defaultconfigs/keys.toml", b"pack keys");
    let share: Vec<(&str, &[u8])> = vec![
        ("mods/pack.jar", b"pack mod"),
        ("config/xaerominimap.txt", b"minimap settings"),
        ("defaultconfigs/keys.toml", b"pack keys"),
    ];
    let hash = |bytes: &[u8]| sha256_hex(bytes);
    let everything = BTreeMap::from([
        (
            "minecraft/config/xaerominimap.txt".to_string(),
            hash(b"minimap settings"),
        ),
        (
            "minecraft/defaultconfigs/keys.toml".to_string(),
            hash(b"pack keys"),
        ),
        // the test core's own mod
        ("minecraft/mods/base.jar".to_string(), hash(b"base mod")),
        ("minecraft/mods/pack.jar".to_string(), hash(b"pack mod")),
    ]);

    let player = Player::new(&core, vec![b"pack mod".to_vec()]);
    let Answer::View(view) = player.image(&[]).await else {
        panic!("the whole view");
    };
    assert!(view.removed.is_empty());
    assert!(player.fetch(&view).await);
    assert_eq!(player.install(&view, &share), everything);

    // The owner deletes the pack's configs from the server.
    std::fs::remove_file(core.root.join("config/xaerominimap.txt")).unwrap();
    std::fs::remove_file(core.root.join("defaultconfigs/keys.toml")).unwrap();
    let ctx = crate::images::context_of(&core.state, &core.uuid)
        .await
        .unwrap();
    let recorded = core
        .state
        .images
        .record_now(ctx, "Deleted Configs".into())
        .await
        .unwrap()
        .unwrap();
    assert!(recorded.changed);
    let image = core
        .state
        .images()
        .store()
        .load_manifest(core.uuid.as_ref(), &recorded.id)
        .unwrap();
    let removed: Vec<&str> = image.removed.iter().map(|p| p.as_str()).collect();
    assert_eq!(
        removed,
        ["config/xaerominimap.txt", "defaultconfigs/keys.toml"],
        "the server's image keeps the deletion"
    );
    let Answer::Current(current) = player.image(&[&view.id]).await else {
        panic!("the client view didn't change");
    };
    assert_eq!(current, view.id);
    let Answer::View(again) = player.image(&[]).await else {
        panic!("the whole view");
    };
    assert!(again.removed.is_empty());
    assert_eq!(
        player.install(&again, &share),
        everything,
        "players keep the pack's configs"
    );

    // A pack mod the owner deletes is still taken out of players' installs.
    std::fs::remove_file(core.root.join("mods/pack.jar")).unwrap();
    let ctx = crate::images::context_of(&core.state, &core.uuid)
        .await
        .unwrap();
    core.state
        .images
        .record_now(ctx, "Deleted Pack Mod".into())
        .await
        .unwrap();
    let next = match player.image(&[&view.id]).await {
        Answer::Delta(delta, current) => apply_received(&view, &delta, &current),
        Answer::View(next) => *next,
        Answer::Current(_) => panic!("the view changed"),
    };
    let removed: Vec<&str> = next.removed.iter().map(|p| p.as_str()).collect();
    assert_eq!(removed, ["mods/pack.jar"]);
    assert!(player.fetch(&next).await);
    let mut without_mod = everything.clone();
    without_mod.remove("minecraft/mods/pack.jar");
    assert_eq!(player.install(&next, &share), without_mod);
}

/// An object the current view names that the store lost (deleted when found damaged): the
/// server says so with a 503 and stores it again from its disk in the background, and the
/// player gets it on a later try.
#[tokio::test]
async fn a_lost_object_is_stored_again_for_the_player() {
    let core = content::core(Kind::Fabric).await;
    write(&core.root, "mods/own.jar", b"the server's own mod");
    let recorded = core.record_initial().await;
    let player = Player::new(&core, Vec::new());
    let Answer::View(view) = player.image(&[]).await else {
        panic!("the whole view");
    };
    let own = view
        .entries
        .iter()
        .find(|e| e.path.as_str() == "mods/own.jar")
        .unwrap()
        .clone();
    let store = core.state.images().store();
    let path = store.object_path(&own.sha256).unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    std::fs::set_permissions(&path, permissions).unwrap();
    std::fs::remove_file(&path).unwrap();

    let response = player
        .http
        .get(player.url(&format!("objects/{}", own.sha256)))
        .bearer_auth(&core.owner)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()["retry-after"], "5");
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["kind"], "Conflict");
    // a HEAD looks too (the file may be back already)
    let head = player
        .http
        .head(player.url(&format!("objects/{}", own.sha256)))
        .bearer_auth(&core.owner)
        .send()
        .await
        .unwrap();
    assert!(matches!(
        head.status(),
        StatusCode::SERVICE_UNAVAILABLE | StatusCode::OK
    ));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let (status, bytes) = player.object(&own.sha256).await;
        if status == StatusCode::OK {
            assert_eq!(bytes, b"the server's own mod");
            break;
        }
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(std::time::Instant::now() < deadline);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    // the same image: nothing about the content changed
    assert_eq!(core.current().await.id, recorded.id);
}

#[tokio::test]
async fn players_keep_downloading_while_the_server_records_and_collects() {
    let core = Arc::new(content::core(Kind::Fabric).await);
    for n in 0..5 {
        write(
            &core.root,
            &format!("mods/own-{n}.jar"),
            format!("own mod {n}").as_bytes(),
        );
    }
    core.record_initial().await;
    age_store(core.state.images().root());

    let done = Arc::new(AtomicBool::new(false));
    // the owner keeps changing mods, each change recorded at once
    let writer = {
        let core = core.clone();
        let done = done.clone();
        tokio::spawn(async move {
            for n in 0..8 {
                let status = core
                    .write_file(
                        &format!("mods/gen-{n}.jar"),
                        &core.owner,
                        &format!("gen {n}"),
                    )
                    .await;
                assert_eq!(status, StatusCode::OK);
                core.flush().await.unwrap();
            }
            done.store(true, Ordering::SeqCst);
        })
    };
    // and collections run all the while
    let collector = {
        let images = core.state.images().clone();
        let done = done.clone();
        tokio::spawn(async move {
            let mut runs = 0;
            while !done.load(Ordering::SeqCst) || runs == 0 {
                let images = images.clone();
                tokio::task::spawn_blocking(move || images.collect_blocking().unwrap())
                    .await
                    .unwrap();
                runs += 1;
            }
            runs
        })
    };
    // a player downloads whatever is current, again and again
    let player = Player::new(&core, Vec::new());
    let mut rounds = 0;
    let mut superseded = 0;
    while !done.load(Ordering::SeqCst) || rounds == 0 {
        let Answer::View(view) = player.image(&[]).await else {
            panic!("the whole view");
        };
        if player.fetch(&view).await {
            assert!(player.place(&view).contains("minecraft/mods/own-0.jar"));
        } else {
            // a newer image took its place meanwhile: the player asks again
            superseded += 1;
        }
        rounds += 1;
    }
    writer.await.unwrap();
    assert!(collector.await.unwrap() > 0);
    // once it settles, every file of the current view downloads
    let Answer::View(last) = player.image(&[]).await else {
        panic!("the whole view");
    };
    assert!(player.fetch(&last).await);
    let placed = player.place(&last);
    for n in 0..8 {
        assert!(
            placed.contains(&format!("minecraft/mods/gen-{n}.jar")),
            "gen {n}"
        );
    }
    println!("{rounds} downloads, {superseded} superseded");
}

/// Forge and NeoForge jars say which sides they are for in `mods.toml`: a server-side mod
/// stays off players, a client-only one goes to them.
#[tokio::test]
async fn forge_jars_give_their_sides() {
    let core = content::core(Kind::Forge).await;
    let mods = core.root.join("mods");
    make_zip(
        &mods.join("serverside.jar"),
        &[(
            "META-INF/mods.toml",
            "modLoader=\"javafml\"\n[[mods]]\nmodId=\"serverside\"\ndisplayTest=\"IGNORE_SERVER_VERSION\"\n",
        )],
    );
    make_zip(
        &mods.join("clientonly.jar"),
        &[(
            "META-INF/mods.toml",
            "modLoader=\"javafml\"\nclientSideOnly=true\n[[mods]]\nmodId=\"clientonly\"\n",
        )],
    );
    make_zip(
        &mods.join("both.jar"),
        &[(
            "META-INF/mods.toml",
            "modLoader=\"javafml\"\n[[mods]]\nmodId=\"both\"\n",
        )],
    );
    core.record_initial().await;
    let image = core.current().await;
    let env_of = |path: &str| {
        let entry = image
            .entries
            .iter()
            .find(|e| e.path.as_str() == path)
            .unwrap();
        (entry.env, entry.env_from.clone())
    };
    let (env, from) = env_of("mods/serverside.jar");
    assert_eq!(
        (env.client, env.server),
        (EnvValue::Unsupported, EnvValue::Required)
    );
    assert_eq!(from, vec![EnvFrom::Jar]);
    let (env, _) = env_of("mods/clientonly.jar");
    assert_eq!(
        (env.client, env.server),
        (EnvValue::Required, EnvValue::Unsupported)
    );
    let (env, from) = env_of("mods/both.jar");
    assert_eq!(
        (env.client, env.server),
        (EnvValue::Unknown, EnvValue::Unknown)
    );
    assert!(from.is_empty());

    let player = Player::new(&core, Vec::new());
    let Answer::View(view) = player.image(&[]).await else {
        panic!("the whole view");
    };
    assert!(player.fetch(&view).await);
    assert_eq!(
        player.place(&view),
        set(&[
            "minecraft/mods/base.jar",
            "minecraft/mods/both.jar",
            "minecraft/mods/clientonly.jar",
        ])
    );
}

/// Bytes that don't repeat, so a file of them is cut into many chunks.
fn noise(len: usize) -> Vec<u8> {
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u8
        })
        .collect()
}

/// A large file (a chunk list in the store) the current view names loses a chunk, found
/// damaged as it is sent or gone some other way. The download that finds it ends short;
/// the next request gets the 503 that stores the file again from the server's disk, and
/// the player then gets the whole file.
async fn a_lost_chunk_comes_back(damaged: bool) {
    let core = content::core(Kind::Fabric).await;
    let bytes = noise(3 << 20);
    write(&core.root, "mods/big.jar", &bytes);
    let recorded = core.record_initial().await;
    let player = Player::new(&core, Vec::new());
    let Answer::View(view) = player.image(&[]).await else {
        panic!("the whole view");
    };
    let big = view
        .entries
        .iter()
        .find(|e| e.path.as_str() == "mods/big.jar")
        .unwrap()
        .clone();
    let store = core.state.images().store();
    let lodestone_image::store::ObjectForm::Chunked(list) =
        store.object_form(&big.sha256, big.size).unwrap()
    else {
        panic!("a large file is a chunk list");
    };
    let chunk = list.chunks[list.chunks.len() / 2].clone();
    let path = store.chunk_path(&chunk.sha256).unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    std::fs::set_permissions(&path, permissions).unwrap();
    if damaged {
        std::fs::write(&path, b"not this chunk").unwrap();
    } else {
        std::fs::remove_file(&path).unwrap();
    }
    let get = || {
        player
            .http
            .get(player.url(&format!("objects/{}", big.sha256)))
            .bearer_auth(&core.owner)
            .send()
    };

    // the chunk list still looks sound: the answer promises the file and ends short
    let response = get().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.bytes().await.map(|b| b.len()).unwrap_or(0);
    assert!((body as u64) < big.size, "a body cut short");
    // the download took the incomplete list away: now the server says it lacks the file,
    // and stores it again
    let response = get().await.unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["kind"], "Conflict");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let (status, got) = player.object(&big.sha256).await;
        if status == StatusCode::OK {
            assert!(got == bytes);
            break;
        }
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(std::time::Instant::now() < deadline);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(store.object_complete(&big.sha256, big.size));
    assert_eq!(core.current().await.id, recorded.id);
}

#[tokio::test]
async fn a_large_file_with_a_damaged_chunk_is_stored_again_for_the_player() {
    a_lost_chunk_comes_back(true).await;
}

#[tokio::test]
async fn a_large_file_that_lost_a_chunk_is_stored_again_for_the_player() {
    a_lost_chunk_comes_back(false).await;
}

/// Backup excludes keep backups small; they never take the server's mods from its players.
/// A pattern that could reach `mods/` (`*.jar`, `mods/`) is left out of content images, so
/// the client view still has every mod; one that can't (a config folder) still applies.
#[tokio::test]
async fn backup_excludes_never_take_mods_from_players() {
    let core = content::core(Kind::Fabric).await;
    write(&core.root, "mods/own.jar", b"the server's own mod");
    write(&core.root, "config/secret/a.toml", b"token = 1");
    write(&core.root, "config/b.toml", b"b = 1");
    let path = format!("/instance/{}/backups/policy", core.uuid);
    let (status, mut policy) = core.call(Method::GET, &path, &core.owner, None).await;
    assert_eq!(status, StatusCode::OK, "{policy}");
    policy["exclude"] = json!(["*.jar", "mods/", "config/secret/"]);
    let (status, body) = core
        .call(Method::PUT, &path, &core.owner, Some(policy))
        .await;
    assert!(status.is_success(), "{body}");
    let recorded = core.record_initial().await;
    let content = core.current().await;
    assert_eq!(content.id, recorded.id);
    assert_eq!(content.excludes, ["config/secret/"]);
    let paths: BTreeSet<&str> = content.entries.iter().map(|e| e.path.as_str()).collect();
    assert!(paths.contains("mods/own.jar"));
    assert!(paths.contains("config/b.toml"));
    assert!(!paths.contains("config/secret/a.toml"));

    let player = Player::new(&core, Vec::new());
    let Answer::View(view) = player.image(&[]).await else {
        panic!("the whole view");
    };
    assert_eq!(view.excludes, ["config/secret/"]);
    assert!(player.fetch(&view).await);
    assert!(player.place(&view).contains("minecraft/mods/own.jar"));
}

/// "Not for players" decides only a mod whose sides leave players free: one Modrinth says
/// players need, marked for players and then unmarked (which leaves it `excluded` in the
/// marks file), stays in the client view, as the Mods tab shows it going to players.
#[tokio::test]
async fn unmarking_a_mod_players_need_keeps_it_in_the_view() {
    let core = content::core(Kind::Fabric).await;
    let mods = core.root.join("mods");
    fabric_jar(&mods.join("needed.jar"), "needed", "*");
    fabric_jar(&mods.join("free.jar"), "free", "*");
    let hint = |path: &Path, client: &str| {
        let bytes = std::fs::read(path).unwrap();
        core.state.images().note_modrinth(
            &format!("{:x}", sha2::Sha512::digest(&bytes)),
            ModrinthHint {
                client_side: Some(client.into()),
                server_side: Some("required".into()),
                ..ModrinthHint::default()
            },
        );
    };
    hint(&mods.join("needed.jar"), "required");
    hint(&mods.join("free.jar"), "optional");
    for name in ["needed.jar", "free.jar"] {
        for required in [true, false] {
            let (status, body) = core
                .call(
                    Method::PUT,
                    &format!("/instance/{}/mods/{name}/client", core.uuid),
                    &core.owner,
                    Some(json!({ "required": required })),
                )
                .await;
            assert!(status.is_success(), "{body}");
        }
    }
    let marks = std::fs::read_to_string(mods.join(".lodestone_client_mods.json")).unwrap();
    assert!(marks.contains("needed.jar") && marks.contains("free.jar"));

    let player = Player::new(&core, Vec::new());
    let Answer::View(view) = player.image(&[]).await else {
        panic!("the whole view");
    };
    let paths: BTreeSet<&str> = view.entries.iter().map(|e| e.path.as_str()).collect();
    assert!(paths.contains("mods/needed.jar"), "{paths:?}");
    assert!(!paths.contains("mods/free.jar"), "{paths:?}");
}

/// Before the server has a content image, an object request gets the image route's 503 and
/// starts the first record, rather than a 404 that would tell the player the file isn't
/// served; once the image is there, the object is.
#[tokio::test]
async fn objects_before_the_first_image_wait_for_it() {
    let core = content::core(Kind::Fabric).await;
    write(&core.root, "mods/own.jar", b"the server's own mod");
    let player = Player::new(&core, Vec::new());
    let own = sha256_hex(b"the server's own mod");
    let response = player
        .http
        .get(player.url(&format!("objects/{own}")))
        .bearer_auth(&core.owner)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()["retry-after"], "5");
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["kind"], "Conflict");
    let Answer::View(view) = player.image(&[]).await else {
        panic!("the whole view");
    };
    assert!(view.entries.iter().any(|e| e.sha256 == own));
    let (status, bytes) = player.object(&own).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, b"the server's own mod");
}
