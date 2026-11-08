//! The download engine against a loopback file server: hash mismatches and retries,
//! skipping files that are already there, cancelling, and the concurrency limit.

mod files;

use std::{fs, path::Path, sync::Arc, time::Duration};

use files::{Entry, FileServer};
use futures::executor::block_on;
use launcher_core::{
    USER_AGENT,
    game::{
        download::{
            CancelToken, DownloadError, DownloadItem, Downloader, Progress, Sleep, recording_sleep,
            sha1_hex,
        },
        mojang::MojangUrls,
    },
    http,
};

fn downloader(sleep: Sleep) -> Downloader {
    Downloader::new(http::download_client().unwrap(), sleep)
}

/// An item for a file served at a Mojang URL, saved under `dir`.
fn item(server: &FileServer, dir: &Path, name: &str, body: &[u8]) -> DownloadItem {
    let url = format!("https://piston-data.mojang.com/v1/objects/{name}");
    server.serve(&url, Entry::ok(body));
    DownloadItem {
        url: MojangUrls::mirror(&server.base).resolve(&url).unwrap(),
        dest: dir.join(name),
        sha1: sha1_hex(body),
        size: Some(body.len() as u64),
        executable: false,
    }
}

fn mojang(name: &str) -> String {
    format!("https://piston-data.mojang.com/v1/objects/{name}")
}

/// The names in `dir`, to check no temporary files are left.
fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn downloads_and_reports_progress() {
    let server = FileServer::start();
    let dir = tempfile::tempdir().unwrap();
    let items = vec![
        item(&server, dir.path(), "a.jar", b"first file"),
        item(&server, dir.path(), "b.jar", &vec![7u8; 200_000]),
    ];
    let progress = Progress::default();
    let (sleep, waits) = recording_sleep();
    block_on(downloader(sleep).run(items, &progress, &CancelToken::new())).unwrap();
    assert_eq!(fs::read(dir.path().join("a.jar")).unwrap(), b"first file");
    assert_eq!(fs::read(dir.path().join("b.jar")).unwrap().len(), 200_000);
    let done = progress.snapshot();
    assert_eq!((done.files_done, done.files_total), (2, 2));
    assert_eq!((done.bytes_done, done.bytes_total), (200_010, 200_010));
    assert!(waits.lock().unwrap().is_empty());
    assert_eq!(names(dir.path()), ["a.jar", "b.jar"]);
    assert!(server.state().user_agents.iter().all(|ua| ua == USER_AGENT));
}

#[test]
fn retries_a_hash_mismatch_with_backoff() {
    let server = FileServer::start();
    let dir = tempfile::tempdir().unwrap();
    let item = item(&server, dir.path(), "flaky.jar", b"the real contents");
    server
        .state()
        .entries
        .values_mut()
        .for_each(|e| e.corrupt_first = 2);
    let (sleep, waits) = recording_sleep();
    let progress = Progress::default();
    block_on(
        downloader(sleep)
            .with_backoff(Duration::from_millis(100))
            .run(vec![item], &progress, &CancelToken::new()),
    )
    .unwrap();
    assert_eq!(server.hits(&mojang("flaky.jar")), 3);
    assert_eq!(
        *waits.lock().unwrap(),
        [Duration::from_millis(100), Duration::from_millis(200)]
    );
    assert_eq!(
        fs::read(dir.path().join("flaky.jar")).unwrap(),
        b"the real contents"
    );
    // Bytes from the failed attempts are not counted twice.
    assert_eq!(progress.snapshot().bytes_done, 17);
}

#[test]
fn gives_up_on_a_file_that_never_matches() {
    let server = FileServer::start();
    let dir = tempfile::tempdir().unwrap();
    let mut item = item(&server, dir.path(), "wrong.jar", b"contents");
    item.sha1 = sha1_hex(b"other contents");
    let (sleep, waits) = recording_sleep();
    let err = block_on(downloader(sleep).with_attempts(3).run(
        vec![item],
        &Progress::default(),
        &CancelToken::new(),
    ))
    .unwrap_err();
    assert!(
        matches!(&err, DownloadError::Failed { reason, .. } if reason.contains("SHA-1")),
        "{err}"
    );
    assert_eq!(server.hits(&mojang("wrong.jar")), 3);
    assert_eq!(waits.lock().unwrap().len(), 2);
    // Nothing is left behind: no file, no temporary file.
    assert!(names(dir.path()).is_empty(), "{:?}", names(dir.path()));
}

