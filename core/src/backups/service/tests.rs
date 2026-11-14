//! The service around the images: records and images on delete and retention, leftovers of
//! a crash, collections held off, usage, and the rules that need no server.

use std::path::Path;
use std::time::SystemTime;

use lodestone_image::store::materialize::{missing_objects, Selection};
use lodestone_image::Withheld;

use super::*;
use crate::backups::store::tests::record;
use crate::backups::take::tests::{request, take_image, uuid, write};
use crate::implementations::minecraft::Flavour;

fn service(dir: &Path) -> Arc<BackupService> {
    let images = Arc::new(ImageService::new(dir.join("backups")));
    Arc::new(BackupService::new(images, dir.join("instances")))
}

/// Makes every object, list and chunk of the store old enough for a collection to take.
fn age_store(root: &Path) {
    for area in ["objects", "lists", "chunks"] {
        for entry in walkdir::WalkDir::new(root.join(area))
            .min_depth(2)
            .into_iter()
            .flatten()
        {
            if entry.file_type().is_file() {
                let mut permissions = entry.metadata().unwrap().permissions();
                #[allow(clippy::permissions_set_readonly_false)]
                permissions.set_readonly(false);
                std::fs::set_permissions(entry.path(), permissions).unwrap();
                std::fs::File::options()
                    .write(true)
                    .open(entry.path())
                    .unwrap()
                    .set_modified(SystemTime::now() - Duration::from_secs(3 * 3600))
                    .unwrap();
            }
        }
    }
}

/// A server folder with a world, backed up into the service's store as an image (no
/// record). Returns the image.
fn backed_up(service: &BackupService, root: &Path, level: &[u8]) -> Image {
    write(root, "world/level.dat", level);
    write(root, "server.jar", b"jar");
    take_image(&service.image_store(), request(root, Flavour::Vanilla, &[])).2
}

#[test]
fn unchanged_backups_are_skipped_only_when_the_last_one_is_kept() {
    let scheduled = record("a", &"aa".repeat(32), BackupKind::Scheduled, 1);
    let manual = record("b", &"aa".repeat(32), BackupKind::Manual, 2);
    let pinned = BackupRecord {
        pinned: true,
        ..scheduled.clone()
    };
    // scheduled backups skip whatever the newest backup is
    assert!(skips_unchanged(BackupKind::Scheduled, false, &scheduled));
    assert!(skips_unchanged(BackupKind::Scheduled, false, &manual));
    // "Back up now" after a scheduled backup cleanup may delete still makes one
    assert!(!skips_unchanged(BackupKind::Manual, false, &scheduled));
    assert!(skips_unchanged(BackupKind::Manual, false, &manual));
    assert!(skips_unchanged(BackupKind::Manual, false, &pinned));
    // a named backup, and a safety backup, are always made
    assert!(!skips_unchanged(BackupKind::Manual, true, &manual));
    assert!(!skips_unchanged(BackupKind::BeforeRestore, false, &manual));
}

/// Restoring a safety backup nests the names; each one still fits, and a short name is
/// kept whole.
#[test]
fn safety_backup_names_fit() {
    let mut backup = record(
        "20260101-000000-aaaa",
        &"aa".repeat(32),
        BackupKind::Manual,
        1_767_225_600,
    );
    assert_eq!(
        safety_backup_name(&backup),
        "Before restoring “2026-01-01 00:00 UTC”"
    );
    for _ in 0..10 {
        let name = safety_backup_name(&backup);
        assert!(name.chars().count() <= MAX_NAME_LEN, "{name}");
        assert_eq!(check_backup_name(&name), Ok(Some(name.clone())));
        backup.name = Some(name);
    }
    let name = backup.name.unwrap();
    assert!(
        name.starts_with("Before restoring “Before restoring “"),
        "{name}"
    );
    assert!(name.ends_with("…”"), "{name}");
}

