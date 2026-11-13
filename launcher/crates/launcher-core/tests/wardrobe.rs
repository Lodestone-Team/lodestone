//! Changing the skin and cape against the loopback mock of the Minecraft services.

mod support;

use futures::executor::block_on;
use launcher_core::{
    Secret,
    auth::wardrobe::{self, SkinModel, WardrobeError},
    http, skin,
};
use support::{CAPES, MC_TOKEN, MockServer, SKIN_HASH, UPLOADED_SKIN_HASH};

fn skin_png() -> Vec<u8> {
    skin::encode_png(&skin::sample_skin(true)).unwrap()
}

#[test]
fn the_skin_is_uploaded_reset_and_read_back() {
    let mock = MockServer::start();
    let client = http::default_client().unwrap();
    let endpoints = mock.config().endpoints;
    let token = Secret::new(MC_TOKEN);

    let before = block_on(wardrobe::wardrobe(&*client, &endpoints, &token)).unwrap();
    assert_eq!(
        before.skin_url.as_deref(),
        Some(format!("http://textures.minecraft.net/texture/{SKIN_HASH}").as_str())
    );
    assert_eq!(before.model, SkinModel::Classic);
    assert_eq!(
        before
            .capes
            .iter()
            .map(|cape| (cape.id.as_str(), cape.name.as_str(), cape.active))
            .collect::<Vec<_>>(),
        vec![
            (CAPES[0].0, CAPES[0].1, false),
            (CAPES[1].0, CAPES[1].1, false)
        ]
    );

    let png = skin_png();
    let uploaded = block_on(wardrobe::upload_skin(
        &*client,
        &endpoints,
        &token,
        &png,
        SkinModel::Slim,
    ))
    .unwrap();
    assert_eq!(
        uploaded.skin_url.as_deref(),
        Some(format!("http://textures.minecraft.net/texture/{UPLOADED_SKIN_HASH}").as_str())
    );
    assert_eq!(uploaded.model, SkinModel::Slim);
    let upload = mock
        .requests()
        .into_iter()
        .find(|request| request.method == "POST" && request.path == "/mc/minecraft/profile/skins")
        .unwrap();
    assert!(upload.body.contains("name=\"variant\"\r\n\r\nslim\r\n"));
    assert!(upload.body.contains("filename=\"skin.png\""));
    // the token goes in the header, never in the body
    assert!(!upload.body.contains(MC_TOKEN));

    let reset = block_on(wardrobe::reset_skin(&*client, &endpoints, &token)).unwrap();
    assert_eq!(reset.skin_url, None);
    assert_eq!(reset.model, SkinModel::Classic);
}

#[test]
fn a_cape_is_put_on_and_taken_off() {
    let mock = MockServer::start();
    let client = http::default_client().unwrap();
    let endpoints = mock.config().endpoints;
    let token = Secret::new(MC_TOKEN);

    let worn = block_on(wardrobe::set_cape(
        &*client,
        &endpoints,
        &token,
        Some(CAPES[1].0),
    ))
    .unwrap();
    assert_eq!(
        worn.active_cape().map(|cape| cape.name.as_str()),
        Some("Vanilla")
    );
    assert_eq!(mock.state().active_cape.as_deref(), Some(CAPES[1].0));

    let none = block_on(wardrobe::set_cape(&*client, &endpoints, &token, None)).unwrap();
    assert_eq!(none.active_cape(), None);

    // a cape the account doesn't own
    let error = block_on(wardrobe::set_cape(
        &*client,
        &endpoints,
        &token,
        Some("someone-elses"),
    ))
    .unwrap_err();
    assert_eq!(
        error,
        WardrobeError::Refused {
            status: 400,
            message: Some("profile does not own cape".into())
        }
    );
}

#[test]
fn refusals_say_why_and_nothing_bad_is_sent() {
    let mock = MockServer::start();
    let client = http::default_client().unwrap();
    let endpoints = mock.config().endpoints;
    let token = Secret::new(MC_TOKEN);

    // not a skin: refused here, before anything is sent
    let error = block_on(wardrobe::upload_skin(
        &*client,
        &endpoints,
        &token,
        b"not a png",
        SkinModel::Classic,
    ))
    .unwrap_err();
    assert!(matches!(error, WardrobeError::Refused { status: 400, .. }));
    assert!(mock.requests().is_empty());

    // an expired token
    let stale = Secret::new("stale");
    assert_eq!(
        block_on(wardrobe::reset_skin(&*client, &endpoints, &stale)).unwrap_err(),
        WardrobeError::SignedOut
    );
    assert_eq!(
        block_on(wardrobe::wardrobe(&*client, &endpoints, &stale)).unwrap_err(),
        WardrobeError::SignedOut
    );

    // the services' own refusals
    mock.state().wardrobe_error = Some((429, ""));
    assert_eq!(
        block_on(wardrobe::set_cape(&*client, &endpoints, &token, None)).unwrap_err(),
        WardrobeError::RateLimited
    );
    mock.state().wardrobe_error = Some((400, "Invalid skin image"));
    let error = block_on(wardrobe::upload_skin(
        &*client,
        &endpoints,
        &token,
        &skin_png(),
        SkinModel::Classic,
    ))
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "the Minecraft services refused (400): Invalid skin image"
    );
    // and the skin is as it was
    assert_eq!(mock.state().skin.as_ref().unwrap().0, SKIN_HASH);
}
