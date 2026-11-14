//! Content management against a loopback Modrinth mock: matching files by SHA-1,
//! installing with dependencies, replacing older versions, damaged downloads, updates,
//! links, the rate limit, and the shader mod offer per loader and version.

mod modrinth;

use std::{fs, path::Path};

use futures::executor::block_on;
use launcher_core::{
    content::{
        ContentError, ContentKind, InstanceDirs, ModrinthError, Target,
        install::{self, InstallProgress, InstallRequest},
        listing,
        modrinth::USER_AGENT,
        options::{self, PackChange},
        provenance::Provenance,
        shaders::{self, ShaderLoader, ShaderModOffer},
    },
    game::download::{CancelToken, Downloader, recording_sleep},
    http,
    instances::Loader,
};
use lodestone_minecraft::modrinth::{ClientProjectType, ClientSearchParams};
use modrinth::{FakeVersion, ModrinthMock};

const IRIS: &str = shaders::IRIS_PROJECT;
const OCULUS: &str = shaders::OCULUS_PROJECT;
const SODIUM: &str = "AANobbMI";
const EMBEDDIUM: &str = "sk9rgfiA";
const FABRIC_API: &str = "P7dR8mSH";

fn downloader() -> Downloader {
    Downloader::new(http::download_client().unwrap(), recording_sleep().0)
}

fn instance(dir: &Path) -> InstanceDirs {
    let dirs = InstanceDirs::new(dir.join("instance"));
    fs::create_dir_all(&dirs.game).unwrap();
    dirs
}

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

/// Iris, Sodium and Fabric API for Fabric 1.20.1, as on Modrinth.
fn fabric_mods(mock: &ModrinthMock) {
    mock.project(IRIS, "iris", "mod")
        .project(SODIUM, "sodium", "mod")
        .project(FABRIC_API, "fabric-api", "mod")
        .version(
            FakeVersion::new(
                "iris176",
                IRIS,
                "1.7.6+1.20.1",
                &["fabric", "quilt"],
                &["1.20.1"],
                "2025-01-02T00:00:00Z",
            )
            .requires(Some(SODIUM), Some("sod0513")),
        )
        .version(
            FakeVersion::new(
                "sod0513",
                SODIUM,
                "0.5.13",
                &["fabric", "quilt"],
                &["1.20.1"],
                "2024-10-01T00:00:00Z",
            )
            .requires(Some(FABRIC_API), None),
        )
        .version(FakeVersion::new(
            "sod0600",
            SODIUM,
            "0.6.0",
            &["fabric"],
            &["1.21.1"],
            "2025-01-01T00:00:00Z",
        ))
        .version(FakeVersion::new(
            "fapi-old",
            FABRIC_API,
            "0.90.0",
            &["fabric"],
            &["1.20.1"],
            "2023-01-01T00:00:00Z",
        ))
        .version(FakeVersion::new(
            "fapi-rel",
            FABRIC_API,
            "0.92.2",
            &["fabric"],
            &["1.20.1"],
            "2024-06-01T00:00:00Z",
        ))
        .version(
            FakeVersion::new(
                "fapi-beta",
                FABRIC_API,
                "0.93.0-beta",
                &["fabric"],
                &["1.20.1"],
                "2024-12-01T00:00:00Z",
            )
            .kind("beta"),
        )
        .version(FakeVersion::new(
            "fapi-121",
            FABRIC_API,
            "0.110.0",
            &["fabric"],
            &["1.21.1"],
            "2025-02-01T00:00:00Z",
        ));
}

fn install_version(
    mock: &ModrinthMock,
    dirs: &InstanceDirs,
    target: &Target,
    kind: ContentKind,
    version_id: &str,
) -> Result<install::InstallOutcome, ContentError> {
    let api = mock.client();
    block_on(async {
        let version = api.version(version_id).await?;
        let plan = install::plan(
            &api,
            dirs,
            target,
            InstallRequest {
                kind,
                version,
                project: None,
                install_dependencies: true,
                force: false,
            },
        )
        .await?;
        install::execute(
            &api,
            &downloader(),
            dirs,
            target,
            plan,
            &InstallProgress::default(),
            &CancelToken::new(),
        )
        .await
    })
}