#[test]
fn a_failed_restore_starts_the_server_again_only_when_nothing_changed() {
    let unchanged = RestoreFailure::Unchanged(err(ErrorKind::Conflict, "no space"));
    let broken = RestoreFailure::Broken(err(ErrorKind::Internal, "broken"));
    assert!(start_again_after(&unchanged, true, true, false));
    // the swap couldn't be undone: the server must stay down
    assert!(!start_again_after(&broken, true, true, false));
    // it wasn't running, or wasn't to start after, or Lodestone is shutting down
    assert!(!start_again_after(&unchanged, false, true, false));
    assert!(!start_again_after(&unchanged, true, false, false));
    assert!(!start_again_after(&unchanged, true, true, true));
    // `?` on an ordinary error means nothing changed yet
    let failure: RestoreFailure = err(ErrorKind::Internal, "x").into();
    assert!(matches!(failure, RestoreFailure::Unchanged(_)));
}

/// A swap whose task panicked may have stopped half way through its renames: it is
/// broken (the journal stays and the server stays locked), never started again.
#[tokio::test]
async fn a_swap_that_panicked_is_broken() {
    let joined = tokio::task::spawn_blocking(|| -> Result<(), SwapError> {
        panic!("half way through the renames")
    })
    .await;
    assert!(joined.is_err());
    assert!(matches!(swap_outcome(joined), Err(SwapOutcome::Broken(_))));

    let rolled_back = Ok(Err(SwapError::RolledBack(err(ErrorKind::Internal, "x"))));
    assert!(matches!(
        swap_outcome(rolled_back),
        Err(SwapOutcome::RolledBack(_))
    ));
    let broken = Ok(Err(SwapError::Broken(err(ErrorKind::Internal, "x"))));
    assert!(matches!(swap_outcome(broken), Err(SwapOutcome::Broken(_))));
    assert!(swap_outcome(Ok(Ok(()))).is_ok());
}

#[test]
fn only_files_that_couldnt_be_read_stop_a_restore() {
    let mut image = Image::new(
        Kind::Data,
        crate::images::layout::game_of("1.21.1", &Flavour::Vanilla),
    );
    image.withheld = vec![
        Withheld {
            path: "saves/world/a.dat".to_string(),
            reason: WithheldReason::Unreadable,
        },
        Withheld {
            path: "saves/world/b.dat".to_string(),
            reason: WithheldReason::KeptChanging,
        },
        Withheld {
            path: "saves/world/link".to_string(),
            reason: WithheldReason::Symlink,
        },
    ];
    assert_eq!(unreadable_in(&image), 2);
    let plan = RestorePlan {
        scope: RestoreScope::World,
        owned: Vec::new(),
        files: Vec::new(),
        current: ["saves/world/a.dat", "saves/world/link"]
            .into_iter()
            .map(|path| restore::Current {
                disk: path.trim_start_matches("saves/").to_string(),
                path: lodestone_image::CanonicalPath::new(path).unwrap(),
                size: 1,
                modified_ns: 0,
            })
            .collect(),
    };
    // b.dat isn't replaced; the link isn't a file that couldn't be read
    assert_eq!(not_in_safety_backup(&plan, &image), ["saves/world/a.dat"]);
}

/// Two backups can name one image (nothing changed between them): deleting one keeps it,
/// deleting the last deletes it, and the next collection takes its files.
#[tokio::test]
async fn an_image_goes_with_the_last_backup_that_names_it() {
    let dir = tempfile::tempdir().unwrap();
    let service = service(dir.path());
    let root = dir.path().join("server");
    let first = backed_up(&service, &root, b"level 1");
    let second = backed_up(&service, &root, b"level 2");
    service
        .store
        .save_records(
            &uuid(),
            &Records {
                backups: vec![
                    record("b1", &first.id, BackupKind::Manual, 1),
                    record("b2", &first.id, BackupKind::Manual, 2),
                    record("b3", &second.id, BackupKind::Manual, 3),
                ],
                scheduled_since_full: 0,
            },
        )
        .unwrap();
    let images = service.image_store();
    let has = |id: &str| images.manifest_path(uuid().as_ref(), id).unwrap().is_file();

    service.delete(&uuid(), "b1").await.unwrap();
    assert!(has(&first.id), "b2 still names it");
    let missing = service.delete(&uuid(), "b1").await.unwrap_err();
    assert!(matches!(missing.kind, ErrorKind::NotFound));
    assert!(matches!(
        service.delete(&uuid(), "../x").await.unwrap_err().kind,
        ErrorKind::NotFound
    ));

    service.delete(&uuid(), "b2").await.unwrap();
    assert!(!has(&first.id));
    assert!(has(&second.id));
    let ids: Vec<String> = service
        .records(&uuid())
        .await
        .unwrap()
        .backups
        .into_iter()
        .map(|b| b.id)
        .collect();
    assert_eq!(ids, ["b3"]);

    // the first image's own file goes at the next collection; the second's stay
    age_store(images.root());
    service.images.collect_blocking().unwrap();
    assert!(missing_objects(&images, &second, Selection::All).is_empty());
    let level_1 = first
        .entries
        .iter()
        .find(|e| e.path.as_str() == "saves/world/level.dat")
        .unwrap();
    assert!(!images.has_object(&level_1.sha256, level_1.size));
}

