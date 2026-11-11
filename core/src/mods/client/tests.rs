use std::collections::HashMap;
use std::io::Read;

use super::*;
use crate::modrinth::fake::{sha1_hex, sha512_hex, FakeModrinth, FakeVersion};
use crate::mods::metadata::tests::make_zip;
use crate::mods::{build_list, InstalledModList};

struct Setup {
    _dir: tempfile::TempDir,
    folder: ModFolder,
    fake: FakeModrinth,
}

impl Setup {
    fn new(loader: InstalledModLoader) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let folder = ModFolder {
            loader,
            game_version: "1.20.1".into(),
            dir: dir.path().join("mods"),
        };
        std::fs::create_dir_all(&folder.dir).unwrap();
        Setup {
            _dir: dir,
            folder,
            fake: FakeModrinth::default(),
        }
    }

    /// Puts a Modrinth version's file into the folder as `file_name`.
    fn add_matched(&self, version: FakeVersion, client_side: &str, file_name: &str) {
        std::fs::write(self.folder.dir.join(file_name), &version.content).unwrap();
        self.fake
            .set_client_side(&version.version.project_id, client_side);
        self.fake.add(version);
    }

    /// Puts a jar that is not on Modrinth into the folder.
    fn add_jar(&self, file_name: &str, fabric_mod_json: &str) {
        make_zip(
            &self.folder.dir.join(file_name),
            &[("fabric.mod.json", fabric_mod_json)],
        );
    }

    async fn list(&self) -> InstalledModList {
        let files = scan_folder(&self.folder).await.unwrap();
        build_list(
            &self.fake,
            &self.folder,
            files,
            false,
            false,
            Some("0.15.11".into()),
        )
        .await
        .unwrap()
    }
}

fn by_file(list: &InstalledModList) -> HashMap<String, (ClientRequirement, Vec<String>)> {
    list.mods
        .iter()
        .map(|m| {
            (
                m.file_name.clone(),
                (m.client_requirement, m.required_by.clone()),
            )
        })
        .collect()
}

