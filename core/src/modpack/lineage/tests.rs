//! The modpack history as content images, on temporary server folders and image stores:
//! installs, updates and restores through the HTTP handlers with a fake Modrinth, and the
//! restore planner on hand-made images. No server is started and nothing is downloaded
//! from the network.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use axum::extract::{Path as AxumPath, State};
use axum::Json;
use lodestone_image::{Base, CanonicalPath, Env, EnvFrom, EnvValue, Image, Kind, Source};
use serde_json::json;

use super::*;
use crate::auth::user::User;
use crate::handlers::modpack_update::{
    modpack_history, plan_update, update_modpack, ModpackUpdateRequest, UpdateSource,
};
use crate::handlers::util::AuthedUser;
use crate::images::layout::server_layout;
use crate::implementations::minecraft::MinecraftInstance;
use crate::modpack::tests::{file_entry, index, pack, Entry as ZipEntry, TestPack};
use crate::modpack::update::{
    begin_update, check_as_planned, commit_update, place_files, recover_interrupted, stage_files,
    Expected, JOURNAL_FILE,
};
use crate::modpack::{install_pack_contents, read_pack};
use crate::modrinth::fake::{sha1_hex, sha512_hex, FakeModrinth, FakeVersion};
use crate::prelude::GameInstance;

