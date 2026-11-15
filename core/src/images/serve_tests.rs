//! What a server serves its players, against a temporary store with content images built
//! by hand.

use std::sync::Arc;
use std::time::{Duration, Instant};

use lodestone_image::env::{self, modrinth_environment, EnvInputs, JarEnvironment, OwnerMark};
use lodestone_image::settings::{ClientSettings, StoredFile};
use lodestone_image::store::Store;
use lodestone_image::{
    apply, Base, CanonicalPath, Delta, Entry, Env, EnvFrom, EnvValue, Game, Image, Kind, Loader,
    Side, Source,
};

use super::*;
use crate::images::state::{self, ContentState};
use crate::implementations::minecraft::Flavour;

struct Fixture {
    dir: tempfile::TempDir,
    service: Arc<ImageService>,
    uuid: InstanceUuid,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let service = Arc::new(ImageService::new(dir.path().join("backups")));
    Fixture {
        dir,
        service,
        uuid: InstanceUuid::default(),
    }
}

impl Fixture {
    fn store(&self) -> Arc<Store> {
        self.service.store()
    }

    /// A file only the store has, with the sides `inputs` give it.
    fn object(&self, path: &str, bytes: &[u8], inputs: EnvInputs) -> Entry {
        let stored = self.store().put_bytes(bytes).unwrap();
        let mut entry = Entry::new(
            CanonicalPath::new(path).unwrap(),
            stored.size,
            stored.sha256,
            Source::Object {},
        );
        (entry.env, entry.env_from) = env::resolve(&inputs);
        entry
    }

    /// A content image of `entries`, saved (not made current).
    fn save(&self, mut entries: Vec<Entry>, label: &str) -> Image {
        let mut image = Image::new(
            Kind::Content,
            Game {
                minecraft: "1.20.1".into(),
                loader: Loader::Fabric,
                loader_version: Some("0.16.10".into()),
            },
        );
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        image.entries = entries;
        image.label = Some(label.into());
        image.created = 1_794_645_624;
        image.seal();
        self.store()
            .save_manifest(self.uuid.as_ref(), &image)
            .unwrap();
        image
    }

    fn set_current(&self, id: &str) {
        state::save(
            self.service.root(),
            &self.uuid,
            &ContentState {
                current: Some(id.to_string()),
                ..ContentState::default()
            },
        )
        .unwrap();
    }

    async fn answer(&self, have: &[&str]) -> ClientAnswer {
        let have: Vec<String> = have.iter().map(|id| id.to_string()).collect();
        self.service
            .client_answer(&self.uuid, &have)
            .await
            .unwrap()
            .expect("a content image")
    }
}

/// A Modrinth file whose version says `environment`.
fn modrinth(path: &str, bytes: &[u8], environment: &str) -> Entry {
    let sha256 = lodestone_image::hex::sha256_hex(bytes);
    let mut entry = Entry::new(
        CanonicalPath::new(path).unwrap(),
        bytes.len() as u64,
        sha256.clone(),
        Source::Modrinth {
            project: "AANobbMI".into(),
            version: "IZskON6d".into(),
            url: format!("https://cdn.modrinth.com/data/AANobbMI/versions/IZskON6d/{sha256}.jar"),
        },
    );
    entry.sha512 = Some("e".repeat(128));
    (entry.env, entry.env_from) = env::resolve(&EnvInputs {
        modrinth_environment: modrinth_environment(environment),
        ..Default::default()
    });
    entry
}

fn jar(environment: JarEnvironment) -> EnvInputs {
    EnvInputs {
        jar: Some(environment),
        ..Default::default()
    }
}

fn marked(mark: OwnerMark) -> EnvInputs {
    EnvInputs {
        owner_mark: Some(mark),
        ..Default::default()
    }
}

fn view_of(image: &Image) -> Image {
    lodestone_image::view(image, Side::Client, lodestone_image::RULES).unwrap()
}

fn paths(image: &Image) -> Vec<&str> {
    image.entries.iter().map(|e| e.path.as_str()).collect()
}

