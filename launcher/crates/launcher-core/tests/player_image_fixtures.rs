//! The real core's answers to a player, recorded by the core's own test
//! (`core/src/images/player_tests.rs`, `records_the_answers_a_player_gets`, with
//! `LODESTONE_RECORD_PLAYER_FIXTURES` set to `tests/fixtures/player_image/`), replayed
//! through the Launcher's client and checks: the whole view, in step (204), a delta after
//! the server added a mod, and that mod's bytes.

mod image_core;

use std::path::Path;

use futures::executor::block_on;
use image_core::{Fault, ImageCore, KEY, SERVER};
use launcher_core::{
    Secret,
    cores::{
        CoreClient,
        image_sync::received::{apply_delta, check_view},
        player_image::{ImageAnswer, ServerImage},
    },
    game::download::{CancelToken, Progress, recording_sleep},
    http,
};

/// A recorded answer, as the fake core's `Raw` fault.
fn answer(name: &str) -> Fault {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/player_image");
    let meta: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join(format!("{name}.json"))).unwrap()).unwrap();
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
    let server = ServerImage::new(client, Some(Secret::new(KEY)), SERVER)
        .unwrap()
        .with_sleep(recording_sleep().0);
    let ask = |have: &[String]| {
        block_on(http::in_http_runtime(server.image(
            have,
            &|| {},
            &CancelToken::new(),
        )))
        .unwrap()
    };

    core.fault(answer("view"));
    let ImageAnswer::View { image, view } = ask(&[]) else {
        panic!("the whole view");
    };
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
}