#[test]
fn installs_required_dependencies_and_records_provenance() {
    let mock = ModrinthMock::start();
    fabric_mods(&mock);
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    let target = Target::new("1.20.1", Loader::Fabric);
    let outcome = install_version(&mock, &dirs, &target, ContentKind::Mod, "iris176").unwrap();
    // Iris, the Sodium version it pins, and the newest Fabric API release (not the
    // newer beta, nor the 1.21.1 build)
    let installed: Vec<&str> = outcome
        .installed
        .iter()
        .map(|(_, name)| name.as_str())
        .collect();
    assert_eq!(
        installed,
        [
            "YL57xq9U-1.7.6+1.20.1.jar",
            "AANobbMI-0.5.13.jar",
            "P7dR8mSH-0.92.2.jar"
        ]
    );
    assert_eq!(outcome.installed[0].0, "iris title");
    let mods = dirs.content_dir(ContentKind::Mod);
    assert_eq!(
        names(&mods),
        [
            "AANobbMI-0.5.13.jar",
            "P7dR8mSH-0.92.2.jar",
            "YL57xq9U-1.7.6+1.20.1.jar"
        ]
    );
    assert_eq!(
        fs::read(mods.join("AANobbMI-0.5.13.jar")).unwrap(),
        b"AANobbMI 0.5.13"
    );
    let provenance = Provenance::load(&dirs.instance);
    let iris = provenance
        .get(ContentKind::Mod, "YL57xq9U-1.7.6+1.20.1.jar")
        .unwrap();
    let source = iris.modrinth.as_ref().unwrap();
    assert_eq!(
        (source.project_id.as_str(), source.version_id.as_str()),
        (IRIS, "iris176")
    );
    assert_eq!(source.title, "iris title");
    assert_eq!(
        source.url.as_deref(),
        Some("https://cdn.modrinth.com/data/YL57xq9U/versions/iris176/YL57xq9U-1.7.6+1.20.1.jar")
    );
    assert_eq!(iris.sha512.len(), 128);
    // every request carried Lodestone's User-Agent
    assert!(
        mock.requests()
            .iter()
            .all(|r| r.user_agent.as_deref() == Some(USER_AGENT)
                || r.path.starts_with("/cdn.modrinth.com"))
    );

    // installing again: nothing downloads, the installed dependencies are not planned again
    let before = mock
        .api_requests(
            "GET",
            "/cdn.modrinth.com/data/YL57xq9U/versions/iris176/YL57xq9U-1.7.6+1.20.1.jar",
        )
        .len();
    let outcome = install_version(&mock, &dirs, &target, ContentKind::Mod, "iris176").unwrap();
    assert_eq!(outcome.installed.len(), 1);
    assert_eq!(
        mock.api_requests(
            "GET",
            "/cdn.modrinth.com/data/YL57xq9U/versions/iris176/YL57xq9U-1.7.6+1.20.1.jar"
        )
        .len(),
        before
    );
}

#[test]
fn replaces_older_versions_and_keeps_them_disabled() {
    let mock = ModrinthMock::start();
    fabric_mods(&mock);
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    let target = Target::new("1.20.1", Loader::Fabric);
    install_version(&mock, &dirs, &target, ContentKind::Mod, "fapi-old").unwrap();
    launcher_core::content::files::set_mod_enabled(&dirs, "P7dR8mSH-0.90.0.jar", false).unwrap();
    let outcome = install_version(&mock, &dirs, &target, ContentKind::Mod, "fapi-rel").unwrap();
    assert_eq!(outcome.removed, ["P7dR8mSH-0.90.0.jar.disabled"]);
    assert_eq!(
        names(&dirs.content_dir(ContentKind::Mod)),
        ["P7dR8mSH-0.92.2.jar.disabled"]
    );
    let provenance = Provenance::load(&dirs.instance);
    assert_eq!(provenance.files.len(), 1);
    assert_eq!(
        provenance
            .get(ContentKind::Mod, "P7dR8mSH-0.92.2.jar")
            .unwrap()
            .modrinth
            .as_ref()
            .unwrap()
            .version_id,
        "fapi-rel"
    );
}

#[test]
fn refuses_versions_that_dont_fit_and_incompatible_mods() {
    let mock = ModrinthMock::start();
    fabric_mods(&mock);
    mock.project("OPTIF", "optifabric", "mod").version(
        FakeVersion::new(
            "optifabric1",
            "OPTIF",
            "1.0",
            &["fabric"],
            &["1.20.1"],
            "2024-01-01T00:00:00Z",
        )
        .depends(SODIUM, "incompatible"),
    );
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    let target = Target::new("1.20.1", Loader::Fabric);
    let err = install_version(&mock, &dirs, &target, ContentKind::Mod, "sod0600").unwrap_err();
    assert!(
        err.to_string()
            .contains("is for Minecraft 1.21.1, but this instance runs 1.20.1"),
        "{err}"
    );
    let forge = Target::new("1.20.1", Loader::Forge);
    let err = install_version(&mock, &dirs, &forge, ContentKind::Mod, "sod0513").unwrap_err();
    assert!(
        err.to_string().contains("this instance uses Forge"),
        "{err}"
    );
    install_version(&mock, &dirs, &target, ContentKind::Mod, "sod0513").unwrap();
    let err = install_version(&mock, &dirs, &target, ContentKind::Mod, "optifabric1").unwrap_err();
    assert!(
        err.to_string()
            .contains("incompatible with the installed sodium title"),
        "{err}"
    );
    assert!(
        !names(&dirs.content_dir(ContentKind::Mod))
            .iter()
            .any(|n| n.starts_with("OPTIF"))
    );
}

