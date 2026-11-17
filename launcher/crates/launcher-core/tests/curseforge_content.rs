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

#[test]
fn a_pasted_link_finds_its_project_and_file() {
    let mock = CurseForgeMock::start();
    create_and_fabric_api(&mock);
    let cf = mock.client();
    let target = block_on(
        cf.resolve_link("https://www.curseforge.com/minecraft/mc-mods/create-fabric/files/5001"),
    )
    .unwrap();
    assert_eq!(target.project.id, CREATE);
    assert_eq!(target.file.map(|file| file.id), Some(5_001));
    // by id, without a file
    let by_id =
        block_on(cf.resolve_link(&format!("curseforge.com/projects/{FABRIC_API}"))).unwrap();
    assert_eq!((by_id.project.id, by_id.file), (FABRIC_API, None));
    // a slug of another class, and one nobody has
    for link in [
        "https://www.curseforge.com/minecraft/modpacks/create-fabric",
        "https://www.curseforge.com/minecraft/mc-mods/nothing-here",
    ] {
        assert!(
            matches!(
                block_on(cf.resolve_link(link)),
                Err(CurseForgeError::NotFound(_))
            ),
            "{link}"
        );
    }
    // not a CurseForge project link: nothing is asked
    let asked = mock.requests().len();
    assert!(block_on(cf.resolve_link("https://modrinth.com/mod/sodium")).is_err());
    assert_eq!(mock.requests().len(), asked);
}

#[test]
fn a_copy_for_another_version_takes_the_curseforge_mods_with_a_file_for_it() {
    use launcher_core::{
        content::copy_as::{self, CopyAsOptions, CopyAsRequest, ResolvedMods, WorldsRule},
        game::loaders::{LoaderChoice, LoaderKind},
        instances::InstanceStore,
    };
    let mock = CurseForgeMock::start();
    create_and_fabric_api(&mock);
    // For 1.21.1: Fabric API has a file, Create has none, and Sodium's is kept on
    // CurseForge's website.
    mock.project(SODIUM, "sodium", CLASS_MODS)
        .file(FakeFile::new(
            5_200,
            SODIUM,
            "sodium-0.5.jar",
            &["1.20.1", "Fabric"],
            OLD,
            fabric_jar("sodium", "0.5"),
        ))
        .file(
            FakeFile::new(
                5_201,
                SODIUM,
                "sodium-0.6.jar",
                &["1.21.1", "Fabric"],
                NEW,
                fabric_jar("sodium", "0.6"),
            )
            .blocked(),
        )
        .file(FakeFile::new(
            5_101,
            FABRIC_API,
            "fabric-api-0.110-cf.jar",
            &["1.21.1", "Fabric"],
            NEW,
            fabric_jar("fabric-api", "0.110"),
        ));
    let cf = mock.client();
    // Modrinth knows none of them.
    let modrinth = ModrinthMock::start();
    let mr = modrinth.client();
    let root = tempfile::tempdir().unwrap();
    let store = InstanceStore::new(root.path().join("instances"));
    let loader = |version: &str| LoaderChoice {
        kind: LoaderKind::Fabric,
        version: version.into(),
    };
    let source = store
        .create("Survival", "1.20.1", Some(&loader("0.16.9")))
        .unwrap();
    let dirs = InstanceDirs::new(store.dir(&source.id));
    for (project, file) in [(CREATE, 5_001), (SODIUM, 5_200)] {
        let plan = plan_install(&cf, Some(&mr), &dirs, &fabric(), project, file).unwrap();
        execute(&cf, Some(&mr), &dirs, &fabric(), &plan).unwrap();
    }
    launcher_core::content::files::set_mod_enabled(&dirs, "fabric-api-0.92.2-cf.jar", false)
        .unwrap();
    fs::write(
        dirs.content_dir(ContentKind::Mod).join("homemade.jar"),
        b"added by hand",
    )
    .unwrap();

    let platforms = Platforms::new(Some(&mr), Some(&cf));
    let contents = block_on(copy_as::contents_on(platforms, &dirs)).unwrap();
    assert!(contents.mods.is_empty());
    assert_eq!(contents.other_mods, ["homemade"]);
    let mut projects: Vec<u64> = contents
        .curseforge_mods
        .iter()
        .map(|m| m.project_id)
        .collect();
    projects.sort_unstable();
    assert_eq!(projects, [FABRIC_API, CREATE, SODIUM]);
    let fabric_api = contents
        .curseforge_mods
        .iter()
        .find(|m| m.project_id == FABRIC_API)
        .unwrap();
    assert!(!fabric_api.enabled);

    let new_target = Target::new("1.21.1", Loader::Fabric);
    let mut resolved = ResolvedMods::default();
    block_on(copy_as::resolve_curseforge_mods(
        &cf,
        &contents.curseforge_mods,
        &new_target,
        &mut resolved,
    ))
    .unwrap();
    let mut found: Vec<u64> = resolved.curseforge.iter().map(|(_, f)| f.id).collect();
    found.sort_unstable();
    assert_eq!(found, [5_101, 5_201]);
    assert_eq!(resolved.curseforge_missing.len(), 1);
    assert_eq!(resolved.curseforge_missing[0].project_id, CREATE);

    // Without the lookup on CurseForge (it can't be reached) they all stay behind.
    let unresolved = copy_as::plan(
        &contents,
        "1.21.1",
        Loader::Fabric,
        CopyAsOptions::default(),
        WorldsRule::Newer,
        Some(&ResolvedMods::default()),
    );
    assert!(unresolved.curseforge_mods.is_empty());
    assert_eq!(unresolved.left_behind().len(), 4);

    let plan = copy_as::plan(
        &contents,
        "1.21.1",
        Loader::Fabric,
        CopyAsOptions::default(),
        WorldsRule::Newer,
        Some(&resolved),
    );
    assert_eq!(plan.curseforge_mods.len(), 2);
    assert!(plan.needs_confirmation());
    let outcome = block_on(copy_as::execute_on(
        &store,
        platforms,
        &downloader(),
        &source,
        &CopyAsRequest {
            name: None,
            loader: Some(loader("0.16.14")),
            plan,
        },
        &InstallProgress::default(),
        &CancelToken::new(),
        || {},
    ))
    .unwrap();
    // Create has no file for 1.21.1, and nobody knows the mod added by hand.
    assert_eq!(outcome.left_behind.len(), 2, "{:?}", outcome.left_behind);
    assert!(outcome.left_behind[0].contains("create"));
    assert_eq!(outcome.left_behind[1], "homemade");
    // Sodium's file is for the user to download, not left behind.
    let blocked: Vec<u64> = outcome.blocked.iter().map(|b| b.file.id).collect();
    assert_eq!(blocked, [5_201]);
    // Fabric API came, and stays off.
    let copy_dirs = InstanceDirs::new(store.dir(&outcome.instance.id));
    let copied = mods(&copy_dirs);
    assert_eq!(copied.len(), 1, "{copied:?}");
    assert!(copied[0].starts_with("fabric-api-0.110-cf.jar"));
    assert_ne!(copied[0], "fabric-api-0.110-cf.jar", "it was off");
    // The original is as it was.
    assert_eq!(mods(&dirs).len(), 4);
}