/// A server's content with every kind of file the client view must sort.
fn server_content(fx: &Fixture) -> Vec<Entry> {
    vec![
        // optional on both sides (JEI): players get it
        modrinth("mods/jei.jar", b"jei", "client_or_server"),
        // nothing known: players get it, so they can always join
        fx.object("mods/plain.jar", b"plain", EnvInputs::default()),
        // the jar says server only, or the owner said not for players: left out
        fx.object(
            "mods/server.jar",
            b"server only",
            jar(JarEnvironment::Server),
        ),
        fx.object(
            "mods/hidden.jar",
            b"hidden",
            marked(OwnerMark::NotForPlayers),
        ),
        // client only: players get it
        fx.object("mods/minimap.jar", b"minimap", jar(JarEnvironment::Client)),
        // a config only when marked for players
        fx.object("config/a.toml", b"secret = 1", EnvInputs::default()),
        fx.object(
            "config/shared.toml",
            b"shared = 1",
            marked(OwnerMark::ForPlayers),
        ),
        // Lodestone's bookkeeping, disabled files, server-only files and server state
        fx.object(
            "mods/.lodestone_client_mods.json",
            b"{}",
            EnvInputs::default(),
        ),
        fx.object("mods/x.jar.disabled", b"disabled", EnvInputs::default()),
        fx.object(
            "server.properties",
            b"rcon.password=hunter2",
            EnvInputs::default(),
        ),
        fx.object("run.sh", b"java -jar", EnvInputs::default()),
        fx.object("libraries/a.jar", b"library", EnvInputs::default()),
        fx.object(
            "fabric-server-launch.jar",
            b"launcher",
            EnvInputs::default(),
        ),
    ]
}

#[tokio::test]
async fn the_client_view_has_what_players_need_and_nothing_else() {
    let fx = fixture();
    let image = fx.save(server_content(&fx), "Initial");
    fx.set_current(&image.id);
    let served = fx.service.client_view(&fx.uuid).await.unwrap().unwrap();
    assert_eq!(
        paths(&served.view),
        [
            "config/shared.toml",
            "mods/jei.jar",
            "mods/minimap.jar",
            "mods/plain.jar"
        ]
    );
    assert_eq!(served.view.side, Some(Side::Client));
    assert_eq!(
        served.view.projected_from.as_deref(),
        Some(image.id.as_str())
    );
    assert_eq!(served.content_id, image.id);
    served.view.verify().unwrap();
    // what is sent is that view, and it checks out as a received view does
    let sent = Image::from_json(&served.json).unwrap();
    sent.verify().unwrap();
    assert_eq!(sent.id, served.id());
    assert_eq!(view_of(&sent).id, sent.id);
    // only the view's `object` entries can be fetched: never a Modrinth file, a config
    // not shared, server.properties or a server-only mod
    for entry in &image.entries {
        let fetchable = served.object_size(&entry.sha256).is_some();
        let expected = matches!(
            entry.path.as_str(),
            "config/shared.toml" | "mods/minimap.jar" | "mods/plain.jar"
        );
        assert_eq!(fetchable, expected, "{}", entry.path);
    }
    // the same view again is the same value
    let again = fx.service.client_view(&fx.uuid).await.unwrap().unwrap();
    assert!(Arc::ptr_eq(&served, &again));
}

#[tokio::test]
async fn the_first_known_have_decides() {
    let fx = fixture();
    let content = server_content(&fx);
    let old = fx.save(content.clone(), "Initial");
    let mut newer = content.clone();
    newer.push(fx.object("mods/new.jar", b"new mod", EnvInputs::default()));
    newer.retain(|e| e.path.as_str() != "mods/plain.jar");
    let current = fx.save(newer, "Added new.jar");
    fx.set_current(&current.id);
    let old_view = view_of(&old);
    let current_view = view_of(&current);
    let unknown = "f".repeat(64);

    assert!(matches!(
        fx.answer(&[&current_view.id]).await,
        ClientAnswer::Current(_)
    ));
    assert!(matches!(fx.answer(&[]).await, ClientAnswer::View(_)));
    assert!(matches!(
        fx.answer(&[&unknown]).await,
        ClientAnswer::View(_)
    ));
    // a content id is not a view id
    assert!(matches!(fx.answer(&[&old.id]).await, ClientAnswer::View(_)));

    // an older view: a delta that gives the current view
    let ClientAnswer::Delta { view, json } = fx.answer(&[&unknown, &old_view.id]).await else {
        panic!("a delta");
    };
    assert_eq!(view.id(), current_view.id);
    let delta = Delta::from_json(&json).unwrap();
    assert_eq!(delta.from, old_view.id);
    assert_eq!(delta.to, current_view.id);
    let applied = apply(&old_view, &delta).unwrap();
    assert_eq!(applied.id, current_view.id);
    assert_eq!(
        delta
            .new_contents(&old_view)
            .map(|e| e.path.as_str())
            .collect::<Vec<_>>(),
        ["mods/new.jar"]
    );
    // the same delta again comes from the cache
    let ClientAnswer::Delta { json: again, .. } = fx.answer(&[&old_view.id]).await else {
        panic!("a delta");
    };
    assert_eq!(again, json);

    // the first id the core knows decides, in the player's order
    assert!(matches!(
        fx.answer(&[&current_view.id, &old_view.id]).await,
        ClientAnswer::Current(_)
    ));
    assert!(matches!(
        fx.answer(&[&old_view.id, &current_view.id]).await,
        ClientAnswer::Delta { .. }
    ));
}

