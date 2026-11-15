//! The real core's answers to a player, recorded by the core's own test
//! (`core/src/images/player_tests.rs`, `records_the_answers_a_player_gets`, with
//! `LODESTONE_RECORD_PLAYER_FIXTURES` set to `tests/fixtures/player_image/`), replayed
//! through the Launcher's client and checks: the server still preparing its files (503
//! `Conflict`, for the image and for an object), the whole view, in step (204), a delta
//! after the server added a mod, that mod's bytes, and a query the core refuses (400 with
//! its rules), so the error shapes the client depends on (the JSON body and its
//! `Content-Type`, `Retry-After`, `X-Lodestone-Rules`) can't drift from the core's.

mod image_core;

use std::{path::Path, sync::atomic::AtomicUsize, sync::atomic::Ordering, time::Duration};

use futures::executor::block_on;
use image_core::{Fault, ImageCore, KEY, SERVER};
use launcher_core::{
    Secret,
    cores::{
        CoreClient, CoreError,
        image_sync::received::{apply_delta, check_view},
        player_image::{ImageAnswer, ImageError, ServerImage},
    },
    game::download::{CancelToken, Progress, recording_sleep},
    http,
};

/// A recorded answer's status and headers.
fn meta(name: &str) -> serde_json::Value {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/player_image");
    serde_json::from_slice(&std::fs::read(dir.join(format!("{name}.json"))).unwrap()).unwrap()
}

/// A recorded answer, as the fake core's `Raw` fault.
fn answer(name: &str) -> Fault {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/player_image");
    let meta = meta(name);
    let headers = meta["headers"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, value)| (name.clone(), value.as_str().unwrap().to_string()))
        .collect();
    Fault::Raw {
        status: meta["status"].as_u64().unwrap() as u16,
        headers,
        body: std::fs::read(dir.join(format!("{name}.body"))).unwrap(),
    }
}

#[test]
fn the_real_cores_answers_pass_the_launchers_checks() {
    let core = ImageCore::start();
    let client = CoreClient::new(http::default_client().unwrap(), core.base.clone());
    let (sleep, waits) = recording_sleep();
    let server = ServerImage::new(client, Some(Secret::new(KEY)), SERVER)
        .unwrap()
        .with_sleep(sleep);
    let preparing = AtomicUsize::new(0);
    let on_preparing = || {
        preparing.fetch_add(1, Ordering::SeqCst);
    };
    let try_ask = |have: &[String]| {
        block_on(http::in_http_runtime(server.image(
            have,
            &on_preparing,
            &CancelToken::new(),
        )))
    };
    let ask = |have: &[String]| try_ask(have).unwrap();

    // still preparing (`Conflict`, not `Internal`, which would give up): asked again
    // after the server's 5 seconds, then the whole view
    core.fault(answer("image_preparing"));
    core.fault(answer("view"));
    let ImageAnswer::View { image, view } = ask(&[]) else {
        panic!("the whole view");
    };
    assert_eq!(preparing.load(Ordering::SeqCst), 1);
    assert_eq!(*waits.lock().unwrap(), [Duration::from_secs(5)]);
    check_view(&image, &view).unwrap();
    let first = *image;

    core.fault(answer("current"));
    assert!(matches!(
        ask(std::slice::from_ref(&first.id)),
        ImageAnswer::Current { view } if view == first.id
    ));

    core.fault(answer("delta"));
    let ImageAnswer::Delta { delta, view } = ask(std::slice::from_ref(&first.id)) else {
        panic!("a delta");
    };
    let second = apply_delta(&first, &delta, &view).unwrap();
    let added = second
        .entries
        .iter()
        .find(|e| e.path.as_str() == "mods/added.jar")
        .unwrap();

    // an object while the server prepares: its JSON body says `Conflict`, which is waited
    // out (the server's 5 seconds), then the file
    let object_preparing = meta("object_preparing");
    assert!(
        object_preparing["headers"]["content-type"]
            .as_str()
            .unwrap()
            .starts_with("application/json")
    );
    let Fault::Raw { body, .. } = answer("object_preparing") else {
        unreachable!()
    };
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap()["kind"],
        "Conflict"
    );
    waits.lock().unwrap().clear();
    core.fault(answer("object_preparing"));
    core.fault(answer("object"));
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("added.jar");
    block_on(http::in_http_runtime(server.object(
        &added.sha256,
        added.size,
        &dest,
        "mods/added.jar",
        &Progress::default(),
        &CancelToken::new(),
    )))
    .unwrap();
    assert_eq!(std::fs::metadata(&dest).unwrap().len(), added.size);
    assert_eq!(*waits.lock().unwrap(), [Duration::from_secs(5)]);

    // a refused query: the core's error, not "update Lodestone" (its rules are this
    // Launcher's)
    assert_eq!(
        meta("bad_request")["headers"]["x-lodestone-rules"],
        lodestone_image::RULES.to_string()
    );
    core.fault(answer("bad_request"));
    assert!(matches!(
        try_ask(&[]),
        Err(ImageError::Core(CoreError::Server { status: 400, .. }))
    ));
}
