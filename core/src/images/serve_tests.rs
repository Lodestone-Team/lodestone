//! What a server serves its players, against a temporary store with content images built
//! by hand.

use std::sync::Arc;
use std::time::{Duration, Instant};

use lodestone_image::env::{self, modrinth_environment, EnvInputs, JarEnvironment, OwnerMark};
use lodestone_image::store::Store;
use lodestone_image::{
    apply, CanonicalPath, Delta, Entry, Game, Image, Kind, Loader, Side, Source,
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
    // and a scan forgets it
    assert!(matches!(
        fx.answer(&["e".repeat(64).as_str()]).await,
        ClientAnswer::View(_)
    ));
    assert!(fx
        .service
        .views
        .content_of(&fx.uuid, &view_of(&old).id)
        .is_none());
}

#[tokio::test]
async fn a_player_whose_views_were_pruned_sends_its_last_view() {
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
    assert!(matches!(
        fx.answer(&[&old_view.id]).await,
        ClientAnswer::View(_)
    ));
    // but from the view itself it makes a delta
    let answer = fx
        .service
        .client_answer_from(&fx.uuid, old_view.clone())
        .await
        .unwrap()
        .unwrap();
    let ClientAnswer::Delta { json, .. } = answer else {
        panic!("a delta");
    };
    let applied = apply(&old_view, &Delta::from_json(&json).unwrap()).unwrap();
    assert_eq!(applied.id, view_of(&current).id);
    // the current view is current
    let answer = fx
        .service
        .client_answer_from(&fx.uuid, view_of(&current))
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(answer, ClientAnswer::Current(_)));

    let refused = |image: Image| {
        let service = fx.service.clone();
        let uuid = fx.uuid.clone();
        async move {
            let Err(e) = service.client_answer_from(&uuid, image).await else {
                panic!("refused");
            };
            assert!(matches!(e.kind, ErrorKind::BadRequest), "{}", e.source);
        }
    };
    // not a view, a server view, a view whose id doesn't check out, newer rules
    refused(old.clone()).await;
    refused(lodestone_image::view(&old, Side::Server, lodestone_image::RULES).unwrap()).await;
    let mut tampered = old_view.clone();
    tampered.entries.pop();
    refused(tampered).await;
    let mut wrong_id = old_view.clone();
    wrong_id.id = "a".repeat(64);
    refused(wrong_id).await;
    // a view that carries what a client view never does (a server's config)
    let mut smuggled = old_view.clone();
    smuggled
        .entries
        .push(fx.object("config/a.toml", b"secret = 1", EnvInputs::default()));
    smuggled.entries.sort_by(|a, b| a.path.cmp(&b.path));
    smuggled.seal();
    refused(smuggled).await;
    let mut newer_rules = old_view;
    newer_rules.rules = lodestone_image::RULES + 1;
    newer_rules.seal();
    refused(newer_rules).await;
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
        fx.service.record_first(ctx.clone());
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
    fx.service.record_first(ctx);
    crate::test_util::wait_until("the request ends", || async {
        !fx.service.recording_first(&fx.uuid)
    })
    .await;
    assert_eq!(fx.store().manifest_ids(fx.uuid.as_ref()).unwrap().len(), 1);
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
