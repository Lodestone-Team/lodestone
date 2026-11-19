//! CurseForge on a server, end to end: the real `lodestone_core` program, and a mock of
//! CurseForge's API and CDN (launcher-core's). The Host's own code does what its Mods
//! tab does: it asks CurseForge, downloads, and uploads to the core, which has no
//! CurseForge client.
//!
//! Needs the core's program and `LODESTONE_HOST_NETWORK_TESTS=1` (the core downloads a
//! Fabric server to make the instance), as `instance_tabs.rs`; without them the test
//! says so and passes. No server is ever started, and the Minecraft EULA is never
//! accepted.

#[path = "../../launcher-core/tests/curseforge/mod.rs"]
mod curseforge;

use std::{
    io::Write as _,
    sync::Arc,
    time::{Duration, Instant},
};

use curseforge::{CurseForgeMock, FakeFile, fabric_jar};
use futures::executor::block_on;
use launcher_core::{
    Secret,
    content::curseforge::CLASS_MODS,
    cores::{
        CoreClient,
        admin::{
            AdminClient,
            curseforge::{self as server, ServerTarget},
            curseforge_pack,
            mods::{InstalledModKind, InstalledModLoader},
        },
        local_claim::OwnerCredentials,
    },
    game::download::{CancelToken, Downloader, Sleep},
    http,
};
use lodestone_host::{
    core_process::{self, CoreConfig, CoreProcess},
    session,
};

const GAME: &str = "1.21.1";
const TAGS: [&str; 3] = [GAME, "Fabric", "Server"];
const CREATE: u64 = 328_085;
const FABRIC_API: u64 = 306_612;
const PACK: u64 = 900_001;
const NEW: &str = "2025-02-01T00:00:00Z";
const OLD: &str = "2024-02-01T00:00:00Z";

fn sleep() -> Sleep {
    Arc::new(|duration| Box::pin(async move { std::thread::sleep(duration) }))
}

/// A core in a temporary folder, signed in as its owner. Stops when dropped.
struct TestCore {
    process: CoreProcess,
    admin: AdminClient,
    _data: tempfile::TempDir,
}

impl Drop for TestCore {
    fn drop(&mut self) {
        self.process.stop(Duration::from_secs(30));
    }
}

fn start_core(mut ports: std::ops::Range<u16>) -> Option<TestCore> {
    let program = match core_process::find_core() {
        Ok(program) => program,
        Err(why) => {
            eprintln!("skipped: {why}");
            return None;
        }
    };
    let data = tempfile::tempdir().unwrap();
    let port = ports
        .find(|&port| core_process::port_is_free(port))
        .expect("a free port");
    let config = CoreConfig {
        program,
        data_dir: data.path().join("core"),
        port,
        log_file: data.path().join("core.log"),
        connect_service: CoreConfig::connect_service_now(),
    };
    let mut process = CoreProcess::spawn(&config).expect("the core starts");
    let http = http::default_client().unwrap();
    let local = CoreClient::new(http.clone(), config.local_url());
    let started = Instant::now();
    while block_on(local.info()).is_err() {
        assert!(process.try_wait().is_none(), "the core exited");
        assert!(started.elapsed() < Duration::from_secs(90), "no answer");
        std::thread::sleep(Duration::from_millis(250));
    }
    let owner = OwnerCredentials {
        username: "curseforge_owner".into(),
        password: Secret::new(String::from("a long test password 789")),
    };
    let session = block_on(session::claim_owner(
        http,
        config.local_url(),
        &config.data_dir,
        Some(owner),
        sleep(),
    ))
    .expect("signed in");
    Some(TestCore {
        process,
        admin: session.admin,
        _data: data,
    })
}