#[test]
fn retries_server_errors_but_not_missing_files() {
    let server = FileServer::start();
    let dir = tempfile::tempdir().unwrap();
    let flaky = item(&server, dir.path(), "busy.jar", b"busy");
    server
        .state()
        .entries
        .values_mut()
        .for_each(|e| e.fail_first = 1);
    let (sleep, _) = recording_sleep();
    let engine = downloader(sleep);
    block_on(engine.run(vec![flaky], &Progress::default(), &CancelToken::new())).unwrap();
    assert_eq!(server.hits(&mojang("busy.jar")), 2);

    let mut missing = item(&server, dir.path(), "missing.jar", b"x");
    missing.url = missing.url.replace("missing.jar", "gone.jar");
    let err =
        block_on(engine.run(vec![missing], &Progress::default(), &CancelToken::new())).unwrap_err();
    assert!(
        matches!(&err, DownloadError::Failed { reason, .. } if reason == "HTTP 404"),
        "{err}"
    );
    assert_eq!(server.hits(&mojang("gone.jar")), 1);
}

#[test]
fn skips_files_that_are_already_there() {
    let server = FileServer::start();
    let dir = tempfile::tempdir().unwrap();
    let present = item(&server, dir.path(), "present.jar", b"already here");
    let stale = item(&server, dir.path(), "stale.jar", b"new contents");
    fs::write(&present.dest, b"already here").unwrap();
    fs::write(&stale.dest, b"old contents").unwrap();
    let progress = Progress::default();
    let (sleep, _) = recording_sleep();
    block_on(downloader(sleep).run(vec![present, stale], &progress, &CancelToken::new())).unwrap();
    assert_eq!(server.hits(&mojang("present.jar")), 0);
    assert_eq!(server.hits(&mojang("stale.jar")), 1);
    assert_eq!(
        fs::read(dir.path().join("stale.jar")).unwrap(),
        b"new contents"
    );
    let done = progress.snapshot();
    assert_eq!((done.files_done, done.bytes_done), (2, 24));
}

#[cfg(unix)]
#[test]
fn sets_the_executable_bit() {
    use std::os::unix::fs::PermissionsExt as _;
    let server = FileServer::start();
    let dir = tempfile::tempdir().unwrap();
    let mut java = item(&server, dir.path(), "java", b"#!/bin/sh\n");
    java.executable = true;
    let (sleep, _) = recording_sleep();
    block_on(downloader(sleep).run(vec![java], &Progress::default(), &CancelToken::new())).unwrap();
    let mode = fs::metadata(dir.path().join("java"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o111, 0o111);
}

#[test]
fn cancelling_stops_before_the_next_request() {
    let server = FileServer::start();
    let dir = tempfile::tempdir().unwrap();
    let first = item(&server, dir.path(), "first.jar", b"first");
    let second = item(&server, dir.path(), "second.jar", b"second");
    server
        .state()
        .entries
        .values_mut()
        .for_each(|e| e.fail_first = 1);
    // The user presses Cancel while the engine waits to retry the first file.
    let cancel = CancelToken::new();
    let sleep: Sleep = {
        let cancel = cancel.clone();
        Arc::new(move |_| {
            cancel.cancel();
            Box::pin(async {})
        })
    };
    let err = block_on(downloader(sleep).with_concurrency(1).run(
        vec![first, second],
        &Progress::default(),
        &cancel,
    ))
    .unwrap_err();
    assert!(matches!(err, DownloadError::Cancelled), "{err}");
    assert_eq!(server.hits(&mojang("first.jar")), 1);
    assert_eq!(server.hits(&mojang("second.jar")), 0);
    assert!(names(dir.path()).is_empty());

    // Cancelled before it starts: nothing is requested.
    let cancelled = CancelToken::new();
    cancelled.cancel();
    let third = item(&server, dir.path(), "third.jar", b"third");
    let (sleep, _) = recording_sleep();
    let err =
        block_on(downloader(sleep).run(vec![third], &Progress::default(), &cancelled)).unwrap_err();
    assert!(matches!(err, DownloadError::Cancelled));
    assert_eq!(server.hits(&mojang("third.jar")), 0);
}

#[test]
fn keeps_to_the_concurrency_limit() {
    for limit in [1, 3, 8] {
        let server = FileServer::start();
        let dir = tempfile::tempdir().unwrap();
        let items: Vec<_> = (0..20)
            .map(|n| {
                item(
                    &server,
                    dir.path(),
                    &format!("{n}.jar"),
                    format!("file {n}").as_bytes(),
                )
            })
            .collect();
        // Requests wait until `limit` are in flight together, so a lower limit than
        // asked for would show, and a higher one would be recorded.
        server.state().hold_until = Some(limit);
        let (sleep, _) = recording_sleep();
        block_on(downloader(sleep).with_concurrency(limit).run(
            items,
            &Progress::default(),
            &CancelToken::new(),
        ))
        .unwrap();
        let state = server.state();
        assert_eq!(state.max_in_flight, limit, "limit {limit}");
        assert!(
            state.released,
            "limit {limit}: never had {limit} requests in flight"
        );
        drop(state);
        assert_eq!(server.total_hits(), 20);
    }
}
