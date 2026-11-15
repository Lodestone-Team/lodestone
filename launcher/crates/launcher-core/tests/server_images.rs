//! Milestone 4's second half end to end: an instance built from a server's image and kept
//! in step with it through `cores::image_sync`, against a fake core that serves images as
//! milestone 3's does (`image_core`) and a Modrinth mock for the base pack and the CDN,
//! (and a CurseForge mock for CurseForge entries), with the Launcher's data folder
//! (instances and image store) in a temporary folder. No game runs and no server starts.

mod curseforge;
mod image_core;
mod modrinth;

use std::{
    io::{Cursor, Write as _},
    path::Path,
    sync::Arc,
    time::Duration,
};

use futures::executor::block_on;
use image_core::{ImageCore, KEY, SERVER, content_image, object_entry};
use launcher_core::{
    Secret,
    content::{InstanceDirs, Platforms, install::InstallProgress},
    cores::{
        CoreClient,
        image_sync::{
            SyncError,
            apply::{ServerUpdate, build_instance, update_instance},
            prepare::{Lookup, Prepared, Ready, Target, prepare, prepare as prepare_fn},
            views::KeptViews,
        },
        link::{self, ServerLink},
        player_image::ServerImage,
    },
    game::download::{CancelToken, Downloader, recording_sleep},
    http,
    images::{Images, Timing, labels},
    instances::{Instance, InstanceStore},
    modpack::{ModpackError, ModpackRecord, fetch::Sources},
    paths::DataDir,
};
use lodestone_image::{Base, CanonicalPath, Entry, Image, Loader, Source, hex::sha256_hex};
use modrinth::{FakeVersion, ModrinthMock};
use serde_json::{Value, json};

const PACK: &str = "PACKPROJ";

fn sha1(bytes: &[u8]) -> String {
    launcher_core::game::download::sha1_hex(bytes)
}

fn mod_version(project: &str, id: &str, file: &str, bytes: &[u8]) -> FakeVersion {
    FakeVersion::new(
        id,
        project,
        id,
        &["fabric"],
        &["1.21.1"],
        "2026-01-01T00:00:00Z",
    )
    .file(file, bytes)
}

/// The index entry of a Modrinth file in a pack.
fn index_file(version: &FakeVersion, path: &str, client: &str) -> Value {
    json!({
        "path": path,
        "hashes": { "sha1": version.sha1(), "sha512": version.sha512() },
        "env": { "client": client, "server": "required" },
        "downloads": [version.cdn_url()],
        "fileSize": version.bytes.len(),
    })
}

fn make_pack(version: &str, files: Value, overrides: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut out);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("modrinth.index.json", options).unwrap();
        let index = json!({
            "formatVersion": 1, "game": "minecraft", "name": "Cozy Pack",
            "versionId": version,
            "dependencies": { "minecraft": "1.21.1", "fabric-loader": "0.16.10" },
            "files": files,
        });
        zip.write_all(index.to_string().as_bytes()).unwrap();
        for (path, data) in overrides {
            zip.start_file(*path, options).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }
    out.into_inner()
}

/// A Modrinth entry of the server's content image.
fn modrinth_entry(path: &str, version: &FakeVersion) -> Entry {
    let mut entry = Entry::new(
        CanonicalPath::new(path).unwrap(),
        version.bytes.len() as u64,
        sha256_hex(&version.bytes),
        Source::modrinth_from_url(&version.cdn_url()).unwrap(),
    );
    entry.sha1 = Some(version.sha1());
    entry.sha512 = Some(version.sha512());
    entry
}

/// A `base` entry: the pack's own copy.
fn base_entry(path: &str, bytes: &[u8]) -> Entry {
    let mut entry = Entry::new(
        CanonicalPath::new(path).unwrap(),
        bytes.len() as u64,
        sha256_hex(bytes),
        Source::Base {},
    );
    entry.sha1 = Some(sha1(bytes));
    entry
}

struct World {
    _temp: tempfile::TempDir,
    data: DataDir,
    store: InstanceStore,
    images: Arc<Images>,
    core: ImageCore,
    modrinth: ModrinthMock,
    api: launcher_core::content::Modrinth,
    downloader: Downloader,
    server: ServerImage,
    cache: std::path::PathBuf,
    sodium1: FakeVersion,
    sodium2: FakeVersion,
    jei: FakeVersion,
    zoomify: FakeVersion,
    pack1: FakeVersion,
    pack2: FakeVersion,
}

impl World {
    fn new() -> World {
        let temp = tempfile::tempdir().unwrap();
        let data = DataDir::at(temp.path().join("data"));
        let store = InstanceStore::new(data.instances_dir());
        std::fs::create_dir_all(store.root()).unwrap();
        let images = Images::with_timing(
            &data,
            Timing {
                merge: Duration::from_secs(600),
                max_defer: Duration::from_secs(600),
                ..Timing::default()
            },
        );
        let modrinth = ModrinthMock::start();
        let sodium1 = mod_version("AANobbMI", "sodium-1", "sodium.jar", b"sodium 1");
        let sodium2 = mod_version("AANobbMI", "sodium-2", "sodium.jar", b"sodium 2");
        let jei = mod_version("u6dRKJwZ", "jei-1", "jei.jar", b"jei 1");
        let zoomify = mod_version("w7ThoJFB", "zoomify-1", "zoomify.jar", b"zoomify 1");
        // the pack: sodium for both sides, zoomify for clients, a bundled mod, a config
        // both sides get and one only clients get
        let pack1_bytes = make_pack(
            "1.0",
            json!([
                index_file(&sodium1, "mods/sodium.jar", "required"),
                index_file(&zoomify, "mods/zoomify.jar", "required"),
            ]),
            &[
                ("overrides/mods/bundled.jar", b"bundled 1"),
                ("overrides/config/pack.toml", b"pack config 1"),
                ("client-overrides/config/client.toml", b"client config 1"),
            ],
        );
        let pack2_bytes = make_pack(
            "2.0",
            json!([
                index_file(&sodium2, "mods/sodium.jar", "required"),
                index_file(&zoomify, "mods/zoomify.jar", "required"),
            ]),
            &[
                ("overrides/mods/bundled.jar", b"bundled 1"),
                ("overrides/config/pack.toml", b"pack config 1"),
                ("client-overrides/config/client.toml", b"client config 2"),
            ],
        );
        let pack1 = FakeVersion::new("pv1", PACK, "1.0", &["fabric"], &["1.21.1"], "2026-01-01")
            .file("cozy-1.mrpack", &pack1_bytes);
        let pack2 = FakeVersion::new("pv2", PACK, "2.0", &["fabric"], &["1.21.1"], "2026-02-01")
            .file("cozy-2.mrpack", &pack2_bytes);
        modrinth.project(PACK, "cozy", "modpack");
        for version in [&sodium1, &sodium2, &jei, &zoomify, &pack1, &pack2] {
            modrinth.version(version.clone());
        }
        let api = modrinth.client();
        let core = ImageCore::start();
        let client = CoreClient::new(http::default_client().unwrap(), core.base.clone());
        let server = ServerImage::new(client, Some(Secret::new(KEY)), SERVER)
            .unwrap()
            .with_sleep(recording_sleep().0);
        let downloader =
            Downloader::new(http::download_client().unwrap(), recording_sleep().0).with_attempts(1);
        World {
            _temp: temp,
            cache: data.server_pack_cache_dir(),
            data,
            store,
            images,
            core,
            modrinth,
            api,
            downloader,
            server,
            sodium1,
            sodium2,
            jei,
            zoomify,
            pack1,
            pack2,
        }
    }

    /// The server's content image on `pack`: every content file it has, with sources, as
    /// milestone 2's core records them (configs and server files that never reach players
    /// included).
    fn content(&self, pack: &FakeVersion, sodium: &FakeVersion) -> Image {
        let mut image = content_image("1.21.1", Loader::Fabric, Some("0.16.10"));
        image.base = Some(Base::ModrinthPack {
            project: PACK.into(),
            version: pack.id.clone(),
            sha512: pack.sha512(),
        });
        image
            .entries
            .push(modrinth_entry("mods/sodium.jar", sodium));
        image
            .entries
            .push(base_entry("mods/bundled.jar", b"bundled 1"));
        image
            .entries
            .push(object_entry("mods/own.jar", b"the server's own mod"));
        image
            .entries
            .push(modrinth_entry("mods/jei.jar", &self.jei));
        image.entries.push(object_entry(
            "config/pack.toml",
            b"the server's pack config",
        ));
        image
            .entries
            .push(object_entry("server.properties", b"rcon.password=secret"));
        image
    }