/// A server with: a client mod that needs a library (Modrinth dependency), a server-only
/// mod, a jar not on Modrinth that needs another jar (Fabric dependency), a disabled
/// client mod that depends on the server-only mod, and two mods that need each other.
fn typical_server() -> Setup {
    let setup = Setup::new(InstalledModLoader::Fabric);
    setup.add_matched(
        FakeVersion::new("app", "app1", "app.jar").requires("lib", None),
        "required",
        "app.jar",
    );
    setup.add_matched(
        FakeVersion::new("lib", "lib1", "lib.jar"),
        "optional",
        "lib.jar",
    );
    setup.add_matched(
        FakeVersion::new("srv", "srv1", "srv.jar"),
        "unsupported",
        "srv.jar",
    );
    setup.add_matched(
        FakeVersion::new("dis", "dis1", "dis.jar").requires("srv", None),
        "required",
        "dis.jar.disabled",
    );
    setup.add_matched(
        FakeVersion::new("x", "x1", "x.jar").requires("y", None),
        "required",
        "x.jar",
    );
    setup.add_matched(
        FakeVersion::new("y", "y1", "y.jar").requires("x", None),
        "unknown",
        "y.jar",
    );
    setup.add_jar(
        "custom.jar",
        r#"{"id": "custom", "name": "Custom", "environment": "client",
            "depends": {"helperlib": "*", "minecraft": "1.20.1", "fabricloader": "*", "java": ">=17"}}"#,
    );
    setup.add_jar("helper.jar", r#"{"id": "helperlib", "name": "Helper"}"#);
    setup.add_jar(
        "serveronly.jar",
        r#"{"id": "so", "environment": "server", "depends": {"helperlib": "*"}}"#,
    );
    setup.add_jar("plain.jar", r#"{"id": "plain"}"#);
    setup
}

#[tokio::test]
async fn requirements_and_dependency_propagation() {
    let setup = typical_server();
    let list = setup.list().await;
    let mods = by_file(&list);
    let requirement = |name: &str| mods[name].0;
    let required_by = |name: &str| mods[name].1.clone();

    assert_eq!(requirement("app.jar"), ClientRequirement::Required);
    assert!(required_by("app.jar").is_empty());
    // optional on its own, needed because app needs it
    assert_eq!(requirement("lib.jar"), ClientRequirement::Required);
    assert_eq!(required_by("lib.jar"), vec!["Title of app"]);
    // a disabled mod keeps its own requirement but makes nothing needed
    assert_eq!(requirement("dis.jar.disabled"), ClientRequirement::Required);
    assert_eq!(requirement("srv.jar"), ClientRequirement::Unsupported);
    assert!(required_by("srv.jar").is_empty());
    // jars not on Modrinth: environment and depends from fabric.mod.json
    assert_eq!(requirement("custom.jar"), ClientRequirement::Required);
    assert_eq!(requirement("helper.jar"), ClientRequirement::Required);
    assert_eq!(required_by("helper.jar"), vec!["Custom"]);
    assert_eq!(
        requirement("serveronly.jar"),
        ClientRequirement::Unsupported
    );
    assert_eq!(requirement("plain.jar"), ClientRequirement::Unknown);
    // a cycle ends
    assert_eq!(requirement("x.jar"), ClientRequirement::Required);
    assert_eq!(requirement("y.jar"), ClientRequirement::Required);
    assert_eq!(required_by("y.jar"), vec!["Title of x"]);
    assert_eq!(required_by("x.jar"), vec!["Title of y"]);

    // app, lib, custom, helper, x, y; the disabled one is not counted
    assert_eq!(list.client_required_count, 6);
    assert_eq!(list.loader_version.as_deref(), Some("0.15.11"));
    let app = list.mods.iter().find(|m| m.file_name == "app.jar").unwrap();
    let m = app.modrinth.as_ref().unwrap();
    assert_eq!(m.client_side, "required");
    assert_eq!(m.server_side, "required");
}

#[tokio::test]
async fn disabled_dependencies_are_not_made_required() {
    let setup = Setup::new(InstalledModLoader::Fabric);
    setup.add_matched(
        FakeVersion::new("app", "app1", "app.jar").requires("lib", None),
        "required",
        "app.jar",
    );
    setup.add_matched(
        FakeVersion::new("lib", "lib1", "lib.jar"),
        "optional",
        "lib.jar.disabled",
    );
    let mods = by_file(&setup.list().await);
    assert_eq!(mods["lib.jar.disabled"].0, ClientRequirement::Optional);
    assert!(mods["lib.jar.disabled"].1.is_empty());
}

#[tokio::test]
async fn plugins_are_never_required() {
    let setup = Setup::new(InstalledModLoader::Paper);
    let plugin = FakeVersion::new("p", "p1", "p.jar")
        .loaders(&["paper"])
        .requires("q", None);
    setup.add_matched(plugin, "required", "p.jar");
    setup.add_matched(
        FakeVersion::new("q", "q1", "q.jar").loaders(&["paper"]),
        "required",
        "q.jar",
    );
    let list = setup.list().await;
    assert_eq!(list.loader_version, None);
    assert_eq!(list.client_required_count, 0);
    for m in &list.mods {
        assert_eq!(m.client_requirement, ClientRequirement::Unsupported);
        assert!(m.required_by.is_empty());
    }

    let out = setup.folder.dir.join("pack.mrpack");
    let err = build_client_pack(&setup.fake, &setup.folder, "S", Some("1"), &out)
        .await
        .unwrap_err();
    assert!(matches!(err.kind, ErrorKind::BadRequest));
    assert_eq!(
        err.source.to_string(),
        "Client modpacks are only available for Fabric servers"
    );
    assert!(!out.exists());
}

/// Reads a pack back: the index and every other entry's bytes.
fn read_pack(path: &Path) -> (serde_json::Value, PackIndex, HashMap<String, Vec<u8>>) {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    let mut entries = HashMap::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).unwrap();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        entries.insert(entry.name().unwrap().to_string(), bytes);
    }
    let raw = entries.remove("modrinth.index.json").expect("the index");
    let json: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    let index: PackIndex = serde_json::from_slice(&raw).unwrap();
    (json, index, entries)
}

