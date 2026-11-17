//! The client of a server's image routes (`cores::player_image`) against a fake core that
//! answers as milestone 3's core does (`image_core`), over the signed-in route with a key
//! and the public one (the shape a friend's tunnel and an invite's info port have): every
//! status and header the spec lists, and hostile answers.

mod image_core;

use std::{sync::atomic::Ordering, time::Duration};

use futures::executor::block_on;
use image_core::{Fault, ImageCore, KEY, SERVER, content_image, object_entry};
use launcher_core::{
    Secret,
    cores::{
        CoreClient,
        player_image::{ImageAnswer, ImageError, Limits, ObjectItem, ServerImage},
    },
    game::download::{CancelToken, Progress, recording_sleep},
    http,
};
use lodestone_image::{Loader, Side, apply, hex::sha256_hex, view::check_received};

fn source(
    core: &ImageCore,
    signed_in: bool,
) -> (ServerImage, std::sync::Arc<std::sync::Mutex<Vec<Duration>>>) {
    let client = CoreClient::new(http::default_client().unwrap(), core.base.clone());
    let key = signed_in.then(|| Secret::new(KEY));
    let (sleep, waits) = recording_sleep();
    let image = ServerImage::new(client, key, SERVER)
        .unwrap()
        .with_sleep(sleep)
        .with_limits(Limits {
            object_idle: Duration::from_millis(400),
            image_idle: Duration::from_secs(5),
            ..Limits::default()
        });
    (image, waits)
}

fn ask(image: &ServerImage, have: &[String]) -> Result<ImageAnswer, ImageError> {
    block_on(http::in_http_runtime(image.image(
        have,
        &|| {},
        &CancelToken::new(),
    )))
}

fn smp(core: &ImageCore, mods: &[(&str, &[u8])]) {
    let mut image = content_image("1.21.1", Loader::Fabric, Some("0.16.10"));
    for (path, bytes) in mods {
        image.entries.push(object_entry(path, bytes));
    }
    core.publish(&image, mods);
}

/// The Launcher's window polls these futures on its own executor, with no Tokio runtime
/// (Create instance on the Lodestone Host page did and crashed): every timer must be made
/// inside the HTTP client's runtime, so a caller that doesn't enter it still works.
#[test]
fn a_caller_without_a_runtime_gets_the_image_and_its_files() {
    let core = ImageCore::start();
    smp(&core, &[("mods/a.jar", b"mod a")]);
    let (image, _) = source(&core, true);
    let answer = block_on(image.image(&[], &|| {}, &CancelToken::new())).unwrap();
    let ImageAnswer::View { image: first, .. } = answer else {
        panic!("expected the whole view");
    };
    check_received(&first, Side::Client).unwrap();
}

#[test]
fn views_deltas_and_current_answers_over_both_routes() {
    for signed_in in [true, false] {
        let core = ImageCore::start();
        smp(&core, &[("mods/a.jar", b"mod a")]);
        let (image, _) = source(&core, signed_in);

        // nothing yet: the whole view
        let ImageAnswer::View { image: first, view } = ask(&image, &[]).unwrap() else {
            panic!("expected the whole view");
        };
        check_received(&first, Side::Client).unwrap();
        assert_eq!(first.id, view);
        assert_eq!(view, core.current_view().id);

        // in step: 204
        let answer = ask(&image, std::slice::from_ref(&view)).unwrap();
        assert!(matches!(answer, ImageAnswer::Current { view: v } if v == view));

        // the server adds a mod: a delta from the view the player has
        smp(&core, &[("mods/a.jar", b"mod a"), ("mods/b.jar", b"mod b")]);
        let unknown = "f".repeat(64);
        let answer = ask(&image, &[unknown, view.clone()]).unwrap();
        let ImageAnswer::Delta { delta, view: next } = answer else {
            panic!("expected a delta");
        };
        assert_eq!(delta.from, view);
        let applied = apply(&first, &delta).unwrap();
        check_received(&applied, Side::Client).unwrap();
        assert_eq!(applied.id, next);

        // the routes, and the key only on the signed-in one
        let requests = core.requests();
        let prefix = if signed_in { "launcher" } else { "public" };
        for request in &requests {
            assert!(
                request.path.starts_with(&format!(
                    "/api/v1/{prefix}/instances/{SERVER}/image?side=client&rules=1"
                )),
                "{}",
                request.path
            );
            assert_eq!(request.authorization.is_some(), signed_in);
        }
        assert!(requests[1].path.ends_with(&format!("&have={view}")));
    }
}