#[tokio::test]
async fn a_change_only_the_server_sees_keeps_the_view() {
    let fx = fixture();
    let content = server_content(&fx);
    let old = fx.save(content.clone(), "Initial");
    let mut changed = content;
    for entry in &mut changed {
        if entry.path.as_str() == "server.properties" {
            *entry = fx.object("server.properties", b"motd=Other", EnvInputs::default());
        }
    }
    let current = fx.save(changed, "Changed motd");
    assert_ne!(old.id, current.id);
    fx.set_current(&current.id);
    let old_view = view_of(&old);
    assert_eq!(old_view.id, view_of(&current).id);
    assert!(matches!(
        fx.answer(&[&old_view.id]).await,
        ClientAnswer::Current(_)
    ));
}

#[tokio::test]
async fn a_delta_no_smaller_than_the_view_is_the_view() {
    let fx = fixture();
    let many: Vec<Entry> = (0..200)
        .map(|n| {
            fx.object(
                &format!("mods/mod-{n:03}.jar"),
                format!("mod {n}").as_bytes(),
                EnvInputs::default(),
            )
        })
        .collect();
    let old = fx.save(many, "Many");
    let few = vec![fx.object("mods/only.jar", b"only", EnvInputs::default())];
    let current = fx.save(few, "Few");
    fx.set_current(&current.id);
    let ClientAnswer::View(view) = fx.answer(&[&view_of(&old).id]).await else {
        panic!("the whole view");
    };
    assert_eq!(paths(&view.view), ["mods/only.jar"]);
}

#[tokio::test]
async fn a_view_whose_image_is_gone_is_unknown() {
    let fx = fixture();
    let content = server_content(&fx);
    let old = fx.save(content.clone(), "Initial");
    let mut newer = content;
    newer.push(fx.object("mods/new.jar", b"new mod", EnvInputs::default()));
    let current = fx.save(newer, "Added");
    fx.set_current(&current.id);
    // an unknown id makes the core learn every manifest's view id (the old one's too)
    assert!(matches!(
        fx.answer(&["f".repeat(64).as_str()]).await,
        ClientAnswer::View(_)
    ));
    // then the old image is pruned before a player names its view
    fx.store()
        .delete_manifest(fx.uuid.as_ref(), &old.id)
        .unwrap();
    assert!(matches!(
        fx.answer(&[&view_of(&old).id]).await,
        ClientAnswer::View(_)
    ));
    // and the core forgets it
    assert!(matches!(
        fx.answer(&["e".repeat(64).as_str()]).await,
        ClientAnswer::View(_)
    ));
    assert!(!fx.service.views.knows(&fx.uuid, &view_of(&old).id));
}

/// A view the core learnt whose image's manifest was damaged since (and isn't cached) gets
/// the whole view, not an error on every request, and is forgotten.
#[tokio::test]
async fn a_view_whose_image_is_damaged_gets_the_whole_view() {
    let fx = fixture();
    let content = server_content(&fx);
    let old = fx.save(content.clone(), "Initial");
    let mut newer = content;
    newer.push(fx.object("mods/new.jar", b"new mod", EnvInputs::default()));
    let current = fx.save(newer, "Added");
    fx.set_current(&current.id);
    // learns the old view's id without caching the view
    assert!(matches!(
        fx.answer(&["f".repeat(64).as_str()]).await,
        ClientAnswer::View(_)
    ));
    assert!(fx.service.views.knows(&fx.uuid, &view_of(&old).id));
    let path = fx.store().manifest_path(fx.uuid.as_ref(), &old.id).unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    std::fs::set_permissions(&path, permissions).unwrap();
    std::fs::write(&path, b"{ not a manifest").unwrap();
    assert!(matches!(
        fx.answer(&[&view_of(&old).id]).await,
        ClientAnswer::View(_)
    ));
    assert!(!fx.service.views.knows(&fx.uuid, &view_of(&old).id));
}