#[tokio::test]
async fn client_pack_is_a_valid_mrpack() {
    let setup = typical_server();
    let out = setup._dir.path().join("pack.mrpack");
    let plan = build_client_pack(
        &setup.fake,
        &setup.folder,
        "Survival",
        Some("0.15.11"),
        &out,
    )
    .await
    .unwrap();
    assert_eq!(plan.included.len(), 2);

    let (json, index, entries) = read_pack(&out);
    assert_eq!(json["formatVersion"], 1);
    assert_eq!(json["game"], "minecraft");
    assert_eq!(json["versionId"], "Survival (Lodestone server)");
    assert_eq!(json["name"], "Survival client mods");
    assert_eq!(json["summary"], "Mods needed to join Survival");
    assert_eq!(
        json["dependencies"],
        serde_json::json!({"minecraft": "1.20.1", "fabric-loader": "0.15.11"})
    );

    // only enabled, needed mods that are on Modrinth; not srv, dis or the unknown ones
    let paths: Vec<&str> = index.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        paths,
        vec!["mods/app.jar", "mods/lib.jar", "mods/x.jar", "mods/y.jar"]
    );
    for file in &index.files {
        let name = file.path.strip_prefix("mods/").unwrap();
        let bytes = std::fs::read(setup.folder.dir.join(name)).unwrap();
        assert_eq!(file.hashes.sha1, sha1_hex(&bytes), "{name}");
        assert_eq!(file.hashes.sha512, sha512_hex(&bytes), "{name}");
        assert_eq!(file.file_size, bytes.len() as u64);
        assert_eq!(file.env.client, "required");
        assert_eq!(file.env.server, "unsupported");
        assert_eq!(file.downloads.len(), 1);
        assert!(file.downloads[0].starts_with("https://cdn.modrinth.com/"));
    }
    let entry = &json["files"][0];
    assert!(entry["fileSize"].is_u64());
    assert!(entry["hashes"]["sha1"].is_string());

    // jars that are not on Modrinth are inside, and nothing else is
    let mut names: Vec<&String> = entries.keys().collect();
    names.sort();
    assert_eq!(
        names,
        vec!["overrides/mods/custom.jar", "overrides/mods/helper.jar"]
    );
    assert_eq!(
        entries["overrides/mods/custom.jar"],
        std::fs::read(setup.folder.dir.join("custom.jar")).unwrap()
    );

    // the pack is valid for Lodestone's own modpack reader too
    let pack = crate::modpack::read_pack(&out, false).unwrap();
    assert_eq!(pack.info.unsupported_reason, None);
    assert_eq!(pack.info.game_version, "1.20.1");
    assert_eq!(pack.info.loader_version.as_deref(), Some("0.15.11"));
    // every file is client only
    assert_eq!(pack.info.server_file_count, 0);
}

#[tokio::test]
async fn files_not_on_the_modrinth_cdn_are_included() {
    let setup = Setup::new(InstalledModLoader::Fabric);
    setup.add_matched(
        FakeVersion::new("gh", "gh1", "gh.jar").url("https://github.com/x/gh.jar"),
        "required",
        "gh.jar",
    );
    let out = setup._dir.path().join("pack.mrpack");
    let plan = build_client_pack(&setup.fake, &setup.folder, "S", Some("0.15.11"), &out)
        .await
        .unwrap();
    assert!(plan.index.files.is_empty());
    let (_, _, entries) = read_pack(&out);
    assert!(entries.contains_key("overrides/mods/gh.jar"));
}