/// A CurseForge pack zip for Fabric on [`GAME`] listing `files` (project, file).
fn pack_zip(files: &[(u64, u64)]) -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut bytes);
        let options = zip::write::SimpleFileOptions::default();
        let manifest = serde_json::json!({
            "minecraft": { "version": GAME, "modLoaders": [{ "id": "fabric-0.16.9", "primary": true }] },
            "manifestType": "minecraftModpack",
            "manifestVersion": 1,
            "name": "A Pack",
            "version": "2.5",
            "files": files.iter().map(|(p, f)| serde_json::json!({
                "projectID": p, "fileID": f, "required": true
            })).collect::<Vec<_>>(),
            "overrides": "overrides",
        });
        zip.start_file("manifest.json", options).unwrap();
        zip.write_all(manifest.to_string().as_bytes()).unwrap();
        zip.start_file("overrides/config/create.toml", options)
            .unwrap();
        zip.write_all(b"setting = true").unwrap();
        zip.finish().unwrap();
    }
    bytes.into_inner()
}

#[test]
fn the_real_core_takes_what_the_host_makes_of_curseforge() {
    if std::env::var("LODESTONE_HOST_NETWORK_TESTS").as_deref() != Ok("1") {
        eprintln!("skipped: set LODESTONE_HOST_NETWORK_TESTS=1 to download a Fabric server");
        return;
    }
    let Some(core) = start_core(17_700..17_800) else {
        return;
    };
    let admin = &core.admin;
    let setup = serde_json::json!({
        "name": "CurseForge test", "description": "", "auto_start": false,
        "restart_on_crash": false, "accept_eula": false,
        "setting_sections": {
            "section_1": { "settings": {
                "version": { "value": { "type": "Enum", "value": GAME } },
                "port": { "value": { "type": "UnsignedInteger", "value": 25591 } } } },
            "section_2": { "settings": {
                "min_ram": { "value": { "type": "UnsignedInteger", "value": 1024 } },
                "max_ram": { "value": { "type": "UnsignedInteger", "value": 2048 } },
                "cmd_args": { "value": null } } }
        }
    });
    block_on(admin.call(
        http_client::Method::POST,
        "instance/create/MinecraftFabric",
        Some(setup),
    ))
    .expect("created");
    let started = Instant::now();
    let uuid = loop {
        if let Some(i) = block_on(admin.instances()).unwrap().first() {
            break i.uuid.clone();
        }
        assert!(started.elapsed() < Duration::from_secs(300), "not made");
        std::thread::sleep(Duration::from_secs(1));
    };

    let mock = CurseForgeMock::start();
    mock.project(CREATE, "create-fabric", CLASS_MODS)
        .project(FABRIC_API, "fabric-api", CLASS_MODS)
        .project(PACK, "a-pack", 4471);
    mock.file(
        FakeFile::new(
            5001,
            CREATE,
            "create-0.5.1.jar",
            &TAGS,
            NEW,
            fabric_jar("create", "0.5.1"),
        )
        .requires(FABRIC_API),
    )
    .file(FakeFile::new(
        4001,
        FABRIC_API,
        "fabric-api-0.90.jar",
        &TAGS,
        OLD,
        fabric_jar("fabric-api", "0.90"),
    ));
    let cf = mock.client();
    let downloader = Downloader::new(http::download_client().unwrap(), sleep()).with_attempts(1);
    let target = ServerTarget {
        kind: InstalledModKind::Mod,
        loader: InstalledModLoader::Fabric,
        game_version: GAME.into(),
    };
    let install = |file: u64| {
        let list = block_on(admin.installed_mods(&uuid, false)).unwrap();
        let known = block_on(server::identify(&cf, &list.mods)).unwrap();
        let project = mock
            .state()
            .files
            .iter()
            .find(|f| f.id == file)
            .unwrap()
            .mod_id;
        let file = block_on(cf.file(project, file)).unwrap();
        block_on(http::in_http_runtime(server::install_file(
            &cf,
            None,
            &downloader,
            admin,
            &uuid,
            &target,
            &list.mods,
            &known,
            file,
            None,
            true,
            false,
        )))
    };

    // the list the core gives is what the Host's target is made from
    let empty = block_on(admin.installed_mods(&uuid, false)).unwrap();
    assert_eq!((empty.kind, empty.loader), (target.kind, target.loader));
    assert_eq!(empty.game_version, GAME);
    assert!(empty.mods.is_empty());

    // Create and the Fabric API it requires: downloaded here, uploaded to the core
    let outcome = install(5001).expect("installed");
    assert_eq!(outcome.result.installed.len(), 2);
    let list = block_on(admin.installed_mods(&uuid, false)).unwrap();
    let names: Vec<&str> = list.mods.iter().map(|m| m.file_name.as_str()).collect();
    assert_eq!(names, ["create-0.5.1.jar", "fabric-api-0.90.jar"]);
    // the core says each jar's CurseForge fingerprint, and CurseForge knows them by it
    assert!(list.mods.iter().all(|m| m.curseforge_fingerprint.is_some()));
    assert!(list.mods.iter().all(|m| m.sha512.len() == 128));
    assert!(list.mods.iter().all(|m| m.modrinth.is_none()));
    let known = block_on(server::identify(&cf, &list.mods)).unwrap();
    assert_eq!(known.len(), 2);
    assert_eq!(known["create-0.5.1.jar"].project_id, CREATE);
    assert_eq!(known["fabric-api-0.90.jar"].title, "fabric-api title");

    // a newer Fabric API: found, installed in the older one's place, which stays off
    block_on(admin.set_mod_enabled(&uuid, "fabric-api-0.90.jar", false)).unwrap();
    mock.file(FakeFile::new(
        4002,
        FABRIC_API,
        "fabric-api-0.92.jar",
        &TAGS,
        NEW,
        fabric_jar("fabric-api", "0.92"),
    ));
    let list = block_on(admin.installed_mods(&uuid, false)).unwrap();
    let mut known = block_on(server::identify(&cf, &list.mods)).unwrap();
    block_on(server::find_updates(&cf, &target, &list.mods, &mut known)).unwrap();
    let newer = known["fabric-api-0.90.jar.disabled"]
        .update
        .clone()
        .expect("an update");
    assert_eq!(newer.id, 4002);
    assert_eq!(known["create-0.5.1.jar"].update, None);
    let outcome = install(4002).expect("updated");
    assert_eq!(outcome.result.removed, ["fabric-api-0.90.jar.disabled"]);
    let list = block_on(admin.installed_mods(&uuid, false)).unwrap();
    let names: Vec<&str> = list.mods.iter().map(|m| m.file_name.as_str()).collect();
    assert_eq!(names, ["create-0.5.1.jar", "fabric-api-0.92.jar.disabled"]);

    // a file its author keeps on CurseForge's website: never fetched, found in the
    // folder the browser saved it to, and uploaded in the older file's place
    let site_only = fabric_jar("create", "0.5.2");
    mock.file(
        FakeFile::new(
            5002,
            CREATE,
            "create-0.5.2.jar",
            &TAGS,
            "2025-03-01T00:00:00Z",
            site_only.clone(),
        )
        .blocked(),
    );
    let outcome = install(5002).expect("planned");
    assert!(outcome.result.installed.is_empty());
    assert_eq!(outcome.blocked.len(), 1);
    let downloads = tempfile::tempdir().unwrap();
    assert!(server::find_downloaded(downloads.path(), &outcome.blocked).is_empty());
    let saved = downloads.path().join("create-0.5.2 (1).jar");
    std::fs::write(&saved, &site_only).unwrap();
    let found = server::find_downloaded(downloads.path(), &outcome.blocked);
    assert_eq!(found, [(0, saved.clone())]);
    let list = block_on(admin.installed_mods(&uuid, false)).unwrap();
    let known = block_on(server::identify(&cf, &list.mods)).unwrap();
    let result = block_on(server::install_downloaded(
        admin,
        &uuid,
        &list.mods,
        &known,
        &outcome.blocked[0],
        &saved,
    ))
    .expect("uploaded");
    assert_eq!(result.removed, ["create-0.5.1.jar"]);
    let list = block_on(admin.installed_mods(&uuid, false)).unwrap();
    let names: Vec<&str> = list.mods.iter().map(|m| m.file_name.as_str()).collect();
    assert_eq!(names, ["create-0.5.2.jar", "fabric-api-0.92.jar.disabled"]);
    assert!(saved.is_file(), "the download stays where it is");

    // a CurseForge modpack, made a pack here: the core reads it, CurseForge's hosts
    // and all, and keeps which CurseForge file it came from
    mock.file(FakeFile::new(
        9001,
        PACK,
        "a-pack-2.5.zip",
        &[GAME],
        NEW,
        pack_zip(&[(CREATE, 5001), (FABRIC_API, 4002)]),
    ));
    let project = block_on(cf.project(PACK)).unwrap();
    let file = block_on(cf.file(PACK, 9001)).unwrap();
    let (info, by_hand) = block_on(http::in_http_runtime(curseforge_pack::build_and_upload(
        &cf,
        None,
        &downloader,
        admin,
        &project,
        &file,
        &|_| {},
        &CancelToken::new(),
    )))
    .expect("uploaded");
    assert!(by_hand.is_empty());
    assert_eq!(info.unsupported_reason, None);
    assert!(info.upload_id.is_some());
    assert_eq!(info.name, "a-pack title");
    assert_eq!(info.version_number.as_deref(), Some("2.5"));
    assert_eq!(
        (info.game_version.as_str(), info.loader.as_str()),
        (GAME, "fabric")
    );
    assert_eq!(info.loader_version.as_deref(), Some("0.16.9"));
    assert_eq!(info.server_file_count, 2);
    let from = info.curseforge.expect("the pack says where it came from");
    assert_eq!((from.project, from.file), (PACK, 9001));
    assert_eq!(
        from.page_url.as_deref(),
        Some("https://www.curseforge.com/minecraft/mc-mods/a-pack")
    );
}

