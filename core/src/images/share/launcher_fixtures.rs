//! The files the Launcher's tests read from a real core (`launcher/crates/launcher-core/
//! tests/fixtures/core_images/`): two backups exported as `.lodeimg` files, a Fabric
//! server's and a Paper server's, and the end of a creation from a file, with its outcome.
//! The Launcher builds an instance from a core's export ("Download this world") and reads
//! the creation's end ("Host this"), so its tests check against what this core really
//! writes rather than a hand-made copy. Set `LODESTONE_RECORD_CORE_IMAGE_FIXTURES` to a
//! folder to write them there; without it the test only checks that they can be made.

use std::path::{Path, PathBuf};

use lodestone_image::container::{self, Limits};
use reqwest::{Method, StatusCode};
use serde_json::{json, Value};

use super::tests::{core, fabric_jar, launcher_file, lithium, sodium, Core};
use crate::events::{EventInner, ProgressionEndValue, ProgressionEventInner};
use crate::images::ModrinthHint;
use crate::test_util::wait_until;
use crate::types::InstanceUuid;

fn write(root: &Path, relative: &str, contents: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

/// Takes a backup of the server `uuid` and exports it. Returns the downloaded file.
async fn export_server(core: &Core, token: &str, uuid: &InstanceUuid) -> (Value, PathBuf) {
    let backups = format!("/instance/{uuid}/backups");
    let (status, body) = core
        .call(Method::POST, &backups, token, Some(json!({})))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    wait_until("the backup is taken", || async {
        let (_, list) = core.call(Method::GET, &backups, token, None).await;
        list["running"].is_null() && list["backups"][0]["id"].is_string()
    })
    .await;
    let (_, list) = core.call(Method::GET, &backups, token, None).await;
    let id = list["backups"][0]["id"].as_str().unwrap().to_string();
    let (status, reply) = core
        .call(
            Method::POST,
            &format!("/instance/{uuid}/backups/{id}/export"),
            token,
            Some(json!({})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    let bytes = core
        .client
        .get(format!(
            "{}/file/{}",
            core.base,
            reply["key"].as_str().unwrap()
        ))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    let out = core
        .dir
        .path()
        .join(format!("{}.lodeimg", crate::util::rand_alphanumeric(8)));
    std::fs::write(&out, &bytes).unwrap();
    (reply, out)
}

/// The placed paths of an export, data and content.
fn placed_paths(file: &Path) -> Vec<String> {
    let opened = container::open(file, &Limits::default()).unwrap();
    lodestone_image::share::placed_entries(opened.image(), opened.linked())
        .into_iter()
        .map(|e| e.path.as_str().to_string())
        .collect()
}

#[tokio::test]
async fn records_the_files_a_launcher_reads() {
    let core = core().await;
    let owner = core.owner().await;

    // 1. A Fabric server: a Modrinth mod the Launcher downloads (Sodium) and one it can't
    // (Lithium, which the Launcher's test serves as missing), a mod only the server runs,
    // one only clients run, the owner's own mod marked for players (carried), a config no
    // one marked, besides the secrets and server files `core()` has.
    core.modrinth.add(lithium());
    let lithium_file = &lithium().version.files[0];
    core.state.images.note_modrinth(
        &lithium_file.sha512,
        ModrinthHint {
            url: Some(lithium_file.url.clone()),
            ..Default::default()
        },
    );
    write(&core.root, "mods/lithium.jar", &lithium().content);
    write(&core.root, "mods/dash.jar", &fabric_jar("dash", "server"));
    write(&core.root, "mods/zoom.jar", &fabric_jar("zoom", "client"));
    write(
        &core.root,
        &format!("mods/{}", crate::mods::client::MARKS_FILE),
        br#"{"required":["own.jar"],"excluded":[]}"#,
    );
    let (_, fabric) = export_server(&core, &owner, &core.uuid).await;
    let paths = placed_paths(&fabric);
    for path in [
        "mods/sodium.jar",
        "mods/lithium.jar",
        "mods/dash.jar",
        "mods/zoom.jar",
        "mods/own.jar",
        "config/sodium-options.json",
        "server.properties",
        "ops.json",
        "saves/world/level.dat",
    ] {
        assert!(paths.iter().any(|p| p == path), "{path} in {paths:?}");
    }

    // 2. The end of a creation from a Launcher's file that leaves things out: a mod that
    // says it runs only on clients, a CurseForge file this core can't download, a script
    let file = launcher_file(core.dir.path(), |_, content, store| {
        let script = store.put_bytes(b"echo hi").unwrap();
        content.entries.push(lodestone_image::Entry::new(
            lodestone_image::CanonicalPath::new("kubejs/server_scripts/a.js").unwrap(),
            7,
            script.sha256,
            lodestone_image::Source::Object {},
        ));
    });
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    let mut events = core.state.event_broadcaster.subscribe();
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "From a file",
                "port": 25671, "world": "A"
            }),
        )
        .await
        .unwrap();
    // the creation's end, as `GET /events/all/buffer` gives it
    let mut end = None;
    while let Ok(event) = events.try_recv() {
        if let EventInner::ProgressionEvent(p) = &event.event_inner {
            if let ProgressionEventInner::ProgressionEnd {
                inner: Some(ProgressionEndValue::InstanceCreation(created)),
                ..
            } = p.progression_event_inner()
            {
                if created.info.uuid == made.uuid {
                    end = Some(serde_json::to_value(&event).unwrap());
                }
            }
        }
    }
    let end = end.expect("the creation's end was sent");
    let outcome = &end["event_inner"]["progression_event_inner"]["inner"]["outcome"];
    assert_eq!(
        outcome["no_source"]["names"],
        json!(["mods/cf.jar"]),
        "{end}"
    );
    assert_eq!(outcome["client_jars"]["count"], 1, "{end}");
    drop(made);

    // 3. A Paper server made from a Launcher's file, then given what a Paper server has: a
    // plugin and its settings, Paper's per-world settings, and its split Nether and End
    let file = launcher_file(core.dir.path(), |data, content, _| {
        for image in [data, content] {
            image.game.loader = lodestone_image::Loader::Vanilla;
            image.game.loader_version = None;
            image.settings.server = Some(lodestone_image::settings::ServerSettings {
                flavour: Some(lodestone_image::settings::ServerFlavour::Paper),
                ..Default::default()
            });
            image
                .entries
                .retain(|e| !e.path.as_str().starts_with("mods/"));
        }
    });
    let (upload_id, summary) = core.upload_ok(&owner, &file).await;
    let made = core
        .create_and_wait(
            &owner,
            json!({
                "upload_id": upload_id, "image": summary["image"], "name": "Paper",
                "port": 25670, "world": "A"
            }),
        )
        .await
        .unwrap();
    write(&made.root, "plugins/Spy.jar", b"a plugin jar");
    write(&made.root, "plugins/Spy/config.yml", b"spy: true\n");
    write(&made.root, "world/paper-world.yml", b"chunks: {}\n");
    write(&made.root, "world_nether/paper-world.yml", b"chunks: {}\n");
    write(&made.root, "world_nether/level.dat", b"nether level");
    write(&made.root, "world_the_end/DIM1/region/r.0.0.mca", b"end A");
    write(&made.root, "world_the_end/level.dat", b"end level");
    write(
        &made.root,
        "ops.json",
        br#"[{"uuid":"069a79f4-44e9-4726-a5be-fca90e38aaf5","name":"Notch","level":4,"bypassesPlayerLimit":false}]"#,
    );
    let (_, paper) = export_server(&core, &owner, &made.uuid).await;
    let paths = placed_paths(&paper);
    for path in [
        "saves/world/level.dat",
        "saves/world/DIM-1/region/r.0.0.mca",
        "saves/world/DIM1/region/r.0.0.mca",
        "saves/world/paper-world.yml",
        "saves/world/.bukkit_nether/paper-world.yml",
        "saves/world/.bukkit_the_end/level.dat",
        "plugins/Spy.jar",
        "server.properties",
        "ops.json",
    ] {
        assert!(paths.iter().any(|p| p == path), "{path} in {paths:?}");
    }
    // a plugin's settings stay home unless the owner includes them
    assert!(!paths.iter().any(|p| p.starts_with("plugins/Spy/")));

    if let Some(dir) = std::env::var_os("LODESTONE_RECORD_CORE_IMAGE_FIXTURES") {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(&fabric, dir.join("fabric-backup.lodeimg")).unwrap();
        std::fs::copy(&paper, dir.join("paper-backup.lodeimg")).unwrap();
        std::fs::write(
            dir.join("creation-end.json"),
            serde_json::to_vec_pretty(&end).unwrap(),
        )
        .unwrap();
        // what the fake Modrinth serves for the mods the files name
        std::fs::write(dir.join("sodium.jar"), sodium().content).unwrap();
        std::fs::write(dir.join("lithium.jar"), lithium().content).unwrap();
    }
}