    fn lookup(&self) -> Lookup<'_> {
        Lookup {
            modrinth: Some(&self.api),
            curseforge: None,
            downloader: &self.downloader,
            cache: &self.cache,
        }
    }

    fn prepare(
        &self,
        existing: Option<(&InstanceDirs, &Instance)>,
        applied: Option<&str>,
    ) -> Result<Prepared, SyncError> {
        let lookup = self.lookup();
        block_on(http::in_http_runtime(prepare(
            &self.server,
            Target {
                existing,
                applied,
                name: "SMP",
            },
            lookup,
            &|| {},
            &InstallProgress::default(),
            &CancelToken::new(),
        )))
    }

    fn sources<'a>(&'a self, ready: &'a Ready) -> Sources<'a> {
        Sources {
            platforms: Platforms::from(&self.api),
            downloader: &self.downloader,
            pack: ready.base.as_ref().map(|b| b.path.as_path()),
            objects: Some(&self.server),
        }
    }

    fn link(&self) -> ServerLink {
        ServerLink {
            core_uuid: "core-1".into(),
            core_url: self.core.base.to_string(),
            server_uuid: SERVER.into(),
            server_name: "SMP".into(),
            pack_etag: None,
            view: None,
        }
    }

    fn create(&self) -> (Instance, Box<Ready>) {
        let Prepared::Ready(ready) = self.prepare(None, None).unwrap() else {
            panic!("a new instance always has something to place");
        };
        let instance = block_on(http::in_http_runtime(build_instance(
            &self.store,
            &self.sources(&ready),
            &ready.plan,
            "SMP",
            self.link(),
            &InstallProgress::default(),
            &CancelToken::new(),
        )))
        .unwrap();
        self.images
            .record_now(&instance.id, &labels::created_from_server("SMP"))
            .unwrap();
        (instance, ready)
    }

    /// Play on the linked instance: ask, and when the view differs, update through
    /// [`ServerUpdate`] as the app does, saving the loader version as the app's window
    /// does (on the instance in the store).
    fn play(&self, id: &str) -> Result<Option<Arc<Ready>>, SyncError> {
        let dirs = InstanceDirs::new(self.store.dir(id));
        let instance = self.store.get(id).unwrap();
        let applied = link::load_link(&dirs.instance).unwrap().view;
        let ready: Arc<Ready> = match self.prepare(Some((&dirs, &instance)), applied.as_deref())? {
            Prepared::Current => return Ok(None),
            Prepared::Ready(ready) => Arc::from(ready),
        };
        self.update(id, &ready, None::<&str>).unwrap()?;
        Ok(Some(ready))
    }

    /// Updates the instance to `ready`'s view, unless `held`; the inner result is the
    /// update's.
    fn update(
        &self,
        id: &str,
        ready: &Arc<Ready>,
        held: Option<&'static str>,
    ) -> Result<Result<(), SyncError>, &'static str> {
        let dirs = InstanceDirs::new(self.store.dir(id));
        let instance = self.store.get(id).unwrap();
        let update = ServerUpdate::start(
            self.images.clone(),
            instance,
            "SMP",
            dirs,
            ready.clone(),
            held,
        )?;
        let placed = match block_on(http::in_http_runtime(update.place(
            &self.sources(ready),
            &InstallProgress::default(),
            &CancelToken::new(),
        ))) {
            Ok(placed) => placed,
            Err(err) => return Ok(Err(err)),
        };
        let finished = placed
            .finish(|version| {
                let mut instance = self.store.get(id).unwrap();
                instance.loader_version = version;
                self.store.save(&instance).map_err(|e| e.to_string())
            })
            .and_then(|applied| applied.record());
        self.images.flush(id).unwrap();
        Ok(finished)
    }

    fn game(&self, id: &str) -> std::path::PathBuf {
        InstanceDirs::new(self.store.dir(id)).game
    }

    fn labels(&self, id: &str) -> Vec<String> {
        self.images
            .history(id)
            .unwrap()
            .images
            .into_iter()
            .filter_map(|i| i.label)
            .collect()
    }
}

