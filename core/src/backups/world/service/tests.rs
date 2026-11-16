//! The world backup service on temporary folders, without a server: making world backups
//! stopped and "running" (a fake console), cancelling, free space, and restoring with its
//! safety backup and swap.

use std::fs;
use std::path::Path;

use crate::backups::live::tests::{short, system, FakeLines, FakeServer, SAVED};
use crate::backups::world::reader::plan_restore;
use crate::backups::world::writer::tests::{read_zip, write};
use crate::event_broadcaster::EventBroadcaster;

use super::*;

fn uuid() -> InstanceUuid {
    InstanceUuid::from("INSTANCE_world".to_string())
}

struct Env {
    dir: tempfile::TempDir,
    service: Arc<WorldBackupService>,
    broadcaster: EventBroadcaster,
    _events: tokio::sync::broadcast::Receiver<Event>,
}

fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let images = Arc::new(ImageService::new(dir.path().join("backups")));
    let backups = Arc::new(BackupService::new(
        images.clone(),
        dir.path().join("instances"),
    ));
    let service = Arc::new(WorldBackupService::new(
        backups,
        images,
        dir.path().join("world-backups"),
    ));
    let (broadcaster, events) = EventBroadcaster::new(256);
    Env {
        dir,
        service,
        broadcaster,
        _events: events,
    }
}

impl Env {
    fn root(&self) -> PathBuf {
        self.dir.path().join("instances/server")
    }

    fn source(&self, level: &str, split: bool) -> WorldSource {
        WorldSource {
            uuid: uuid(),
            root: self.root(),
            layout: ServerLayout::new(level, [], split).unwrap(),
            instance_name: "Survival".to_string(),
            game_version: "1.21.1".to_string(),
            flavour: if split { "paper" } else { "vanilla" }.to_string(),
        }
    }

    fn job(&self, kind: JobKind) -> Job {
        Job::new(
            self.broadcaster.clone(),
            kind,
            "test".to_string(),
            ProgressionStartValue::WorldBackup {
                instance_uuid: uuid(),
            },
            CausedBy::System,
        )
    }

    async fn back_up(&self, source: &WorldSource, name: &str) -> WorldBackupRecord {
        self.service
            .write_backup(
                source,
                None,
                manual(name),
                self.job(JobKind::WorldBackup).phase(0.0, 100.0),
                &CancellationToken::new(),
            )
            .await
            .unwrap()
    }