/// A turn to compute a key leaves the map however it ends, and never takes out a newer
/// turn's entry.
#[test]
fn turns_leave_no_entry_behind() {
    let turns: Turns<String> = Default::default();
    let first = Turn::take(&turns, "a".to_string());
    let waiter = Turn::take(&turns, "a".to_string());
    assert!(Arc::ptr_eq(&first.mutex, &waiter.mutex));
    drop(first);
    assert!(locked(&turns).is_empty());
    let newer = Turn::take(&turns, "a".to_string());
    assert!(!Arc::ptr_eq(&newer.mutex, &waiter.mutex));
    // the waiter of the old turn leaves the newer one in place
    drop(waiter);
    assert_eq!(locked(&turns).len(), 1);
    drop(newer);
    assert!(locked(&turns).is_empty());
}

/// Many content images share one client view (a change only the server sees). Pruning the
/// one the core happened to learn the view from must not make it forget the view while
/// another image still has it.
#[tokio::test]
async fn a_shared_view_survives_the_pruning_of_one_of_its_images() {
    let fx = fixture();
    let mut content = server_content(&fx);
    content.retain(|e| e.path.as_str() != "server.properties");
    let mut first = content.clone();
    first.push(fx.object("server.properties", b"motd=One\n", EnvInputs::default()));
    let mut second = content.clone();
    second.push(fx.object("server.properties", b"motd=Two\n", EnvInputs::default()));
    let c1 = fx.save(first, "One");
    let c2 = fx.save(second.clone(), "Two");
    assert_eq!(view_of(&c1).id, view_of(&c2).id);
    let shared = view_of(&c1).id;
    let mut third = second;
    third.push(fx.object("mods/new.jar", b"new mod", EnvInputs::default()));
    let current = fx.save(third, "Added");
    fx.set_current(&current.id);
    assert!(matches!(
        fx.answer(&[&shared]).await,
        ClientAnswer::Delta { .. }
    ));
    // retention prunes one of the two, whichever the core learnt the view from
    let learnt = fx.service.views.contents_of(&fx.uuid, &shared);
    assert_eq!(learnt.len(), 2, "both images are known to have the view");
    fx.store()
        .delete_manifest(fx.uuid.as_ref(), &c1.id)
        .unwrap();
    // a scan (an unknown id) forgets the pruned image, not the view
    assert!(matches!(
        fx.answer(&["f".repeat(64).as_str()]).await,
        ClientAnswer::View(_)
    ));
    assert_eq!(
        fx.service.views.contents_of(&fx.uuid, &shared),
        vec![c2.id.clone()]
    );
    assert!(matches!(
        fx.answer(&[&shared]).await,
        ClientAnswer::Delta { .. }
    ));
    // and once both are gone, the next scan forgets the view (its view was cached by
    // content id, which still answers until then: a content id's view never changes)
    fx.store()
        .delete_manifest(fx.uuid.as_ref(), &c2.id)
        .unwrap();
    assert!(matches!(
        fx.answer(&["f".repeat(64).as_str()]).await,
        ClientAnswer::View(_)
    ));
    assert!(!fx.service.views.knows(&fx.uuid, &shared));
    // the delta computed from it still answers while cached (a view id is its contents);
    // once it isn't, the whole view does
    assert!(matches!(
        fx.answer(&[&shared]).await,
        ClientAnswer::Delta { .. }
    ));
    locked(&fx.service.views.deltas)
        .entries
        .retain(|(from, _), _| *from != shared);
    assert!(matches!(fx.answer(&[&shared]).await, ClientAnswer::View(_)));
}