/// The same against CurseForge itself, for a developer to run by hand: needs
/// `LODESTONE_HOST_CURSEFORGE_REAL=1` on top of what the test above needs, and
/// Lodestone's key (`LODESTONE_CURSEFORGE_API_KEY`, in the environment or compiled
/// in). It asks CurseForge and Modrinth for real, and the core downloads a modpack's
/// server files: a server is made from a CurseForge modpack, updated to its newest
/// file, and given a mod. Never part of an automated run.
#[test]
fn curseforge_itself_through_the_real_core() {
    use launcher_core::{
        content::{CurseForge, Modrinth},
        cores::admin::instances::{CreateModpackRequest, ModpackSource},
    };
    if std::env::var("LODESTONE_HOST_CURSEFORGE_REAL").as_deref() != Ok("1") {
        eprintln!("skipped: set LODESTONE_HOST_CURSEFORGE_REAL=1 to ask CurseForge itself");
        return;
    }
    let Some(cf) = CurseForge::production(http::default_client().unwrap()) else {
        eprintln!("skipped: no CurseForge key in this build or the environment");
        return;
    };
    let Some(core) = start_core(17_800..17_900) else {
        return;
    };
    let admin = &core.admin;
    // `LODESTONE_HOST_CURSEFORGE_ALONE=1`: without Modrinth, so every file of the pack
    // comes from CurseForge's CDN, here to work out its SHA-512 and then in the core
    let alone = std::env::var("LODESTONE_HOST_CURSEFORGE_ALONE").as_deref() == Ok("1");
    let modrinth = (!alone).then(|| Modrinth::production(http::default_client().unwrap()));
    let downloader = Downloader::new(http::download_client().unwrap(), sleep());
    let upload = |project, file| {
        block_on(http::in_http_runtime(curseforge_pack::build_and_upload(
            &cf,
            modrinth.as_ref(),
            &downloader,
            admin,
            project,
            file,
            &|step| eprintln!("  {step:?}"),
            &CancelToken::new(),
        )))
    };

    // a modpack, by its link: its two newest releases for the same game version
    let link = std::env::var("LODESTONE_HOST_CURSEFORGE_PACK").unwrap_or_else(|_| {
        "https://www.curseforge.com/minecraft/modpacks/fabulously-optimized".to_string()
    });
    let pack = block_on(cf.resolve_server_link(&link))
        .expect("the modpack")
        .project;
    let files = block_on(cf.latest_files(pack.id)).expect("its files");
    let releases: Vec<_> = files.iter().filter(|f| f.release_type == 1).collect();
    // the newest release that has an older one for the same game version (a pack with
    // one file a game version has none: then its two newest releases)
    let pair = releases.iter().enumerate().find_map(|(at, newer)| {
        let game = newer.tags().game_versions;
        let older = releases[at + 1..]
            .iter()
            .find(|f| f.tags().game_versions == game)?;
        Some((*newer, *older))
    });
    let (newest, older) = pair.unwrap_or_else(|| {
        (
            *releases.first().expect("a release"),
            *releases.get(1).expect("an older release"),
        )
    });
    let game = newest.tags().game_versions;
    let same_game = older.tags().game_versions == game;
    eprintln!(
        "pack: {} ({}), from {} to {}, for Minecraft {game:?}",
        pack.name,
        pack.id,
        server::file_label(older),
        server::file_label(newest)
    );

    let (info, by_hand) = upload(&pack, older).expect("the older file, uploaded");
    eprintln!(
        "read by the core: {} {:?}, {} {:?}, {} server files, {} bytes; by hand: {:?}",
        info.name,
        info.version_number,
        info.loader,
        info.loader_version,
        info.server_file_count,
        info.server_download_size,
        by_hand.iter().map(|f| f.title()).collect::<Vec<_>>()
    );
    assert_eq!(info.unsupported_reason, None);
    let uuid = block_on(admin.create_modpack_instance(&CreateModpackRequest {
        source: ModpackSource::Upload {
            upload_id: info.upload_id.clone().expect("an upload id"),
        },
        name: "CurseForge pack".into(),
        port: 25592,
        description: String::new(),
        min_ram: 1024,
        max_ram: 2048,
        auto_start: false,
        restart_on_crash: false,
        // never accepted here
        accept_eula: false,
    }))
    .expect("the server is being made");
    let started = Instant::now();
    let instance = loop {
        let made = block_on(admin.instances())
            .unwrap()
            .into_iter()
            .find(|i| i.uuid == uuid);
        if let Some(instance) = made {
            break instance;
        }
        assert!(started.elapsed() < Duration::from_secs(900), "not made");
        std::thread::sleep(Duration::from_secs(2));
    };
    let origin = instance
        .modpack
        .as_ref()
        .and_then(|m| m.curseforge.clone())
        .expect("the server says which CurseForge file it was made from");
    assert_eq!((origin.project, origin.file), (pack.id, older.id));
    eprintln!(
        "made: {} on {}, origin {origin:?}",
        instance.name, instance.version
    );
    let list = block_on(admin.installed_mods(&uuid, false)).unwrap();
    let known = block_on(server::identify(&cf, &list.mods)).unwrap();
    eprintln!(
        "mods on the server: {}, of which Modrinth knows {} and CurseForge {}",
        list.mods.len(),
        list.mods.iter().filter(|m| m.modrinth.is_some()).count(),
        known.len()
    );
    assert_eq!(list.mods.len() as u32, info.server_file_count);

    // CurseForge has a newer file, and the server takes it
    let found = block_on(curseforge_pack::newer_file(&cf, &origin, &instance.version))
        .unwrap()
        .expect("a newer file");
    eprintln!("CurseForge has {} for it", server::file_label(&found));
    // the update goes to the pair's newer file, which is the pack's newest only when
    // the pack has two files for its newest game version
    let found = if found.id == newest.id {
        found
    } else {
        newest.clone()
    };
    let (info, _) = upload(&pack, &found).expect("the newer file, uploaded");
    let source = ModpackSource::Upload {
        upload_id: info.upload_id.clone().expect("an upload id"),
    };
    let plan = block_on(admin.plan_modpack_update(&uuid, &source)).expect("a plan");
    eprintln!(
        "update: +{} ~{} -{} kept {}",
        plan.added.len(),
        plan.replaced.len(),
        plan.removed.len(),
        plan.kept.len()
    );
    block_on(admin.update_modpack(&uuid, &source)).expect("updated");
    let now = block_on(admin.instances())
        .unwrap()
        .into_iter()
        .find(|i| i.uuid == uuid)
        .unwrap();
    let origin = now.modpack.and_then(|m| m.curseforge).unwrap();
    assert_eq!(origin.file, newest.id);
    let after = block_on(curseforge_pack::newer_file(&cf, &origin, &instance.version)).unwrap();
    eprintln!(
        "after the update, CurseForge has {:?} newer (same game version: {same_game})",
        after.as_ref().map(server::file_label)
    );

    // a mod, by its link: its newest file for the server, with what it requires
    let list = block_on(admin.installed_mods(&uuid, false)).unwrap();
    let target = ServerTarget {
        kind: list.kind,
        loader: list.loader,
        game_version: list.game_version.clone(),
    };
    let known = block_on(server::identify(&cf, &list.mods)).unwrap();
    let wanted = std::env::var("LODESTONE_HOST_CURSEFORGE_MOD").unwrap_or_else(|_| "chunky".into());
    let found = block_on(server::search(
        &cf,
        target.class_id(),
        &wanted,
        &server::SearchFilter::for_server(&target),
        launcher_core::cores::admin::mods::ModrinthSearchIndex::Relevance,
        0,
        10,
    ))
    .expect("a search");
    eprintln!(
        "search for the server: {:?}",
        found.hits.iter().map(|h| &h.title).collect::<Vec<_>>()
    );
    // one the server doesn't have yet
    let hit = found
        .hits
        .iter()
        .find(|hit| {
            !known
                .values()
                .any(|k| k.project_id.to_string() == hit.project_id)
        })
        .expect("a result that isn't installed");
    let project = block_on(cf.project(hit.project_id.parse().unwrap())).expect("the mod");
    let files = block_on(server::files(&cf, project.id, &target, false)).expect("its files");
    let file = server::pick_file(&target, files).expect("a file for the server");
    eprintln!(
        "installing {} ({}), from {:?}",
        project.name,
        server::file_label(&file),
        file.download_url
    );
    let before = list.mods.len();
    let outcome = block_on(http::in_http_runtime(server::install_file(
        &cf,
        modrinth.as_ref(),
        &downloader,
        admin,
        &uuid,
        &target,
        &list.mods,
        &known,
        file.clone(),
        Some(project.clone()),
        true,
        false,
    )))
    .expect("installed");
    eprintln!(
        "installed {:?}, removed {:?}, by hand {:?}",
        outcome
            .result
            .installed
            .iter()
            .map(|m| &m.file_name)
            .collect::<Vec<_>>(),
        outcome.result.removed,
        outcome
            .blocked
            .iter()
            .map(|b| b.title())
            .collect::<Vec<_>>()
    );
    let list = block_on(admin.installed_mods(&uuid, true)).unwrap();
    let mut known = block_on(server::identify(&cf, &list.mods)).unwrap();
    block_on(server::find_updates(&cf, &target, &list.mods, &mut known)).unwrap();
    assert!(known.values().any(|k| k.project_id == project.id));
    eprintln!(
        "now {} mods (was {before}); CurseForge knows {}, with {} updates",
        list.mods.len(),
        known.len(),
        known.values().filter(|k| k.update.is_some()).count()
    );
}