pub(crate) fn write(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

pub(crate) fn read(root: &Path, relative: &str) -> Option<Vec<u8>> {
    std::fs::read(root.join(relative)).ok()
}

fn fabric() -> Flavour {
    Flavour::Fabric {
        loader_version: Some(FabricLoaderVersion::new("0.15.11".into())),
        installer_version: None,
    }
}

fn origin(version: &str) -> ModpackOrigin {
    ModpackOrigin {
        project_id: "pack".into(),
        version_id: format!("v{version}"),
        name: "Test Pack".into(),
        version_number: Some(version.into()),
        icon_url: None,
    }
}

#[test]
fn labels() {
    assert_eq!(update_label(&origin("2")), "Updated Test Pack to 2");
    assert_eq!(install_label(&origin("1")), "Installed Test Pack 1");
    let mut unnumbered = origin("1");
    unnumbered.version_number = None;
    assert_eq!(update_label(&unnumbered), "Updated Test Pack");
    assert_eq!(
        restore_label(Some("Added JEI")),
        "Restored \u{201c}Added JEI\u{201d}"
    );
    assert_eq!(restore_label(Some("  ")), "Restored an earlier version");
    assert_eq!(restore_label(None), "Restored an earlier version");
}

#[test]
fn what_a_restore_changes() {
    for path in [
        "mods/a.jar",
        "mods/a.jar.disabled",
        "config/a.toml",
        "kubejs/startup_scripts/a.js",
        "plugins/x.jar",
        "bukkit.yml",
    ] {
        assert!(restorable(path, true), "{path}");
    }
    for path in [
        "server.properties",
        "Server.Properties",
        "ops.json",
        "whitelist.json",
        "eula.txt",
        "run.sh",
        "user_jvm_args.txt",
        "server.jar",
        "fabric-server-launch.jar",
        "libraries/net/x.jar",
        "versions/1.20.1/server.jar",
        ".lodestone_config",
        "mods/.lodestone_client_mods.json",
    ] {
        assert!(!restorable(path, true), "{path}");
    }
    // scripts only for whom a pack may write them
    assert!(restorable("kubejs/server_scripts/start.js", true));
    assert!(!restorable("kubejs/server_scripts/start.js", false));
}

// ---- the restore planner on hand-made images ----

/// A store and a server folder, with images made by hand.
struct Planner {
    _dir: tempfile::TempDir,
    root: PathBuf,
    images: Arc<ImageService>,
    store: Arc<Store>,
    uuid: InstanceUuid,
}

impl Planner {
    fn new() -> Planner {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("server");
        std::fs::create_dir_all(&root).unwrap();
        let images = Arc::new(ImageService::new(dir.path().join("backups")));
        let store = images.store();
        Planner {
            _dir: dir,
            root,
            images,
            store,
            uuid: InstanceUuid::from("INSTANCE_aaaaaaaa-2222-3333-4444-555555555555".to_string()),
        }
    }

    /// Saves a content image of `files` (path, bytes), recorded at `created` after
    /// `parent`.
    fn image(
        &self,
        files: &[(&str, &[u8])],
        created: i64,
        label: &str,
        parent: Option<&Image>,
    ) -> Image {
        self.image_with(files, created, label, parent, |_| {})
    }

    /// [`Self::image`], changed by `edit` before it is sealed.
    fn image_with(
        &self,
        files: &[(&str, &[u8])],
        created: i64,
        label: &str,
        parent: Option<&Image>,
        edit: impl FnOnce(&mut Image),
    ) -> Image {
        let mut image = Image::new(
            Kind::Content,
            crate::images::layout::game_of("1.20.1", &fabric()),
        );
        for (path, bytes) in files {
            let stored = self.store.put_bytes(bytes).unwrap();
            image.entries.push(lodestone_image::Entry::new(
                CanonicalPath::new(*path).unwrap(),
                stored.size,
                stored.sha256,
                Source::Object {},
            ));
        }
        image.created = created;
        image.label = Some(label.to_string());
        edit(&mut image);
        image.normalize();
        image.seal();
        image.parent = parent.map(|p| p.id.clone());
        self.store
            .save_manifest(self.uuid.as_ref(), &image)
            .unwrap();
        image
    }

    fn plan(&self, current: &str, target: &str, allow_scripts: bool) -> Result<RestorePlan, Error> {
        let (layout, _) = server_layout(&self.root, "world", &fabric()).unwrap();
        plan_restore(
            &self.store,
            &self.uuid,
            &self.root,
            &layout,
            current,
            target,
            allow_scripts,
        )
    }
}

fn actions(plan: &RestorePlan) -> Vec<(String, FileAction)> {
    let mut actions: Vec<(String, FileAction)> = plan
        .plan
        .changes
        .iter()
        .map(|c| (c.target.clone(), c.action))
        .collect();
    actions.sort_by(|a, b| a.0.cmp(&b.0));
    actions
}

#[test]
fn a_restore_puts_back_mods_and_configs_only() {
    let p = Planner::new();
    let old = p.image(
        &[
            ("mods/a.jar", b"a1"),
            ("mods/gone.jar", b"gone"),
            ("config/a.toml", b"old config"),
            ("kubejs/server_scripts/start.js", b"echo old"),
            ("server.properties", b"motd=old"),
            ("ops.json", b"[\"removed op\"]"),
            ("libraries/lib.jar", b"old lib"),
            ("server.jar", b"old server"),
        ],
        1,
        "Installed Test Pack 1",
        None,
    );
    let now: &[(&str, &[u8])] = &[
        ("mods/a.jar", b"a2"),
        ("mods/new.jar", b"new"),
        ("config/a.toml", b"old config"),
        ("kubejs/server_scripts/start.js", b"echo new"),
        ("server.properties", b"motd=new"),
        ("ops.json", b"[]"),
        ("libraries/lib.jar", b"new lib"),
        ("server.jar", b"new server"),
    ];
    let current = p.image(now, 2, "Updated Test Pack to 2", Some(&old));
    for (path, bytes) in now {
        write(&p.root, path, bytes);
    }
    // a file no image has (left out by the backup policy, say) is never touched
    write(&p.root, "config/excluded.toml", b"mine");

    let plan = p.plan(&current.id, &old.id, false).unwrap();
    assert_eq!(
        actions(&plan),
        [
            ("mods/a.jar".to_string(), FileAction::Replace),
            ("mods/gone.jar".to_string(), FileAction::Add),
            ("mods/new.jar".to_string(), FileAction::Remove),
        ]
    );
    assert!(plan.missing.is_empty() && plan.blocked.is_empty());
    assert_eq!(plan.target.id, old.id);
    assert_eq!(plan.target.label.as_deref(), Some("Installed Test Pack 1"));
    // mods and configs; not the script (without the right to), the settings, the admin
    // lists or the server software
    assert_eq!(plan.file_count, 3);
    assert!(matches!(
        plan.sources["mods/gone.jar"],
        FileSource::Image(ImageFile { size: 4, .. })
    ));

    // with the right to write scripts, a script goes back too
    let plan = p.plan(&current.id, &old.id, true).unwrap();
    assert!(actions(&plan).contains(&(
        "kubejs/server_scripts/start.js".to_string(),
        FileAction::Replace
    )));

    // a file that is already gone isn't removed; one that is there that the current image
    // has no copy of is never replaced: the restore is refused instead
    std::fs::remove_file(p.root.join("mods/new.jar")).unwrap();
    write(&p.root, "mods/gone.jar", b"something else");
    let plan = p.plan(&current.id, &old.id, false).unwrap();
    assert_eq!(
        actions(&plan),
        [("mods/a.jar".to_string(), FileAction::Replace)]
    );
    assert_eq!(plan.unrecorded, ["mods/gone.jar"]);
    // each file it replaces must still be as the current image has it when it is placed
    assert_eq!(
        plan.plan.changes[0].expected,
        Expected::Sha256(lodestone_image::hex::sha256_hex(b"a2"))
    );
    check_as_planned(&p.root, &plan.plan).unwrap();
    write(&p.root, "mods/a.jar", b"edited meanwhile");
    let error = check_as_planned(&p.root, &plan.plan).unwrap_err();
    assert!(matches!(error.kind, ErrorKind::Conflict));
    write(&p.root, "mods/a.jar", b"a2");
    // and nothing may have appeared where it adds a file
    std::fs::remove_file(p.root.join("mods/gone.jar")).unwrap();
    let plan = p.plan(&current.id, &old.id, false).unwrap();
    let added = plan
        .plan
        .changes
        .iter()
        .find(|c| c.target == "mods/gone.jar")
        .unwrap();
    assert_eq!(added.expected, Expected::Nothing);
    check_as_planned(&p.root, &plan.plan).unwrap();
    write(&p.root, "mods/gone.jar", b"uploaded meanwhile");
    assert!(check_as_planned(&p.root, &plan.plan).is_err());

    // a folder where a file goes is never moved
    std::fs::remove_file(p.root.join("mods/gone.jar")).unwrap();
    std::fs::create_dir_all(p.root.join("mods/gone.jar/inside")).unwrap();
    let plan = p.plan(&current.id, &old.id, false).unwrap();
    assert_eq!(plan.blocked, ["mods/gone.jar"]);
    assert!(!actions(&plan)
        .iter()
        .any(|(path, _)| path == "mods/gone.jar"));
}

#[test]
fn a_restore_keeps_what_the_target_left_out_or_withheld() {
    use lodestone_image::manifest::{Withheld, WithheldReason};
    let p = Planner::new();
    // The target left config/keep/ out (the policy excluded it then), couldn't read
    // config/secret.toml, and couldn't list config/locked/.
    let target = p.image_with(&[("mods/a.jar", b"a")], 1, "Target", None, |image| {
        image.excludes = vec!["config/keep/".into()];
        image.withheld = vec![
            Withheld {
                path: "config/secret.toml".into(),
                reason: WithheldReason::Unreadable,
            },
            Withheld {
                path: "config/locked".into(),
                reason: WithheldReason::Unreadable,
            },
        ];
    });
    let files: [(&str, &[u8]); 5] = [
        ("mods/a.jar", b"a"),
        ("config/keep/user.toml", b"user"),
        ("config/secret.toml", b"secret"),
        ("config/locked/x.toml", b"x"),
        ("config/other.toml", b"other"),
    ];
    let current = p.image(&files, 2, "Current", Some(&target));
    for (path, bytes) in files {
        write(&p.root, path, bytes);
    }
    let plan = p.plan(&current.id, &target.id, true).unwrap();
    // Only the file the target had no copy of on purpose is left; config/other.toml it
    // simply didn't have.
    assert_eq!(
        actions(&plan),
        vec![("config/other.toml".to_string(), FileAction::Remove)]
    );
}

#[test]
fn a_restore_needs_the_files_or_their_downloads() {
    let p = Planner::new();
    let old = p.image(&[("mods/a.jar", b"a1")], 1, "old", None);
    let current = p.image(&[], 2, "now", Some(&old));
    // the store lost the object
    let object = p
        .store
        .root()
        .join("objects")
        .join(&old.entries[0].sha256[..2])
        .join(&old.entries[0].sha256);
    let mut permissions = std::fs::metadata(&object).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    std::fs::set_permissions(&object, permissions).unwrap();
    std::fs::remove_file(&object).unwrap();
    let plan = p.plan(&current.id, &old.id, false).unwrap();
    assert_eq!(plan.missing, ["mods/a.jar"]);
    assert!(plan.plan.changes.is_empty());
    let history = history(&p.images, &p.uuid, None);
    let entry = history.entries.iter().find(|e| e.id == old.id).unwrap();
    assert_eq!(entry.missing, 1);

    // a Modrinth file with its hashes is downloaded again
    let mut image = old.clone();
    image.entries[0].source = Source::Modrinth {
        project: "p".into(),
        version: "v".into(),
        url: "https://cdn.modrinth.com/data/p/versions/v/a.jar".into(),
    };
    image.entries[0].sha1 = Some(sha1_hex(b"a1"));
    image.entries[0].sha512 = Some(sha512_hex(b"a1"));
    image.created = 3;
    image.seal();
    p.store.save_manifest(p.uuid.as_ref(), &image).unwrap();
    let plan = p.plan(&current.id, &image.id, false).unwrap();
    assert!(plan.missing.is_empty());
    assert!(matches!(&plan.sources["mods/a.jar"], FileSource::Download(d) if d.size == 2));
    assert_eq!(plan.download_size, 2);
}

#[test]
fn only_content_images_in_the_history_restore() {
    let p = Planner::new();
    let current = p.image(&[("mods/a.jar", b"a")], 1, "now", None);
    let mut data = Image::new(Kind::Data, current.game.clone());
    data.content = Some(current.id.clone());
    data.seal();
    p.store.save_manifest(p.uuid.as_ref(), &data).unwrap();
    for target in [data.id.as_str(), &"0".repeat(64), "../../etc", ""] {
        let error = p.plan(&current.id, target, false).unwrap_err();
        assert!(matches!(error.kind, ErrorKind::NotFound), "{target}");
    }
}

#[test]
fn the_history_lists_content_images_newest_first() {
    let p = Planner::new();
    let first = p.image(
        &[
            ("mods/a-1.jar", b"a1"),
            ("mods/keep.jar", b"keep"),
            ("config/x.toml", b"x1"),
        ],
        1,
        "Installed Test Pack 1",
        None,
    );
    let mut second = Image::new(Kind::Content, first.game.clone());
    second.entries = first.entries.clone();
    // the mod disabled: same contents, another name
    for entry in &mut second.entries {
        if entry.path.as_str() == "mods/a-1.jar" {
            entry.path = CanonicalPath::new("mods/a-1.jar.disabled").unwrap();
        }
    }
    second.created = 2;
    second.label = Some("Disabled a-1.jar".into());
    second.parent = Some(first.id.clone());
    second.normalize();
    second.seal();
    p.store.save_manifest(p.uuid.as_ref(), &second).unwrap();
    let third = p.image(
        &[
            ("mods/a-2.jar", b"a2"),
            ("mods/keep.jar", b"keep"),
            ("config/x.toml", b"x2"),
            ("server.properties", b"motd=x"),
        ],
        3,
        "Updated Test Pack to 2",
        Some(&second),
    );
    // backups are no entries of their own
    let mut data = Image::new(Kind::Data, first.game.clone());
    data.content = Some(first.id.clone());
    data.created = 4;
    data.seal();
    p.store.save_manifest(p.uuid.as_ref(), &data).unwrap();
    crate::images::state::save(
        p.store.root(),
        &p.uuid,
        &crate::images::state::ContentState {
            current: Some(third.id.clone()),
            ..Default::default()
        },
    )
    .unwrap();

    let history = history(&p.images, &p.uuid, None);
    let ids: Vec<&str> = history.entries.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, [&third.id, &second.id, &first.id]);
    let labels: Vec<&str> = history.entries.iter().map(|e| e.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Updated Test Pack to 2",
            "Disabled a-1.jar",
            "Installed Test Pack 1"
        ]
    );
    assert!(history.entries[0].current);
    assert!(!history.entries[1].current);
    assert_eq!(history.entries[0].time, 3000);
    assert_eq!(history.entries[0].loader, "fabric");
    assert_eq!(
        history.entries[0].loader_version.as_deref(),
        Some("0.15.11")
    );
    // server.properties isn't what a restore puts back
    assert_eq!(history.entries[0].file_count, 3);

    // the first image changed nothing that came before it
    assert!(history.entries[2].changes.is_empty());
    // disabling a mod is an update of its name
    assert_eq!(
        history.entries[1].changes,
        [ChangeInfo {
            kind: ChangeKind::Updated,
            path: "mods/a-1.jar.disabled".into(),
            old_path: Some("mods/a-1.jar".into()),
            project_id: None,
        }]
    );
    // the update, from the image it was recorded after
    let kinds: Vec<(ChangeKind, &str)> = history.entries[0]
        .changes
        .iter()
        .map(|c| (c.kind, c.path.as_str()))
        .collect();
    assert_eq!(
        kinds,
        [
            (ChangeKind::Added, "mods/a-2.jar"),
            (ChangeKind::Replaced, "config/x.toml"),
            (ChangeKind::Removed, "mods/a-1.jar.disabled"),
        ]
    );

    // each current file came with the oldest image of the newest run that had it
    assert_eq!(history.blame["mods/keep.jar"], first.id);
    assert_eq!(history.blame["mods/a-2.jar"], third.id);
    assert_eq!(history.blame["config/x.toml"], third.id);
    assert!(!history.blame.contains_key("server.properties"));

    // a manifest that can't be read is left out
    let damaged = p
        .store
        .manifest_path(p.uuid.as_ref(), &"1".repeat(64))
        .unwrap();
    std::fs::write(damaged, b"{").unwrap();
    assert_eq!(history_len(&p), 3);
    // a server the store can't name has none
    assert!(
        super::history(&p.images, &InstanceUuid::from("a/b".to_string()), None)
            .entries
            .is_empty()
    );
}

