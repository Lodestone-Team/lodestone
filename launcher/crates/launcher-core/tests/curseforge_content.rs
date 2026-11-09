//! CurseForge content against loopback CurseForge and Modrinth mocks: the API key, paged
//! file lists, installs with dependencies, mixing platforms in one instance (the same
//! mod from both, dependencies already installed from the other), files the author keeps
//! on CurseForge (from Modrinth instead, or downloaded by the user), matching unknown
//! files by fingerprint, updates, refused keys and rate limits, and damaged downloads.

mod curseforge;
mod modrinth;

use std::{fs, path::Path};

use curseforge::{CurseForgeMock, FakeFile, KEY, fabric_jar};
use futures::executor::block_on;
use launcher_core::{
    content::{
        ContentError, ContentKind, CurseForge, CurseForgeError, InstanceDirs, Modrinth, Platforms,
        Target,
        curseforge::CLASS_MODS,
        curseforge_install::{self, CfInstallPlan, CfInstallRequest},
        install::{self, InstallOutcome, InstallProgress, InstallRequest, Placed},
        listing,
        provenance::Provenance,
    },
    game::download::{CancelToken, Downloader, recording_sleep},
    http,
    instances::Loader,
};
use modrinth::{FakeVersion, ModrinthMock};

const CREATE: u64 = 328_085;
const FABRIC_API: u64 = 306_612;
const SODIUM: u64 = 394_468;
const SECRET_MOD: u64 = 555_555;
const MR_FABRIC_API: &str = "P7dR8mSH";
const MR_SODIUM: &str = "AANobbMI";

const NEW: &str = "2025-02-01T00:00:00Z";
const OLD: &str = "2024-02-01T00:00:00Z";

fn downloader() -> Downloader {
    Downloader::new(http::download_client().unwrap(), recording_sleep().0)
}

fn instance(dir: &Path) -> InstanceDirs {
    let dirs = InstanceDirs::new(dir.join("instance"));
    fs::create_dir_all(&dirs.game).unwrap();
    dirs
}

fn mods(dirs: &InstanceDirs) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dirs.content_dir(ContentKind::Mod))
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

fn fabric() -> Target {
    Target::new("1.20.1", Loader::Fabric)
}

/// Create (requires Fabric API) and Fabric API for Fabric 1.20.1.
fn create_and_fabric_api(mock: &CurseForgeMock) {
    mock.project(CREATE, "create-fabric", CLASS_MODS)
        .project(FABRIC_API, "fabric-api", CLASS_MODS)
        .file(
            FakeFile::new(
                5_001,
                CREATE,
                "create-fabric-0.5.1.jar",
                &["1.20.1", "Fabric"],
                NEW,
                fabric_jar("create", "0.5.1"),
            )
            .requires(FABRIC_API),
        )
        .file(FakeFile::new(
            5_100,
            FABRIC_API,
            "fabric-api-0.92.2-cf.jar",
            &["1.20.1", "Fabric", "Quilt"],
            NEW,
            fabric_jar("fabric-api", "0.92.2"),
        ));
}

fn plan_install(
    cf: &CurseForge,
    mr: Option<&Modrinth>,
    dirs: &InstanceDirs,
    target: &Target,
    project: u64,
    file: u64,
) -> Result<CfInstallPlan, ContentError> {
    block_on(async {
        let file = cf.file(project, file).await?;
        curseforge_install::plan(
            cf,
            Platforms::new(mr, Some(cf)),
            dirs,
            target,
            CfInstallRequest {
                kind: ContentKind::Mod,
                file,
                project: None,
                install_dependencies: true,
                force: false,
            },
        )
        .await
    })
}