#[test]
fn a_curseforge_pack_updates_to_a_newer_file_of_itself() {
    use launcher_core::{
        instances::InstanceStore,
        modpack::{
            self, CurseForgePack, ModpackRecord, curseforge as cf_pack,
            import::{self as pack_import, ImportOptions},
            update,
        },
    };
    let mock = CurseForgeMock::start();
    create_and_fabric_api(&mock);
    mock.file(FakeFile::new(
        5_102,
        FABRIC_API,
        "fabric-api-0.93.0-cf.jar",
        &["1.20.1", "Fabric"],
        NEW,
        fabric_jar("fabric-api", "0.93.0"),
    ));
    let cf = mock.client();
    let platforms = Platforms::new(None, Some(&cf));
    let dir = tempfile::tempdir().unwrap();
    let record = |file_id: u64, version: &str| ModpackRecord {
        title: "Mixed Pack".into(),
        version_number: version.into(),
        curseforge: Some(CurseForgePack {
            project_id: 1,
            file_id,
            ..CurseForgePack::default()
        }),
        ..ModpackRecord::default()
    };

    // 1.0: Create and the Fabric API it needs.
    let first_dir = dir.path().join("first");
    fs::create_dir_all(&first_dir).unwrap();
    let first = curseforge_pack(
        &first_dir,
        "fabric-0.16.10",
        &[(CREATE, 5_001, true), (FABRIC_API, 5_100, true)],
    );
    let read = cf_pack::read_curseforge_pack(&first).unwrap();
    let resolved = block_on(cf_pack::resolve(&cf, None, &read)).unwrap();
    let store = InstanceStore::new(dir.path().join("instances"));
    fs::create_dir_all(store.root()).unwrap();
    let (kind, version) = resolved.info.loader.clone().unwrap();
    let loader = modpack::loader_choice(kind, &version, "1.20.1", None).unwrap();
    let instance = block_on(pack_import::import_with(
        &store,
        platforms,
        &downloader(),
        &first,
        &resolved.info,
        ImportOptions {
            name: "Mixed Pack".into(),
            loader: Some(loader.clone()),
            record: record(2, "1.0"),
            icon: None,
        },
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    let dirs = InstanceDirs::new(store.dir(&instance.id));
    assert_eq!(
        mods(&dirs),
        ["create-fabric-0.5.1.jar", "fabric-api-0.92.2-cf.jar"]
    );
    // The player's own mod, and a config of the pack they changed.
    fs::write(dirs.content_dir(ContentKind::Mod).join("mine.jar"), b"mine").unwrap();
    fs::write(dirs.game.join("config/create.toml"), b"setting = false").unwrap();

    // 2.0 drops Create and has a newer Fabric API.
    let second_dir = dir.path().join("second");
    fs::create_dir_all(&second_dir).unwrap();
    let second = curseforge_pack(&second_dir, "fabric-0.16.10", &[(FABRIC_API, 5_102, true)]);
    let read = cf_pack::read_curseforge_pack(&second).unwrap();
    let resolved = block_on(cf_pack::resolve(&cf, None, &read)).unwrap();
    assert!(resolved.blocked.is_empty());
    let current = ModpackRecord::load(&dirs.instance).unwrap();
    assert!(current.updatable() && !current.on_modrinth());
    let plan = update::plan(
        &dirs,
        &current,
        &resolved.info,
        record(3, "2.0"),
        Some(loader),
    );
    // CurseForge's file URLs don't name their project, so a newer file of a mod is
    // listed as the old one removed and the new one added.
    assert_eq!(
        plan.removed,
        [
            "mods/create-fabric-0.5.1.jar",
            "mods/fabric-api-0.92.2-cf.jar"
        ]
    );
    assert_eq!(plan.added, ["mods/fabric-api-0.93.0-cf.jar"]);
    assert!(plan.changes_files());

    // Without the CurseForge client its files can't be downloaded, and nothing changes.
    let without = block_on(update::apply(
        None,
        &downloader(),
        &dirs,
        &plan,
        &second,
        &InstallProgress::default(),
        &CancelToken::new(),
    ));
    assert!(without.is_err());
    assert_eq!(mods(&dirs).len(), 3);

    block_on(update::apply_on(
        platforms,
        &downloader(),
        &dirs,
        &plan,
        &second,
        &InstallProgress::default(),
        &CancelToken::new(),
    ))
    .unwrap();
    assert_eq!(mods(&dirs), ["fabric-api-0.93.0-cf.jar", "mine.jar"]);
    assert_eq!(
        fs::read(dirs.game.join("config/create.toml")).unwrap(),
        b"setting = false",
        "the changed config stays"
    );
    let updated = ModpackRecord::load(&dirs.instance).unwrap();
    assert_eq!(updated.version_number, "2.0");
    assert_eq!(updated.curseforge.unwrap().file_id, 3);
}

#[test]
fn curseforge_pack_exports_list_its_files_and_leave_modrinth_only_ones_out() {
    use launcher_core::{export, instances::Instance, modpack::curseforge as cf_pack};
    // Sodium and Lithium came from Modrinth; CurseForge has the same Sodium jar.
    let sodium_jar = fabric_jar("sodium", "0.5.13");
    let mr_mock = ModrinthMock::start();
    mr_mock
        .project(MR_SODIUM, "sodium", "mod")
        .version(
            FakeVersion::new(
                "sod0513",
                MR_SODIUM,
                "0.5.13",
                &["fabric"],
                &["1.20.1"],
                NEW,
            )
            .file("sodium-fabric-0.5.13+mc1.20.1.jar", &sodium_jar),
        )
        .project("gvQqBUqZ", "lithium", "mod")
        .version(
            FakeVersion::new(
                "lith0116",
                "gvQqBUqZ",
                "0.11.6",
                &["fabric"],
                &["1.20.1"],
                NEW,
            )
            .file("lithium-0.11.6.jar", &fabric_jar("lithium", "0.11.6")),
        );
    let mr = mr_mock.client();
    let mock = CurseForgeMock::start();
    create_and_fabric_api(&mock);
    mock.project(SODIUM, "sodium", CLASS_MODS)
        .file(FakeFile::new(
            7_001,
            SODIUM,
            "sodium-0.5.13.jar",
            &["1.20.1", "Fabric"],
            NEW,
            sodium_jar.clone(),
        ));
    let cf = mock.client();
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    // Create (with Fabric API, turned off) from CurseForge.
    let plan = plan_install(&cf, None, &dirs, &fabric(), CREATE, 5_001).unwrap();
    execute(&cf, None, &dirs, &fabric(), &plan).unwrap();
    launcher_core::content::files::set_mod_enabled(&dirs, "fabric-api-0.92.2-cf.jar", false)
        .unwrap();
    block_on(async {
        for id in ["sod0513", "lith0116"] {
            let version = mr.version(id).await.unwrap();
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
        }
    });
    fs::write(
        dirs.content_dir(ContentKind::Mod).join("mine.jar"),
        fabric_jar("mine", "1"),
    )
    .unwrap();
    fs::create_dir_all(dirs.game.join("config")).unwrap();
    fs::write(dirs.game.join("config/create.toml"), b"setting = true").unwrap();

    let instance = Instance {
        name: "Mixed".into(),
        game_version: "1.20.1".into(),
        loader: Loader::Fabric,
        loader_version: Some("0.16.10".into()),
        ..Instance::default()
    };
    let mut on_curseforge = export::curseforge_files(&dirs.instance);
    assert_eq!(on_curseforge.len(), 2);
    let modrinth_only = export::modrinth_only_files(&dirs.instance);
    assert_eq!(modrinth_only.len(), 2);
    // CurseForge has Sodium's jar too, by its fingerprint; not Lithium's.
    let found = block_on(export::find_on_curseforge(&cf, &dirs.game, &modrinth_only)).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found.values().next().unwrap().file_id, 7_001);
    on_curseforge.extend(found);

    let pack =
        export::plan_curseforge(&instance, &dirs.game, &on_curseforge, &modrinth_only).unwrap();
    assert_eq!(pack.modrinth_left_out, ["lithium title"]);
    let copied: Vec<&str> = pack.overrides.iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(
        copied,
        ["overrides/config/create.toml", "overrides/mods/mine.jar"]
    );
    let out = dir.path().join("Mixed.zip");
    export::write_curseforge(&pack, &out).unwrap();
    assert!(!dir.path().join("Mixed.zip.part").exists());

    // The pack reads back as a CurseForge pack with the same files.
    assert!(cf_pack::is_curseforge_pack(&out));
    let read = cf_pack::read_curseforge_pack(&out).unwrap();
    assert_eq!(read.name, "Mixed");
    assert_eq!(read.game_version, "1.20.1");
    assert_eq!(
        read.loader,
        Some((
            launcher_core::game::loaders::LoaderKind::Fabric,
            "0.16.10".into()
        ))
    );
    let mut listed: Vec<(u64, bool)> = read.files.iter().map(|f| (f.file_id, f.required)).collect();
    listed.sort_unstable();
    // Fabric API was off, so it is optional: it comes in turned off.
    assert_eq!(listed, [(5_001, true), (5_100, false), (7_001, true)]);
    let overrides: Vec<&str> = read.overrides.iter().map(|o| o.path.as_str()).collect();
    assert_eq!(overrides, ["config/create.toml", "mods/mine.jar"]);
}

#[test]
fn searches_filter_by_curseforges_categories() {
    use launcher_core::content::curseforge::CfSearch;
    let mock = CurseForgeMock::start();
    create_and_fabric_api(&mock);
    let cf = mock.client();
    // The class itself isn't a category; the rest come by name.
    let categories = block_on(cf.categories(CLASS_MODS)).unwrap();
    let names: Vec<&str> = categories.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["automation", "Storage", "Technology"]);
    let asked = mock
        .requests()
        .into_iter()
        .find(|r| r.path == "/v1/categories")
        .unwrap();
    assert_eq!(asked.query["gameId"], "432");
    assert_eq!(asked.query["classId"], "6");

    let last_search = || {
        mock.requests()
            .into_iter()
            .rev()
            .find(|r| r.path == "/v1/mods/search")
            .unwrap()
    };
    let mut search = CfSearch::new(CLASS_MODS, "");
    block_on(cf.search(&search)).unwrap();
    let none = last_search();
    assert!(!none.query.contains_key("categoryId") && !none.query.contains_key("categoryIds"));
    search.categories = vec![420];
    block_on(cf.search(&search)).unwrap();
    assert_eq!(last_search().query["categoryId"], "420");
    // Several go as a list, ten at most.
    search.categories = (400..415).collect();
    block_on(cf.search(&search)).unwrap();
    let many = last_search();
    assert!(!many.query.contains_key("categoryId"));
    assert_eq!(
        many.query["categoryIds"],
        "[400,401,402,403,404,405,406,407,408,409]"
    );
}