fn history_len(p: &Planner) -> usize {
    history(&p.images, &p.uuid, None).entries.len()
}

#[tokio::test(flavor = "multi_thread")]
async fn retention_keeps_what_a_restore_goes_back_to() {
    let dir = tempfile::tempdir().unwrap();
    let images = Arc::new(ImageService::new(dir.path().join("backups")));
    let uuid = InstanceUuid::from("INSTANCE_bbbbbbbb-2222-3333-4444-555555555555".to_string());
    let store = images.store();
    let mut ids = Vec::new();
    for n in 0..35 {
        let mut image = Image::new(
            Kind::Content,
            lodestone_image::Game {
                minecraft: format!("1.{n}"),
                loader: lodestone_image::Loader::Vanilla,
                loader_version: None,
            },
        );
        image.created = 1_000 + n;
        image.seal();
        store.save_manifest(uuid.as_ref(), &image).unwrap();
        ids.push(image.id);
    }
    let restoring = images.keep_while_restoring(&uuid, &ids[0]);
    // another server's restore keeps nothing here
    let _other =
        images.keep_while_restoring(&InstanceUuid::from("INSTANCE_other".to_string()), &ids[1]);
    assert_eq!(images.prune(&uuid).await.unwrap(), 4);
    let kept: Vec<String> = store
        .list_manifests(uuid.as_ref())
        .unwrap()
        .images
        .into_iter()
        .map(|i| i.id)
        .collect();
    assert!(kept.contains(&ids[0]));
    assert!(!kept.contains(&ids[1]));
    drop(restoring);
    assert_eq!(images.prune(&uuid).await.unwrap(), 1);
    assert!(store.load_manifest(uuid.as_ref(), &ids[0]).is_err());
}