#[test]
fn damaged_and_off_cdn_downloads_leave_nothing_behind() {
    let mock = ModrinthMock::start();
    mock.project("BAD", "bad", "mod");
    let mut damaged = FakeVersion::new(
        "bad1",
        "BAD",
        "1.0",
        &["fabric"],
        &["1.20.1"],
        "2024-01-01T00:00:00Z",
    );
    damaged.wrong_sha512 = true;
    mock.version(damaged);
    let mut elsewhere = FakeVersion::new(
        "bad2",
        "BAD",
        "2.0",
        &["fabric"],
        &["1.20.1"],
        "2024-02-01T00:00:00Z",
    );
    elsewhere.url_override = Some("https://evil.example/x.jar".into());
    mock.version(elsewhere);
    let mut traversal = FakeVersion::new(
        "bad3",
        "BAD",
        "3.0",
        &["fabric"],
        &["1.20.1"],
        "2024-03-01T00:00:00Z",
    );
    traversal.filename = "../../../evil.txt".into();
    mock.version(traversal);
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    let target = Target::new("1.20.1", Loader::Fabric);
    let err = install_version(&mock, &dirs, &target, ContentKind::Mod, "bad1").unwrap_err();
    assert!(err.to_string().contains("damaged"), "{err}");
    let err = install_version(&mock, &dirs, &target, ContentKind::Mod, "bad2").unwrap_err();
    assert!(
        err.to_string()
            .contains("files only come from cdn.modrinth.com"),
        "{err}"
    );
    let err = install_version(&mock, &dirs, &target, ContentKind::Mod, "bad3").unwrap_err();
    assert!(err.to_string().contains("isn't a file name"), "{err}");
    assert!(names(&dirs.content_dir(ContentKind::Mod)).is_empty());
    assert!(!dir.path().join("evil.txt").exists());
}

#[test]
fn matches_unknown_files_by_sha1_in_one_batch() {
    let mock = ModrinthMock::start();
    fabric_mods(&mock);
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    let mods = dirs.content_dir(ContentKind::Mod);
    fs::create_dir_all(&mods).unwrap();
    // a manual drop of the Sodium file, one of Fabric API, and a jar Modrinth doesn't know
    fs::write(mods.join("sodium-manual.jar"), b"AANobbMI 0.5.13").unwrap();
    fs::write(mods.join("fabric-api.jar.disabled"), b"P7dR8mSH 0.90.0").unwrap();
    fs::write(mods.join("my-own.jar"), b"homemade").unwrap();
    let target = Target::new("1.20.1", Loader::Fabric);
    let api = mock.client();
    let listing = block_on(listing::list(Some(&api), &dirs, &target, ContentKind::Mod)).unwrap();
    assert_eq!(listing.lookup_error, None);
    let posts = mock.api_requests("POST", "/v2/version_files");
    assert_eq!(posts.len(), 1);
    let body: serde_json::Value = serde_json::from_str(&posts[0].body).unwrap();
    assert_eq!(body["hashes"].as_array().unwrap().len(), 3);
    let by_name = |name: &str| {
        listing
            .items
            .iter()
            .find(|i| i.file.file_name == name)
            .unwrap()
            .clone()
    };
    let sodium = by_name("sodium-manual.jar");
    assert_eq!(sodium.display_name(), "sodium title");
    assert_eq!(sodium.display_version().as_deref(), Some("0.5.13"));
    assert!(sodium.enabled);
    let fabric_api = by_name("fabric-api.jar.disabled");
    assert!(!fabric_api.enabled);
    assert_eq!(fabric_api.modrinth.unwrap().version_id, "fapi-old");
    let own = by_name("my-own.jar");
    assert!(own.modrinth.is_none());
    assert_eq!(own.display_name(), "my-own");

    // the second listing hashes nothing new and asks Modrinth nothing
    let listing2 = block_on(listing::list(Some(&api), &dirs, &target, ContentKind::Mod)).unwrap();
    assert_eq!(listing2.items, listing.items);
    assert_eq!(mock.api_requests("POST", "/v2/version_files").len(), 1);
    // enabling keeps the match (provenance is keyed without .disabled)
    launcher_core::content::files::set_mod_enabled(&dirs, "fabric-api.jar.disabled", true).unwrap();
    let listing3 = block_on(listing::list(None, &dirs, &target, ContentKind::Mod)).unwrap();
    assert!(
        listing3
            .items
            .iter()
            .any(|i| i.file.file_name == "fabric-api.jar" && i.modrinth.is_some())
    );

    // updates: Fabric API's newest for 1.20.1 is a beta, offered only with
    // pre-releases on; Sodium is the newest
    let updates = block_on(listing::check_updates(
        &api,
        &dirs,
        &target,
        ContentKind::Mod,
        true,
    ))
    .unwrap();
    assert_eq!(updates.len(), 1);
    assert_eq!(updates["fabric-api.jar"].id, "fapi-beta");
    // with them off, the installed release (0.90.0) updates to the newest release
    let stable = block_on(listing::check_updates(
        &api,
        &dirs,
        &target,
        ContentKind::Mod,
        false,
    ))
    .unwrap();
    assert_eq!(stable.len(), 1);
    assert_eq!(stable["fabric-api.jar"].id, "fapi-rel");
}