/// A player that keeps pruned ids after one the core knows costs no scan: the known one
/// decides first.
#[tokio::test]
async fn known_ids_answer_before_unknown_ones_are_looked_up() {
    let fx = fixture();
    let content = server_content(&fx);
    let current = fx.save(content, "Initial");
    fx.set_current(&current.id);
    let current_view = view_of(&current).id;
    let pruned = "d".repeat(64);
    assert!(matches!(
        fx.answer(&[&current_view, &pruned]).await,
        ClientAnswer::Current(_)
    ));
    // no scan ran: the store's manifests were never looked at
    assert!(locked(&fx.service.views.known)
        .get(&fx.uuid)
        .is_none_or(|known| !known.seen.contains_key(&pruned) && known.seen.len() == 1));
    assert!(locked(&fx.service.views.lookups).get(&fx.uuid).is_none());
}

/// A scan of the manifests whose request was dropped (its player hung up) runs to its end
/// and keeps what it learnt: hanging up at once can't make the core read every manifest
/// again on each request.
#[tokio::test]
async fn a_scan_outlives_a_request_that_was_dropped() {
    let fx = fixture();
    let content = server_content(&fx);
    let mut last = None;
    for i in 0..16 {
        let mut entries = content.clone();
        entries.retain(|e| e.path.as_str() != "server.properties");
        let properties = format!("motd=Version {i}\n");
        entries.push(fx.object(
            "server.properties",
            properties.as_bytes(),
            EnvInputs::default(),
        ));
        last = Some(fx.save(entries, &format!("Version {i}")));
    }
    let current = last.unwrap();
    fx.set_current(&current.id);
    // the current view is cached; nothing else was read
    assert!(matches!(fx.answer(&[]).await, ClientAnswer::View(_)));
    let seen = |fx: &Fixture| {
        locked(&fx.service.views.known)
            .get(&fx.uuid)
            .map_or(0, |known| known.seen.len())
    };
    assert_eq!(seen(&fx), 1);
    let lookup = locked(&fx.service.views.lookups)
        .entry(fx.uuid.clone())
        .or_default()
        .clone();
    let unknown = vec!["f".repeat(64)];
    {
        let mut request = std::pin::pin!(fx.service.client_answer(&fx.uuid, &unknown));
        let deadline = Instant::now() + Duration::from_secs(10);
        // polled until the scan holds the instance's turn, then dropped
        loop {
            if futures::poll!(request.as_mut()).is_ready() {
                panic!("the request ended before it could be dropped mid-scan");
            }
            if lookup.try_lock().is_err() {
                break;
            }
            assert!(Instant::now() < deadline, "no scan started");
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }
    // the scan still holds the turn until it ends, so no second one starts meanwhile...
    let deadline = Instant::now() + Duration::from_secs(30);
    while lookup.try_lock().is_err() {
        assert!(Instant::now() < deadline, "the scan never ended");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    // ...and what it learnt is kept: every manifest was looked at once
    assert_eq!(seen(&fx), 16);
}

#[tokio::test]
async fn a_player_whose_views_were_pruned_gets_the_whole_view() {
    let fx = fixture();
    let content = server_content(&fx);
    let old = fx.save(content.clone(), "Initial");
    let mut newer = content;
    newer.push(fx.object("mods/new.jar", b"new mod", EnvInputs::default()));
    let current = fx.save(newer, "Added");
    fx.set_current(&current.id);
    // pruned: the core doesn't know the view by its id any more
    fx.store()
        .delete_manifest(fx.uuid.as_ref(), &old.id)
        .unwrap();
    let old_view = view_of(&old);
    let ClientAnswer::View(whole) = fx.answer(&[&old_view.id]).await else {
        panic!("the whole view");
    };
    // and diffs it against the view it applied, which gives what the server's delta would
    let changes = lodestone_image::diff(&old_view, &whole.view);
    assert_eq!(apply(&old_view, &changes).unwrap().id, whole.id());
}

/// A server built on a pack, with a CurseForge file and client settings: the view keeps
/// the base and what players need of it, and only `object` entries and the stored
/// settings (icon, background) can be fetched from the server.
#[tokio::test]
async fn a_pack_servers_view_keeps_its_base_and_serves_only_its_own_bytes() {
    let fx = fixture();
    let referenced = |path: &str, source: Source, client: EnvValue| {
        let mut entry = Entry::new(
            CanonicalPath::new(path).unwrap(),
            10,
            lodestone_image::hex::sha256_hex(path.as_bytes()),
            source,
        );
        entry.env = Env {
            client,
            server: EnvValue::Required,
        };
        entry.env_from = vec![EnvFrom::BasePack];
        entry
    };
    let mut content = vec![
        referenced("mods/pack.jar", Source::Base {}, EnvValue::Required),
        referenced(
            "mods/pack-server.jar",
            Source::Base {},
            EnvValue::Unsupported,
        ),
        referenced(
            "mods/cf.jar",
            Source::Curseforge {
                project: 1,
                file: 2,
                downloadable: false,
            },
            EnvValue::Optional,
        ),
        // the pack's config, which the server changed and didn't share
        fx.object("config/pack.toml", b"changed = 1", EnvInputs::default()),
        fx.object("mods/own.jar", b"own", EnvInputs::default()),
    ];
    content.sort_by(|a, b| a.path.cmp(&b.path));
    let icon = fx.store().put_bytes(b"icon").unwrap();
    let background = fx.store().put_bytes(b"background").unwrap();
    let mut image = Image::new(
        Kind::Content,
        Game {
            minecraft: "1.20.1".into(),
            loader: Loader::Fabric,
            loader_version: Some("0.16.10".into()),
        },
    );
    image.base = Some(Base::ModrinthPack {
        project: "pack".into(),
        version: "1.0.0".into(),
        sha512: "c".repeat(128),
    });
    image.entries = content;
    image.settings.client = Some(ClientSettings {
        recommended_memory_mb: Some(6144),
        icon: Some(StoredFile {
            sha256: icon.sha256.clone(),
            size: icon.size,
        }),
        background: Some(StoredFile {
            sha256: background.sha256.clone(),
            size: background.size,
        }),
        ..Default::default()
    });
    image.seal();
    fx.store().save_manifest(fx.uuid.as_ref(), &image).unwrap();
    fx.set_current(&image.id);

    let served = fx.service.client_view(&fx.uuid).await.unwrap().unwrap();
    assert_eq!(served.view.base, image.base);
    assert_eq!(
        paths(&served.view),
        ["mods/cf.jar", "mods/own.jar", "mods/pack.jar"]
    );
    assert_eq!(served.view.settings.client, image.settings.client);
    for entry in &image.entries {
        let expected = (entry.path.as_str() == "mods/own.jar").then_some(entry.size);
        assert_eq!(
            served.object_size(&entry.sha256),
            expected,
            "{}",
            entry.path
        );
    }
    assert_eq!(served.object_size(&icon.sha256), Some(icon.size));
    assert_eq!(
        served.object_size(&background.sha256),
        Some(background.size)
    );
}

#[tokio::test]
async fn without_a_content_image_the_first_is_recorded_once() {
    let fx = fixture();
    assert!(fx.service.client_view(&fx.uuid).await.unwrap().is_none());
    assert!(fx
        .service
        .client_answer(&fx.uuid, &[])
        .await
        .unwrap()
        .is_none());
    // Docker containers and ids the store can't name aren't served
    let docker = InstanceUuid::from("DOCKER-abc".to_string());
    assert!(fx.service.client_view(&docker).await.is_err());

    // a running server, which the startup sweep leaves for its stop
    let root = fx.dir.path().join("server");
    std::fs::create_dir_all(root.join("mods")).unwrap();
    std::fs::write(root.join("mods/a.jar"), b"a mod").unwrap();
    std::fs::write(root.join("server.properties"), b"motd=Hi\n").unwrap();
    state::save(
        fx.service.root(),
        &fx.uuid,
        &ContentState {
            running_since: Some(5),
            ..ContentState::default()
        },
    )
    .unwrap();
    let ctx = InstanceContext {
        uuid: fx.uuid.clone(),
        root,
        game_version: "1.20.1".into(),
        flavour: Flavour::Fabric {
            loader_version: None,
            installer_version: None,
        },
        level_name: "world".into(),
        policy_exclude: Vec::new(),
        running: true,
    };
    // many players ask at once: one record
    for _ in 0..5 {
        assert_eq!(fx.service.record_first(ctx.clone()), FirstRecord::Recording);
    }
    crate::test_util::wait_until("the first record ends", || async {
        !fx.service.recording_first(&fx.uuid)
    })
    .await;
    let manifests = fx.store().manifest_ids(fx.uuid.as_ref()).unwrap();
    assert_eq!(manifests.len(), 1);
    let saved = state::load(fx.service.root(), &fx.uuid);
    assert_eq!(saved.current.as_deref(), Some(manifests[0].as_str()));
    // still running: the stop takes the baseline again as usual
    assert_eq!(saved.running_since, Some(5));
    let served = fx.service.client_view(&fx.uuid).await.unwrap().unwrap();
    assert_eq!(paths(&served.view), ["mods/a.jar"]);
    // with an image, asking again records nothing
    assert_eq!(fx.service.record_first(ctx), FirstRecord::Recording);
    crate::test_util::wait_until("the request ends", || async {
        !fx.service.recording_first(&fx.uuid)
    })
    .await;
    assert_eq!(fx.store().manifest_ids(fx.uuid.as_ref()).unwrap().len(), 1);
}

/// A first record that fails isn't tried again at once: players polling a server (the
/// public route needs no sign-in) can't make the core rescan it over and over, holding its
/// owner's operations off.
#[tokio::test]
async fn a_failed_first_record_waits_before_the_next() {
    let dir = tempfile::tempdir().unwrap();
    let clock = crate::images::tests::ManualClock::new();
    let service = Arc::new(ImageService::with_clock(
        dir.path().join("backups"),
        clock.clone(),
    ));
    let uuid = InstanceUuid::default();
    // the server's folder isn't there: recording fails
    let root = dir.path().join("server");
    let ctx = InstanceContext {
        uuid: uuid.clone(),
        root: root.clone(),
        game_version: "1.20.1".into(),
        flavour: Flavour::Fabric {
            loader_version: None,
            installer_version: None,
        },
        level_name: "world".into(),
        policy_exclude: Vec::new(),
        running: false,
    };
    let settled = || async { !service.recording_first(&uuid) };
    assert_eq!(service.record_first(ctx.clone()), FirstRecord::Recording);
    crate::test_util::wait_until("the first record ends", settled).await;
    assert!(service.client_view(&uuid).await.unwrap().is_none());

    // none starts again until the wait has passed
    assert_eq!(
        service.record_first(ctx.clone()),
        FirstRecord::Failed {
            retry_after: FIRST_RECORD_WAIT
        }
    );
    assert!(!service.recording_first(&uuid));
    clock.advance(FIRST_RECORD_WAIT - Duration::from_secs(1));
    assert_eq!(
        service.record_first(ctx.clone()),
        FirstRecord::Failed {
            retry_after: Duration::from_secs(1)
        }
    );
    // then one does, and failing again doubles the wait
    clock.advance(Duration::from_secs(1));
    assert_eq!(service.record_first(ctx.clone()), FirstRecord::Recording);
    crate::test_util::wait_until("the second record ends", settled).await;
    assert_eq!(
        service.record_first(ctx.clone()),
        FirstRecord::Failed {
            retry_after: FIRST_RECORD_WAIT * 2
        }
    );

    // the folder is back: after the wait the record succeeds, and the failures are forgotten
    std::fs::create_dir_all(root.join("mods")).unwrap();
    std::fs::write(root.join("mods/a.jar"), b"a mod").unwrap();
    clock.advance(FIRST_RECORD_WAIT * 2);
    assert_eq!(service.record_first(ctx), FirstRecord::Recording);
    crate::test_util::wait_until("the third record ends", settled).await;
    let served = service.client_view(&uuid).await.unwrap().unwrap();
    assert_eq!(paths(&served.view), ["mods/a.jar"]);
    assert!(locked(&service.views.first_records).is_empty());
}

#[test]
fn first_record_waits_double_up_to_a_limit() {
    assert_eq!(first_record_wait(1), FIRST_RECORD_WAIT);
    assert_eq!(first_record_wait(2), FIRST_RECORD_WAIT * 2);
    assert_eq!(first_record_wait(3), FIRST_RECORD_WAIT * 4);
    assert_eq!(first_record_wait(6), FIRST_RECORD_MAX_WAIT);
    assert_eq!(first_record_wait(u32::MAX), FIRST_RECORD_MAX_WAIT);
}

#[tokio::test]
async fn a_pointer_to_a_missing_manifest_is_no_image() {
    let fx = fixture();
    fx.set_current(&"a".repeat(64));
    assert!(fx.service.client_view(&fx.uuid).await.unwrap().is_none());
}

/// A pack of 300 mods: the view, a delta and the cached answers stay fast.
#[tokio::test]
async fn a_large_pack_is_served_fast() {
    let fx = fixture();
    let pack: Vec<Entry> = (0..300)
        .map(|n| {
            let bytes = format!("mod number {n}");
            if n % 10 == 0 {
                fx.object(
                    &format!("mods/own-{n:03}.jar"),
                    bytes.as_bytes(),
                    EnvInputs::default(),
                )
            } else {
                modrinth(
                    &format!("mods/mod-{n:03}.jar"),
                    bytes.as_bytes(),
                    "client_and_server",
                )
            }
        })
        .collect();
    let old = fx.save(pack.clone(), "Pack");
    let mut changed = pack;
    changed[5] = modrinth("mods/mod-005.jar", b"updated", "client_and_server");
    changed.push(fx.object("mods/extra.jar", b"extra", EnvInputs::default()));
    let current = fx.save(changed, "Updated");
    fx.set_current(&current.id);
    let old_view = view_of(&old).id;

    let started = Instant::now();
    let view = fx.answer(&[]).await;
    let first_view = started.elapsed();
    assert_eq!(view.current().view.entries.len(), 301);
    let started = Instant::now();
    let ClientAnswer::Delta { json, .. } = fx.answer(&[&old_view]).await else {
        panic!("a delta");
    };
    let first_delta = started.elapsed();
    assert!(json.len() < view.current().json.len() / 10);
    let started = Instant::now();
    for _ in 0..100 {
        fx.answer(&[&old_view]).await;
    }
    let cached = started.elapsed() / 100;
    println!("300 mods: view {first_view:?}, delta {first_delta:?}, cached answer {cached:?}");
    // generous: a debug build on a slow machine
    assert!(first_view + first_delta < Duration::from_secs(10));
    assert!(cached < Duration::from_millis(200));
}

#[test]
fn bounded_maps_forget_the_least_recently_used() {
    let mut map: Bounded<u32, Vec<u8>> = Bounded::new();
    let weigh: fn(&Vec<u8>) -> usize = Vec::len;
    for n in 0..4 {
        map.insert(n, vec![0; 10], 3, 1000, weigh);
    }
    assert!(map.get(&0).is_none());
    assert!(map.get(&1).is_some());
    // 1 was used last: 2 goes next
    map.insert(4, vec![0; 10], 3, 1000, weigh);
    assert!(map.get(&2).is_none());
    assert!(map.get(&1).is_some());
    // by weight too, keeping at least the newest
    map.insert(5, vec![0; 2000], 3, 1000, weigh);
    assert_eq!(map.entries.len(), 1);
    assert!(map.get(&5).is_some());
    assert_eq!(map.weight, 2000);
    map.insert(5, vec![0; 1], 3, 1000, weigh);
    assert_eq!(map.weight, 1);
}

/// A background record whose task ends without an outcome (it panicked) counts as failed,
/// so the instance isn't left "being recorded" for good; a record that stored lost objects
/// again makes the next wait however it went; and deleting the instance forgets it all.
#[tokio::test]
async fn background_records_always_settle() {
    let fx = fixture();
    let state = |fx: &Fixture| match locked(&fx.service.views.first_records).get(&fx.uuid) {
        None => None,
        Some(FirstRecordState::Running) => Some(None),
        Some(FirstRecordState::Waiting { failures, .. }) => Some(Some(*failures)),
    };
    let settle = |again: bool, outcome: Option<Result<(), Error>>| {
        locked(&fx.service.views.first_records).insert(fx.uuid.clone(), FirstRecordState::Running);
        Settle {
            service: fx.service.clone(),
            uuid: fx.uuid.clone(),
            failures: 0,
            again,
            outcome,
        }
    };
    let panicking = settle(false, None);
    let ended = tokio::spawn(async move {
        let _settle = panicking;
        panic!("a bug while recording");
    })
    .await;
    assert!(ended.is_err());
    assert_eq!(state(&fx), Some(Some(1)));
    drop(settle(false, Some(Ok(()))));
    assert_eq!(state(&fx), None);
    drop(settle(true, Some(Ok(()))));
    assert_eq!(state(&fx), Some(Some(0)));
    fx.service.views.forget(&fx.uuid);
    assert_eq!(state(&fx), None);
    // a record that settles after its instance was deleted leaves nothing behind
    let late = settle(true, Some(Ok(())));
    fx.service.views.forget(&fx.uuid);
    drop(late);
    assert_eq!(state(&fx), None);
}