/// A restore the core stopped in the middle of is undone at the next start, and one that
/// committed keeps its files: the journal works the same for files from the image store.
#[tokio::test(flavor = "multi_thread")]
async fn an_interrupted_restore_is_undone() {
    let p = Planner::new();
    let old = p.image(
        &[("mods/a.jar", b"a1"), ("config/c.toml", b"c1")],
        1,
        "old",
        None,
    );
    let now: &[(&str, &[u8])] = &[("mods/a.jar", b"a2"), ("mods/new.jar", b"new")];
    let current = p.image(now, 2, "now", Some(&old));
    for (path, bytes) in now {
        write(&p.root, path, bytes);
    }
    let plan = p.plan(&current.id, &old.id, false).unwrap();
    let api = FakeModrinth::default();
    for commit in [false, true] {
        stage_files(
            &api,
            None,
            &plan.sources,
            &plan.plan,
            &p.root,
            false,
            Some(p.store.clone()),
            &|_| {},
        )
        .await
        .unwrap();
        begin_update(&p.root, &plan.plan).unwrap();
        place_files(&p.root, &plan.plan).unwrap();
        assert_eq!(read(&p.root, "mods/a.jar").unwrap(), b"a1");
        assert_eq!(read(&p.root, "config/c.toml").unwrap(), b"c1");
        assert!(read(&p.root, "mods/new.jar").is_none());
        if commit {
            commit_update(&p.root).unwrap();
        }
        // the core stops here
        assert!(recover_interrupted(&p.root));
        assert!(!p.root.join(JOURNAL_FILE).exists());
        if commit {
            assert_eq!(read(&p.root, "mods/a.jar").unwrap(), b"a1");
            assert!(read(&p.root, "mods/new.jar").is_none());
        } else {
            assert_eq!(read(&p.root, "mods/a.jar").unwrap(), b"a2");
            assert_eq!(read(&p.root, "mods/new.jar").unwrap(), b"new");
            assert!(read(&p.root, "config/c.toml").is_none());
        }
    }
}

/// Staging copies out of the image store only what checks out: a damaged object fails the
/// restore before anything moves, and leaves no half-written file.
#[tokio::test(flavor = "multi_thread")]
async fn a_damaged_object_stops_the_restore_before_it_moves_anything() {
    let p = Planner::new();
    let old = p.image(&[("mods/a.jar", b"a1")], 1, "old", None);
    let current = p.image(&[], 2, "now", Some(&old));
    let plan = p.plan(&current.id, &old.id, false).unwrap();
    // the plan saw the object, then a bit of it rotted
    let sha256 = &old.entries[0].sha256;
    let object = p
        .store
        .root()
        .join("objects")
        .join(&sha256[..2])
        .join(sha256);
    let mut bytes = std::fs::read(&object).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    let mut permissions = std::fs::metadata(&object).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    std::fs::set_permissions(&object, permissions).unwrap();
    std::fs::write(&object, bytes).unwrap();
    let api = FakeModrinth::default();
    assert!(stage_files(
        &api,
        None,
        &plan.sources,
        &plan.plan,
        &p.root,
        false,
        Some(p.store.clone()),
        &|_| {},
    )
    .await
    .is_err());
    assert!(read(&p.root, "mods/a.jar").is_none());
    assert!(read(
        &p.root,
        &format!("{}/mods/a.jar", crate::modpack::update::STAGING_DIR)
    )
    .is_none());
}

// ---- through the handlers ----

const CDN: &str = "https://cdn.modrinth.com/data";