#[test]
fn offline_listing_still_works() {
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    let mods = dirs.content_dir(ContentKind::Mod);
    fs::create_dir_all(&mods).unwrap();
    fs::write(mods.join("a.jar"), b"a").unwrap();
    // a client pointed at a closed port
    let api = launcher_core::content::Modrinth::with_urls(
        http::default_client().unwrap(),
        url::Url::parse("http://127.0.0.1:9/v2").unwrap(),
        None,
    );
    let target = Target::new("1.20.1", Loader::Fabric);
    let listing = block_on(listing::list(Some(&api), &dirs, &target, ContentKind::Mod)).unwrap();
    assert_eq!(listing.items.len(), 1);
    assert!(listing.lookup_error.unwrap().contains("isn't reachable"));
    // nothing was marked as unknown, so the next listing asks again
    assert!(
        !Provenance::load(&dirs.instance)
            .get(ContentKind::Mod, "a.jar")
            .unwrap()
            .looked_up
    );
}

#[test]
fn resource_pack_and_shader_updates_keep_their_place() {
    let mock = ModrinthMock::start();
    mock.project("FAITH", "faithful", "resourcepack")
        .project("BSL", "bsl", "shader")
        .version(
            FakeVersion::new(
                "f1",
                "FAITH",
                "1",
                &["minecraft"],
                &["1.20.1"],
                "2024-01-01T00:00:00Z",
            )
            .file("Faithful 1.zip", b"faithful one"),
        )
        .version(
            FakeVersion::new(
                "f2",
                "FAITH",
                "2",
                &["minecraft"],
                &["1.20.1"],
                "2024-02-01T00:00:00Z",
            )
            .file("Faithful 2.zip", b"faithful two"),
        )
        .version(
            FakeVersion::new(
                "b1",
                "BSL",
                "8.2",
                &["iris", "optifine"],
                &["1.20.1"],
                "2024-01-01T00:00:00Z",
            )
            .file("BSL 8.2.zip", b"bsl one")
            // a shader pack that names Iris: not installed into shaderpacks
            .requires(Some(IRIS), None),
        )
        .version(
            FakeVersion::new(
                "b2",
                "BSL",
                "8.3",
                &["iris", "optifine"],
                &["1.20.1"],
                "2024-02-01T00:00:00Z",
            )
            .file("BSL 8.3.zip", b"bsl two"),
        );
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    let mut target = Target::new("1.20.1", Loader::Fabric);
    target.shader_loader = Some(ShaderLoader::Iris);
    fs::write(
        dirs.game.join("options.txt"),
        "fov:0.5\nresourcePacks:[\"vanilla\",\"file/Other.zip\"]\n",
    )
    .unwrap();

    install_version(&mock, &dirs, &target, ContentKind::ResourcePack, "f1").unwrap();
    options::change_packs(
        &dirs.game,
        "1.20.1",
        &PackChange::Enable {
            name: "Faithful 1.zip".into(),
            compatible: true,
        },
    )
    .unwrap();
    options::change_packs(
        &dirs.game,
        "1.20.1",
        &PackChange::Lower {
            name: "Faithful 1.zip".into(),
        },
    )
    .unwrap();
    let outcome = install_version(&mock, &dirs, &target, ContentKind::ResourcePack, "f2").unwrap();
    assert_eq!(outcome.removed, ["Faithful 1.zip"]);
    assert_eq!(
        fs::read_to_string(dirs.game.join("options.txt")).unwrap(),
        "fov:0.5\nresourcePacks:[\"vanilla\",\"file/Faithful 2.zip\",\"file/Other.zip\"]\n"
    );

    let outcome = install_version(&mock, &dirs, &target, ContentKind::ShaderPack, "b1").unwrap();
    assert_eq!(outcome.installed.len(), 1);
    shaders::write_selection(&dirs.game, ShaderLoader::Iris, Some("BSL 8.2.zip")).unwrap();
    install_version(&mock, &dirs, &target, ContentKind::ShaderPack, "b2").unwrap();
    assert_eq!(
        names(&dirs.content_dir(ContentKind::ShaderPack)),
        ["BSL 8.3.zip"]
    );
    assert_eq!(
        shaders::read_selection(&dirs.game, ShaderLoader::Iris)
            .unwrap()
            .pack
            .as_deref(),
        Some("BSL 8.3.zip")
    );
    assert!(names(&dirs.content_dir(ContentKind::Mod)).is_empty());

    // the listing shows the pack enabled, at its priority, with its source
    let listing = block_on(listing::list(
        Some(&mock.client()),
        &dirs,
        &target,
        ContentKind::ResourcePack,
    ))
    .unwrap();
    let pack = &listing.items[0];
    assert_eq!(pack.file.file_name, "Faithful 2.zip");
    assert!(pack.enabled);
    assert_eq!(pack.priority, Some(1));
    assert_eq!(pack.modrinth.as_ref().unwrap().version_id, "f2");
}