/// A data image no record names (a crash between its manifest and its record, or a
/// `backups.json` that went missing) is listed as a backup again, never deleted: it holds a
/// whole copy of the server. Content images and unreadable manifests are left as they are.
#[tokio::test]
async fn an_image_without_a_record_is_listed_as_a_backup() {
    let dir = tempfile::tempdir().unwrap();
    let service = service(dir.path());
    let root = dir.path().join("server");
    let named = backed_up(&service, &root, b"level 1");
    let orphan = backed_up(&service, &root, b"level 2");
    let mut content = Image::new(
        Kind::Content,
        crate::images::layout::game_of("1.21.1", &Flavour::Vanilla),
    );
    content.seal();
    let images = service.image_store();
    images.save_manifest(uuid().as_ref(), &content).unwrap();
    // a manifest that can't be read is kept too
    let unreadable = "ee".repeat(32);
    std::fs::write(
        images.manifest_path(uuid().as_ref(), &unreadable).unwrap(),
        b"{",
    )
    .unwrap();
    service
        .store
        .save_records(
            &uuid(),
            &Records {
                backups: vec![record("b1", &named.id, BackupKind::Manual, 1)],
                scheduled_since_full: 0,
            },
        )
        .unwrap();
    service.apply_retention(&uuid(), None).await;
    let has = |id: &str| images.manifest_path(uuid().as_ref(), id).unwrap().is_file();
    assert!(has(&named.id));
    assert!(has(&orphan.id));
    assert!(has(&content.id));
    assert!(has(&unreadable));
    let records = service.store.load_records(&uuid()).unwrap();
    assert_eq!(records.backups.len(), 2);
    let adopted = records
        .backups
        .iter()
        .find(|b| b.image == orphan.id)
        .unwrap();
    assert_eq!(adopted.kind, BackupKind::Manual);
    assert_eq!(adopted.created, orphan.created);
    assert_eq!(adopted.file_count, orphan.entries.len() as u64);
    assert!(valid_backup_id(&adopted.id));

    // With the list gone altogether, every backup comes back on the next retention.
    std::fs::remove_file(service.store.records_path(&uuid()).unwrap()).unwrap();
    service.apply_retention(&uuid(), None).await;
    let records = service.store.load_records(&uuid()).unwrap();
    let mut listed: Vec<&str> = records.backups.iter().map(|b| b.image.as_str()).collect();
    listed.sort();
    let mut expected = vec![named.id.as_str(), orphan.id.as_str()];
    expected.sort();
    assert_eq!(listed, expected);
    assert!(has(&named.id) && has(&orphan.id));
}