/// Version `version` of a pack: mods from the CDN (version 2's `new.jar` is for servers only,
/// says the index) and configs in its overrides (version 2 has one in `server-overrides/`).
fn test_pack(version: u8, fake: &FakeModrinth) -> TestPack {
    let mut files = vec![];
    let mut add = |path: &str, content: &[u8], project: &str, env: Option<serde_json::Value>| {
        let url = format!(
            "{CDN}/{project}/versions/v{version}/{}",
            path.rsplit('/').next().unwrap()
        );
        let mut entry = file_entry(path, content, None, &url);
        if let Some(env) = env {
            entry["env"] = env;
        }
        files.push(entry);
        fake.add_file(&url, content.to_vec());
    };
    add("mods/keep.jar", b"keep", "k", None);
    let config: &[u8] = match version {
        1 => {
            add("mods/a-1.jar", b"a1", "a", None);
            add("mods/dropped.jar", b"dropped", "d", None);
            b"v1"
        }
        2 => {
            add("mods/a-2.jar", b"a2", "a", None);
            add(
                "mods/new.jar",
                b"new",
                "n",
                Some(json!({"client": "unsupported", "server": "required"})),
            );
            b"v2"
        }
        _ => {
            // a file no one can download: the update fails
            files.push(file_entry(
                "mods/gone.jar",
                b"gone",
                None,
                &format!("{CDN}/g/versions/v3/gone.jar"),
            ));
            b"v3"
        }
    };
    let mut entries = vec![
        ZipEntry::File("overrides/config/pack.toml", config),
        ZipEntry::File("overrides/config/edited.toml", config),
    ];
    if version == 2 {
        entries.push(ZipEntry::File(
            "overrides/config/only-v2.toml",
            b"only in v2",
        ));
        entries.push(ZipEntry::File(
            "server-overrides/config/server-only.toml",
            b"server only",
        ));
    }
    pack(
        index(
            files,
            json!({"minecraft": "1.20.1", "fabric-loader": "0.15.11"}),
        ),
        &entries,
    )
}

/// A core with a fake Modrinth and one stopped Fabric server made from version 1 of the
/// test pack, as `create_modpack` makes it: its files, `pack.json` and first image.
pub(crate) struct Core {
    pub(crate) _dir: tempfile::TempDir,
    pub(crate) state: crate::AppState,
    pub(crate) uuid: InstanceUuid,
    pub(crate) root: PathBuf,
    packs: Vec<TestPack>,
    pub(crate) title: String,
}

pub(crate) fn owner() -> AuthedUser {
    AuthedUser(User::new(
        "owner".to_string(),
        "pw",
        true,
        true,
        Default::default(),
    ))
}

impl Core {
    pub(crate) async fn new() -> Core {
        static LODESTONE: OnceLock<tempfile::TempDir> = OnceLock::new();
        let lodestone = LODESTONE.get_or_init(|| tempfile::tempdir().unwrap());
        crate::prelude::try_init_paths(lodestone.path().to_owned()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut state = crate::test_util::test_app_state(dir.path(), Default::default()).await;
        let fake = Arc::new(FakeModrinth::default());
        state.mods = Arc::new(crate::mods::ModsState::new(
            fake.clone(),
            crate::modpack::store::PackStore::new(dir.path().join("modpacks")),
        ));
        let mut packs = Vec::new();
        for version in 1..=3u8 {
            let id = format!("v{version}");
            fake.add(FakeVersion::new("pack", &id, "pack.mrpack"));
            let test_pack = test_pack(version, &fake);
            let temp = state.mods.packs.new_file().unwrap();
            std::fs::copy(&test_pack.path, &temp).unwrap();
            let info = read_pack(&test_pack.path, false).unwrap().info;
            state.mods.packs.insert(temp, info, Some(id), None);
            packs.push(test_pack);
        }
        let title = crate::modrinth::ModrinthApi::project(fake.as_ref(), "pack")
            .await
            .unwrap()
            .title;
        let origin = ModpackOrigin {
            project_id: "pack".into(),
            version_id: "v1".into(),
            name: title.clone(),
            version_number: Some("v1-number".into()),
            icon_url: None,
        };

        let root = dir.path().join("instance");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join(".lodestone_minecraft_config.json"),
            json!({
                "name": "Packed", "version": "1.20.1",
                "flavour": {"fabric": {"loader_version": "0.15.11", "installer_version": null}},
                "description": "", "cmd_args": [], "java_cmd": root.join("no-java-here"),
                "port": 25599, "min_ram": 1024, "max_ram": 2048, "auto_start": false,
                "restart_on_crash": false, "backup_period": null, "jre_major_version": 17,
                "has_started": false, "modpack": origin,
            })
            .to_string(),
        )
        .unwrap();
        write(&root, "server.properties", b"server-port=25599\n");
        let v1 = read_pack(&packs[0].path, false).unwrap();
        let report =
            install_pack_contents(fake.as_ref(), &v1, &packs[0].path, &root, false, &|_, _| {})
                .await
                .unwrap();
        let uuid = InstanceUuid::default();
        record_install(
            &state,
            &uuid,
            &root,
            &origin,
            &packs[0].path,
            &report.files,
            "1.20.1",
            &fabric(),
        )
        .await
        .expect("the install records an image");
        let instance = MinecraftInstance::restore(
            root.clone(),
            crate::types::DotLodestoneConfig::new(
                uuid.clone(),
                crate::traits::t_configurable::GameType::MinecraftJava,
            ),
            state.event_broadcaster.clone(),
            crate::global_settings::CodeExecutionSwitch::new(false),
        )
        .await
        .unwrap();
        state
            .instances
            .insert(uuid.clone(), GameInstance::MinecraftInstance(instance));
        Core {
            _dir: dir,
            state,
            uuid,
            root,
            packs,
            title,
        }
    }

    pub(crate) async fn change(&self, source: UpdateSource) -> Result<(), Error> {
        update_modpack(
            State(self.state.clone()),
            AxumPath(self.uuid.clone()),
            owner(),
            Json(ModpackUpdateRequest { source }),
        )
        .await
        .map(|_| ())
    }

    pub(crate) async fn restore(&self, id: &str) -> Result<(), Error> {
        self.change(UpdateSource::History {
            entry_id: id.to_string(),
        })
        .await
    }

    pub(crate) async fn history(&self) -> ModpackHistory {
        modpack_history(
            State(self.state.clone()),
            AxumPath(self.uuid.clone()),
            owner(),
        )
        .await
        .unwrap()
        .0
    }

    pub(crate) fn image(&self, id: &str) -> Image {
        self.state
            .images()
            .store()
            .load_manifest(self.uuid.as_ref(), id)
            .unwrap()
    }

