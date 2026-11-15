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
            prepare::{Lookup, Prepared, Ready, Target, prepare},
        },
        link::{self, ServerLink},
        player_image::ServerImage,
    },
    game::download::{CancelToken, Downloader, recording_sleep},
    http,
    images::{Images, Timing, labels},
    instances::{Instance, InstanceStore},
    modpack::{ModpackRecord, fetch::Sources},
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
        let finished = placed.finish(|version| {
            let mut instance = self.store.get(id).unwrap();
            instance.loader_version = version;
            self.store.save(&instance).map_err(|e| e.to_string())
        });
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
    assert_eq!(added, ["jei.jar", "own.jar", "sodium.jar"]);
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
    assert_eq!(asked.len(), 1);
    assert!(asked[0].body.contains("1001") && asked[0].body.contains("1003"));
    assert!(!asked[0].body.contains("1002"));
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