#[test]
fn waits_while_the_server_prepares_its_files_and_gives_up_when_it_failed() {
    let core = ImageCore::start();
    smp(&core, &[("mods/a.jar", b"mod a")]);
    let (image, waits) = source(&core, true);
    core.fault(Fault::ImageUnavailable("Conflict", 5));
    core.fault(Fault::ImageUnavailable("Conflict", 3600));
    let preparing = std::sync::atomic::AtomicUsize::new(0);
    let answer = block_on(http::in_http_runtime(image.image(
        &[],
        &|| {
            preparing.fetch_add(1, Ordering::SeqCst);
        },
        &CancelToken::new(),
    )))
    .unwrap();
    assert!(matches!(answer, ImageAnswer::View { .. }));
    assert_eq!(preparing.load(Ordering::SeqCst), 2);
    // a Retry-After of an hour is waited 30 seconds at most
    assert_eq!(
        *waits.lock().unwrap(),
        [Duration::from_secs(5), Duration::from_secs(30)]
    );

    // the record failed: no retry
    core.fault(Fault::ImageUnavailable("Internal", 60));
    let before = core.requests().len();
    assert_eq!(
        ask(&image, &[]).unwrap_err(),
        ImageError::PrepareFailed { retry_after: 60 }
    );
    assert_eq!(core.requests().len(), before + 1);

    // preparing for longer than the Launcher waits
    for _ in 0..40 {
        core.fault(Fault::ImageUnavailable("Conflict", 30));
    }
    assert_eq!(ask(&image, &[]).unwrap_err(), ImageError::StillPreparing);
    // 600 s of waits in 30 s steps: 21 asks
    assert_eq!(core.requests().len(), before + 1 + 21);
}

/// A `Retry-After` of 0 is waited a second, so the waits still add up to their limits and
/// a hostile server can't make the Launcher ask again and again without pausing.
#[test]
fn a_retry_after_of_zero_still_waits_and_gives_up() {
    let core = ImageCore::start();
    let bytes = b"a small mod".to_vec();
    smp(&core, &[("mods/a.jar", &bytes)]);
    let (image, waits) = source(&core, true);
    let image = image.with_limits(Limits {
        object_idle: Duration::from_millis(400),
        image_idle: Duration::from_secs(5),
        preparing: Duration::from_secs(10),
        conflict: Duration::from_secs(10),
        busy: Duration::from_secs(10),
        ..Limits::default()
    });
    for _ in 0..100 {
        core.fault(Fault::ImageUnavailable("Conflict", 0));
    }
    assert_eq!(ask(&image, &[]).unwrap_err(), ImageError::StillPreparing);
    // 10 s of one-second waits: 11 asks
    assert_eq!(core.requests().len(), 11);
    assert!(
        waits
            .lock()
            .unwrap()
            .iter()
            .all(|w| *w == Duration::from_secs(1))
    );
    core.state().faults.clear();

    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("a.jar");
    for kind in ["TooManyRequests", "Conflict"] {
        for _ in 0..100 {
            core.fault(Fault::ObjectUnavailable(kind, 0));
        }
        let before = core.requests().len();
        assert!(
            matches!(
                fetch(&image, &bytes, &dest),
                Err(ImageError::FileFailed { .. })
            ),
            "{kind}"
        );
        assert_eq!(core.requests().len(), before + 11, "{kind}");
        core.state().faults.clear();
    }
}