    pub(crate) async fn current(&self) -> String {
        self.state
            .images()
            .current_content(&self.uuid)
            .await
            .expect("a current image")
    }

    pub(crate) fn pack_json(&self) -> crate::images::base::BaseInfo {
        crate::images::base::load_base(self.state.images().root(), &self.uuid).unwrap()
    }

    pub(crate) async fn origin(&self) -> ModpackOrigin {
        let instance = self
            .state
            .instances
            .get(&self.uuid)
            .unwrap()
            .value()
            .clone();
        let GameInstance::MinecraftInstance(instance) = instance;
        instance.modpack_origin().await.unwrap()
    }

    /// A Mods-tab-like change: an operation that writes a file and succeeds.
    pub(crate) async fn content_change(&self, path: &str, bytes: &[u8], label: &str) -> String {
        let images = self.state.images();
        let op = images.begin(&self.uuid, label).await;
        write(&self.root, path, bytes);
        let ctx = crate::images::context_of(&self.state, &self.uuid)
            .await
            .unwrap();
        op.succeed_and_record(ctx).await.unwrap().unwrap().id
    }
}

fn entry<'a>(image: &'a Image, path: &str) -> &'a lodestone_image::Entry {
    image
        .entries
        .iter()
        .find(|e| e.path.as_str() == path)
        .unwrap_or_else(|| panic!("{path} in {:?}", image.label))
}

#[tokio::test(flavor = "multi_thread")]
async fn installs_updates_and_restores_are_the_history() {
    let core = Core::new().await;
    let title = core.title.clone();

    // ---- the install is the first entry, on the pack ----
    let installed = core.current().await;
    let history = core.history().await;
    assert_eq!(history.entries.len(), 1);
    assert_eq!(history.entries[0].id, installed);
    assert_eq!(
        history.entries[0].label,
        format!("Installed {title} v1-number")
    );
    assert!(history.entries[0].current);
    let record = crate::modpack::pack_info::record_of_base(&core.pack_json());
    assert!(record.files.contains_key("mods/a-1.jar"));
    assert!(record.files["config/pack.toml"].from_overrides);
    assert!(!core.root.join(".lodestone_modpack").exists());

    // ---- a Mods-tab change is an entry too ----
    let added = core
        .content_change("mods/extra.jar", b"extra", "Added extra.jar")
        .await;
    let history = core.history().await;
    assert_eq!(history.entries.len(), 2);
    assert_eq!(history.entries[0].id, added);
    assert_eq!(history.entries[0].changes.len(), 1);
    assert_eq!(history.entries[0].changes[0].kind, ChangeKind::Added);

    // ---- an update records one image of the whole server, on the new pack ----
    core.change(UpdateSource::Modrinth {
        version_id: "v2".into(),
    })
    .await
    .unwrap();
    let updated = core.current().await;
    let image = core.image(&updated);
    assert_eq!(
        image.label.as_deref(),
        Some(format!("Updated {title} to v2-number").as_str())
    );
    assert_eq!(image.parent.as_deref(), Some(added.as_str()));
    let pack_sha512 = sha512_hex(&std::fs::read(&core.packs[1].path).unwrap());
    assert_eq!(
        image.base,
        Some(Base::ModrinthPack {
            project: "pack".into(),
            version: "v2".into(),
            sha512: pack_sha512,
        })
    );
    let config = entry(&image, "config/pack.toml");
    assert_eq!(config.source, Source::Base {});
    assert_eq!(config.env, Env::new(EnvValue::Required, EnvValue::Required));
    assert_eq!(config.env_from, [EnvFrom::BasePack]);
    assert_eq!(
        entry(&image, "config/server-only.toml").env.client,
        EnvValue::Unsupported
    );
    let new = entry(&image, "mods/new.jar");
    assert!(matches!(&new.source, Source::Modrinth { project, .. } if project == "n"));
    assert_eq!(new.env, Env::new(EnvValue::Unsupported, EnvValue::Required));
    // pack.json now names version 2's files; the user's own mod is no pack file
    let record = crate::modpack::pack_info::record_of_base(&core.pack_json());
    assert!(record.files.contains_key("mods/new.jar"));
    assert!(!record.files.contains_key("mods/a-1.jar"));
    assert!(!record.files.contains_key("mods/extra.jar"));
    assert_eq!(read(&core.root, "mods/extra.jar").unwrap(), b"extra");
    assert_eq!(core.origin().await.version_id, "v2");
    let history = core.history().await;
    assert_eq!(history.entries.len(), 3);
    let changes: Vec<(ChangeKind, &str)> = history.entries[0]
        .changes
        .iter()
        .map(|c| (c.kind, c.path.as_str()))
        .collect();
    assert!(changes.contains(&(ChangeKind::Updated, "mods/a-2.jar")));
    assert!(changes.contains(&(ChangeKind::Removed, "mods/dropped.jar")));
    assert!(changes.contains(&(ChangeKind::Replaced, "config/pack.toml")));
    assert_eq!(history.blame["mods/keep.jar"], installed);
    assert_eq!(history.blame["mods/new.jar"], updated);

    // ---- a failed update records nothing and changes nothing ----
    assert!(core
        .change(UpdateSource::Modrinth {
            version_id: "v3".into(),
        })
        .await
        .is_err());
    assert_eq!(core.history().await.entries.len(), 3);
    assert_eq!(core.current().await, updated);
    assert_eq!(read(&core.root, "mods/new.jar").unwrap(), b"new");
    assert!(!core.root.join(JOURNAL_FILE).exists());

    // ---- what changed outside Lodestone is recorded before a restore ----
    write(&core.root, "config/outside.toml", b"by hand");
    write(
        &core.root,
        "server.properties",
        b"server-port=25599\nmotd=later\n",
    );
    let plan = plan_update(
        State(core.state.clone()),
        AxumPath(core.uuid.clone()),
        owner(),
        Json(ModpackUpdateRequest {
            source: UpdateSource::History {
                entry_id: installed.clone(),
            },
        }),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(plan.target.unsupported_reason, None);
    assert_eq!(plan.target.name, format!("Installed {title} v1-number"));
    assert!(!plan.reinstalls_server);
    assert!(plan.removed.contains(&"mods/new.jar".to_string()));
    // planning records nothing
    assert_eq!(core.history().await.entries.len(), 3);

    core.restore(&installed).await.unwrap();
    assert_eq!(read(&core.root, "config/pack.toml").unwrap(), b"v1");
    assert_eq!(read(&core.root, "mods/a-1.jar").unwrap(), b"a1");
    assert_eq!(read(&core.root, "mods/dropped.jar").unwrap(), b"dropped");
    assert!(read(&core.root, "mods/new.jar").is_none());
    assert!(read(&core.root, "mods/extra.jar").is_none());
    assert!(read(&core.root, "config/outside.toml").is_none());
    // the server's own settings stay as they are
    assert_eq!(
        read(&core.root, "server.properties").unwrap(),
        b"server-port=25599\nmotd=later\n"
    );
    let history = core.history().await;
    let labels: Vec<&str> = history.entries.iter().map(|e| e.label.as_str()).collect();
    assert_eq!(
        labels[..2],
        [
            format!("Restored \u{201c}Installed {title} v1-number\u{201d}").as_str(),
            "Changed outside Lodestone",
        ]
    );
    let outside = history.entries[1].id.clone();
    let restored = core.image(&history.entries[0].id);
    assert_eq!(restored.parent.as_deref(), Some(outside.as_str()));
    // the restored version's pack is the base again, with its files
    assert!(matches!(
        &restored.base,
        Some(Base::ModrinthPack { version, .. }) if version == "v1"
    ));
    assert_eq!(entry(&restored, "config/pack.toml").source, Source::Base {});
    let record = crate::modpack::pack_info::record_of_base(&core.pack_json());
    assert!(record.files.contains_key("mods/a-1.jar"));
    assert_eq!(core.origin().await.version_id, "v1");

    // ---- restoring the image from before the restore undoes it ----
    core.restore(&outside).await.unwrap();
    assert_eq!(read(&core.root, "config/outside.toml").unwrap(), b"by hand");
    assert_eq!(read(&core.root, "mods/new.jar").unwrap(), b"new");
    assert_eq!(read(&core.root, "mods/extra.jar").unwrap(), b"extra");
    assert_eq!(core.origin().await.version_id, "v2");
    // back at the same contents: the same image again, as the current one
    assert_eq!(core.current().await, outside);

    // ---- the current entry can't be restored, nor one that's gone ----
    let error = core.restore(&outside).await.unwrap_err();
    assert!(matches!(error.kind, ErrorKind::BadRequest));
    let error = core.restore(&"0".repeat(64)).await.unwrap_err();
    assert!(matches!(error.kind, ErrorKind::NotFound));

    // ---- the History API's shape ----
    let json = serde_json::to_value(core.history().await).unwrap();
    let mut keys: Vec<&String> = json.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["blame", "entries", "update"]);
    let mut keys: Vec<&String> = json["entries"][0].as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "changes",
            "current",
            "file_count",
            "game_version",
            "id",
            "label",
            "loader",
            "loader_version",
            "missing",
            "time"
        ]
    );
}