#[tokio::test]
async fn no_needed_mods_is_an_error() {
    let setup = Setup::new(InstalledModLoader::Fabric);
    setup.add_matched(
        FakeVersion::new("srv", "srv1", "srv.jar"),
        "unsupported",
        "srv.jar",
    );
    setup.add_matched(
        FakeVersion::new("cl", "cl1", "cl.jar"),
        "required",
        "cl.jar.disabled",
    );
    let out = setup._dir.path().join("pack.mrpack");
    let err = build_client_pack(&setup.fake, &setup.folder, "S", Some("0.15.11"), &out)
        .await
        .unwrap_err();
    assert!(matches!(err.kind, ErrorKind::BadRequest));
    assert_eq!(
        err.source.to_string(),
        "Players don't need any mods to join this server"
    );
    assert!(!out.exists());

    // an empty folder too
    let empty = Setup::new(InstalledModLoader::Fabric);
    assert!(
        build_client_pack(&empty.fake, &empty.folder, "S", Some("0.15.11"), &out)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn a_marked_uploaded_jar_goes_to_players_with_what_it_needs() {
    let setup = Setup::new(InstalledModLoader::Fabric);
    // an uploaded jar that says nothing about where it runs, needing another such jar
    setup.add_jar(
        "custom.jar",
        r#"{"schemaVersion":1,"id":"custom","name":"Custom","depends":{"helper":"*"}}"#,
    );
    setup.add_jar("helper.jar", r#"{"schemaVersion":1,"id":"helper"}"#);
    let list = setup.list().await;
    let found = by_file(&list);
    assert_eq!(found["custom.jar"].0, ClientRequirement::Unknown);
    assert_eq!(list.client_required_count, 0);
    let out = setup._dir.path().join("pack.mrpack");
    assert!(
        build_client_pack(&setup.fake, &setup.folder, "S", Some("0.15.11"), &out)
            .await
            .is_err()
    );

    set_marked(&setup.folder, "custom.jar", true).await.unwrap();
    let list = setup.list().await;
    let found = by_file(&list);
    assert_eq!(found["custom.jar"].0, ClientRequirement::Required);
    assert_eq!(
        found["helper.jar"],
        (ClientRequirement::Required, vec!["Custom".to_string()])
    );
    let marked: Vec<_> = list
        .mods
        .iter()
        .filter(|m| m.client_marked)
        .map(|m| m.file_name.as_str())
        .collect();
    assert_eq!(marked, ["custom.jar"]);
    assert_eq!(list.client_required_count, 2);
    // the marks file isn't a mod
    assert_eq!(list.mods.len(), 2);

    let plan = build_client_pack(&setup.fake, &setup.folder, "S", Some("0.15.11"), &out)
        .await
        .unwrap();
    let mut included: Vec<_> = plan.included.iter().map(|(n, _)| n.as_str()).collect();
    included.sort();
    assert_eq!(included, ["custom.jar", "helper.jar"]);
    let (_, _, entries) = read_pack(&out);
    assert!(entries.contains_key("overrides/mods/custom.jar"));
    assert!(!entries.keys().any(|k| k.contains(MARKS_FILE)));

    // disabling and enabling keep the mark; a disabled mod goes to nobody
    crate::mods::set_enabled(&setup.folder, "custom.jar", false)
        .await
        .unwrap();
    assert!(read_marks(&setup.folder).await.contains("custom.jar"));
    let list = setup.list().await;
    assert!(list.mods.iter().any(|m| m.client_marked && !m.enabled));
    crate::mods::set_enabled(&setup.folder, "custom.jar.disabled", true)
        .await
        .unwrap();
    assert_eq!(
        by_file(&setup.list().await)["custom.jar"].0,
        ClientRequirement::Required
    );

    // taking the mark off gives back what the jar says
    set_marked(&setup.folder, "custom.jar", false)
        .await
        .unwrap();
    assert_eq!(
        by_file(&setup.list().await)["custom.jar"].0,
        ClientRequirement::Unknown
    );
    assert!(
        !setup.folder.dir.join(MARKS_FILE).exists(),
        "no marks, no file"
    );

    // a deleted mod's mark goes with it
    set_marked(&setup.folder, "custom.jar", true).await.unwrap();
    crate::mods::delete(&setup.folder, "custom.jar")
        .await
        .unwrap();
    forget_mark(&setup.folder, "custom.jar").await.unwrap();
    assert!(read_marks(&setup.folder).await.is_empty());
}

#[tokio::test]
async fn marks_need_an_existing_mod_and_survive_damage() {
    let setup = Setup::new(InstalledModLoader::Fabric);
    let err = set_marked(&setup.folder, "missing.jar", true)
        .await
        .unwrap_err();
    assert!(matches!(err.kind, ErrorKind::NotFound));
    let err = set_marked(&setup.folder, "../x.jar", true)
        .await
        .unwrap_err();
    assert!(matches!(err.kind, ErrorKind::BadRequest));

    // a damaged file marks nothing, and marking again replaces it
    setup.add_jar("a.jar", r#"{"schemaVersion":1,"id":"a"}"#);
    std::fs::write(setup.folder.dir.join(MARKS_FILE), "not json").unwrap();
    assert!(read_marks(&setup.folder).await.is_empty());
    set_marked(&setup.folder, "a.jar", true).await.unwrap();
    assert!(read_marks(&setup.folder).await.contains("a.jar"));

    // plugins only run on the server
    let paper = Setup::new(InstalledModLoader::Paper);
    paper.add_matched(
        FakeVersion::new("p", "p1", "p.jar").loaders(&["paper"]),
        "required",
        "p.jar",
    );
    let err = set_marked(&paper.folder, "p.jar", true).await.unwrap_err();
    assert!(matches!(err.kind, ErrorKind::BadRequest));
}