#[test]
fn an_older_core_a_newer_core_and_unknown_schemas() {
    let core = ImageCore::start();
    smp(&core, &[("mods/a.jar", b"mod a")]);
    let (image, _) = source(&core, false);
    core.fault(Fault::ImageNotFound);
    assert_eq!(ask(&image, &[]).unwrap_err(), ImageError::OlderCore);
    core.fault(Fault::NewerRules);
    assert_eq!(ask(&image, &[]).unwrap_err(), ImageError::UpdateLodestone);
    // a newer core's rules on any answer, a 503 too
    core.fault(Fault::Raw {
        status: 503,
        headers: vec![
            ("X-Lodestone-Rules".into(), "99999999999".into()),
            ("Retry-After".into(), "1".into()),
        ],
        body: Vec::new(),
    });
    assert_eq!(ask(&image, &[]).unwrap_err(), ImageError::UpdateLodestone);
    // a view of a schema this Launcher doesn't read
    let id = "a".repeat(64);
    core.fault(Fault::Raw {
        status: 200,
        headers: vec![
            ("X-Lodestone-View".into(), id.clone()),
            ("X-Lodestone-Answer".into(), "view".into()),
            ("Content-Type".into(), "application/json".into()),
        ],
        body: br#"{"schema": 3, "kind": "content"}"#.to_vec(),
    });
    assert_eq!(ask(&image, &[]).unwrap_err(), ImageError::UpdateLodestone);
}

#[test]
fn hostile_and_broken_answers_are_refused() {
    let core = ImageCore::start();
    smp(&core, &[("mods/a.jar", b"mod a")]);
    let (image, _) = source(&core, true);
    let image = image.with_limits(Limits {
        max_answer: 1 << 20,
        ..Limits::default()
    });
    let invalid = |result: Result<ImageAnswer, ImageError>| {
        assert!(matches!(result, Err(ImageError::Invalid(_))), "{result:?}");
    };
    // a body that never ends stops at the bound
    core.fault(Fault::Endless);
    invalid(ask(&image, &[]));
    // one that says it is too large isn't read
    core.fault(Fault::DeclaredLength(600 << 20));
    invalid(ask(&image, &[]));
    core.fault(Fault::NotJson);
    invalid(ask(&image, &[]));
    // a 204 naming a view the player doesn't have
    core.fault(Fault::Raw {
        status: 204,
        headers: vec![("X-Lodestone-View".into(), "b".repeat(64))],
        body: Vec::new(),
    });
    invalid(ask(&image, &["c".repeat(64)]));
    // a 200 that doesn't say what it is, and one naming no view
    core.fault(Fault::Raw {
        status: 200,
        headers: vec![
            ("X-Lodestone-View".into(), "b".repeat(64)),
            ("Content-Type".into(), "application/json".into()),
        ],
        body: b"{}".to_vec(),
    });
    invalid(ask(&image, &[]));
    core.fault(Fault::Raw {
        status: 200,
        headers: vec![
            ("X-Lodestone-Answer".into(), "view".into()),
            ("Content-Type".into(), "application/json".into()),
        ],
        body: b"{}".to_vec(),
    });
    invalid(ask(&image, &[]));
    // a delta that isn't one: said so, for the caller to ask for the whole view (which
    // says whether the server needs a newer Lodestone), never used
    core.fault(Fault::Raw {
        status: 200,
        headers: vec![
            ("X-Lodestone-View".into(), "b".repeat(64)),
            ("X-Lodestone-Answer".into(), "delta".into()),
            ("Content-Type".into(), "application/json".into()),
        ],
        body: b"[1,2,3]".to_vec(),
    });
    assert!(matches!(
        ask(&image, &[]),
        Ok(ImageAnswer::UnreadableDelta { view, .. }) if view == "b".repeat(64)
    ));
    // a sign-in the core refuses
    let client = CoreClient::new(http::default_client().unwrap(), core.base.clone());
    let wrong = ServerImage::new(client, Some(Secret::new("lsk_wrong")), SERVER).unwrap();
    assert!(matches!(
        ask(&wrong, &[]),
        Err(ImageError::Core(launcher_core::cores::CoreError::SignedOut))
    ));
    // nothing names the key
    let err = ask(&wrong, &[]).unwrap_err();
    assert!(!format!("{err} {err:?}").contains("SECRET"));
}

fn fetch(image: &ServerImage, bytes: &[u8], dest: &std::path::Path) -> Result<(), ImageError> {
    block_on(http::in_http_runtime(image.object(
        &sha256_hex(bytes),
        bytes.len() as u64,
        dest,
        "mods/big.jar",
        &Progress::default(),
        &CancelToken::new(),
    )))
}