/// Deleting a backup deletes its image before its record: a crash between them leaves a
/// record whose image is gone, which the next delete or retention drops, and never an image
/// without a record, which would come back as a manual backup that retention never takes.
#[tokio::test]
async fn a_deleted_backup_never_comes_back() {
    let dir = tempfile::tempdir().unwrap();
    let service = service(dir.path());
    let root = dir.path().join("server");
    let kept = backed_up(&service, &root, b"level 1");
    let doomed = backed_up(&service, &root, b"level 2");
    service
        .store
        .save_records(
            &uuid(),
            &Records {
                backups: vec![
                    record("b1", &kept.id, BackupKind::Manual, 1),
                    record("s1", &doomed.id, BackupKind::Scheduled, 2),
                ],
                scheduled_since_full: 0,
            },
        )
        .unwrap();
    let images = service.image_store();
    // a crash after the manifest was deleted, before the record was
    assert!(images.delete_manifest(uuid().as_ref(), &doomed.id).unwrap());
    service.apply_retention(&uuid(), None).await;
    let ids: Vec<String> = service
        .store
        .load_records(&uuid())
        .unwrap()
        .backups
        .into_iter()
        .map(|b| b.id)
        .collect();
    assert_eq!(ids, ["b1"]);
    assert!(images
        .manifest_path(uuid().as_ref(), &kept.id)
        .unwrap()
        .is_file());

    // a delete takes the image away before the record
    service.delete(&uuid(), "b1").await.unwrap();
    assert!(!images
        .manifest_path(uuid().as_ref(), &kept.id)
        .unwrap()
        .is_file());
    service.apply_retention(&uuid(), None).await;
    assert!(service
        .store
        .load_records(&uuid())
        .unwrap()
        .backups
        .is_empty());
}

/// A backup list that can't be read is an error, and then nothing is deleted: the images
/// it names would otherwise look like leftovers.
#[tokio::test]
async fn nothing_is_deleted_while_the_list_cant_be_read() {
    let dir = tempfile::tempdir().unwrap();
    let service = service(dir.path());
    let root = dir.path().join("server");
    let image = backed_up(&service, &root, b"level 1");
    std::fs::write(service.store.records_path(&uuid()).unwrap(), b"{").unwrap();
    service.apply_retention(&uuid(), None).await;
    assert!(service.delete(&uuid(), "b1").await.is_err());
    assert!(service.list(&uuid()).await.is_err());
    let images = service.image_store();
    assert!(images
        .manifest_path(uuid().as_ref(), &image.id)
        .unwrap()
        .is_file());
}

/// Retention deletes the records it doesn't keep, and the images only they named.
#[tokio::test]
async fn retention_deletes_backups_and_the_images_only_they_name() {
    let dir = tempfile::tempdir().unwrap();
    let service = service(dir.path());
    let root = dir.path().join("server");
    let old = backed_up(&service, &root, b"level 1");
    let new = backed_up(&service, &root, b"level 2");
    let records = Records {
        backups: (0..8)
            .map(|n| {
                let image = if n == 7 { &new.id } else { &old.id };
                record(&format!("s{n}"), image, BackupKind::Scheduled, 1_000 + n)
            })
            .collect(),
        scheduled_since_full: 0,
    };
    service.store.save_records(&uuid(), &records).unwrap();
    service
        .set_policy(
            &uuid(),
            BackupPolicy {
                keep_last: 1,
                keep_hourly: 0,
                keep_daily: 0,
                keep_weekly: 0,
                ..BackupPolicy::default()
            },
        )
        .await
        .unwrap();
    service.apply_retention(&uuid(), Some("s7")).await;
    let kept: Vec<String> = service
        .records(&uuid())
        .await
        .unwrap()
        .backups
        .into_iter()
        .map(|b| b.id)
        .collect();
    assert_eq!(kept, ["s7"]);
    let images = service.image_store();
    assert!(!images
        .manifest_path(uuid().as_ref(), &old.id)
        .unwrap()
        .is_file());
    assert!(images
        .manifest_path(uuid().as_ref(), &new.id)
        .unwrap()
        .is_file());
}

/// A collection waits while a job holds collections off, so it can't take what the job
/// relies on in between; then it runs.
#[tokio::test]
async fn a_collection_waits_for_a_job_that_holds_it_off() {
    let dir = tempfile::tempdir().unwrap();
    let service = service(dir.path());
    let root = dir.path().join("server");
    let image = backed_up(&service, &root, b"level 1");
    // nothing names the image any more, as if its backup was deleted during a restore
    service
        .image_store()
        .delete_manifest(uuid().as_ref(), &image.id)
        .unwrap();
    age_store(service.image_store().root());
    let hold = hold_off_collection(service.image_store()).await.unwrap();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let collector = service.clone();
    std::thread::spawn(move || {
        let report = collector.images.collect_blocking();
        let _ = done_tx.send(report.is_ok());
    });
    std::thread::sleep(Duration::from_millis(300));
    assert!(done_rx.try_recv().is_err(), "the collection waits");
    assert!(missing_objects(&service.image_store(), &image, Selection::All).is_empty());
    drop(hold);
    assert!(done_rx.recv_timeout(Duration::from_secs(30)).unwrap());
    assert!(!missing_objects(&service.image_store(), &image, Selection::All).is_empty());
}

