//! Milestone 3 end to end: a test client does what a player's Launcher will (milestone 4):
//! it downloads a server's client view, fetches the files only the server has into a store
//! of its own, places them into an empty folder with `lodestone_image`, and keeps up with
//! the server's changes through deltas, all over the HTTP API. Every core lives in a
//! temporary folder; no server is started, nothing is downloaded and no EULA is accepted.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use lodestone_image::hex::sha256_hex;
use lodestone_image::store::materialize::{materialize, missing_objects, MaterializeOptions};
use lodestone_image::store::{Encoding, Store};
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
        let mut url = self.url("image?side=client");
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
                view.verify().unwrap();
                assert_eq!(view.side, Some(Side::Client));
                assert_eq!(
                    lodestone_image::view(&view, Side::Client, view.rules)
                        .unwrap()
                        .id,
                    view.id
                );
                assert_eq!(view.id, current);
                Answer::View(Box::new(view))
            }
            "delta" => Answer::Delta(Delta::from_json(&body).unwrap(), current),
            other => panic!("unknown answer {other}"),
        }
    }

    /// `GET …/objects/{sha256}`: the status, and the bytes when they check out.
    async fn object(&self, sha256: &str) -> (StatusCode, Vec<u8>) {
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
        if status == StatusCode::OK {
            assert_eq!(sha256_hex(&bytes), sha256, "the bytes the server sent");
        }
        (status, bytes)
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
    let second = apply(&first, &delta).unwrap();
    assert_eq!(second.id, current);
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
    assert_eq!(apply(&first, &changes).unwrap().id, second.id);
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