/// A large file (over 1 MiB, which the core sends in ranges).
fn big() -> Vec<u8> {
    (0..1_600_000u32).map(|i| (i % 251) as u8).collect()
}

#[test]
fn objects_resume_and_check_their_hash() {
    let core = ImageCore::start();
    let bytes = big();
    smp(&core, &[("mods/big.jar", &bytes)]);
    let (image, waits) = source(&core, false);
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("big.jar");

    // a body cut short, then the rest from where it ended
    core.fault(Fault::Short(700_000));
    fetch(&image, &bytes, &dest).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), bytes);
    let ranges: Vec<Option<String>> = core.requests().into_iter().map(|r| r.range).collect();
    assert_eq!(ranges, [None, Some("bytes=700000-".to_string())]);

    // a stall: what came is kept, and the rest resumed
    std::fs::remove_file(&dest).unwrap();
    core.fault(Fault::Stall(Duration::from_secs(3)));
    fetch(&image, &bytes, &dest).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), bytes);
    // from what had arrived when it stalled (the half sent, less what was in flight)
    let resumed = core.requests().last().unwrap().range.clone().unwrap();
    let from: u64 = resumed
        .strip_prefix("bytes=")
        .and_then(|r| r.strip_suffix('-'))
        .unwrap()
        .parse()
        .unwrap();
    assert!(from > 0 && from <= 800_000, "{resumed}");

    // a server that ignores the range sends the whole file again: it starts over
    std::fs::write(&dest, &bytes[..1000]).unwrap();
    core.fault(Fault::IgnoreRange);
    fetch(&image, &bytes, &dest).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), bytes);

    // 416: the partial file goes, and the next fetch is whole
    std::fs::write(&dest, &bytes[..1000]).unwrap();
    core.fault(Fault::Raw {
        status: 416,
        headers: vec![("Content-Range".into(), format!("bytes */{}", bytes.len()))],
        body: Vec::new(),
    });
    let before = core.requests().len();
    fetch(&image, &bytes, &dest).unwrap();
    assert_eq!(core.requests()[before + 1].range, None);

    // wrong bytes once, then right
    std::fs::remove_file(&dest).unwrap();
    core.fault(Fault::WrongBytes);
    fetch(&image, &bytes, &dest).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), bytes);

    // wrong bytes three times in a row: fetched again each time, the fourth works
    std::fs::remove_file(&dest).unwrap();
    for _ in 0..3 {
        core.fault(Fault::WrongBytes);
    }
    let before = core.requests().len();
    fetch(&image, &bytes, &dest).unwrap();
    assert_eq!(core.requests().len(), before + 4);
    assert_eq!(std::fs::read(&dest).unwrap(), bytes);

    // wrong bytes every time: given up on after three retries, nothing left behind
    std::fs::remove_file(&dest).unwrap();
    for _ in 0..4 {
        core.fault(Fault::WrongBytes);
    }
    let before = core.requests().len();
    assert_eq!(
        fetch(&image, &bytes, &dest).unwrap_err(),
        ImageError::FileFailed {
            path: "mods/big.jar".into()
        }
    );
    assert_eq!(core.requests().len(), before + 4);
    assert!(!dest.exists());
    // each failed try waited a moment first
    assert!(
        waits
            .lock()
            .unwrap()
            .iter()
            .all(|w| *w <= Duration::from_secs(30))
    );
}