#[test]
fn offers_the_shader_mod_for_each_loader_and_version() {
    let mock = ModrinthMock::start();
    mock.project(IRIS, "iris", "mod")
        .project(OCULUS, "oculus", "mod")
        .project(SODIUM, "sodium", "mod")
        .project(EMBEDDIUM, "embeddium", "mod")
        .version(
            FakeVersion::new(
                "iris-fabric",
                IRIS,
                "1.7.6+1.20.1",
                &["fabric", "quilt"],
                &["1.20.1"],
                "2025-01-02T00:00:00Z",
            )
            .requires(Some(SODIUM), None),
        )
        .version(
            FakeVersion::new(
                "iris-neo",
                IRIS,
                "1.8.12+1.21.1-neoforge",
                &["neoforge"],
                &["1.21.1"],
                "2025-03-01T00:00:00Z",
            )
            .requires(Some(SODIUM), None),
        )
        .version(
            FakeVersion::new(
                "iris-neo-beta",
                IRIS,
                "1.8.14-beta.1+1.21.1-neoforge",
                &["neoforge"],
                &["1.21.1"],
                "2025-05-01T00:00:00Z",
            )
            .kind("beta"),
        )
        .version(
            FakeVersion::new(
                "oculus-120",
                OCULUS,
                "1.20.1-1.8.0",
                &["forge", "neoforge"],
                &["1.20.1"],
                "2024-12-24T00:00:00Z",
            )
            .requires(Some(EMBEDDIUM), None),
        )
        .version(FakeVersion::new(
            "sodium-fabric",
            SODIUM,
            "0.5.13",
            &["fabric", "quilt"],
            &["1.20.1"],
            "2024-10-01T00:00:00Z",
        ))
        .version(FakeVersion::new(
            "sodium-neo",
            SODIUM,
            "0.6.13",
            &["neoforge"],
            &["1.21.1"],
            "2025-02-01T00:00:00Z",
        ))
        .version(FakeVersion::new(
            "embeddium-120",
            EMBEDDIUM,
            "0.3.31",
            &["forge"],
            &["1.20.1"],
            "2024-08-01T00:00:00Z",
        ));
    let api = mock.client();
    let offer = |loader, version: &str| {
        block_on(shaders::find_shader_mod(
            &api,
            &Target::new(version, loader),
        ))
        .unwrap()
    };
    let install = |offer: ShaderModOffer| match offer {
        ShaderModOffer::Install { loader, version } => (loader, version.id),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        install(offer(Loader::Fabric, "1.20.1")),
        (ShaderLoader::Iris, "iris-fabric".to_string())
    );
    assert_eq!(
        install(offer(Loader::Quilt, "1.20.1")),
        (ShaderLoader::Iris, "iris-fabric".to_string())
    );
    // the release, not the newer beta
    assert_eq!(
        install(offer(Loader::NeoForge, "1.21.1")),
        (ShaderLoader::Iris, "iris-neo".to_string())
    );
    assert_eq!(
        install(offer(Loader::Forge, "1.20.1")),
        (ShaderLoader::Oculus, "oculus-120".to_string())
    );
    match offer(Loader::Forge, "1.21.1") {
        ShaderModOffer::Unavailable { reason } => {
            assert_eq!(reason, "Oculus has no build for Forge on Minecraft 1.21.1.")
        }
        other => panic!("{other:?}"),
    }
    match offer(Loader::NeoForge, "1.20.4") {
        ShaderModOffer::Unavailable { reason } => {
            assert!(reason.starts_with("Iris or Oculus"), "{reason}")
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        offer(Loader::None, "1.21.1"),
        ShaderModOffer::NeedsModLoader
    ));

    // installing the Fabric offer brings Sodium; the listing then sees Iris
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    let target = Target::new("1.20.1", Loader::Fabric);
    let outcome = install_version(&mock, &dirs, &target, ContentKind::Mod, "iris-fabric").unwrap();
    assert_eq!(outcome.installed.len(), 2);
    assert_eq!(
        listing::shader_state(&dirs).unwrap().loader,
        Some(ShaderLoader::Iris)
    );
    // and on Forge, Oculus brings Embeddium
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    let forge = Target::new("1.20.1", Loader::Forge);
    let outcome = install_version(&mock, &dirs, &forge, ContentKind::Mod, "oculus-120").unwrap();
    let installed: Vec<&str> = outcome.installed.iter().map(|(_, n)| n.as_str()).collect();
    assert_eq!(
        installed,
        ["GchcoXML-1.20.1-1.8.0.jar", "sk9rgfiA-0.3.31.jar"]
    );
    assert_eq!(
        listing::shader_state(&dirs).unwrap().loader,
        Some(ShaderLoader::Oculus)
    );
}