/// The Launcher asks whether a core makes servers from files before it writes one: an
/// upload with no file is 400 on a core that does, 403 for a user who can't create
/// servers, and (on a core older than milestone 6) 404.
#[tokio::test]
async fn an_upload_without_a_file_says_the_core_takes_files() {
    let core = core().await;
    let owner = core.owner().await;
    let probe = |token: String| {
        let request = core
            .client
            .post(format!("{}/instance/image/upload", core.base))
            .bearer_auth(token)
            .header(
                "Content-Type",
                "multipart/form-data; boundary=----lodestone-probe",
            )
            .body("------lodestone-probe--\r\n");
        async move { request.send().await.unwrap().status() }
    };
    assert_eq!(probe(owner.clone()).await, StatusCode::BAD_REQUEST);
    let member = core.member("member", |_| {}).await;
    assert_eq!(probe(member).await, StatusCode::FORBIDDEN);
    // a route the core doesn't have: what an older core answers
    let status = core
        .client
        .post(format!("{}/instance/image/nothing-here", core.base))
        .bearer_auth(&owner)
        .send()
        .await
        .unwrap()
        .status();
    assert!(
        status == StatusCode::NOT_FOUND || status == StatusCode::METHOD_NOT_ALLOWED,
        "{status}"
    );
}