/// A restore and a Mods-tab change at the same time: the change waits for the restore's
/// operation to end or goes first, each is recorded once, and no manifest is damaged.
#[tokio::test(flavor = "multi_thread")]
async fn a_restore_and_a_content_change_at_once() {
    let core = Arc::new(Core::new().await);
    let installed = core.current().await;
    core.change(UpdateSource::Modrinth {
        version_id: "v2".into(),
    })
    .await
    .unwrap();
    let restore = {
        let core = core.clone();
        let installed = installed.clone();
        tokio::spawn(async move { core.restore(&installed).await })
    };
    let change = {
        let core = core.clone();
        tokio::spawn(async move {
            core.content_change("config/meanwhile.toml", b"meanwhile", "Edited meanwhile")
                .await
        })
    };
    restore.await.unwrap().unwrap();
    change.await.unwrap();
    // a collection now finds every manifest readable and keeps what they name
    let images = core.state.images().clone();
    tokio::task::spawn_blocking(move || images.collect_blocking())
        .await
        .unwrap()
        .unwrap();
    let history = core.history().await;
    assert!(history
        .entries
        .iter()
        .any(|e| e.label == "Edited meanwhile"));
    assert!(history
        .entries
        .iter()
        .any(|e| e.label.starts_with("Restored")));
    // whichever went first, the current image is what is on disk: no change was lost
    let ctx = crate::images::context_of(&core.state, &core.uuid)
        .await
        .unwrap();
    let recorded = core
        .state
        .images()
        .record_now(ctx, "check".into())
        .await
        .unwrap()
        .unwrap();
    assert!(!recorded.changed);
    // every entry can still be restored: the store kept their files
    assert!(history.entries.iter().all(|e| e.missing == 0));
}