#[test]
fn resolves_links_and_rejects_others_without_asking() {
    let mock = ModrinthMock::start();
    fabric_mods(&mock);
    let api = mock.client();
    let target = block_on(api.resolve_link("https://modrinth.com/mod/sodium")).unwrap();
    assert_eq!(target.project.id, SODIUM);
    assert!(target.version.is_none());
    let target = block_on(api.resolve_link("modrinth.com/mod/fabric-api/version/0.92.2")).unwrap();
    assert_eq!(target.version.unwrap().id, "fapi-rel");
    let err = block_on(api.resolve_link("https://modrinth.com/mod/missing")).unwrap_err();
    assert!(matches!(err, ModrinthError::NotFound(_)));
    let sent = api.requests_sent();
    for bad in [
        "https://evil.com/mod/sodium",
        "https://modrinth.com.evil.com/mod/sodium",
        "https://modrinth.com/mod/../../v2/user",
        "https://modrinth.com/mod/a%2F..%2Fx",
        "https://modrinth.com/mod/sodium%3Fx=1",
        "https://user:pass@modrinth.com/mod/sodium",
        "javascript:alert(1)//modrinth.com/mod/x",
        "file:///modrinth.com/mod/x",
        "https://modrinth.com:8080/mod/sodium",
        "https://modrinth.com/mod/sodium/version/1.0%00",
    ] {
        let err = block_on(api.resolve_link(bad)).unwrap_err();
        assert!(matches!(err, ModrinthError::Invalid(_)), "{bad}: {err:?}");
    }
    assert_eq!(api.requests_sent(), sent);
}

#[test]
fn the_rate_limit_fails_fast() {
    let mock = ModrinthMock::start();
    fabric_mods(&mock);
    let api = mock.client();
    mock.state().next_status = Some((429, vec![("X-Ratelimit-Reset".into(), "42".into())]));
    let err = block_on(api.project("sodium")).unwrap_err();
    assert_eq!(err, ModrinthError::RateLimited { seconds: 42 });
    let sent = api.requests_sent();
    let err = block_on(api.project("iris")).unwrap_err();
    assert!(matches!(err, ModrinthError::RateLimited { .. }));
    assert_eq!(api.requests_sent(), sent, "no request while rate limited");

    // "none left" on a good answer also stops the next calls
    let mock = ModrinthMock::start();
    fabric_mods(&mock);
    let api = mock.client();
    mock.state().headers = vec![
        ("X-Ratelimit-Remaining".into(), "0".into()),
        ("X-Ratelimit-Reset".into(), "10".into()),
    ];
    assert_eq!(block_on(api.project("sodium")).unwrap().id, SODIUM);
    assert!(matches!(
        block_on(api.project("iris")),
        Err(ModrinthError::RateLimited { .. })
    ));
}

#[test]
fn page_images_load_from_github_and_imgur_only() {
    let mock = ModrinthMock::start();
    let api = mock.client();
    mock.serve("https://i.imgur.com/AbC123.png", b"imgur image");
    mock.serve(
        "https://raw.githubusercontent.com/o/r/main/shot.png",
        b"github image",
    );
    assert_eq!(
        block_on(api.image("https://imgur.com/AbC123.png")).unwrap(),
        b"imgur image"
    );
    assert_eq!(
        block_on(api.image("https://github.com/o/r/blob/main/shot.png?raw=true")).unwrap(),
        b"github image"
    );

    // An attachment redirects to a signed URL on GitHub's image host.
    mock.serve(
        "https://private-user-images.githubusercontent.com/1/signed.png",
        b"attachment",
    );
    mock.state().next_status = Some((
        302,
        vec![(
            "Location".into(),
            "https://private-user-images.githubusercontent.com/1/signed.png?jwt=x".into(),
        )],
    ));
    assert_eq!(
        block_on(api.image("https://github.com/user-attachments/assets/1234")).unwrap(),
        b"attachment"
    );

    // Not to anywhere else.
    mock.state().next_status = Some((
        302,
        vec![("Location".into(), "https://evil.example/track.gif".into())],
    ));
    assert!(block_on(api.image("https://i.imgur.com/AbC123.png")).is_err());
    let before = mock.requests().len();
    assert!(block_on(api.image("https://evil.example/track.gif")).is_err());
    assert!(block_on(api.image("https://img.shields.io/badge/x.svg")).is_err());
    assert_eq!(
        mock.requests().len(),
        before,
        "nothing is sent for other hosts"
    );
}

#[test]
fn searches_send_client_facets_and_are_cached() {
    let mock = ModrinthMock::start();
    fabric_mods(&mock);
    let api = mock.client();
    let target = Target::new("1.20.1", Loader::Quilt);
    let params = ClientSearchParams::new(
        Some("sodium".into()),
        ClientProjectType::Mod,
        Some(target.game_version.clone()),
        target.search_loaders(ContentKind::Mod),
        None,
        Some(20),
        None,
    )
    .unwrap();
    let results = block_on(api.search(&params)).unwrap();
    assert_eq!(results.hits.len(), 3);
    assert_eq!(results.hits[0].icon_url, None);
    block_on(api.search(&params)).unwrap();
    let searches = mock.api_requests("GET", "/v2/search");
    assert_eq!(searches.len(), 1, "the second search came from the cache");
    let facets: Vec<Vec<String>> = serde_json::from_str(&searches[0].query["facets"]).unwrap();
    assert_eq!(
        facets,
        [
            vec!["project_type:mod"],
            vec!["categories:quilt", "categories:fabric"],
            vec!["versions:1.20.1"],
            vec!["client_side:required", "client_side:optional"],
        ]
    );
    assert_eq!(searches[0].query["offset"], "20");
    assert_eq!(searches[0].query["query"], "sodium");
}