    fn server_files(&self) -> Vec<String> {
        let mut files: Vec<String> = walkdir::WalkDir::new(self.root())
            .into_iter()
            .flatten()
            .filter(|e| e.file_type().is_file())
            .map(|e| {
                e.path()
                    .strip_prefix(self.root())
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        files.sort();
        files
    }

    fn folder_names(&self) -> Vec<String> {
        let dir = self.service.store().dir(&uuid()).unwrap();
        let mut names: Vec<String> = fs::read_dir(dir)
            .map(|d| {
                d.flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    /// Restores `record` into the server as `restore_locked` does in the background.
    async fn restore(
        &self,
        source: &WorldSource,
        record: &WorldBackupRecord,
        token: &CancellationToken,
    ) -> Result<Restored, Error> {
        let zip = self.service.store().zip_path(&uuid(), &record.id).unwrap();
        let plan = plan_restore(&zip, &record.level, &source.layout)?;
        self.service
            .restore_locked(
                source,
                record,
                plan,
                &mut None,
                &Arc::new(StdMutex::new(true)),
                &self.job(JobKind::WorldRestore),
                token,
            )
            .await
    }
}

fn manual(name: &str) -> NewBackup {
    NewBackup {
        kind: WorldBackupKind::Manual,
        name: Some(name.to_string()),
        before_restoring: None,
        require_level_dat: true,
    }
}

fn vanilla_world(root: &Path) {
    write(root, "server.properties", b"level-name=world");
    write(root, "mods/a.jar", b"mod");
    write(root, "world/level.dat", b"level v1");
    write(root, "world/region/r.0.0.mca", b"region v1");
    write(root, "world/DIM-1/region/r.0.0.mca", b"nether v1");
}

#[tokio::test]
async fn a_stopped_vanilla_world_is_backed_up_and_listed() {
    let env = env();
    vanilla_world(&env.root());
    let record = env.back_up(&env.source("world", false), "Spawn").await;
    assert_eq!(record.name.as_deref(), Some("Spawn"));
    assert_eq!(record.kind, WorldBackupKind::Manual);
    assert_eq!(record.level, "world");
    assert_eq!(record.folders, ["world"]);
    assert_eq!(record.file_count, 3);
    assert!(!record.live && record.quiesced);
    let list = env.service.list(&uuid()).await.unwrap();
    assert_eq!(list.backups, vec![WorldBackupInfo::from(&record)]);
    assert_eq!(list.usage.total_bytes, record.size);
    assert!(list.running.is_none());
    // a plain zip of the world, and nothing beside it but its sidecar
    let zip = env.service.store().zip_path(&uuid(), &record.id).unwrap();
    let entries = read_zip(&zip);
    assert_eq!(entries["world/level.dat"].0, b"level v1");
    assert_eq!(entries["world/DIM-1/region/r.0.0.mca"].0, b"nether v1");
    assert!(entries.keys().all(|name| name.starts_with("world/")));
    assert_eq!(
        env.folder_names(),
        [format!("{}.json", record.id), format!("{}.zip", record.id)]
    );
}

#[tokio::test]
async fn a_paper_world_keeps_its_split_folders() {
    let env = env();
    let root = env.root();
    write(&root, "world/level.dat", b"level");
    write(&root, "world_nether/DIM-1/region/r.0.0.mca", b"nether");
    write(&root, "world_the_end/DIM1/region/r.0.0.mca", b"end");
    let record = env.back_up(&env.source("world", true), "Paper").await;
    assert_eq!(record.folders, ["world", "world_nether", "world_the_end"]);
    let entries = read_zip(&env.service.store().zip_path(&uuid(), &record.id).unwrap());
    assert_eq!(entries["world_nether/DIM-1/region/r.0.0.mca"].0, b"nether");
    assert_eq!(entries["world_the_end/DIM1/region/r.0.0.mca"].0, b"end");
}

/// A running server's saving is paused around the copy, and always turned back on.
#[tokio::test]
async fn a_running_world_is_quiesced() {
    let env = env();
    vanilla_world(&env.root());
    let server = FakeServer::new();
    let mut lines = FakeLines::new(vec![system(SAVED)]);
    let record = env
        .service
        .write_backup(
            &env.source("world", false),
            Some(Live {
                server: &server,
                lines: &mut lines,
                limits: short(),
            }),
            manual("Live"),
            env.job(JobKind::WorldBackup).phase(0.0, 100.0),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(server.commands(), ["save-off", "save-all flush", "save-on"]);
    assert!(record.live && record.quiesced);
    // no marker is left
    assert!(!env
        .service
        .backups
        .store()
        .marker_path(&uuid())
        .unwrap()
        .exists());

    // the server never says it saved: kept, but marked as possibly inconsistent
    let server = FakeServer::new();
    let mut lines = FakeLines::new(Vec::new());
    let record = env
        .service
        .write_backup(
            &env.source("world", false),
            Some(Live {
                server: &server,
                lines: &mut lines,
                limits: short(),
            }),
            manual("Quiet"),
            env.job(JobKind::WorldBackup).phase(0.0, 100.0),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(record.live && !record.quiesced);
    assert!(WorldBackupService::backup_message("Survival", &record).contains("may be inconsistent"));
    assert_eq!(env.service.list(&uuid()).await.unwrap().backups.len(), 2);
}

#[tokio::test]
async fn a_cancelled_or_failed_backup_leaves_nothing() {
    let env = env();
    vanilla_world(&env.root());
    let token = CancellationToken::new();
    token.cancel();
    let server = FakeServer::new();
    let mut lines = FakeLines::new(vec![system(SAVED)]);
    let error = env
        .service
        .write_backup(
            &env.source("world", false),
            Some(Live {
                server: &server,
                lines: &mut lines,
                limits: short(),
            }),
            manual("Cancelled"),
            env.job(JobKind::WorldBackup).phase(0.0, 100.0),
            &token,
        )
        .await
        .unwrap_err();
    assert_eq!(error.source.to_string(), "Cancelled");
    // saving is on again however it ended
    assert_eq!(
        server.commands().last().map(String::as_str),
        Some("save-on")
    );
    assert!(env.service.list(&uuid()).await.unwrap().backups.is_empty());
    assert!(env.folder_names().is_empty());

    // stopped, cancelled
    let error = env
        .service
        .write_backup(
            &env.source("world", false),
            None,
            manual("Cancelled"),
            env.job(JobKind::WorldBackup).phase(0.0, 100.0),
            &token,
        )
        .await
        .unwrap_err();
    assert_eq!(error.source.to_string(), "Cancelled");
    assert!(env.folder_names().is_empty());

    // no world
    let error = env
        .service
        .write_backup(
            &env.source("survival", false),
            None,
            manual("None"),
            env.job(JobKind::WorldBackup).phase(0.0, 100.0),
            &CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(matches!(error.kind, ErrorKind::NotFound));
    assert!(env.folder_names().is_empty());
}

/// Space another large write is filling isn't counted on.
#[tokio::test]
async fn no_room_is_refused() {
    let env = env();
    vanilla_world(&env.root());
    let held = env.service.images.large_writes.reserve(u64::MAX / 2);
    let error = env
        .service
        .write_backup(
            &env.source("world", false),
            None,
            manual("Full"),
            env.job(JobKind::WorldBackup).phase(0.0, 100.0),
            &CancellationToken::new(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(error.kind, ErrorKind::Conflict),
        "{}",
        error.source
    );
    assert!(env.folder_names().is_empty());
    drop(held);
    env.back_up(&env.source("world", false), "Room").await;
}

#[tokio::test]
async fn a_restore_takes_a_safety_backup_then_replaces_only_the_world() {
    let env = env();
    let root = env.root();
    vanilla_world(&root);
    let source = env.source("world", false);
    let first = env.back_up(&source, "First").await;
    // the world changes, and so does everything else
    write(&root, "world/level.dat", b"level v2");
    write(&root, "world/region/r.1.0.mca", b"new region");
    write(&root, "mods/b.jar", b"another mod");
    let restored = env
        .restore(&source, &first, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(fs::read(root.join("world/level.dat")).unwrap(), b"level v1");
    assert!(!root.join("world/region/r.1.0.mca").exists());
    // nothing outside the world changed
    assert_eq!(fs::read(root.join("mods/b.jar")).unwrap(), b"another mod");
    assert_eq!(
        env.server_files(),
        [
            "mods/a.jar",
            "mods/b.jar",
            "server.properties",
            "world/DIM-1/region/r.0.0.mca",
            "world/level.dat",
            "world/region/r.0.0.mca",
        ]
    );
    // the safety backup has the world as it was
    let safety = env
        .service
        .load(&uuid(), restored.safety_id.as_deref().unwrap())
        .await
        .unwrap();
    assert_eq!(safety.kind, WorldBackupKind::BeforeRestore);
    assert_eq!(safety.name.as_deref(), Some("Before restoring “First”"));
    assert_eq!(safety.before_restoring.as_deref(), Some("First"));
    let entries = read_zip(&env.service.store().zip_path(&uuid(), &safety.id).unwrap());
    assert_eq!(entries["world/level.dat"].0, b"level v2");
    assert_eq!(entries["world/region/r.1.0.mca"].0, b"new region");
    // no work folders or journal are left
    assert!(fs::read_dir(&root)
        .unwrap()
        .flatten()
        .all(|e| !e.file_name().to_string_lossy().starts_with(".lodestone")));
    assert!(!env
        .service
        .backups
        .store()
        .journal_path(&uuid())
        .unwrap()
        .exists());

    // and back: the safety backup restores the change
    env.restore(&source, &safety, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(fs::read(root.join("world/level.dat")).unwrap(), b"level v2");
}

#[tokio::test]
async fn a_vanilla_backup_restores_into_paper() {
    let env = env();
    let root = env.root();
    vanilla_world(&root);
    let first = env.back_up(&env.source("world", false), "Vanilla").await;
    // the server became a Paper server since, with its own split folders
    fs::remove_dir_all(root.join("world/DIM-1")).unwrap();
    write(
        &root,
        "world_nether/DIM-1/region/r.0.0.mca",
        b"paper nether",
    );
    write(&root, "world_the_end/DIM1/region/r.0.0.mca", b"paper end");
    env.restore(
        &env.source("world", true),
        &first,
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        fs::read(root.join("world_nether/DIM-1/region/r.0.0.mca")).unwrap(),
        b"nether v1"
    );
    // the backup had no End: it is gone (the safety backup has it)
    assert!(!root.join("world_the_end").exists());
    assert!(!root.join("world/DIM-1").exists());
}

#[tokio::test]
async fn a_restore_without_a_world_takes_no_safety_backup() {
    let env = env();
    let root = env.root();
    vanilla_world(&root);
    let source = env.source("world", false);
    let first = env.back_up(&source, "First").await;
    fs::remove_dir_all(root.join("world")).unwrap();
    let restored = env
        .restore(&source, &first, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(restored.safety_id, None);
    assert_eq!(fs::read(root.join("world/level.dat")).unwrap(), b"level v1");
    assert_eq!(env.service.list(&uuid()).await.unwrap().backups.len(), 1);

    // a level folder without its level.dat is kept all the same
    fs::remove_file(root.join("world/level.dat")).unwrap();
    let restored = env
        .restore(&source, &first, &CancellationToken::new())
        .await
        .unwrap();
    assert!(restored.safety_id.is_some());
}

#[tokio::test]
async fn a_restore_that_fails_or_is_cancelled_leaves_the_world() {
    let env = env();
    let root = env.root();
    vanilla_world(&root);
    let source = env.source("world", false);
    let first = env.back_up(&source, "First").await;
    write(&root, "world/level.dat", b"level v2");
    let before = env.server_files();

    // cancelled: the safety backup is the first thing that stops
    let token = CancellationToken::new();
    token.cancel();
    assert!(env.restore(&source, &first, &token).await.is_err());
    assert_eq!(fs::read(root.join("world/level.dat")).unwrap(), b"level v2");

    // a zip that changed since it was planned fails while it is unpacked
    let zip = env.service.store().zip_path(&uuid(), &first.id).unwrap();
    let plan = plan_restore(&zip, &first.level, &source.layout).unwrap();
    fs::write(&zip, b"not a zip any more").unwrap();
    let error = env
        .service
        .restore_locked(
            &source,
            &first,
            plan,
            &mut None,
            &Arc::new(StdMutex::new(true)),
            &env.job(JobKind::WorldRestore),
            &CancellationToken::new(),
        )
        .await
        .err()
        .unwrap();
    assert!(
        matches!(error.kind, ErrorKind::BadRequest),
        "{}",
        error.source
    );
    assert_eq!(env.server_files(), before);
    assert_eq!(fs::read(root.join("world/level.dat")).unwrap(), b"level v2");
}

#[tokio::test]
async fn cancel_finds_only_its_own_task_and_stops_at_the_swap() {
    let env = env();
    let token = CancellationToken::new();
    let cancellable = Arc::new(StdMutex::new(true));
    env.service.add_job(
        &uuid(),
        WorldJob {
            task_id: "7".to_string(),
            kind: JobKind::WorldRestore,
            cancel: token.clone(),
            cancellable: cancellable.clone(),
        },
    );
    assert_eq!(
        env.service.job_kind(&uuid(), "7"),
        Some(JobKind::WorldRestore)
    );
    assert_eq!(env.service.job_kind(&uuid(), "8"), None);
    assert!(matches!(
        env.service.cancel(&uuid(), "8").unwrap_err().kind,
        ErrorKind::NotFound
    ));
    // a delete waits for the job
    assert!(matches!(
        env.service
            .delete(&uuid(), "20260101-000000-aaaa")
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Conflict
    ));
    *cancellable.lock().unwrap() = false;
    assert!(matches!(
        env.service.cancel(&uuid(), "7").unwrap_err().kind,
        ErrorKind::Conflict
    ));
    assert!(!token.is_cancelled());
    *cancellable.lock().unwrap() = true;
    env.service.cancel(&uuid(), "7").unwrap();
    assert!(token.is_cancelled());
    env.service.remove_job(&uuid());
    assert_eq!(env.service.job_kind(&uuid(), "7"), None);
}

/// The Files API refuses changes while a world backup is restored, as for a snapshot.
#[tokio::test]
async fn a_world_restore_counts_as_restoring() {
    let env = env();
    let job = env.job(JobKind::WorldRestore);
    job.begin();
    env.service.backups.register(&uuid(), &job);
    assert!(env.service.backups.restoring(&uuid()));
    let running = env.service.list(&uuid()).await.unwrap().running.unwrap();
    assert_eq!(running.kind, JobKind::WorldRestore);
    env.service.backups.unregister(&uuid());
    let job = env.job(JobKind::WorldBackup);
    job.begin();
    env.service.backups.register(&uuid(), &job);
    assert!(!env.service.backups.restoring(&uuid()));
    env.service.backups.unregister(&uuid());
}

#[test]
fn safety_names_fit() {
    assert_eq!(safety_name("Spawn"), "Before restoring “Spawn”");
    let mut label = "x".repeat(MAX_NAME_LEN);
    for _ in 0..5 {
        let name = safety_name(&label);
        assert!(name.chars().count() <= MAX_NAME_LEN, "{name}");
        assert_eq!(check_backup_name(&name), Ok(Some(name.clone())));
        label = name;
    }
    assert!(label.ends_with("…”"), "{label}");
    let mut record =
        crate::backups::world::store::tests::record("20250101-000000-aaaa", 1735689600);
    assert_eq!(backup_label(&record), "Spawn");
    record.name = None;
    assert_eq!(backup_label(&record), "2025-01-01 00:00 UTC");
}