/// A file the record before a restore couldn't read (so no image has it) is never replaced:
/// the restore is refused, and the file stays as the user left it. The record before a
/// restore failing refuses it too.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_restore_never_replaces_a_file_no_image_has() {
    use std::os::unix::fs::PermissionsExt;
    let core = Core::new().await;
    let first = core
        .content_change("config/probe.toml", b"A", "Set A")
        .await;
    core.content_change("config/probe.toml", b"B", "Set B")
        .await;
    let probe = core.root.join("config/probe.toml");
    write(&core.root, "config/probe.toml", b"USER-EDIT");
    std::fs::set_permissions(&probe, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read(&probe).is_ok() {
        // running as root: the file can't be made unreadable
        std::fs::set_permissions(&probe, std::fs::Permissions::from_mode(0o644)).unwrap();
        return;
    }
    let refused = core.restore(&first).await;
    std::fs::set_permissions(&probe, std::fs::Permissions::from_mode(0o644)).unwrap();
    let error = refused.unwrap_err();
    assert!(
        error.source.to_string().contains("no copy"),
        "{}",
        error.source
    );
    assert_eq!(std::fs::read(&probe).unwrap(), b"USER-EDIT");
    // readable again, the edit is recorded first and the restore goes ahead
    core.restore(&first).await.unwrap();
    assert_eq!(std::fs::read(&probe).unwrap(), b"A");
    // the image before the restore's: the file, now readable, wasn't changed since the
    // record that withheld it, so it isn't called a change outside Lodestone
    let history = core.history().await;
    assert!(history.entries[0].label.starts_with("Restored"));
    let edited = &history.entries[1];
    assert_eq!(edited.label, BEFORE_RESTORE_LABEL);
    assert_eq!(history.entries[2].label, "Changed outside Lodestone");
    let edit = core.image(&edited.id);
    let entry = entry(&edit, "config/probe.toml");
    assert!(core
        .state
        .images()
        .store()
        .object_complete(&entry.sha256, entry.size));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_server_records_its_first_image() {
    let dir = tempfile::tempdir().unwrap();
    let state = crate::test_util::test_app_state(dir.path(), Default::default()).await;
    let fake = FakeModrinth::default();
    let v2 = test_pack(2, &fake);
    let pack = read_pack(&v2.path, false).unwrap();
    let root = dir.path().join("new-server");
    std::fs::create_dir_all(&root).unwrap();
    let report = install_pack_contents(&fake, &pack, &v2.path, &root, false, &|_, _| {})
        .await
        .unwrap();
    let uuid = InstanceUuid::default();
    let image = record_install(
        &state,
        &uuid,
        &root,
        &origin("2"),
        &v2.path,
        &report.files,
        "1.20.1",
        &fabric(),
    )
    .await
    .expect("recorded");
    assert_eq!(
        state.images().current_content(&uuid).await.as_deref(),
        Some(image.as_str())
    );
    let manifest = state
        .images()
        .store()
        .load_manifest(uuid.as_ref(), &image)
        .unwrap();
    assert_eq!(manifest.label.as_deref(), Some("Installed Test Pack 2"));
    assert!(matches!(
        &manifest.base,
        Some(Base::ModrinthPack { version, .. }) if version == "v2"
    ));
    assert_eq!(
        entry(&manifest, "mods/new.jar").env,
        Env::new(EnvValue::Unsupported, EnvValue::Required)
    );
    assert_eq!(manifest.parent, None);
    // The first record matched the pack's files by SHA-1 and noted their SHA-256, so the
    // next ones read nothing; pack.json lists each file once.
    let info = crate::images::base::load_base(state.images().root(), &uuid).unwrap();
    assert!(!info.files.contains_key("mods/new.jar.disabled"));
    let new_jar = &info.files["mods/new.jar"];
    assert_eq!(
        new_jar.sha256.as_deref(),
        Some(entry(&manifest, "mods/new.jar").sha256.as_str())
    );
    // A pack mod disabled in the Mods tab is still the pack's file, with what the pack says.
    std::fs::rename(
        root.join("mods/new.jar"),
        root.join("mods/new.jar.disabled"),
    )
    .unwrap();
    let ctx = new_server_context(
        uuid.clone(),
        root.clone(),
        "1.20.1".into(),
        fabric(),
        Vec::new(),
    )
    .await;
    let recorded = state
        .images()
        .record_now(ctx, "Disabled new.jar".into())
        .await
        .unwrap()
        .unwrap();
    let disabled = state
        .images()
        .store()
        .load_manifest(uuid.as_ref(), &recorded.id)
        .unwrap();
    let jar = entry(&disabled, "mods/new.jar.disabled");
    assert_eq!(jar.env, Env::new(EnvValue::Unsupported, EnvValue::Required));
    assert!(jar.source != Source::Object {}, "{:?}", jar.source);
}

/// What the server wrote while it ran isn't called a change outside Lodestone: the record
/// before an update takes it in under its own label, and an update of a server that didn't
/// change records nothing before it.
#[tokio::test(flavor = "multi_thread")]
async fn the_record_before_an_update_names_what_the_server_wrote() {
    let core = Core::new().await;
    let title = core.title.clone();
    let images = core.state.images().clone();
    images.server_started(&core.uuid).await;
    // a mod rewriting its config at startup
    write(&core.root, "config/pack.toml", b"rewritten by a mod");
    let ctx = crate::images::context_of(&core.state, &core.uuid)
        .await
        .unwrap();
    images.server_stopped(ctx).await.unwrap();
    // stopping recorded nothing
    assert_eq!(core.history().await.entries.len(), 1);
    core.change(UpdateSource::Modrinth {
        version_id: "v2".into(),
    })
    .await
    .unwrap();
    let labels: Vec<String> = core
        .history()
        .await
        .entries
        .iter()
        .map(|e| e.label.clone())
        .collect();
    assert_eq!(
        labels,
        [
            format!("Updated {title} to v2-number"),
            BEFORE_UPDATE_LABEL.to_string(),
            format!("Installed {title} v1-number"),
        ]
    );
    // nothing changed since: the next change records nothing before it
    let installed = core.history().await.entries[2].id.clone();
    core.restore(&installed).await.unwrap();
    assert_eq!(read(&core.root, "mods/a-1.jar").unwrap(), b"a1");
    let before: Vec<String> = core
        .history()
        .await
        .entries
        .iter()
        .map(|e| e.label.clone())
        .filter(|l| {
            l == BEFORE_UPDATE_LABEL
                || l == BEFORE_RESTORE_LABEL
                || l == crate::images::recorder::OUTSIDE_LABEL
        })
        .collect();
    assert_eq!(before.len(), 1, "{before:?}");
}

/// A restore of a version that isn't in the history is refused before anything is
/// recorded, though the content changed since the last image.
#[tokio::test(flavor = "multi_thread")]
async fn a_restore_of_a_version_not_in_the_history_records_nothing() {
    let core = Core::new().await;
    let installed = core.current().await;
    write(&core.root, "config/outside.toml", b"by hand");
    for entry in ["a".repeat(64), "not an id".to_string()] {
        let e = core.restore(&entry).await.unwrap_err();
        assert!(matches!(e.kind, ErrorKind::NotFound), "{}", e.source);
    }
    assert_eq!(core.current().await, installed);
    assert_eq!(core.history().await.entries.len(), 1);
}