#[test]
fn a_share_code_recreates_the_content_of_an_instance() {
    use launcher_core::content::share::{self, LeftOutReason, ShareCode};
    let mock = ModrinthMock::start();
    fabric_mods(&mock);
    let api = mock.client();
    let target = Target::new("1.20.1", Loader::Fabric);

    // The instance to share: Iris (with Sodium and Fabric API), Fabric API turned off,
    // and a mod added by hand.
    let from = tempfile::tempdir().unwrap();
    let from_dirs = instance(from.path());
    install_version(&mock, &from_dirs, &target, ContentKind::Mod, "iris176").unwrap();
    let mods = from_dirs.content_dir(ContentKind::Mod);
    let fapi = fs::read_dir(&mods)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .find(|n| n.starts_with("P7dR8mSH"))
        .expect("Fabric API installed");
    launcher_core::content::files::set_mod_enabled(&from_dirs, &fapi, false).unwrap();
    fs::write(mods.join("homemade.jar"), b"not on modrinth").unwrap();

    let plan = block_on(share::collect(
        Some(&api),
        &from_dirs,
        &target,
        Some("Survival".into()),
        Loader::Fabric,
        Some("0.16.14".into()),
    ))
    .unwrap();
    assert_eq!(plan.code.count(ContentKind::Mod), 3);
    assert_eq!(plan.left_out.len(), 1);
    assert_eq!(plan.left_out[0].file_name, "homemade.jar");
    assert_eq!(plan.left_out[0].reason, LeftOutReason::NotOnModrinth);
    let text = plan.code.encode();
    let code = ShareCode::decode(&text).unwrap();
    assert_eq!(code, plan.code);

    // A new instance from the code: the same versions, Fabric API still off, and
    // nothing of the hand-added mod.
    let to = tempfile::tempdir().unwrap();
    let to_dirs = instance(to.path());
    let outcome = block_on(share::install(
        &api,
        &downloader(),
        &to_dirs,
        &target,
        &code,
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    assert!(outcome.failed.is_empty(), "{:?}", outcome.failed);
    assert_eq!(outcome.installed.len(), 3);
    let mut names: Vec<String> = fs::read_dir(to_dirs.content_dir(ContentKind::Mod))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    let mut expected: Vec<String> = fs::read_dir(&mods)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n != "homemade.jar")
        .collect();
    expected.sort();
    assert_eq!(names, expected);
    assert!(
        names
            .iter()
            .any(|n| n.starts_with("P7dR8mSH") && n.ends_with(".disabled"))
    );

    // A version Modrinth no longer has is reported, and the rest still installs.
    let mut gone = code.clone();
    gone.items[0].version = "missing1".into();
    let again = tempfile::tempdir().unwrap();
    let again_dirs = instance(again.path());
    let outcome = block_on(share::install(
        &api,
        &downloader(),
        &again_dirs,
        &target,
        &gone,
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    assert_eq!(outcome.failed.len(), 1);
    assert_eq!(outcome.installed.len(), 2);
}

#[test]
fn a_copy_for_another_version_takes_the_mods_with_a_build_for_it() {
    use launcher_core::{
        content::copy_as::{self, CopyAsOptions, CopyAsRequest, WorldsRule},
        game::loaders::{LoaderChoice, LoaderKind},
        instances::InstanceStore,
    };
    let mock = ModrinthMock::start();
    fabric_mods(&mock);
    let api = mock.client();
    let root = tempfile::tempdir().unwrap();
    let store = InstanceStore::new(root.path().join("instances"));
    let fabric = |version: &str| LoaderChoice {
        kind: LoaderKind::Fabric,
        version: version.into(),
    };
    let source = store
        .create("Survival", "1.20.1", Some(&fabric("0.16.9")))
        .unwrap();
    let dirs = InstanceDirs::new(store.dir(&source.id));
    let target = Target::new("1.20.1", Loader::Fabric);
    // Iris (with Sodium and Fabric API), Fabric API turned off, a mod added by hand, a
    // world, a resource pack and the options
    install_version(&mock, &dirs, &target, ContentKind::Mod, "iris176").unwrap();
    let mods = dirs.content_dir(ContentKind::Mod);
    launcher_core::content::files::set_mod_enabled(&dirs, "P7dR8mSH-0.92.2.jar", false).unwrap();
    fs::write(mods.join("homemade.jar"), b"not on modrinth").unwrap();
    fs::create_dir_all(dirs.game.join("saves/New World")).unwrap();
    fs::write(dirs.game.join("saves/New World/level.dat"), b"world").unwrap();
    fs::create_dir_all(dirs.content_dir(ContentKind::ResourcePack)).unwrap();
    fs::write(
        dirs.content_dir(ContentKind::ResourcePack)
            .join("Faithful.zip"),
        b"pack",
    )
    .unwrap();
    fs::write(
        dirs.game.join("options.txt"),
        "resourcePacks:[\"vanilla\",\"file/Faithful.zip\"]\n",
    )
    .unwrap();
    fs::write(dirs.game.join("servers.dat"), b"servers").unwrap();

    let contents = block_on(copy_as::contents(Some(&api), &dirs)).unwrap();
    assert!(contents.unchecked.is_empty());
    assert_eq!(contents.lookup_error, None);
    assert_eq!(contents.worlds, ["New World"]);
    assert_eq!(contents.resource_packs, ["Faithful.zip"]);
    assert!(contents.options);
    assert_eq!(contents.mods.len(), 3);
    assert_eq!(contents.other_mods, ["homemade"]);
    let fapi = contents
        .mods
        .iter()
        .find(|m| m.project_id == FABRIC_API)
        .unwrap();
    assert!(!fapi.enabled);

    // To 1.21.1: Sodium and Fabric API have builds for it, Iris doesn't.
    let new_target = Target::new("1.21.1", Loader::Fabric);
    let resolved = block_on(copy_as::resolve_mods(&api, &contents.mods, &new_target)).unwrap();
    let found: Vec<&str> = resolved.found.iter().map(|(_, v)| v.id.as_str()).collect();
    assert_eq!(found, ["sod0600", "fapi-121"]);
    assert_eq!(resolved.missing.len(), 1);
    assert_eq!(resolved.missing[0].project_id, IRIS);
    // vanilla runs no mods: none is asked about
    let asked = mock.requests().len();
    let vanilla = block_on(copy_as::resolve_mods(
        &api,
        &contents.mods,
        &Target::new("1.21.1", Loader::None),
    ))
    .unwrap();
    assert_eq!((vanilla.found.len(), vanilla.missing.len()), (0, 3));
    assert_eq!(mock.requests().len(), asked);

    let options = CopyAsOptions {
        worlds: true,
        ..CopyAsOptions::default()
    };
    let rule = copy_as::worlds_rule("1.20.1", "1.21.1", &[]);
    assert_eq!(rule, WorldsRule::Newer);
    let plan = copy_as::plan(
        &contents,
        "1.21.1",
        Loader::Fabric,
        options,
        rule,
        Some(&resolved),
    );
    assert!(plan.needs_confirmation());
    let read_done = std::cell::Cell::new(false);
    let outcome = block_on(copy_as::execute(
        &store,
        Some(&api),
        &downloader(),
        &source,
        &CopyAsRequest {
            name: None,
            loader: Some(fabric("0.16.14")),
            plan,
        },
        &InstallProgress::default(),
        &CancelToken::new(),
        || read_done.set(true),
    ))
    .unwrap();
    assert!(read_done.get(), "the original's files were read");
    // Iris has no build, and the mod added by hand isn't on Modrinth
    assert_eq!(outcome.left_behind, ["iris title", "homemade"]);
    assert!(outcome.missing_dependencies.is_empty());
    let copy = outcome.instance;
    assert_eq!(copy.name, "Survival (1.21.1 Fabric)");
    assert_ne!(copy.id, source.id);
    assert_eq!(copy.game_version, "1.21.1");
    assert_eq!(copy.loader_version.as_deref(), Some("0.16.14"));
    let copy_dirs = InstanceDirs::new(store.dir(&copy.id));
    assert_eq!(
        names(&copy_dirs.content_dir(ContentKind::Mod)),
        ["AANobbMI-0.6.0.jar", "P7dR8mSH-0.110.0.jar.disabled"]
    );
    assert_eq!(
        fs::read(copy_dirs.game.join("saves/New World/level.dat")).unwrap(),
        b"world"
    );
    assert_eq!(
        names(&copy_dirs.content_dir(ContentKind::ResourcePack)),
        ["Faithful.zip"]
    );
    assert!(copy_dirs.game.join("options.txt").is_file());
    assert_eq!(
        fs::read(copy_dirs.game.join("servers.dat")).unwrap(),
        b"servers"
    );
    // the original is as it was
    let original = store.get(&source.id).unwrap();
    assert_eq!(
        (original.game_version.as_str(), original.loader),
        ("1.20.1", Loader::Fabric)
    );
    assert!(names(&mods).contains(&"YL57xq9U-1.7.6+1.20.1.jar".to_string()));
    assert_eq!(store.list().0.len(), 2);

    // A cancelled copy leaves nothing behind.
    let cancel = CancelToken::new();
    cancel.cancel();
    let plan = copy_as::plan(
        &contents,
        "1.21.1",
        Loader::Fabric,
        CopyAsOptions::default(),
        rule,
        Some(&resolved),
    );
    let cancelled = block_on(copy_as::execute(
        &store,
        Some(&api),
        &downloader(),
        &source,
        &CopyAsRequest {
            name: Some("Cancelled".into()),
            loader: Some(fabric("0.16.14")),
            plan,
        },
        &InstallProgress::default(),
        &cancel,
        || {},
    ));
    assert!(matches!(cancelled, Err(copy_as::CopyAsError::Cancelled)));
    assert_eq!(names(store.root()).len(), 2, "{:?}", names(store.root()));
}