/// The list's usage counts each file the server's backups use once, at its size in the
/// store (compressed, and as chunks for a large file).
#[tokio::test]
async fn usage_counts_each_stored_file_once() {
    let dir = tempfile::tempdir().unwrap();
    let service = service(dir.path());
    let root = dir.path().join("server");
    // a large file the store keeps as chunks
    let large: Vec<u8> = (0..3_000_000u64).map(|n| (n * 7919 % 251) as u8).collect();
    write(&root, "world/region/r.0.0.mca", &large);
    let image = backed_up(&service, &root, b"level 1");
    service
        .store
        .save_records(
            &uuid(),
            &Records {
                backups: vec![
                    record("b1", &image.id, BackupKind::Manual, 1),
                    record("b2", &image.id, BackupKind::Manual, 2),
                ],
                scheduled_since_full: 0,
            },
        )
        .unwrap();
    let list = service.list(&uuid()).await.unwrap();
    assert_eq!(list.backups.len(), 2);
    assert_eq!(list.backups[0].id, "b2", "newest first");
    let stored = list.usage.stored_bytes;
    assert!(stored > 0);
    // well under the files' size: zstd, and the large file once
    assert!(stored < large.len() as u64, "{stored}");
    let images = service.image_store();
    let region = image
        .entries
        .iter()
        .find(|e| e.path.as_str() == "saves/world/region/r.0.0.mca")
        .unwrap();
    assert!(matches!(
        images.object_form(&region.sha256, region.size).unwrap(),
        ObjectForm::Chunked(_)
    ));
    let region_stored = stored_size(&images, &region.sha256, region.size);
    assert!(region_stored > 0 && region_stored <= stored);
    assert_eq!(stored_size(&images, &"00".repeat(32), 5), 0);
    // the same again from the cache
    assert_eq!(
        service.list(&uuid()).await.unwrap().usage.stored_bytes,
        stored
    );
}

#[tokio::test]
async fn renaming_and_pinning_change_only_the_record() {
    let dir = tempfile::tempdir().unwrap();
    let service = service(dir.path());
    let root = dir.path().join("server");
    let image = backed_up(&service, &root, b"level 1");
    let manifest = service
        .image_store()
        .manifest_path(uuid().as_ref(), &image.id)
        .unwrap();
    let before = std::fs::read(&manifest).unwrap();
    service
        .store
        .save_records(
            &uuid(),
            &Records {
                backups: vec![record("b1", &image.id, BackupKind::Manual, 1)],
                scheduled_since_full: 4,
            },
        )
        .unwrap();
    let info = service
        .update(
            &uuid(),
            "b1",
            Some(Some("  Named  ".to_string())),
            Some(true),
        )
        .await
        .unwrap();
    assert_eq!(info.name.as_deref(), Some("Named"));
    assert!(info.pinned);
    let info = service
        .update(&uuid(), "b1", Some(None), None)
        .await
        .unwrap();
    assert_eq!(info.name, None);
    assert!(info.pinned);
    assert!(matches!(
        service
            .update(&uuid(), "nope", None, Some(false))
            .await
            .unwrap_err()
            .kind,
        ErrorKind::NotFound
    ));
    assert!(matches!(
        service
            .update(&uuid(), "b1", Some(Some("a\nb".to_string())), None)
            .await
            .unwrap_err()
            .kind,
        ErrorKind::BadRequest
    ));
    assert_eq!(std::fs::read(&manifest).unwrap(), before);
    let records = service.records(&uuid()).await.unwrap();
    assert_eq!(records.scheduled_since_full, 4);
}