#[test]
fn curseforge_data_packs_install_into_a_world_and_update_in_place() {
    use std::io::Write as _;

    use launcher_core::{
        content::curseforge::CLASS_DATA_PACKS,
        datapacks::{self, PackState},
        game::download::Progress,
        worlds::{self, WorldError},
    };
    const LEVEL_DAT: &[u8] =
        include_bytes!("../../../../crates/lodestone-minecraft/testdata/level/1.21.11.dat");
    const TERRA: u64 = 800_001;
    let pack = |marker: &str| {
        let mut bytes = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut bytes));
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("pack.mcmeta", options).unwrap();
            zip.write_all(br#"{"pack": {"pack_format": 48, "description": "A test pack"}}"#)
                .unwrap();
            zip.start_file("data/a/b.json", options).unwrap();
            zip.write_all(marker.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        bytes
    };
    let mock = CurseForgeMock::start();
    mock.project(TERRA, "terra", CLASS_DATA_PACKS)
        .file(FakeFile::new(
            8_101,
            TERRA,
            "terra-1.zip",
            &["1.21.11"],
            OLD,
            pack("one"),
        ))
        .file(FakeFile::new(
            8_102,
            TERRA,
            "terra-2.zip",
            &["1.21.11"],
            NEW,
            pack("two"),
        ))
        // Newer still, but its author keeps it on CurseForge's website.
        .file(
            FakeFile::new(
                8_103,
                TERRA,
                "terra-3.zip",
                &["1.21.11"],
                "2025-03-01T00:00:00Z",
                pack("three"),
            )
            .blocked(),
        )
        .file(FakeFile::new(
            8_104,
            TERRA,
            "terra-old.zip",
            &["1.20.1"],
            NEW,
            pack("old"),
        ));
    let cf = mock.client();
    let dir = tempfile::tempdir().unwrap();
    let dirs = instance(dir.path());
    let world = worlds::saves_dir(&dirs).join("New World");
    fs::create_dir_all(&world).unwrap();
    fs::write(world.join("level.dat"), LEVEL_DAT).unwrap();
    let install = |file: u64| -> Result<datapacks::Installed, WorldError> {
        let file = block_on(cf.file(TERRA, file)).unwrap();
        block_on(datapacks::install_curseforge_with(
            &cf,
            &downloader(),
            &dirs,
            "New World",
            &file,
            &Progress::default(),
            &CancelToken::new(),
            |path| fs::remove_file(path).map_err(|e| e.to_string()),
        ))
    };

    // The newest file for the version that can be downloaded is picked.
    let files = block_on(cf.files(TERRA, None, None)).unwrap();
    assert_eq!(files.len(), 4);
    let picked = datapacks::pick_curseforge_file("1.21.11", files.clone()).unwrap();
    assert_eq!(picked.id, 8_102);
    assert!(datapacks::pick_curseforge_file("1.19.2", files).is_none());

    let first = install(8_101).unwrap();
    assert_eq!((first.name.as_str(), first.replaced), ("terra-1.zip", None));
    datapacks::set_enabled(&dirs, "New World", "terra-1.zip", false).unwrap();
    // The newer file replaces it, and stays off.
    let second = install(8_102).unwrap();
    assert_eq!(second.name, "terra-2.zip");
    assert_eq!(second.replaced.as_deref(), Some("terra-1.zip"));
    let listed = datapacks::list(&dirs, "New World").unwrap();
    assert_eq!(listed.packs.len(), 1);
    let only = &listed.packs[0];
    assert_eq!(only.name, "terra-2.zip");
    assert_eq!(only.state, PackState::Disabled);
    assert_eq!(only.curseforge_project, Some(TERRA));
    assert_eq!(only.modrinth_project, None);
    // The same file again changes nothing.
    let again = install(8_102).unwrap();
    assert_eq!(again.replaced, None);
    // One kept on CurseForge's website can't be downloaded, and says so.
    let blocked = install(8_103).unwrap_err();
    assert!(
        blocked.to_string().contains("CurseForge's website"),
        "{blocked}"
    );
    assert_eq!(datapacks::list(&dirs, "New World").unwrap().packs.len(), 1);
}