fn read(path: &Path) -> String {
    String::from_utf8(std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn builds_an_instance_from_a_servers_image_and_keeps_it_in_step() {
    let world = World::new();
    let mut content = world.content(&world.pack1, &world.sodium1);
    content.withheld.push(lodestone_image::Withheld {
        path: "mods/linked.jar".into(),
        reason: lodestone_image::WithheldReason::Symlink,
    });
    world.core.publish(
        &content,
        &[
            ("", b"the server's own mod"),
            ("", b"the server's pack config"),
            ("", b"rcon.password=secret"),
        ],
    );

    // 1. What will be installed
    let Prepared::Ready(ready) = world.prepare(None, None).unwrap() else {
        panic!("expected a plan");
    };
    let summary = &ready.plan.summary;
    assert_eq!(
        summary.base,
        Some(("Cozy Pack".to_string(), "1.0".to_string()))
    );
    assert_eq!(summary.loader, "Fabric 0.16.10");
    let mut added = summary.added_mods.clone();
    added.sort();
    // sodium is the pack's own, at the pack's version, though the core names it by its
    // Modrinth URL
    assert_eq!(added, ["jei.jar", "own.jar"]);
    assert_eq!(summary.server_only, ["mods/own.jar"]);
    assert_eq!(summary.missing.len(), 1);
    assert_eq!(summary.missing[0].0, "mods/linked.jar");

    // 2. The instance
    let (instance, ready) = world.create();
    let id = instance.id.clone();
    let game = world.game(&id);
    assert_eq!(read(&game.join("mods/sodium.jar")), "sodium 1");
    assert_eq!(read(&game.join("mods/zoomify.jar")), "zoomify 1");
    assert_eq!(read(&game.join("mods/bundled.jar")), "bundled 1");
    assert_eq!(read(&game.join("mods/own.jar")), "the server's own mod");
    assert_eq!(read(&game.join("mods/jei.jar")), "jei 1");
    // the pack's configs, not the server's; the client-only one too
    assert_eq!(read(&game.join("config/pack.toml")), "pack config 1");
    assert_eq!(read(&game.join("config/client.toml")), "client config 1");
    // never the server's own files
    assert!(!game.join("server.properties").exists());
    assert_eq!(instance.loader_version.as_deref(), Some("0.16.10"));
    assert_eq!(instance.recommended_memory, None);
    let dirs = InstanceDirs::new(world.store.dir(&id));
    let record = ModpackRecord::load(&dirs.instance).unwrap();
    assert!(record.from_server && !record.on_modrinth());
    assert_eq!(record.modrinth_ids(), Some((PACK, "pv1")));
    assert_eq!(
        record.files["mods/own.jar"].sha256.as_deref(),
        Some(sha256_hex(b"the server's own mod").as_str())
    );
    let link = link::load_link(&dirs.instance).unwrap();
    assert_eq!(link.view.as_deref(), Some(ready.plan.view.id.as_str()));
    assert!(
        dirs.instance
            .join("server-views")
            .join(format!("{}.json", ready.plan.view.id))
            .is_file()
    );
    // the base pack is cached by its SHA-512
    assert!(
        world
            .data
            .server_pack_cache_dir()
            .join(format!("{}.mrpack", world.pack1.sha512()))
            .is_file()
    );
    // its content image names the server's base
    let history = world.images.history(&id).unwrap();
    assert_eq!(history.images[0].label.as_deref(), Some("Created from SMP"));
    assert!(
        matches!(&history.images[0].base, Some(Base::ModrinthPack { version, .. }) if version == "pv1")
    );

    // 3. In step: a 204
    assert!(world.play(&id).unwrap().is_none());
    let last = world.core.requests().pop().unwrap();
    assert!(last.path.contains(&format!("have={}", ready.plan.view.id)));

    // 4. The player adds a mod and edits a config; the game writes a config in a session
    std::fs::write(game.join("mods/mine.jar"), b"the player's own mod").unwrap();
    std::fs::write(game.join("config/pack.toml"), b"edited by the player").unwrap();
    world.images.flush(&id).unwrap();

    // 5. The server adds a mod and removes JEI
    let mut second = world.content(&world.pack1, &world.sodium1);
    second.entries.retain(|e| e.path.as_str() != "mods/jei.jar");
    second
        .entries
        .push(object_entry("mods/new.jar", b"a new mod"));
    world.core.publish(&second, &[("", b"a new mod")]);
    let ready = world.play(&id).unwrap().unwrap();
    // a delta from the view it had
    assert!(
        world
            .core
            .requests()
            .iter()
            .any(|r| r.path.contains("have="))
    );
    assert_eq!(ready.plan.summary.changes.added, ["mods/new.jar"]);
    assert_eq!(ready.plan.summary.changes.removed, ["mods/jei.jar"]);
    assert_eq!(read(&game.join("mods/new.jar")), "a new mod");
    assert!(!game.join("mods/jei.jar").exists());
    // the player's own mod and edited config stay
    assert_eq!(read(&game.join("mods/mine.jar")), "the player's own mod");
    assert_eq!(read(&game.join("config/pack.toml")), "edited by the player");
    assert_eq!(world.labels(&id)[0], "Updated from SMP");

    // 6. The server takes the pack's bundled mod out ("removed"), then puts it back
    let mut third = second.clone();
    third
        .entries
        .retain(|e| e.path.as_str() != "mods/bundled.jar");
    third
        .removed
        .push(CanonicalPath::new("mods/bundled.jar").unwrap());
    world.core.publish(&third, &[]);
    world.play(&id).unwrap().unwrap();
    assert!(!game.join("mods/bundled.jar").exists());
    world.core.publish(&second, &[]);
    world.play(&id).unwrap().unwrap();
    assert_eq!(read(&game.join("mods/bundled.jar")), "bundled 1");

    // 7. The server updates its pack: sodium 2, a new client-only config, and the loader
    // version follows; what the game wrote while it ran is recorded first
    world.images.before_launch(&id);
    std::fs::write(game.join("config/game-wrote.json"), b"{}").unwrap();
    world.images.end_session(&id);
    let mut fourth = world.content(&world.pack2, &world.sodium2);
    fourth.entries.retain(|e| e.path.as_str() != "mods/jei.jar");
    fourth
        .entries
        .push(object_entry("mods/new.jar", b"a new mod"));
    fourth.game.loader_version = Some("0.16.11".into());
    world.core.publish(&fourth, &[]);
    let ready = world.play(&id).unwrap().unwrap();
    assert_eq!(ready.plan.loader.as_ref().unwrap().version, "0.16.11");
    assert_eq!(read(&game.join("mods/sodium.jar")), "sodium 2");
    assert_eq!(read(&game.join("config/client.toml")), "client config 2");
    assert_eq!(read(&game.join("config/pack.toml")), "edited by the player");
    assert_eq!(
        world.store.get(&id).unwrap().loader_version.as_deref(),
        Some("0.16.11")
    );
    let labels = world.labels(&id);
    assert_eq!(labels[0], "Updated from SMP");
    assert_eq!(labels[1], "Before a server update");
    assert!(labels.contains(&"Created from SMP".to_string()));
    assert!(world.play(&id).unwrap().is_none());

    // 8. A player whose views the server pruned gets the whole view, and works out the
    // difference itself: nothing to change
    {
        let mut state = world.core.state();
        let older = state.images.len() - 1;
        state.images.drain(..older);
    }
    std::fs::remove_dir_all(dirs.instance.join("server-views")).unwrap();
    let Prepared::Ready(ready) = world
        .prepare(
            Some((&dirs, &world.store.get(&id).unwrap())),
            link::load_link(&dirs.instance).unwrap().view.as_deref(),
        )
        .unwrap()
    else {
        panic!("a view it doesn't keep is worked out again");
    };
    assert!(!ready.changes(Some(&world.store.get(&id).unwrap())));

    // 9. The server moves to Forge: the instance keeps its game
    let mut forge = fourth.clone();
    forge.game.loader = Loader::Forge;
    forge.game.loader_version = Some("47.4.0".into());
    world.core.publish(&forge, &[]);
    assert!(matches!(
        world.play(&id),
        Err(SyncError::GameChanged { .. })
    ));
}

#[test]
fn a_204_naming_another_kept_view_switches_to_it_and_a_crash_converges() {
    let world = World::new();
    let first = world.content(&world.pack1, &world.sodium1);
    world.core.publish(&first, &[("", b"the server's own mod")]);
    let (instance, ready) = world.create();
    let id = instance.id;
    let first_view = ready.plan.view.id.clone();
    let game = world.game(&id);
    let dirs = InstanceDirs::new(world.store.dir(&id));

    let mut second = first.clone();
    second
        .entries
        .push(object_entry("mods/new.jar", b"a new mod"));
    world.core.publish(&second, &[("", b"a new mod")]);

    // a crash after the files changed but before the link names the view: the next Play
    // finds the files in place and only records the view
    let instance = world.store.get(&id).unwrap();
    let Prepared::Ready(ready) = world
        .prepare(Some((&dirs, &instance)), Some(&first_view))
        .unwrap()
    else {
        panic!("expected a plan");
    };
    block_on(http::in_http_runtime(update_instance(
        &world.sources(&ready),
        &dirs,
        &ready.plan,
        &InstallProgress::default(),
        &CancelToken::new(),
    )))
    .unwrap();
    assert_eq!(
        link::load_link(&dirs.instance).unwrap().view.as_deref(),
        Some(first_view.as_str())
    );
    let converged = world.play(&id).unwrap().unwrap();
    assert!(!converged.changes(Some(&world.store.get(&id).unwrap())));
    let second_view = converged.plan.view.id.clone();
    assert_eq!(
        link::load_link(&dirs.instance).unwrap().view.as_deref(),
        Some(second_view.as_str())
    );
    assert_eq!(read(&game.join("mods/new.jar")), "a new mod");

    // the server goes back to the first content and forgets the second: the player names
    // the second first, the core doesn't know it, and says the first (which the player
    // keeps) is current
    {
        let mut state = world.core.state();
        state.images.clear();
    }
    world.core.publish(&first, &[]);
    let ready = world.play(&id).unwrap().unwrap();
    assert_eq!(ready.plan.view.id, first_view);
    assert!(!game.join("mods/new.jar").exists());
    let last = world.core.requests().pop().unwrap();
    assert!(
        last.path
            .contains(&format!("have={second_view},{first_view}")),
        "{}",
        last.path
    );
}

#[test]
fn a_base_pack_gone_from_modrinth_or_not_the_one_it_names_is_refused() {
    let world = World::new();
    let mut content = world.content(&world.pack1, &world.sodium1);
    // a version Modrinth doesn't have
    content.base = Some(Base::ModrinthPack {
        project: PACK.into(),
        version: "gone".into(),
        sha512: world.pack1.sha512(),
    });
    world.core.publish(&content, &[]);
    assert!(matches!(
        world.prepare(None, None),
        Err(SyncError::BasePackGone { version, .. }) if version == "gone"
    ));
    // another project's version
    content.base = Some(Base::ModrinthPack {
        project: "OTHER".into(),
        version: "pv1".into(),
        sha512: world.pack1.sha512(),
    });
    world.core.publish(&content, &[]);
    assert!(matches!(
        world.prepare(None, None),
        Err(SyncError::Refused(_))
    ));
    // a SHA-512 Modrinth doesn't list
    content.base = Some(Base::ModrinthPack {
        project: PACK.into(),
        version: "pv1".into(),
        sha512: "0".repeat(128),
    });
    world.core.publish(&content, &[]);
    assert!(matches!(
        world.prepare(None, None),
        Err(SyncError::Refused(_))
    ));
    // the cached pack is hashed again when used: a damaged one is downloaded again
    content.base = Some(Base::ModrinthPack {
        project: PACK.into(),
        version: "pv1".into(),
        sha512: world.pack1.sha512(),
    });
    world.core.publish(&content, &[]);
    let cache = world.data.server_pack_cache_dir();
    std::fs::create_dir_all(&cache).unwrap();
    std::fs::write(
        cache.join(format!("{}.mrpack", world.pack1.sha512())),
        b"damaged",
    )
    .unwrap();
    assert!(matches!(
        world.prepare(None, None).unwrap(),
        Prepared::Ready(_)
    ));
    assert_eq!(
        std::fs::read(cache.join(format!("{}.mrpack", world.pack1.sha512()))).unwrap(),
        world.pack1.bytes
    );
    let downloads = world
        .modrinth
        .requests()
        .into_iter()
        .filter(|r| r.path.ends_with("cozy-1.mrpack"))
        .count();
    // used again from the cache, no download
    assert!(matches!(
        world.prepare(None, None).unwrap(),
        Prepared::Ready(_)
    ));
    assert_eq!(
        world
            .modrinth
            .requests()
            .into_iter()
            .filter(|r| r.path.ends_with("cozy-1.mrpack"))
            .count(),
        downloads
    );
    let _ = (&world.zoomify, &world.sodium2, &world.pack2);
}

#[test]
fn forge_neoforge_and_quilt_views_install_their_loader() {
    for (loader, version, expected) in [
        (Loader::Forge, "47.4.0", "1.21.1-47.4.0"),
        (Loader::NeoForge, "21.1.209", "21.1.209"),
        (Loader::Quilt, "0.29.2", "0.29.2"),
    ] {
        let world = World::new();
        let mut content = content_image("1.21.1", loader.clone(), Some(version));
        content.entries.push(object_entry("mods/own.jar", b"own"));
        world.core.publish(&content, &[("", b"own")]);
        let (instance, _) = world.create();
        assert_eq!(
            instance.loader_version.as_deref(),
            Some(expected),
            "{loader:?}"
        );
        assert_eq!(read(&world.game(&instance.id).join("mods/own.jar")), "own");
    }
}

/// A hostile server: a view whose files a server must never write, and deltas to a view
/// the player keeps that add what a client view never carries. Nothing is installed.
#[test]
fn hostile_views_and_deltas_install_nothing() {
    let world = World::new();
    let first = world.content(&world.pack1, &world.sodium1);
    world.core.publish(&first, &[("", b"the server's own mod")]);
    let (instance, ready) = world.create();
    let dirs = InstanceDirs::new(world.store.dir(&instance.id));
    let applied = ready.plan.view.id.clone();

    // the player's options and server list, as client view entries
    for path in ["options.txt", "servers.dat"] {
        let mut hostile = first.clone();
        hostile.entries.push(object_entry(path, b"evil"));
        world.core.publish(&hostile, &[("", b"evil")]);
        let result = world.play(&instance.id);
        assert!(
            matches!(result, Err(SyncError::Refused(_))),
            "{path}: {result:?}"
        );
        assert!(!dirs.game.join(path).exists());
    }

    // a delta that adds server.properties or a Lodestone file to the kept view
    for path in [
        "server.properties",
        ".lodestone_config",
        "config/unshared.toml",
    ] {
        let mut target = ready.plan.view.clone();
        target.entries.push(object_entry(path, b"evil"));
        target.seal();
        let delta = lodestone_image::diff(&ready.plan.view, &target);
        world.core.fault(image_core::Fault::Raw {
            status: 200,
            headers: vec![
                ("X-Lodestone-View".into(), target.id.clone()),
                ("X-Lodestone-Answer".into(), "delta".into()),
                ("Content-Type".into(), "application/json".into()),
            ],
            body: serde_json::to_vec(&delta).unwrap(),
        });
        let instance = world.store.get(&instance.id).unwrap();
        let result = world.prepare(Some((&dirs, &instance)), Some(&applied));
        assert!(
            matches!(result, Err(SyncError::Refused(_))),
            "{path}: {result:?}"
        );
        assert!(!dirs.game.join(path).exists());
    }
    // a view whose id isn't the one it names
    world.core.publish(&first, &[]);
    world
        .core
        .fault(image_core::Fault::WrongView("a".repeat(64)));
    assert!(matches!(
        world.prepare(None, None),
        Err(SyncError::Refused(_))
    ));
    // the link still names what was applied
    assert_eq!(
        link::load_link(&dirs.instance).unwrap().view.as_deref(),
        Some(applied.as_str())
    );
}

/// The update's order, as the app runs it: refused while the instance's files are held
/// (its game runs) without recording or changing anything; the loader version saved
/// while the link still names the old view; a failed save leaves the link on the old
/// view, so the next Play places the view again.
#[test]
fn an_update_is_refused_while_held_and_names_the_view_last() {
    let world = World::new();
    let first = world.content(&world.pack1, &world.sodium1);
    world.core.publish(&first, &[("", b"the server's own mod")]);
    let (instance, ready) = world.create();
    let id = instance.id;
    let first_view = ready.plan.view.id.clone();
    let dirs = InstanceDirs::new(world.store.dir(&id));
    let game = world.game(&id);

    let mut second = first.clone();
    second
        .entries
        .push(object_entry("mods/new.jar", b"a new mod"));
    second.game.loader_version = Some("0.16.11".into());
    world.core.publish(&second, &[("", b"a new mod")]);
    let instance = world.store.get(&id).unwrap();
    let Prepared::Ready(ready) = world
        .prepare(Some((&dirs, &instance)), Some(&first_view))
        .unwrap()
    else {
        panic!("expected a plan");
    };
    let ready: Arc<Ready> = Arc::from(ready);
    assert!(ready.asks(Some(&instance)));
    let labels_before = world.labels(&id);

    // the game runs: nothing recorded, nothing placed
    assert_eq!(
        world
            .update(&id, &ready, Some("the game runs"))
            .unwrap_err(),
        "the game runs"
    );
    world.images.flush(&id).unwrap();
    assert_eq!(world.labels(&id), labels_before);
    assert!(!game.join("mods/new.jar").exists());
    assert_eq!(
        link::load_link(&dirs.instance).unwrap().view.as_deref(),
        Some(first_view.as_str())
    );

    // the loader version can't be saved: the files are placed and recorded, the link
    // still names the old view
    let update = ServerUpdate::start(
        world.images.clone(),
        world.store.get(&id).unwrap(),
        "SMP",
        dirs.clone(),
        ready.clone(),
        None::<()>,
    )
    .unwrap();
    let placed = block_on(http::in_http_runtime(update.place(
        &world.sources(&ready),
        &InstallProgress::default(),
        &CancelToken::new(),
    )))
    .unwrap();
    assert_eq!(placed.loader_version().as_deref(), Some("0.16.11"));
    let result = placed.finish(|version| {
        assert_eq!(version.as_deref(), Some("0.16.11"));
        // saved before the link names the view
        assert_eq!(
            link::load_link(&dirs.instance).unwrap().view.as_deref(),
            Some(first_view.as_str())
        );
        Err("the disk is full".into())
    });
    assert!(matches!(result, Err(SyncError::Io(_))));
    world.images.flush(&id).unwrap();
    assert_eq!(read(&game.join("mods/new.jar")), "a new mod");
    // (nothing changed since "Created", so the record before it is that image)
    assert_eq!(
        world.labels(&id)[..2],
        ["Updated from SMP", "Created from SMP"]
    );
    assert_eq!(
        link::load_link(&dirs.instance).unwrap().view.as_deref(),
        Some(first_view.as_str())
    );

    // the next Play: the files are in place, the loader version isn't saved, so the view
    // is placed again (nothing to download) and applied once the loader is saved
    let again = world.play(&id).unwrap().unwrap();
    assert!(!again.plan.update.changes_files());
    assert_eq!(
        world.store.get(&id).unwrap().loader_version.as_deref(),
        Some("0.16.11")
    );
    assert_eq!(
        link::load_link(&dirs.instance).unwrap().view.as_deref(),
        Some(again.plan.view.id.as_str())
    );
    assert!(world.play(&id).unwrap().is_none());
}

/// A view that changes nothing on disk but withholds a file players need: the player is
/// asked (the update prompt lists it) before playing, once; a withheld file players don't
/// need (a server config) asks nothing.
#[test]
fn a_view_that_only_lacks_files_asks_before_playing() {
    let world = World::new();
    let first = world.content(&world.pack1, &world.sodium1);
    world.core.publish(&first, &[("", b"the server's own mod")]);
    let (instance, ready) = world.create();
    let id = instance.id;
    let dirs = InstanceDirs::new(world.store.dir(&id));
    assert!(ready.newly_lacking.is_empty());

    // the owner adds a mod that is a link on the server
    let mut second = first.clone();
    second.withheld.push(lodestone_image::Withheld {
        path: "mods/linked.jar".into(),
        reason: lodestone_image::WithheldReason::Symlink,
    });
    world.core.publish(&second, &[]);
    let instance = world.store.get(&id).unwrap();
    let applied = link::load_link(&dirs.instance).unwrap().view;
    let Prepared::Ready(ready) = world
        .prepare(Some((&dirs, &instance)), applied.as_deref())
        .unwrap()
    else {
        panic!("another view");
    };
    assert!(!ready.changes(Some(&instance)));
    assert_eq!(ready.newly_lacking, ["mods/linked.jar"]);
    assert!(ready.asks(Some(&instance)));
    assert_eq!(ready.plan.summary.missing[0].0, "mods/linked.jar");
    // playing on records the view (nothing changes on disk, nothing is labelled)
    let labels = world.labels(&id);
    world.play(&id).unwrap().unwrap();
    assert_eq!(world.labels(&id), labels);
    assert_eq!(
        link::load_link(&dirs.instance).unwrap().view.as_deref(),
        Some(ready.plan.view.id.as_str())
    );
    // told once: the next Play joins
    assert!(world.play(&id).unwrap().is_none());

    // a withheld server config isn't something a player lacks
    let mut third = second.clone();
    third.withheld.push(lodestone_image::Withheld {
        path: "config/secret.toml".into(),
        reason: lodestone_image::WithheldReason::KeptChanging,
    });
    world.core.publish(&third, &[]);
    let instance = world.store.get(&id).unwrap();
    let applied = link::load_link(&dirs.instance).unwrap().view;
    let Prepared::Ready(ready) = world
        .prepare(Some((&dirs, &instance)), applied.as_deref())
        .unwrap()
    else {
        panic!("another view");
    };
    assert!(ready.newly_lacking.is_empty());
    assert!(!ready.asks(Some(&instance)));
}

/// CurseForge entries of a view: one CurseForge lets other apps download is fetched from
/// its CDN (through the CurseForge client, `files_by_id` then `download_url`) and checked
/// by CurseForge's SHA-1 and the entry's SHA-256; one the server marks not downloadable,
/// and one CurseForge has no download for, are listed for the player and not placed. A
/// file whose bytes aren't the entry's SHA-256 is refused, and no instance is left.
#[test]
fn curseforge_entries_are_downloaded_when_allowed_and_listed_otherwise() {
    use curseforge::{CurseForgeMock, FakeFile};
    let world = World::new();
    let cf = CurseForgeMock::start();
    let cf_api = cf.client();
    let date = "2026-01-01T00:00:00Z";
    let allowed = FakeFile::new(
        1001,
        77,
        "allowed.jar",
        &["1.21.1", "Fabric"],
        date,
        b"allowed".to_vec(),
    );
    let blocked = FakeFile::new(
        1003,
        79,
        "blocked.jar",
        &["1.21.1", "Fabric"],
        date,
        b"blocked".to_vec(),
    )
    .blocked();
    cf.file(allowed.clone()).file(blocked.clone());
    let curseforge_entry =
        |path: &str, project: u64, file: u64, bytes: &[u8], downloadable: bool| {
            Entry::new(
                CanonicalPath::new(path).unwrap(),
                bytes.len() as u64,
                sha256_hex(bytes),
                Source::Curseforge {
                    project,
                    file,
                    downloadable,
                },
            )
        };
    let mut content = content_image("1.21.1", Loader::Fabric, Some("0.16.10"));
    content.entries.push(curseforge_entry(
        "mods/allowed.jar",
        77,
        1001,
        b"allowed",
        true,
    ));
    content.entries.push(curseforge_entry(
        "mods/server-says-no.jar",
        78,
        1002,
        b"no",
        false,
    ));
    content.entries.push(curseforge_entry(
        "mods/blocked.jar",
        79,
        1003,
        b"blocked",
        true,
    ));
    world.core.publish(&content, &[]);

    let lookup = Lookup {
        curseforge: Some(&cf_api),
        ..world.lookup()
    };
    let prepare = || {
        block_on(http::in_http_runtime(prepare(
            &world.server,
            Target {
                existing: None,
                applied: None,
                name: "SMP",
            },
            lookup,
            &|| {},
            &InstallProgress::default(),
            &CancelToken::new(),
        )))
    };
    let build = |ready: &Ready| {
        let sources = Sources {
            platforms: Platforms::new(Some(&world.api), Some(&cf_api)),
            downloader: &world.downloader,
            pack: None,
            objects: Some(&world.server),
        };
        block_on(http::in_http_runtime(build_instance(
            &world.store,
            &sources,
            &ready.plan,
            "SMP",
            world.link(),
            &InstallProgress::default(),
            &CancelToken::new(),
        )))
    };
    // CurseForge out of reach, or no CurseForge at all: nothing is planned, so no file is
    // taken for one the player must get by hand
    cf.state().next_status = Some(500);
    assert!(matches!(
        prepare(),
        Err(SyncError::CurseForgeUnavailable(_))
    ));
    let without = block_on(http::in_http_runtime(prepare_fn(
        &world.server,
        Target {
            existing: None,
            applied: None,
            name: "SMP",
        },
        world.lookup(),
        &|| {},
        &InstallProgress::default(),
        &CancelToken::new(),
    )));
    assert!(matches!(without, Err(SyncError::CurseForgeUnavailable(_))));
    let Prepared::Ready(ready) = prepare().unwrap() else {
        panic!("a plan");
    };
    let mut listed = ready.plan.summary.curseforge_listed.clone();
    listed.sort();
    assert_eq!(listed, ["mods/blocked.jar", "mods/server-says-no.jar"]);
    // one request for the downloadable ones
    let asked: Vec<_> = cf
        .requests()
        .into_iter()
        .filter(|r| r.method == "POST" && r.path.ends_with("mods/files"))
        .collect();
    // (the first was answered 500)
    assert_eq!(asked.len(), 2);
    let asked = &asked[1];
    assert!(asked.body.contains("1001") && asked.body.contains("1003"));
    assert!(!asked.body.contains("1002"));
    let instance = build(&ready).unwrap();
    let game = world.game(&instance.id);
    assert_eq!(read(&game.join("mods/allowed.jar")), "allowed");
    assert!(!game.join("mods/blocked.jar").exists());
    assert!(!game.join("mods/server-says-no.jar").exists());
    assert!(
        cf.requests()
            .iter()
            .any(|r| r.path.contains("edge.forgecdn.net") && r.path.ends_with("allowed.jar"))
    );
    let record =
        ModpackRecord::load(&InstanceDirs::new(world.store.dir(&instance.id)).instance).unwrap();
    assert_eq!(record.files["mods/allowed.jar"].sha1, allowed.sha1());

    // the entry's SHA-256 isn't CurseForge's file: refused, nothing left
    let mut wrong = content.clone();
    wrong
        .entries
        .retain(|e| e.path.as_str() != "mods/allowed.jar");
    let mut entry = curseforge_entry("mods/allowed.jar", 77, 1001, b"allowed", true);
    entry.sha256 = sha256_hex(b"something else");
    wrong.entries.push(entry);
    world.core.publish(&wrong, &[]);
    let Prepared::Ready(ready) = prepare().unwrap() else {
        panic!("a plan");
    };
    let before = std::fs::read_dir(world.store.root()).unwrap().count();
    assert!(build(&ready).is_err());
    assert_eq!(
        std::fs::read_dir(world.store.root()).unwrap().count(),
        before
    );
}

/// The server answers 204 (in step), but a mod of the server's is gone or changed on disk
/// (deleted, updated through the Content tab, damaged): the applied view is placed again,
/// so the server's `mods/` stays authoritative and the player can join.
#[test]
fn a_server_mod_changed_on_disk_is_put_back_though_the_server_says_in_step() {
    let world = World::new();
    let first = world.content(&world.pack1, &world.sodium1);
    world.core.publish(&first, &[("", b"the server's own mod")]);
    let (instance, _) = world.create();
    let id = instance.id;
    let game = world.game(&id);
    assert!(world.play(&id).unwrap().is_none());

    std::fs::remove_file(game.join("mods/jei.jar")).unwrap();
    let ready = world.play(&id).unwrap().expect("the applied view, again");
    assert_eq!(ready.plan.summary.changes.added, ["mods/jei.jar"]);
    assert_eq!(read(&game.join("mods/jei.jar")), "jei 1");
    // a 204 both times: nothing new was asked for
    assert!(world.play(&id).unwrap().is_none());

    // the same size, other bytes: read, not taken as unchanged by its record
    std::fs::write(game.join("mods/jei.jar"), b"JEI 9").unwrap();
    std::fs::write(game.join("mods/own.jar"), b"another mod entirely").unwrap();
    let ready = world.play(&id).unwrap().expect("the applied view, again");
    let mut updated = ready.plan.summary.changes.updated.clone();
    updated.sort();
    assert_eq!(updated, ["mods/jei.jar", "mods/own.jar"]);
    assert_eq!(read(&game.join("mods/jei.jar")), "jei 1");
    assert_eq!(read(&game.join("mods/own.jar")), "the server's own mod");
    assert!(world.play(&id).unwrap().is_none());
}

/// A file the applied view placed that the next view can't give (the server's copy kept
/// changing while it recorded) stays on disk and in `modpack.json`: removing it would
/// leave the player without a mod the server still runs. Once the view gives it again,
/// it is updated as usual.
#[test]
fn files_a_view_cant_give_stay_on_disk() {
    let world = World::new();
    let mut first = world.content(&world.pack1, &world.sodium1);
    first.entries.push(object_entry("mods/foo.jar", b"foo 1"));
    world
        .core
        .publish(&first, &[("", b"the server's own mod"), ("", b"foo 1")]);
    let (instance, _) = world.create();
    let id = instance.id;
    let game = world.game(&id);
    let dirs = InstanceDirs::new(world.store.dir(&id));
    assert_eq!(read(&game.join("mods/foo.jar")), "foo 1");

    let mut second = first.clone();
    second.entries.retain(|e| e.path.as_str() != "mods/foo.jar");
    second.withheld.push(lodestone_image::Withheld {
        path: "mods/foo.jar".into(),
        reason: lodestone_image::WithheldReason::KeptChanging,
    });
    world.core.publish(&second, &[]);
    let ready = world.play(&id).unwrap().expect("another view");
    assert!(ready.plan.summary.changes.removed.is_empty());
    assert_eq!(ready.plan.summary.missing[0].0, "mods/foo.jar");
    assert_eq!(read(&game.join("mods/foo.jar")), "foo 1");
    let record = ModpackRecord::load(&dirs.instance).unwrap();
    assert!(record.files.contains_key("mods/foo.jar"));

    let mut third = first.clone();
    third.entries.retain(|e| e.path.as_str() != "mods/foo.jar");
    third.entries.push(object_entry("mods/foo.jar", b"foo 2"));
    world.core.publish(&third, &[("", b"foo 2")]);
    let ready = world.play(&id).unwrap().expect("another view");
    assert_eq!(ready.plan.summary.changes.updated, ["mods/foo.jar"]);
    assert_eq!(read(&game.join("mods/foo.jar")), "foo 2");
}

/// An update that stopped part way (after placing files, before its records) and a server
/// that moves on before the next Play: the files the stopped update placed are the
/// server's, so the next view removes the ones it drops, instead of keeping them as the
/// player's.
#[test]
fn files_a_stopped_update_placed_are_the_servers() {
    let world = World::new();
    let first = world.content(&world.pack1, &world.sodium1);
    world.core.publish(&first, &[("", b"the server's own mod")]);
    let (instance, ready) = world.create();
    let id = instance.id;
    let first_view = ready.plan.view.id.clone();
    let game = world.game(&id);
    let dirs = InstanceDirs::new(world.store.dir(&id));

    // view 2 adds a mod and changes the server's own one; its update stops before the
    // records
    let mut second = first.clone();
    second.entries.retain(|e| e.path.as_str() != "mods/own.jar");
    second
        .entries
        .push(object_entry("mods/own.jar", b"the server's own mod, 2"));
    second
        .entries
        .push(object_entry("mods/new.jar", b"a new mod"));
    world.core.publish(
        &second,
        &[("", b"the server's own mod, 2"), ("", b"a new mod")],
    );
    let instance = world.store.get(&id).unwrap();
    let Prepared::Ready(ready) = world
        .prepare(Some((&dirs, &instance)), Some(&first_view))
        .unwrap()
    else {
        panic!("expected a plan");
    };
    KeptViews::open(&dirs.instance)
        .begin_placing(&ready.plan.view)
        .unwrap();
    block_on(http::in_http_runtime(update_instance(
        &world.sources(&ready),
        &dirs,
        &ready.plan,
        &InstallProgress::default(),
        &CancelToken::new(),
    )))
    .unwrap();
    // as if `modpack.json` hadn't been saved
    let mut record = ModpackRecord::load(&dirs.instance).unwrap();
    record.files.remove("mods/new.jar");
    record.files.get_mut("mods/own.jar").unwrap().sha256 =
        Some(sha256_hex(b"the server's own mod"));
    record.save(&dirs.instance).unwrap();

    // view 3 drops both
    let mut third = first.clone();
    third.entries.retain(|e| e.path.as_str() != "mods/own.jar");
    world.core.publish(&third, &[]);
    let ready = world.play(&id).unwrap().expect("another view");
    let mut removed = ready.plan.summary.changes.removed.clone();
    removed.sort();
    assert_eq!(removed, ["mods/new.jar", "mods/own.jar"]);
    assert!(!game.join("mods/new.jar").exists());
    assert!(!game.join("mods/own.jar").exists());
    assert!(KeptViews::open(&dirs.instance).placing().is_none());
}

/// A pack with a client-only copy of a mod (`client-overrides/`) beside the one both sides
/// get (`overrides/`): the core takes a `base` entry from its server share, so the player
/// gets the `overrides/` copy, read again from the cached pack, instead of the view being
/// refused.
#[test]
fn a_base_entry_from_the_packs_server_share_is_installed() {
    let world = World::new();
    let pack_bytes = make_pack(
        "3.0",
        json!([index_file(&world.sodium1, "mods/sodium.jar", "required")]),
        &[
            ("overrides/mods/both.jar", b"for both sides"),
            ("client-overrides/mods/both.jar", b"for clients"),
        ],
    );
    let pack = FakeVersion::new("pv3", PACK, "3.0", &["fabric"], &["1.21.1"], "2026-03-01")
        .file("cozy-3.mrpack", &pack_bytes);
    world.modrinth.version(pack.clone());
    let mut content = content_image("1.21.1", Loader::Fabric, Some("0.16.10"));
    content.base = Some(Base::ModrinthPack {
        project: PACK.into(),
        version: pack.id.clone(),
        sha512: pack.sha512(),
    });
    content
        .entries
        .push(modrinth_entry("mods/sodium.jar", &world.sodium1));
    content
        .entries
        .push(base_entry("mods/both.jar", b"for both sides"));
    world.core.publish(&content, &[]);
    let (instance, ready) = world.create();
    assert!(ready.plan.summary.added_mods.is_empty());
    assert_eq!(
        read(&world.game(&instance.id).join("mods/both.jar")),
        "for both sides"
    );
}

/// The modpack updater's rollback copy of an update's files, in `backups/modpack-update/`.
fn rollback_copies(world: &World, id: &str) -> Vec<std::path::PathBuf> {
    let dirs = InstanceDirs::new(world.store.dir(id));
    std::fs::read_dir(launcher_core::worlds::backups_dir(&dirs).join("modpack-update"))
        .map(|entries| entries.flatten().map(|e| e.path()).collect())
        .unwrap_or_default()
}

/// The player's own files at paths the server's pack starts placing outside the content
/// folders (CraftTweaker's `scripts/`): the pack's own file there never goes over the
/// player's, as a modpack update keeps it; a file the server changed does (the server is
/// the authority on it), and since no content image holds the player's copy (it was in no
/// pack's record), the update's rollback copy of it stays. An update whose files the
/// record before it holds keeps no rollback copy.
#[test]
fn the_players_own_files_where_the_pack_now_places_some_are_kept() {
    let world = World::new();
    let first = world.content(&world.pack1, &world.sodium1);
    world.core.publish(&first, &[("", b"the server's own mod")]);
    let (instance, _) = world.create();
    let id = instance.id;
    let game = world.game(&id);
    std::fs::create_dir_all(game.join("scripts")).unwrap();
    std::fs::write(game.join("scripts/foo.zs"), b"the player's foo").unwrap();
    std::fs::write(game.join("scripts/bar.zs"), b"the player's bar").unwrap();

    // pack 3 brings two scripts: the server has the pack's foo.zs and its own bar.zs
    let pack3_bytes = make_pack(
        "3.0",
        json!([
            index_file(&world.sodium1, "mods/sodium.jar", "required"),
            index_file(&world.zoomify, "mods/zoomify.jar", "required"),
        ]),
        &[
            ("overrides/mods/bundled.jar", b"bundled 1"),
            ("overrides/config/pack.toml", b"pack config 1"),
            ("client-overrides/config/client.toml", b"client config 1"),
            ("overrides/scripts/foo.zs", b"the pack's foo"),
            ("overrides/scripts/bar.zs", b"the pack's bar"),
        ],
    );
    let pack3 = FakeVersion::new("pv3", PACK, "3.0", &["fabric"], &["1.21.1"], "2026-03-01")
        .file("cozy-3.mrpack", &pack3_bytes);
    world.modrinth.version(pack3.clone());
    let mut third = world.content(&pack3, &world.sodium1);
    third
        .entries
        .push(base_entry("scripts/foo.zs", b"the pack's foo"));
    third
        .entries
        .push(object_entry("scripts/bar.zs", b"the server's bar"));
    world.core.publish(
        &third,
        &[("", b"the server's own mod"), ("", b"the server's bar")],
    );
    let ready = world.play(&id).unwrap().expect("another view");
    assert!(
        ready
            .plan
            .update
            .overrides_kept
            .contains(&"scripts/foo.zs".to_string()),
        "{:?}",
        ready.plan.update
    );
    assert_eq!(read(&game.join("scripts/foo.zs")), "the player's foo");
    assert_eq!(ready.plan.update.replaced_own, ["scripts/bar.zs"]);
    assert_eq!(read(&game.join("scripts/bar.zs")), "the server's bar");
    let copies = rollback_copies(&world, &id);
    assert_eq!(copies.len(), 1, "{copies:?}");
    assert_eq!(
        read(&copies[0].join("minecraft/scripts/bar.zs")),
        "the player's bar"
    );

    // the server changes its own mod: the record before the update holds it
    let mut fourth = third.clone();
    fourth.entries.retain(|e| e.path.as_str() != "mods/own.jar");
    fourth
        .entries
        .push(object_entry("mods/own.jar", b"the server's own mod 2"));
    world.core.publish(
        &fourth,
        &[("", b"the server's own mod 2"), ("", b"the server's bar")],
    );
    world.play(&id).unwrap().expect("another view");
    assert_eq!(read(&game.join("mods/own.jar")), "the server's own mod 2");
    assert_eq!(rollback_copies(&world, &id), copies);
}

/// The update prompt waits for the player, who may change a file the plan would replace
/// meanwhile (a config edited in game): the update stops before anything changes, and the
/// next Play plans with the change, which stays.
#[test]
fn a_file_changed_after_planning_stops_the_update() {
    let world = World::new();
    let first = world.content(&world.pack1, &world.sodium1);
    world.core.publish(&first, &[("", b"the server's own mod")]);
    let (instance, created) = world.create();
    let id = instance.id;
    let game = world.game(&id);
    let dirs = InstanceDirs::new(world.store.dir(&id));
    let second = world.content(&world.pack2, &world.sodium2);
    world
        .core
        .publish(&second, &[("", b"the server's own mod")]);
    let instance = world.store.get(&id).unwrap();
    let applied = created.plan.view.id.clone();
    let Prepared::Ready(ready) = world
        .prepare(Some((&dirs, &instance)), Some(&applied))
        .unwrap()
    else {
        panic!("another view");
    };
    assert!(
        ready
            .plan
            .update
            .overrides_written
            .contains(&"config/client.toml".to_string())
    );
    std::fs::write(game.join("config/client.toml"), b"the player's edit").unwrap();
    let err = world
        .update(&id, &Arc::from(ready), None)
        .unwrap()
        .unwrap_err();
    assert!(
        matches!(
            &err,
            SyncError::Modpack(ModpackError::ChangedSincePlanned { path })
                if path == "config/client.toml"
        ),
        "{err}"
    );
    assert_eq!(read(&game.join("config/client.toml")), "the player's edit");
    assert_eq!(read(&game.join("mods/sodium.jar")), "sodium 1");
    assert_eq!(link::load_link(&dirs.instance).unwrap().view, Some(applied));
    // nothing was placed: the view isn't taken for one being placed, and no copy is left
    assert!(KeptViews::open(&dirs.instance).placing().is_none());
    assert!(rollback_copies(&world, &id).is_empty());

    let ready = world.play(&id).unwrap().expect("the update");
    assert!(
        ready
            .plan
            .update
            .overrides_kept
            .contains(&"config/client.toml".to_string())
    );
    assert_eq!(read(&game.join("config/client.toml")), "the player's edit");
    assert_eq!(read(&game.join("mods/sodium.jar")), "sodium 2");
}

/// A vanilla server's view places nothing: nothing to install, nothing to trust (no
/// advisory: no file in it can run code), nothing missing to tell; Play joins on its 204.
#[test]
fn a_vanilla_servers_view_installs_and_asks_nothing() {
    let world = World::new();
    let mut image = content_image("1.21.1", Loader::Vanilla, None);
    image
        .entries
        .push(object_entry("server.properties", b"motd=hi"));
    world.core.publish(&image, &[("", b"motd=hi")]);
    let Prepared::Ready(ready) = world.prepare(None, None).unwrap() else {
        panic!("a new instance gets a plan");
    };
    assert!(!ready.plan.update.changes_files());
    assert!(launcher_core::cores::image_sync::plan::pulls(&ready.plan.update).is_empty());
    assert!(ready.plan.summary.missing.is_empty());
    assert!(ready.plan.summary.curseforge_listed.is_empty());
    let (instance, _) = world.create();
    assert_eq!(instance.loader, launcher_core::instances::Loader::None);
    let record = ModpackRecord::load(&world.store.dir(&instance.id)).unwrap();
    assert!(record.files.is_empty());
    assert!(world.play(&instance.id).unwrap().is_none());
}

/// A file a view can't give that the player deleted isn't a server's mod gone missing:
/// a 204 joins without planning again, and the next view drops its line.
#[test]
fn a_file_a_view_cant_give_that_the_player_deleted_isnt_checked() {
    let world = World::new();
    let mut first = world.content(&world.pack1, &world.sodium1);
    first.entries.push(object_entry("mods/foo.jar", b"foo 1"));
    world
        .core
        .publish(&first, &[("", b"the server's own mod"), ("", b"foo 1")]);
    let (instance, _) = world.create();
    let id = instance.id;
    let game = world.game(&id);
    let dirs = InstanceDirs::new(world.store.dir(&id));
    let mut second = first.clone();
    second.entries.retain(|e| e.path.as_str() != "mods/foo.jar");
    second.withheld.push(lodestone_image::Withheld {
        path: "mods/foo.jar".into(),
        reason: lodestone_image::WithheldReason::Symlink,
    });
    world.core.publish(&second, &[]);
    world.play(&id).unwrap().expect("another view");
    std::fs::remove_file(game.join("mods/foo.jar")).unwrap();
    assert!(world.play(&id).unwrap().is_none());

    let mut third = second.clone();
    third.entries.push(object_entry("mods/extra.jar", b"extra"));
    world
        .core
        .publish(&third, &[("", b"the server's own mod"), ("", b"extra")]);
    world.play(&id).unwrap().expect("another view");
    let record = ModpackRecord::load(&dirs.instance).unwrap();
    assert!(!record.files.contains_key("mods/foo.jar"));
    assert!(world.play(&id).unwrap().is_none());
}

/// A 204 doesn't read every mod: one whose size and modification time are those the
/// last check found is taken as placed, and one whose time changed is read (and put back
/// when the player changed it).
#[test]
fn a_204_reads_only_the_mods_whose_size_or_time_changed() {
    let world = World::new();
    let first = world.content(&world.pack1, &world.sodium1);
    world.core.publish(&first, &[("", b"the server's own mod")]);
    let (instance, _) = world.create();
    let id = instance.id;
    let game = world.game(&id);
    let dirs = InstanceDirs::new(world.store.dir(&id));
    let set_time = |path: &Path, ago: u64| {
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(std::time::SystemTime::now() - Duration::from_secs(ago))
            .unwrap();
    };
    for entry in std::fs::read_dir(game.join("mods")).unwrap().flatten() {
        set_time(&entry.path(), 3600);
    }
    assert!(world.play(&id).unwrap().is_none());
    assert!(
        dirs.instance
            .join("server-views/mods-checked.json")
            .is_file()
    );

    // damaged to the same size, its time put back: not read, so not seen
    let own = game.join("mods/own.jar");
    let len = std::fs::metadata(&own).unwrap().len() as usize;
    let modified = std::fs::metadata(&own).unwrap().modified().unwrap();
    std::fs::write(&own, vec![b'x'; len]).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&own)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    assert!(world.play(&id).unwrap().is_none());

    // its time changed: read, found changed, put back
    set_time(&own, 600);
    world.play(&id).unwrap().expect("the server's mod put back");
    assert_eq!(read(&own), "the server's own mod");
}

/// A pack file outside the content folders (CraftTweaker's `scripts/`) is the server's as
/// any other: it goes when the view's `removed` names it, and when the pack's next
/// version drops it, unless the player changed it; a pack config the server dropped
/// stays ("From one view to the next on disk").
#[test]
fn a_pack_script_the_server_drops_goes() {
    let world = World::new();
    let pack_with = |id: &str, version: &str, date: &str, scripts: &[(&str, &[u8])]| {
        let mut overrides: Vec<(&str, &[u8])> = vec![
            ("overrides/mods/bundled.jar", b"bundled 1"),
            ("overrides/config/pack.toml", b"pack config 1"),
        ];
        overrides.extend_from_slice(scripts);
        let bytes = make_pack(
            version,
            json!([
                index_file(&world.sodium1, "mods/sodium.jar", "required"),
                index_file(&world.zoomify, "mods/zoomify.jar", "required"),
            ]),
            &overrides,
        );
        let pack = FakeVersion::new(id, PACK, version, &["fabric"], &["1.21.1"], date)
            .file(&format!("cozy-{version}.mrpack"), &bytes);
        world.modrinth.version(pack.clone());
        pack
    };
    let pack3 = pack_with(
        "pv3",
        "3.0",
        "2026-03-01",
        &[
            ("overrides/scripts/recipes.zs", b"recipes"),
            ("overrides/scripts/kept.zs", b"kept"),
            ("overrides/scripts/edited.zs", b"the pack's"),
            ("overrides/config/extra.toml", b"extra config"),
        ],
    );
    let mut first = world.content(&pack3, &world.sodium1);
    for (path, bytes) in [
        ("scripts/recipes.zs", &b"recipes"[..]),
        ("scripts/kept.zs", b"kept"),
        ("scripts/edited.zs", b"the pack's"),
    ] {
        first.entries.push(base_entry(path, bytes));
    }
    world.core.publish(&first, &[("", b"the server's own mod")]);
    let (instance, _) = world.create();
    let id = instance.id;
    let game = world.game(&id);
    let dirs = InstanceDirs::new(world.store.dir(&id));
    assert_eq!(read(&game.join("scripts/recipes.zs")), "recipes");
    assert_eq!(read(&game.join("config/extra.toml")), "extra config");
    std::fs::write(game.join("scripts/edited.zs"), b"the player's").unwrap();

    // the owner deletes the script: the view drops its entry and names it in `removed`
    let mut second = first.clone();
    second
        .entries
        .retain(|e| e.path.as_str() != "scripts/recipes.zs");
    second
        .removed
        .push(CanonicalPath::new("scripts/recipes.zs").unwrap());
    world
        .core
        .publish(&second, &[("", b"the server's own mod")]);
    let ready = world.play(&id).unwrap().expect("another view");
    assert_eq!(ready.plan.summary.changes.removed, ["scripts/recipes.zs"]);
    assert!(!game.join("scripts/recipes.zs").exists());
    let record = ModpackRecord::load(&dirs.instance).unwrap();
    assert!(!record.files.contains_key("scripts/recipes.zs"));

    // the pack's next version drops the other scripts and a config: the unchanged script
    // goes, the one the player changed and the config stay
    let pack4 = pack_with("pv4", "4.0", "2026-04-01", &[]);
    let mut third = world.content(&pack4, &world.sodium1);
    third
        .entries
        .retain(|e| !e.path.as_str().starts_with("scripts/"));
    world.core.publish(&third, &[("", b"the server's own mod")]);
    world.play(&id).unwrap().expect("another view");
    assert!(!game.join("scripts/kept.zs").exists());
    assert_eq!(read(&game.join("scripts/edited.zs")), "the player's");
    assert_eq!(read(&game.join("config/extra.toml")), "extra config");
}

/// An update that stopped part way and a server that went back to the applied view: its
/// 204 doesn't leave the files the stopped update added, which the applied view is placed
/// again to remove.
#[test]
fn a_204_after_a_stopped_update_removes_what_it_added() {
    let world = World::new();
    let first = world.content(&world.pack1, &world.sodium1);
    world.core.publish(&first, &[("", b"the server's own mod")]);
    let (instance, ready) = world.create();
    let id = instance.id;
    let first_view = ready.plan.view.id.clone();
    let game = world.game(&id);
    let dirs = InstanceDirs::new(world.store.dir(&id));

    let mut second = first.clone();
    second
        .entries
        .push(object_entry("mods/new.jar", b"a new mod"));
    world.core.publish(
        &second,
        &[("", b"the server's own mod"), ("", b"a new mod")],
    );
    let instance = world.store.get(&id).unwrap();
    let Prepared::Ready(ready) = world
        .prepare(Some((&dirs, &instance)), Some(&first_view))
        .unwrap()
    else {
        panic!("expected a plan");
    };
    KeptViews::open(&dirs.instance)
        .begin_placing(&ready.plan.view)
        .unwrap();
    block_on(http::in_http_runtime(update_instance(
        &world.sources(&ready),
        &dirs,
        &ready.plan,
        &InstallProgress::default(),
        &CancelToken::new(),
    )))
    .unwrap();
    // as if `modpack.json` hadn't been saved
    let mut record = ModpackRecord::load(&dirs.instance).unwrap();
    record.files.remove("mods/new.jar");
    record.save(&dirs.instance).unwrap();
    assert!(game.join("mods/new.jar").exists());

    // the server goes back to the first view: a 204 for the applied view
    world.core.publish(&first, &[("", b"the server's own mod")]);
    let ready = world.play(&id).unwrap().expect("placed again");
    assert_eq!(ready.plan.view.id, first_view);
    assert_eq!(ready.plan.summary.changes.removed, ["mods/new.jar"]);
    assert!(!game.join("mods/new.jar").exists());
    assert!(KeptViews::open(&dirs.instance).placing().is_none());
    // and the next Play joins at once
    assert!(world.play(&id).unwrap().is_none());
}

/// A delta this Launcher can't read (a newer core's change of a kind it doesn't know, say)
/// isn't refused outright: the whole view is asked for, which installs, or says the server
/// needs a newer Lodestone.
#[test]
fn an_unreadable_delta_asks_for_the_whole_view() {
    let world = World::new();
    let first = world.content(&world.pack1, &world.sodium1);
    world.core.publish(&first, &[("", b"the server's own mod")]);
    let (instance, _) = world.create();
    let id = instance.id;
    let mut second = first.clone();
    second
        .entries
        .push(object_entry("mods/new.jar", b"a new mod"));
    world.core.publish(
        &second,
        &[("", b"the server's own mod"), ("", b"a new mod")],
    );
    let current = world.core.current_view().id;
    world.core.fault(image_core::Fault::Raw {
        status: 200,
        headers: vec![
            ("X-Lodestone-View".into(), current.clone()),
            ("X-Lodestone-Answer".into(), "delta".into()),
            ("Content-Type".into(), "application/json".into()),
        ],
        body: br#"{"schema": 1, "from": "x", "changes": [{"kind": "new"}]}"#.to_vec(),
    });
    let asked = world.core.requests().len();
    let ready = world.play(&id).unwrap().expect("the whole view");
    assert_eq!(ready.plan.view.id, current);
    assert_eq!(read(&world.game(&id).join("mods/new.jar")), "a new mod");
    let images: Vec<String> = world.core.requests()[asked..]
        .iter()
        .map(|r| r.path.clone())
        .filter(|path| path.contains("/image?"))
        .collect();
    assert_eq!(images.len(), 2, "{images:?}");
    assert!(images[0].contains("have="));
    assert!(!images[1].contains("have="));
}