#[test]
fn busy_lost_and_failing_objects() {
    let core = ImageCore::start();
    let bytes = b"a small mod".to_vec();
    smp(&core, &[("mods/a.jar", &bytes)]);
    let (image, waits) = source(&core, true);
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("a.jar");

    // too many downloads: waited out, never counted as failures
    for _ in 0..6 {
        core.fault(Fault::ObjectUnavailable("TooManyRequests", 1));
    }
    fetch(&image, &bytes, &dest).unwrap();
    assert_eq!(waits.lock().unwrap().len(), 6);

    // a server error, then the file
    core.fault(Fault::ObjectStatus(500));
    fetch(&image, &bytes, &dest).unwrap();

    // the store lost the file: asked again for two minutes, then given up on
    for _ in 0..30 {
        core.fault(Fault::ObjectUnavailable("Conflict", 5));
    }
    let before = core.requests().len();
    assert!(matches!(
        fetch(&image, &bytes, &dest),
        Err(ImageError::FileFailed { .. })
    ));
    assert_eq!(core.requests().len(), before + 25);
    core.state().faults.clear();

    // three Internal answers in a row: given up on at the third
    for _ in 0..3 {
        core.fault(Fault::ObjectUnavailable("Internal", 60));
    }
    let before = core.requests().len();
    assert!(matches!(
        fetch(&image, &bytes, &dest),
        Err(ImageError::FileFailed { .. })
    ));
    assert_eq!(core.requests().len(), before + 3);
    // two, then the file
    for _ in 0..2 {
        core.fault(Fault::ObjectUnavailable("Internal", 60));
    }
    fetch(&image, &bytes, &dest).unwrap();

    // not served any more: the view changed
    core.fault(Fault::ObjectStatus(404));
    assert_eq!(
        fetch(&image, &bytes, &dest).unwrap_err(),
        ImageError::ViewChanged
    );
    // more bytes than the file: not the file
    let longer = b"a small mod, and then some".to_vec();
    core.state().objects.insert(sha256_hex(&bytes), longer);
    assert!(matches!(
        fetch(&image, &bytes, &dest),
        Err(ImageError::FileFailed { .. })
    ));
}

#[test]
fn at_most_four_objects_download_at_once() {
    let core = ImageCore::start();
    let files: Vec<(String, Vec<u8>)> = (0..10)
        .map(|i| (format!("mods/m{i}.jar"), format!("mod {i}").into_bytes()))
        .collect();
    let refs: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(p, b)| (p.as_str(), b.as_slice()))
        .collect();
    smp(&core, &refs);
    core.state().object_delay = Duration::from_millis(150);
    let (image, _) = source(&core, false);
    let dir = tempfile::tempdir().unwrap();
    let items: Vec<ObjectItem> = files
        .iter()
        .map(|(path, bytes)| ObjectItem {
            sha256: sha256_hex(bytes),
            size: bytes.len() as u64,
            dest: dir.path().join(path),
            path: path.clone(),
        })
        .collect();
    let progress = Progress::default();
    block_on(http::in_http_runtime(image.objects(
        &items,
        &progress,
        &CancelToken::new(),
    )))
    .unwrap();
    for (path, bytes) in &files {
        assert_eq!(&std::fs::read(dir.path().join(path)).unwrap(), bytes);
    }
    let most = core.most.load(Ordering::SeqCst);
    assert!((2..=4).contains(&most), "{most} at once");
    let done = progress.snapshot();
    assert_eq!(done.files_done, 10);
    assert_eq!(
        done.bytes_done,
        files.iter().map(|(_, b)| b.len() as u64).sum::<u64>()
    );
}

/// Cancel ends a download at once, though the server stalls in the middle of a body for
/// far longer than that (a hostile server may send a byte now and then for ever): every
/// wait for the server races the token.
#[test]
fn cancel_ends_a_stalled_download_at_once() {
    let core = ImageCore::start();
    let bytes = big();
    smp(&core, &[("mods/big.jar", &bytes)]);
    let client = CoreClient::new(http::default_client().unwrap(), core.base.clone());
    let image = ServerImage::new(client, None, SERVER)
        .unwrap()
        .with_sleep(recording_sleep().0)
        .with_limits(Limits {
            object_idle: Duration::from_secs(60),
            ..Limits::default()
        });
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("big.jar");
    core.fault(Fault::Stall(Duration::from_secs(20)));
    let cancel = CancelToken::new();
    let canceller = std::thread::spawn({
        let cancel = cancel.clone();
        move || {
            std::thread::sleep(Duration::from_millis(300));
            cancel.cancel();
        }
    });
    let started = std::time::Instant::now();
    let fetched = block_on(http::in_http_runtime(image.object(
        &sha256_hex(&bytes),
        bytes.len() as u64,
        &dest,
        "mods/big.jar",
        &Progress::default(),
        &cancel,
    )));
    canceller.join().unwrap();
    assert_eq!(fetched, Err(ImageError::Cancelled));
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
    // what was kept of it is thrown away
    assert!(!dest.exists());
}