fn execute(
    cf: &CurseForge,
    mr: Option<&Modrinth>,
    dirs: &InstanceDirs,
    target: &Target,
    plan: &CfInstallPlan,
) -> Result<InstallOutcome, ContentError> {
    block_on(curseforge_install::execute(
        cf,
        mr,
        &downloader(),
        dirs,
        target,
        plan,
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
}

#[test]
fn the_key_goes_to_the_api_only() {
    let mock = CurseForgeMock::start();
    create_and_fabric_api(&mock);
    let cf = mock.client();
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    let plan = plan_install(&cf, None, &dirs, &fabric(), CREATE, 5_001).unwrap();
    execute(&cf, None, &dirs, &fabric(), &plan).unwrap();

    let requests = mock.requests();
    let (api, cdn): (Vec<_>, Vec<_>) = requests.iter().partition(|r| r.path.starts_with("/v1/"));
    assert!(!api.is_empty() && !cdn.is_empty());
    assert!(
        api.iter().all(|r| r.api_key.as_deref() == Some(KEY)),
        "{api:?}"
    );
    assert!(cdn.iter().all(|r| r.api_key.is_none()), "{cdn:?}");
}

#[test]
fn file_lists_are_read_in_pages_of_fifty() {
    let mock = CurseForgeMock::start();
    mock.project(CREATE, "create-fabric", CLASS_MODS);
    for i in 0..120u64 {
        mock.file(FakeFile::new(
            10_000 + i,
            CREATE,
            &format!("create-{i}.jar"),
            &["1.20.1", "Fabric"],
            &format!("2024-01-01T00:{:02}:{:02}Z", i / 60, i % 60),
            fabric_jar("create", &i.to_string()),
        ));
    }
    let cf = mock.client();
    let files = block_on(cf.files(CREATE, Some("1.20.1"), Some(Loader::Fabric))).unwrap();
    assert_eq!(files.len(), 120);
    assert_eq!(files[0].id, 10_119, "newest first");
    let pages: Vec<_> = mock
        .requests()
        .into_iter()
        .filter(|r| r.path == format!("/v1/mods/{CREATE}/files"))
        .collect();
    assert_eq!(pages.len(), 3);
    assert!(pages.iter().all(|r| r.query["pageSize"] == "50"));
    assert_eq!(pages[2].query["index"], "100");
    assert_eq!(pages[0].query["modLoaderType"], "4");
}

#[test]
fn installs_dependencies_and_records_where_files_came_from() {
    let mock = CurseForgeMock::start();
    create_and_fabric_api(&mock);
    let cf = mock.client();
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    let plan = plan_install(&cf, None, &dirs, &fabric(), CREATE, 5_001).unwrap();
    assert_eq!(plan.files.len(), 2);
    assert!(plan.blocked.is_empty());
    let outcome = execute(&cf, None, &dirs, &fabric(), &plan).unwrap();
    assert_eq!(outcome.installed.len(), 2);
    assert_eq!(
        mods(&dirs),
        ["create-fabric-0.5.1.jar", "fabric-api-0.92.2-cf.jar"]
    );
    let provenance = Provenance::load(&dirs.instance);
    let entry = provenance
        .get(ContentKind::Mod, "create-fabric-0.5.1.jar")
        .unwrap();
    let source = entry.curseforge.as_ref().unwrap();
    assert_eq!((source.project_id, source.file_id), (CREATE, 5_001));
    assert_eq!(source.slug, "create-fabric");
    assert_eq!(
        source.page_url,
        "https://www.curseforge.com/minecraft/mc-mods/create-fabric"
    );
    assert!(source.downloadable);
    assert!(entry.fingerprint.is_some());
    assert_eq!(entry.sha512.len(), 128);
    assert!(entry.modrinth.is_none());

    // Installing again keeps it.
    let again = plan_install(&cf, None, &dirs, &fabric(), CREATE, 5_001).unwrap();
    assert_eq!(again.files.len(), 1, "Fabric API is installed");
    execute(&cf, None, &dirs, &fabric(), &again).unwrap();
    assert_eq!(mods(&dirs).len(), 2);
}

#[test]
fn a_dependency_installed_from_modrinth_isnt_installed_again() {
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    // Fabric API from Modrinth first.
    let mr_mock = ModrinthMock::start();
    mr_mock.project(MR_FABRIC_API, "fabric-api", "mod").version(
        FakeVersion::new(
            "fapi0922",
            MR_FABRIC_API,
            "0.92.2+1.20.1",
            &["fabric"],
            &["1.20.1"],
            OLD,
        )
        .file(
            "fabric-api-0.92.2+1.20.1.jar",
            &fabric_jar("fabric-api", "0.92.2+mr"),
        ),
    );
    let mr = mr_mock.client();
    block_on(async {
        let version = mr.version("fapi0922").await.unwrap();
        let plan = install::plan(
            &mr,
            &dirs,
            &fabric(),
            InstallRequest {
                kind: ContentKind::Mod,
                version,
                project: None,
                install_dependencies: true,
                force: false,
            },
        )
        .await
        .unwrap();
        install::execute(
            &mr,
            &downloader(),
            &dirs,
            &fabric(),
            plan,
            &InstallProgress::default(),
            &CancelToken::new(),
        )
        .await
        .unwrap();
    });
    assert_eq!(mods(&dirs), ["fabric-api-0.92.2+1.20.1.jar"]);

    // Then Create from CurseForge, which requires CurseForge's Fabric API.
    let mock = CurseForgeMock::start();
    create_and_fabric_api(&mock);
    let cf = mock.client();
    let plan = plan_install(&cf, Some(&mr), &dirs, &fabric(), CREATE, 5_001).unwrap();
    let outcome = execute(&cf, Some(&mr), &dirs, &fabric(), &plan).unwrap();
    assert_eq!(
        mods(&dirs),
        ["create-fabric-0.5.1.jar", "fabric-api-0.92.2+1.20.1.jar"],
        "Modrinth's Fabric API stays, and isn't joined by CurseForge's"
    );
    assert!(
        outcome.skipped.iter().any(|s| s
            .reason
            .contains("Already installed as fabric-api-0.92.2+1.20.1.jar")),
        "{outcome:?}"
    );
}

#[test]
fn a_mod_from_the_other_platform_replaces_the_installed_one() {
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    let mods_dir = dirs.content_dir(ContentKind::Mod);
    fs::create_dir_all(&mods_dir).unwrap();
    // Sodium added by hand, disabled.
    fs::write(
        mods_dir.join("sodium-old.jar.disabled"),
        fabric_jar("sodium", "0.5.0"),
    )
    .unwrap();

    let mock = CurseForgeMock::start();
    mock.project(SODIUM, "sodium", CLASS_MODS)
        .file(FakeFile::new(
            7_001,
            SODIUM,
            "sodium-fabric-0.5.13.jar",
            &["1.20.1", "Fabric"],
            NEW,
            fabric_jar("sodium", "0.5.13"),
        ));
    let cf = mock.client();
    let plan = plan_install(&cf, None, &dirs, &fabric(), SODIUM, 7_001).unwrap();
    let outcome = execute(&cf, None, &dirs, &fabric(), &plan).unwrap();
    assert_eq!(outcome.removed, ["sodium-old.jar.disabled"]);
    assert_eq!(
        mods(&dirs),
        ["sodium-fabric-0.5.13.jar.disabled"],
        "replaced, and still disabled"
    );
}

#[test]
fn files_the_author_keeps_on_curseforge_come_from_modrinth_when_it_has_them() {
    let jar = fabric_jar("sodium", "0.5.13");
    let mr_mock = ModrinthMock::start();
    mr_mock.project(MR_SODIUM, "sodium", "mod").version(
        FakeVersion::new(
            "sod0513",
            MR_SODIUM,
            "0.5.13",
            &["fabric"],
            &["1.20.1"],
            NEW,
        )
        .file("sodium-fabric-0.5.13+mc1.20.1.jar", &jar),
    );
    let mr = mr_mock.client();
    let mock = CurseForgeMock::start();
    mock.project(SODIUM, "sodium", CLASS_MODS).file(
        FakeFile::new(
            7_001,
            SODIUM,
            "sodium-fabric-0.5.13.jar",
            &["1.20.1", "Fabric"],
            NEW,
            jar.clone(),
        )
        .blocked(),
    );
    let cf = mock.client();
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    let plan = plan_install(&cf, Some(&mr), &dirs, &fabric(), SODIUM, 7_001).unwrap();
    assert!(plan.blocked.is_empty());
    assert!(plan.files[0].on_modrinth.is_some());
    execute(&cf, Some(&mr), &dirs, &fabric(), &plan).unwrap();
    assert_eq!(mods(&dirs), ["sodium-fabric-0.5.13.jar"]);
    assert!(
        !mock
            .requests()
            .iter()
            .any(|r| r.path.starts_with("/edge.forgecdn.net")),
        "nothing was fetched from CurseForge's CDN"
    );
    let provenance = Provenance::load(&dirs.instance);
    let entry = provenance
        .get(ContentKind::Mod, "sodium-fabric-0.5.13.jar")
        .unwrap();
    assert_eq!(entry.curseforge.as_ref().unwrap().file_id, 7_001);
    assert!(!entry.curseforge.as_ref().unwrap().downloadable);
    assert_eq!(entry.modrinth.as_ref().unwrap().project_id, MR_SODIUM);
}

#[test]
fn files_the_author_keeps_on_curseforge_wait_for_the_users_download() {
    let jar = fabric_jar("secret", "1.0");
    let mock = CurseForgeMock::start();
    mock.project(SECRET_MOD, "secret-mod", CLASS_MODS).file(
        FakeFile::new(
            8_001,
            SECRET_MOD,
            "Secret-Mod-1.0.jar",
            &["1.20.1", "Fabric"],
            NEW,
            jar.clone(),
        )
        .blocked(),
    );
    let cf = mock.client();
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    let plan = plan_install(&cf, None, &dirs, &fabric(), SECRET_MOD, 8_001).unwrap();
    assert!(plan.files.is_empty());
    assert_eq!(plan.blocked.len(), 1);
    assert_eq!(
        plan.blocked[0].file_page(),
        "https://www.curseforge.com/minecraft/mc-mods/secret-mod/download/8001"
    );
    // Nothing to download, so nothing happens.
    assert!(
        execute(&cf, None, &dirs, &fabric(), &plan)
            .unwrap()
            .installed
            .is_empty()
    );

    let downloads = dir.path().join("Downloads");
    fs::create_dir_all(&downloads).unwrap();
    // Named right but not the file; and the file under the name a second download gets.
    fs::write(downloads.join("Secret-Mod-1.0.jar"), b"something else").unwrap();
    assert!(curseforge_install::find_downloaded(&downloads, &plan.blocked).is_empty());
    fs::write(downloads.join("Secret-Mod-1.0 (1).jar"), &jar).unwrap();
    let found = curseforge_install::find_downloaded(&downloads, &plan.blocked);
    assert_eq!(found.len(), 1);
    assert!(found[0].1.ends_with("Secret-Mod-1.0 (1).jar"));

    let placed =
        curseforge_install::install_downloaded(&dirs, &fabric(), &plan.blocked[0], &found[0].1)
            .unwrap();
    assert_eq!(
        placed,
        Placed::Installed {
            name: "Secret-Mod-1.0.jar".into(),
            removed: vec![]
        }
    );
    assert_eq!(mods(&dirs), ["Secret-Mod-1.0.jar"]);
    assert!(found[0].1.exists(), "the user's download stays where it is");
    let provenance = Provenance::load(&dirs.instance);
    let source = provenance
        .get(ContentKind::Mod, "Secret-Mod-1.0.jar")
        .and_then(|e| e.curseforge.clone())
        .unwrap();
    assert_eq!(source.file_id, 8_001);
    // The wrong file is refused.
    assert!(
        curseforge_install::install_downloaded(
            &dirs,
            &fabric(),
            &plan.blocked[0],
            &downloads.join("Secret-Mod-1.0.jar")
        )
        .is_err()
    );
}

#[test]
fn unknown_files_are_matched_on_curseforge_by_fingerprint() {
    let mock = CurseForgeMock::start();
    create_and_fabric_api(&mock);
    let cf = mock.client();
    let mr_mock = ModrinthMock::start();
    let mr = mr_mock.client();
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    let mods_dir = dirs.content_dir(ContentKind::Mod);
    fs::create_dir_all(&mods_dir).unwrap();
    fs::write(mods_dir.join("create.jar"), fabric_jar("create", "0.5.1")).unwrap();
    fs::write(mods_dir.join("mine.jar"), fabric_jar("mine", "1")).unwrap();

    let listing = block_on(listing::list(
        Platforms::new(Some(&mr), Some(&cf)),
        &dirs,
        &fabric(),
        ContentKind::Mod,
    ))
    .unwrap();
    assert_eq!(listing.lookup_error, None);
    let create = listing
        .items
        .iter()
        .find(|i| i.file.file_name == "create.jar")
        .unwrap();
    assert_eq!(create.curseforge.as_ref().unwrap().project_id, CREATE);
    assert_eq!(create.display_name(), "create-fabric title");
    let mine = listing
        .items
        .iter()
        .find(|i| i.file.file_name == "mine.jar")
        .unwrap();
    assert!(!mine.identified());
    let entry = Provenance::load(&dirs.instance)
        .get(ContentKind::Mod, "mine.jar")
        .cloned()
        .unwrap();
    assert!(entry.looked_up && entry.curseforge_looked_up);

    // Neither platform is asked again.
    let before = mock.requests().len();
    block_on(listing::list(
        Platforms::new(Some(&mr), Some(&cf)),
        &dirs,
        &fabric(),
        ContentKind::Mod,
    ))
    .unwrap();
    assert_eq!(mock.requests().len(), before);
}

#[test]
fn updates_keep_to_stable_files_and_report_ones_to_download_by_hand() {
    let mock = CurseForgeMock::start();
    mock.project(SODIUM, "sodium", CLASS_MODS)
        .file(FakeFile::new(
            7_001,
            SODIUM,
            "sodium-0.5.0.jar",
            &["1.20.1", "Fabric"],
            OLD,
            fabric_jar("sodium", "0.5.0"),
        ))
        .file(
            FakeFile::new(
                7_003,
                SODIUM,
                "sodium-0.6.0-beta.jar",
                &["1.20.1", "Fabric"],
                "2025-03-01T00:00:00Z",
                fabric_jar("sodium", "0.6.0-beta"),
            )
            .beta(),
        )
        .file(
            FakeFile::new(
                7_002,
                SODIUM,
                "sodium-0.5.13.jar",
                &["1.20.1", "Fabric"],
                NEW,
                fabric_jar("sodium", "0.5.13"),
            )
            .blocked(),
        )
        .file(FakeFile::new(
            7_004,
            SODIUM,
            "sodium-0.7.0.jar",
            &["1.21.1", "Fabric"],
            "2025-04-01T00:00:00Z",
            fabric_jar("sodium", "0.7.0"),
        ));
    let cf = mock.client();
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    let plan = plan_install(&cf, None, &dirs, &fabric(), SODIUM, 7_001).unwrap();
    execute(&cf, None, &dirs, &fabric(), &plan).unwrap();

    let updates = block_on(listing::check_curseforge_updates(
        &cf,
        &dirs,
        &fabric(),
        ContentKind::Mod,
        false,
    ))
    .unwrap();
    let update = &updates["sodium-0.5.0.jar"];
    assert_eq!(update.file.id, 7_002, "the newest release for 1.20.1");
    assert!(!update.downloadable());
    assert_eq!(
        update.page_url(),
        "https://www.curseforge.com/minecraft/mc-mods/sodium/download/7002"
    );
    let with_betas = block_on(listing::check_curseforge_updates(
        &cf,
        &dirs,
        &fabric(),
        ContentKind::Mod,
        true,
    ))
    .unwrap();
    assert_eq!(with_betas["sodium-0.5.0.jar"].file.id, 7_003);
}

#[test]
fn quilt_instances_take_fabric_files() {
    let mock = CurseForgeMock::start();
    create_and_fabric_api(&mock);
    let cf = mock.client();
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    let quilt = Target::new("1.20.1", Loader::Quilt);
    let plan = plan_install(&cf, None, &dirs, &quilt, CREATE, 5_001).unwrap();
    assert_eq!(plan.files.len(), 2);
    // Quilt takes two loaders, more than CurseForge filters by, so none is sent.
    assert!(
        mock.requests()
            .iter()
            .filter(|r| r.path.ends_with("/files"))
            .all(|r| !r.query.contains_key("modLoaderType"))
    );
    let forge = Target::new("1.20.1", Loader::Forge);
    assert!(matches!(
        plan_err(&cf, &dirs, &forge),
        ContentError::Invalid(reason) if reason.contains("Fabric")
    ));
}

fn plan_err(cf: &CurseForge, dirs: &InstanceDirs, target: &Target) -> ContentError {
    plan_install(cf, None, dirs, target, CREATE, 5_001).unwrap_err()
}

#[test]
fn a_refused_key_and_the_rate_limit_fail_fast() {
    let mock = CurseForgeMock::start();
    create_and_fabric_api(&mock);
    let cf = mock.client();
    mock.state().next_status = Some(403);
    assert_eq!(
        block_on(cf.project(CREATE)).unwrap_err(),
        CurseForgeError::KeyRefused
    );
    mock.state().next_status = Some(429);
    assert!(matches!(
        block_on(cf.project(CREATE)),
        Err(CurseForgeError::RateLimited { .. })
    ));
    let before = mock.requests().len();
    assert!(matches!(
        block_on(cf.project(CREATE)),
        Err(CurseForgeError::RateLimited { .. })
    ));
    assert_eq!(mock.requests().len(), before, "no request while paused");
}

#[test]
fn a_damaged_download_leaves_nothing_behind() {
    let mock = CurseForgeMock::start();
    mock.project(SODIUM, "sodium", CLASS_MODS).file({
        let mut file = FakeFile::new(
            7_001,
            SODIUM,
            "sodium-0.5.13.jar",
            &["1.20.1", "Fabric"],
            NEW,
            fabric_jar("sodium", "0.5.13"),
        );
        file.wrong_fingerprint = true;
        file
    });
    let cf = mock.client();
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    let plan = plan_install(&cf, None, &dirs, &fabric(), SODIUM, 7_001).unwrap();
    let err = execute(&cf, None, &dirs, &fabric(), &plan).unwrap_err();
    assert!(
        err.to_string()
            .contains("isn't the file CurseForge describes"),
        "{err}"
    );
    assert!(mods(&dirs).is_empty(), "{:?}", mods(&dirs));
}

#[test]
fn searches_filter_by_class_loader_and_version() {
    let mock = CurseForgeMock::start();
    create_and_fabric_api(&mock);
    let cf = mock.client();
    let mut search = launcher_core::content::curseforge::CfSearch::new(CLASS_MODS, "create");
    search.game_version = Some("1.20.1".into());
    search.loaders = vec![Loader::Fabric];
    let results = block_on(cf.search(&search)).unwrap();
    assert_eq!(results.hits.len(), 1);
    assert_eq!(results.hits[0].id, CREATE);
    let request = mock
        .requests()
        .into_iter()
        .find(|r| r.path == "/v1/mods/search")
        .unwrap();
    assert_eq!(request.query["gameId"], "432");
    assert_eq!(request.query["classId"], "6");
    assert_eq!(request.query["modLoaderTypes"], "[4]");
    assert_eq!(request.query["gameVersion"], "1.20.1");
}

/// A CurseForge pack zip: `manifest.json` listing `files` (project, file, required) for
/// `loader` on 1.20.1, and an overrides folder.
fn curseforge_pack(dir: &Path, loader: &str, files: &[(u64, u64, bool)]) -> std::path::PathBuf {
    use std::io::Write as _;
    let path = dir.join(format!("pack-{loader}.zip"));
    let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    let manifest = serde_json::json!({
        "minecraft": { "version": "1.20.1.", "modLoaders": [{ "id": loader, "primary": true }] },
        "manifestType": "minecraftModpack",
        "manifestVersion": 1,
        "name": "Mixed Pack",
        "version": "1.2",
        "author": "someone",
        "files": files.iter().map(|(p, f, r)| serde_json::json!({
            "projectID": p, "fileID": f, "required": r
        })).collect::<Vec<_>>(),
        "overrides": "overrides",
    });
    zip.start_file("manifest.json", options).unwrap();
    zip.write_all(manifest.to_string().as_bytes()).unwrap();
    zip.start_file("overrides/config/create.toml", options)
        .unwrap();
    zip.write_all(b"setting = true").unwrap();
    zip.start_file("modlist.html", options).unwrap();
    zip.write_all(b"<ul></ul>").unwrap();
    zip.finish().unwrap();
    path
}

#[test]
fn curseforge_packs_become_instances_with_files_from_both_platforms() {
    use launcher_core::{
        instances::InstanceStore,
        modpack::{
            self, CurseForgePack, ModpackRecord, curseforge as cf_pack,
            import::{self as pack_import, ImportOptions},
        },
    };
    const OPTIONAL_MOD: u64 = 600_001;
    const FAITHFUL: u64 = 700_001;
    let sodium_jar = fabric_jar("sodium", "0.5.13");
    let mock = CurseForgeMock::start();
    create_and_fabric_api(&mock);
    mock.project(SODIUM, "sodium", CLASS_MODS)
        .project(SECRET_MOD, "secret-mod", CLASS_MODS)
        .project(OPTIONAL_MOD, "optional-extras", CLASS_MODS)
        .project(FAITHFUL, "faithful-32x", 12)
        // Kept on CurseForge's website, but Modrinth has the same jar.
        .file(
            FakeFile::new(
                7_001,
                SODIUM,
                "sodium-0.5.13.jar",
                &["1.20.1", "Fabric"],
                NEW,
                sodium_jar.clone(),
            )
            .blocked(),
        )
        // Kept on CurseForge's website, and nowhere else.
        .file(
            FakeFile::new(
                8_001,
                SECRET_MOD,
                "Secret-Mod-1.0.jar",
                &["1.20.1", "Fabric"],
                NEW,
                fabric_jar("secret", "1.0"),
            )
            .blocked(),
        )
        .file(FakeFile::new(
            9_001,
            OPTIONAL_MOD,
            "extras-1.0.jar",
            &["1.20.1", "Fabric"],
            NEW,
            fabric_jar("extras", "1.0"),
        ))
        .file(FakeFile::new(
            9_100,
            FAITHFUL,
            "Faithful-32x.zip",
            &["1.20.1"],
            NEW,
            b"PK resource pack".to_vec(),
        ));
    let mr_mock = ModrinthMock::start();
    mr_mock.project(MR_SODIUM, "sodium", "mod").version(
        FakeVersion::new(
            "sod0513",
            MR_SODIUM,
            "0.5.13",
            &["fabric"],
            &["1.20.1"],
            NEW,
        )
        .file("sodium-fabric-0.5.13+mc1.20.1.jar", &sodium_jar),
    );
    let (cf, mr) = (mock.client(), mr_mock.client());

    let dir = tempfile::tempdir().unwrap();
    let path = curseforge_pack(
        dir.path(),
        "fabric-0.16.10",
        &[
            (CREATE, 5_001, true),
            (FABRIC_API, 5_100, true),
            (SODIUM, 7_001, true),
            (SECRET_MOD, 8_001, true),
            (OPTIONAL_MOD, 9_001, false),
            (FAITHFUL, 9_100, true),
            (CREATE, 99_999, true),
        ],
    );
    assert!(cf_pack::is_curseforge_pack(&path));
    let pack = cf_pack::read_curseforge_pack(&path).unwrap();
    assert_eq!(pack.game_version, "1.20.1", "the trailing dot is dropped");
    let resolved = block_on(cf_pack::resolve(&cf, Some(&mr), &pack)).unwrap();
    assert_eq!(resolved.blocked.len(), 1);
    assert_eq!(resolved.blocked[0].file.id, 8_001);
    assert!(
        resolved
            .info
            .skipped
            .iter()
            .any(|s| s.reason.contains("no longer")),
        "{:?}",
        resolved.info.skipped
    );

    let store = InstanceStore::new(dir.path().join("instances"));
    fs::create_dir_all(store.root()).unwrap();
    let (kind, version) = resolved.info.loader.clone().unwrap();
    let loader = modpack::loader_choice(kind, &version, "1.20.1", None).unwrap();
    let instance = block_on(pack_import::import_with(
        &store,
        Platforms::new(Some(&mr), Some(&cf)),
        &downloader(),
        &path,
        &resolved.info,
        ImportOptions {
            name: "Mixed Pack".into(),
            loader: Some(loader),
            record: ModpackRecord {
                title: "Mixed Pack".into(),
                curseforge: Some(CurseForgePack {
                    project_id: 1,
                    file_id: 2,
                    ..CurseForgePack::default()
                }),
                ..ModpackRecord::default()
            },
            icon: None,
        },
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    assert_eq!(instance.loader, Loader::Fabric);
    let dirs = InstanceDirs::new(store.dir(&instance.id));
    assert_eq!(
        mods(&dirs),
        [
            "create-fabric-0.5.1.jar",
            "extras-1.0.jar.disabled",
            "fabric-api-0.92.2-cf.jar",
            "sodium-0.5.13.jar",
        ],
        "optional mods come disabled; the website-only one waits for the user"
    );
    assert!(
        dirs.content_dir(ContentKind::ResourcePack)
            .join("Faithful-32x.zip")
            .exists()
    );
    assert_eq!(
        fs::read_to_string(dirs.game.join("config/create.toml")).unwrap(),
        "setting = true"
    );
    let record = ModpackRecord::load(&dirs.instance).unwrap();
    assert!(record.on_curseforge());
    let provenance = Provenance::load(&dirs.instance);
    let create = provenance
        .get(ContentKind::Mod, "create-fabric-0.5.1.jar")
        .unwrap();
    assert_eq!(
        create.sha512.len(),
        128,
        "computed for files CurseForge gave no SHA-512 for"
    );
    assert!(
        !mock
            .requests()
            .iter()
            .any(|r| r.path.contains("sodium-0.5.13.jar")),
        "the website-only Sodium came from Modrinth"
    );

    // The listing then finds the CurseForge files by fingerprint.
    let listing = block_on(listing::list(
        Platforms::new(Some(&mr), Some(&cf)),
        &dirs,
        &fabric(),
        ContentKind::Mod,
    ))
    .unwrap();
    let create_item = listing
        .items
        .iter()
        .find(|i| i.file.file_name == "create-fabric-0.5.1.jar")
        .unwrap();
    assert_eq!(create_item.curseforge.as_ref().unwrap().project_id, CREATE);

    // And the website-only file goes in once the user has downloaded it.
    let downloads = dir.path().join("Downloads");
    fs::create_dir_all(&downloads).unwrap();
    fs::write(
        downloads.join("Secret-Mod-1.0.jar"),
        fabric_jar("secret", "1.0"),
    )
    .unwrap();
    let found = curseforge_install::find_downloaded(&downloads, &resolved.blocked);
    curseforge_install::install_downloaded(&dirs, &fabric(), &resolved.blocked[0], &found[0].1)
        .unwrap();
    assert!(mods(&dirs).contains(&"Secret-Mod-1.0.jar".to_string()));
}

#[test]
fn curseforge_pack_manifests_are_checked() {
    use launcher_core::modpack::curseforge as cf_pack;
    let dir = tempfile::tempdir().unwrap();
    let pack = cf_pack::read_curseforge_pack(&curseforge_pack(
        dir.path(),
        "neoforge-1.20.1-47.1.84",
        &[(1, 2, true), (1, 2, true)],
    ))
    .unwrap();
    assert_eq!(pack.files.len(), 1, "a file listed twice is installed once");
    assert_eq!(
        pack.loader,
        Some((
            launcher_core::game::loaders::LoaderKind::NeoForge,
            "47.1.84".into()
        ))
    );
    assert_eq!(pack.overrides.len(), 1);
    assert_eq!(pack.overrides[0].path, "config/create.toml");
    assert!(
        cf_pack::read_curseforge_pack(&curseforge_pack(dir.path(), "liteloader-1.0", &[])).is_err()
    );
}

#[test]
fn modrinth_pack_exports_leave_curseforge_only_files_out() {
    use launcher_core::{export, instances::Instance};
    let mock = CurseForgeMock::start();
    create_and_fabric_api(&mock);
    let cf = mock.client();
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    let plan = plan_install(&cf, None, &dirs, &fabric(), CREATE, 5_001).unwrap();
    execute(&cf, None, &dirs, &fabric(), &plan).unwrap();
    fs::write(
        dirs.content_dir(ContentKind::Mod).join("mine.jar"),
        fabric_jar("mine", "1"),
    )
    .unwrap();

    let instance = Instance {
        name: "Mixed".into(),
        game_version: "1.20.1".into(),
        loader: Loader::Fabric,
        loader_version: Some("0.16.10".into()),
        ..Instance::default()
    };
    let curseforge_only = export::curseforge_only_files(&dirs.instance);
    assert_eq!(curseforge_only.len(), 2);
    let pack = export::plan_leaving_out(
        &instance,
        &dirs.game,
        &export::modrinth_files(&dirs.instance),
        &curseforge_only,
    )
    .unwrap();
    let mut left_out = pack.curseforge_left_out.clone();
    left_out.sort();
    assert_eq!(left_out, ["create-fabric title", "fabric-api title"]);
    let copied: Vec<&str> = pack
        .overrides
        .iter()
        .map(|(path, _)| path.as_str())
        .collect();
    assert_eq!(
        copied,
        ["overrides/mods/mine.jar"],
        "a jar added by hand is still copied"
    );

    // A pack for the user's own server keeps everything.
    let own = export::plan(
        &instance,
        &dirs.game,
        &export::modrinth_files(&dirs.instance),
    )
    .unwrap();
    assert_eq!(own.overrides.len(), 3);
    assert!(own.curseforge_left_out.is_empty());
}
