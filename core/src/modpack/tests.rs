use std::io::Write;

use serde_json::json;

use lodestone_minecraft::mrpack::{PathCheck, INDEX_FILE};

use super::*;
use crate::modrinth::fake::{sha1_hex, sha512_hex, FakeModrinth};

/// A zip entry for [`make_pack`].
enum Entry<'a> {
    File(&'a str, &'a [u8]),
    Dir(&'a str),
    Link(&'a str, &'a str),
}

fn make_pack(path: &Path, index: &serde_json::Value, entries: &[Entry]) {
    let file = std::fs::File::create(path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    writer.start_file(INDEX_FILE, options).unwrap();
    writer
        .write_all(serde_json::to_string(index).unwrap().as_bytes())
        .unwrap();
    for entry in entries {
        match entry {
            Entry::File(name, contents) => {
                writer.start_file(*name, options).unwrap();
                writer.write_all(contents).unwrap();
            }
            Entry::Dir(name) => writer.add_directory(*name, options).unwrap(),
            Entry::Link(name, target) => writer.add_symlink(*name, *target, options).unwrap(),
        }
    }
    writer.finish().unwrap();
}

fn file_entry(path: &str, content: &[u8], server: Option<&str>, url: &str) -> serde_json::Value {
    let mut entry = json!({
        "path": path,
        "hashes": {"sha1": sha1_hex(content), "sha512": sha512_hex(content)},
        "downloads": [url],
        "fileSize": content.len(),
    });
    if let Some(server) = server {
        entry["env"] = json!({"client": "required", "server": server});
    }
    entry
}

fn index(files: Vec<serde_json::Value>, dependencies: serde_json::Value) -> serde_json::Value {
    json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1.2.3",
        "name": "Test Pack",
        "summary": "A pack for tests",
        "files": files,
        "dependencies": dependencies,
    })
}

fn fabric() -> serde_json::Value {
    json!({"minecraft": "1.20.1", "fabric-loader": "0.15.11"})
}

struct TestPack {
    /// Keeps the folder of the pack alive.
    _dir: tempfile::TempDir,
    path: PathBuf,
}

fn pack(index: serde_json::Value, entries: &[Entry]) -> TestPack {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pack.mrpack");
    make_pack(&path, &index, entries);
    TestPack { _dir: dir, path }
}

fn read(index: serde_json::Value, entries: &[Entry]) -> Result<Pack, Error> {
    let pack = pack(index, entries);
    read_pack(&pack.path, false)
}

#[test]
fn reads_a_fabric_pack() {
    let jar = b"jar bytes";
    let files = vec![
        file_entry(
            "mods/a.jar",
            jar,
            None,
            "https://cdn.modrinth.com/data/a/a.jar",
        ),
        file_entry(
            "mods/b.jar",
            b"bb",
            Some("optional"),
            "https://github.com/x/b.jar",
        ),
        file_entry(
            "mods/c.jar",
            b"ccc",
            Some("required"),
            "https://gitlab.com/c.jar",
        ),
        file_entry(
            "mods/client.jar",
            b"client only",
            Some("unsupported"),
            "https://example.com/never-checked.jar",
        ),
    ];
    let pack = read(index(files, fabric()), &[]).unwrap();
    let info = &pack.info;
    assert_eq!(info.name, "Test Pack");
    assert_eq!(info.version_number.as_deref(), Some("1.2.3"));
    assert_eq!(info.summary.as_deref(), Some("A pack for tests"));
    assert_eq!(info.game_version, "1.20.1");
    assert_eq!(info.loader, "fabric");
    assert_eq!(info.loader_version.as_deref(), Some("0.15.11"));
    assert_eq!(info.unsupported_reason, None);
    // the client-only file is left out
    assert_eq!(info.server_file_count, 3);
    assert_eq!(info.server_download_size, (jar.len() + 2 + 3) as u64);
    let paths: Vec<&Path> = pack.downloads.iter().map(|d| d.path.as_path()).collect();
    assert_eq!(
        paths,
        vec![
            Path::new("mods/a.jar"),
            Path::new("mods/b.jar"),
            Path::new("mods/c.jar")
        ]
    );
}

#[test]
fn reports_unsupported_packs() {
    let cases = [
        (json!({"minecraft": "1.20.1", "forge": "47.2.0"}), "forge"),
        (
            json!({"minecraft": "1.20.1", "neoforge": "20.4.1"}),
            "neoforge",
        ),
        (
            json!({"minecraft": "1.20.1", "quilt-loader": "0.20.0"}),
            "quilt-loader",
        ),
        (
            json!({"minecraft": "1.20.1", "fabric-loader": "0.15.0", "forge": "1"}),
            "forge",
        ),
    ];
    for (dependencies, needs) in cases {
        let pack = read(index(vec![], dependencies.clone()), &[]).unwrap();
        let reason = pack.info.unsupported_reason.unwrap();
        assert!(
            reason.starts_with("Only Fabric modpacks are supported"),
            "{reason}"
        );
        assert!(reason.contains(needs), "{dependencies}: {reason}");
    }
    let pack = read(index(vec![], json!({"minecraft": "1.20.1"})), &[]).unwrap();
    assert!(pack
        .info
        .unsupported_reason
        .unwrap()
        .contains("needs no mod loader"));

    let pack = read(index(vec![], json!({"fabric-loader": "0.15.0"})), &[]).unwrap();
    assert!(pack
        .info
        .unsupported_reason
        .unwrap()
        .contains("which Minecraft version"));

    let mut bad_format = index(vec![], fabric());
    bad_format["formatVersion"] = json!(2);
    let pack = read(bad_format, &[]).unwrap();
    assert!(pack
        .info
        .unsupported_reason
        .unwrap()
        .contains("format version 2"));

    let mut other_game = index(vec![], fabric());
    other_game["game"] = json!("terraria");
    let pack = read(other_game, &[]).unwrap();
    assert!(pack
        .info
        .unsupported_reason
        .unwrap()
        .contains("not Minecraft"));

    // versions go into URLs, so they must look like versions
    let pack = read(
        index(
            vec![],
            json!({"minecraft": "1.20.1/../x", "fabric-loader": "0.15.0"}),
        ),
        &[],
    )
    .unwrap();
    assert!(pack.info.unsupported_reason.is_some());
    let pack = read(
        index(
            vec![],
            json!({"minecraft": "1.20.1", "fabric-loader": "0.15?x=1"}),
        ),
        &[],
    )
    .unwrap();
    assert!(pack.info.unsupported_reason.is_some());
}

#[test]
fn rejects_invalid_packs() {
    let dir = tempfile::tempdir().unwrap();
    let not_zip = dir.path().join("x.mrpack");
    std::fs::write(&not_zip, "hello").unwrap();
    let err = read_pack(&not_zip, false).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::BadRequest));
    assert!(err
        .source
        .to_string()
        .starts_with("Not a valid Modrinth modpack"));

    let no_index = dir.path().join("y.mrpack");
    crate::mods::metadata::tests::make_zip(&no_index, &[("overrides/a.txt", "a")]);
    let err = read_pack(&no_index, false).unwrap_err();
    assert!(err.source.to_string().contains("modrinth.index.json"));

    let bad_json = dir.path().join("z.mrpack");
    crate::mods::metadata::tests::make_zip(&bad_json, &[(INDEX_FILE, "{not json")]);
    assert!(read_pack(&bad_json, false).is_err());
}

#[test]
fn rejects_files_from_other_hosts_or_without_hashes() {
    for url in [
        "https://example.com/a.jar",
        "http://cdn.modrinth.com/a.jar",
        "https://cdn.modrinth.com.evil.com/a.jar",
        "https://cdn.modrinth.com:444/a.jar",
        "file:///etc/passwd",
    ] {
        let files = vec![file_entry("mods/a.jar", b"a", None, url)];
        let err = read(index(files, fabric()), &[]).unwrap_err();
        assert!(
            err.source.to_string().contains("allowed host"),
            "{url}: {}",
            err.source
        );
    }
    let mut no_sha1 = file_entry("mods/a.jar", b"a", None, "https://cdn.modrinth.com/a.jar");
    no_sha1["hashes"].as_object_mut().unwrap().remove("sha1");
    assert!(read(index(vec![no_sha1], fabric()), &[]).is_err());

    let mut huge = file_entry("mods/a.jar", b"a", None, "https://cdn.modrinth.com/a.jar");
    huge["fileSize"] = json!(MAX_PACK_FILE_BYTES + 1);
    let err = read(index(vec![huge], fabric()), &[]).unwrap_err();
    assert!(err.source.to_string().contains("larger than"));
}

fn check_pack_path(
    path: &str,
    is_dir: bool,
    allow_scripts: bool,
) -> Result<PathCheck, lodestone_minecraft::Error> {
    mrpack::check_pack_path(path, is_dir, &InstancePathPolicy { allow_scripts })
}

#[test]
fn path_safety_skips() {
    let skipped = |path: &str, is_dir: bool, allow_scripts: bool| {
        matches!(
            check_pack_path(path, is_dir, allow_scripts).unwrap(),
            PathCheck::Skip(_)
        )
    };
    for path in [
        ".lodestone_config",
        ".lodestone_minecraft_config.json",
        "config/.lodestone_x",
        ".Lodestone/anything",
        "eula.txt",
        "EULA.TXT",
        "server.jar",
        "user_jvm_args.txt",
    ] {
        assert!(skipped(path, false, true), "{path}");
    }
    // scripts need the owner with code execution on
    for path in [
        "kubejs/server_scripts/main.js",
        "config/x.ts",
        "prelaunch.ts",
        "a.LUA",
    ] {
        assert!(skipped(path, false, false), "{path}");
        assert!(!skipped(path, false, true), "{path}");
    }
    // a folder named like a script is a folder
    assert!(!skipped("config/x.ts/", true, false));
    for path in [
        "config/eula.txt",
        "mods/a.jar",
        "world/macros/x",
        "macros/data.json",
        "config/sodium.json",
    ] {
        assert_eq!(
            check_pack_path(path, false, false).unwrap(),
            PathCheck::Write(PathBuf::from(path)),
            "{path}"
        );
    }
}

#[test]
fn too_many_entries_are_rejected() {
    let entries: Vec<String> = (0..MAX_ENTRIES)
        .map(|i| format!("overrides/f{i}"))
        .collect();
    let refs: Vec<Entry> = entries.iter().map(|n| Entry::File(n, b"")).collect();
    // the index makes one entry more than the limit
    let err = read(index(vec![], fabric()), &refs).unwrap_err();
    assert!(err.source.to_string().contains("more than 20000 files"));
}

#[test]
fn unsafe_override_paths_reject_the_pack() {
    for name in [
        "overrides/../x.txt",
        "server-overrides/a\\b.txt",
        "overrides/C:x",
    ] {
        let err = read(index(vec![], fabric()), &[Entry::File(name, b"x")]).unwrap_err();
        assert!(
            err.source.to_string().starts_with("The modpack is unsafe"),
            "{name}"
        );
    }
    // client overrides are never used, so they are not checked
    assert!(read(
        index(vec![], fabric()),
        &[Entry::File("client-overrides/../x.txt", b"x")]
    )
    .is_ok());
}

#[tokio::test]
async fn installs_files_and_overrides() {
    let jar = b"mod jar".to_vec();
    let lib = b"library".to_vec();
    let files = vec![
        file_entry("mods/a.jar", &jar, None, "https://cdn.modrinth.com/a.jar"),
        {
            // the first URL fails, the second works
            let mut entry = file_entry(
                "mods/lib.jar",
                &lib,
                Some("required"),
                "https://github.com/missing.jar",
            );
            entry["downloads"]
                .as_array_mut()
                .unwrap()
                .push(json!("https://raw.githubusercontent.com/lib.jar"));
            entry
        },
        file_entry(
            "mods/client.jar",
            b"c",
            Some("unsupported"),
            "https://cdn.modrinth.com/c.jar",
        ),
        file_entry(
            "macros/evil.ts",
            b"evil",
            None,
            "https://cdn.modrinth.com/evil.ts",
        ),
    ];
    let p = pack(
        index(files, fabric()),
        &[
            Entry::Dir("overrides/config/"),
            Entry::File("overrides/config/a.json", b"from overrides"),
            Entry::File("overrides/config/b.json", b"only in overrides"),
            Entry::File("server-overrides/config/a.json", b"from server-overrides"),
            Entry::File("client-overrides/config/a.json", b"from client-overrides"),
            Entry::File("client-overrides/options.txt", b"client"),
            Entry::File(
                "overrides/server.properties",
                b"motd=Pack\nserver-port=1234\n",
            ),
            Entry::File("overrides/eula.txt", b"eula=true"),
            Entry::File("overrides/.lodestone_config", b"{}"),
            Entry::File("overrides/kubejs/server_scripts/x.js", b"code"),
            Entry::Dir("overrides/world/"),
            Entry::Link("overrides/link", "/etc"),
        ],
    );
    let instance = tempfile::tempdir().unwrap();
    let root = instance.path();
    std::fs::write(root.join("eula.txt"), "#generated by Lodestone\neula=true").unwrap();

    let fake = FakeModrinth::default();
    fake.add_file("https://cdn.modrinth.com/a.jar", jar.clone());
    fake.add_file("https://raw.githubusercontent.com/lib.jar", lib.clone());

    let parsed = read_pack(&p.path, false).unwrap();
    let report = install_pack_contents(&fake, &parsed, &p.path, root, false, &|_, _| {})
        .await
        .unwrap();
    assert_eq!(report.downloaded, 2);
    assert_eq!(std::fs::read(root.join("mods/a.jar")).unwrap(), jar);
    assert_eq!(std::fs::read(root.join("mods/lib.jar")).unwrap(), lib);
    assert!(!root.join("mods/client.jar").exists());
    assert!(!root.join("macros").exists());

    let read = |p: &str| std::fs::read_to_string(root.join(p)).unwrap();
    assert_eq!(read("config/a.json"), "from server-overrides");
    assert_eq!(read("config/b.json"), "only in overrides");
    assert!(!root.join("options.txt").exists());
    assert!(root.join("world").is_dir());
    assert!(!root.join("link").exists());
    assert!(!root.join(".lodestone_config").exists());
    assert!(!root.join("kubejs/server_scripts/x.js").exists());
    assert_eq!(read("eula.txt"), "#generated by Lodestone\neula=true");

    let skipped: Vec<&str> = report.skipped.iter().map(|s| s.path.as_str()).collect();
    for path in [
        "macros/evil.ts",
        "eula.txt",
        ".lodestone_config",
        "kubejs/server_scripts/x.js",
        "link",
    ] {
        assert!(skipped.contains(&path), "{path} not in {skipped:?}");
    }
    let message = describe_skipped(&report.skipped).unwrap();
    assert!(
        message.starts_with("5 modpack file(s) were skipped"),
        "{message}"
    );

    // the chosen port wins over the one from the overrides
    write_port(root, 25570).await.unwrap();
    let properties = read("server.properties");
    assert!(properties.contains("server-port=25570"));
    assert!(properties.contains("motd=Pack"));
    assert!(!properties.contains("1234"));

    // no temporary files are left behind
    for entry in walkdir::WalkDir::new(root) {
        let name = entry.unwrap().file_name().to_string_lossy().to_string();
        assert!(!name.ends_with(".tmp"), "{name}");
    }
}

#[tokio::test]
async fn owner_scripts_are_installed() {
    let p = pack(
        index(vec![], fabric()),
        &[Entry::File("overrides/kubejs/server_scripts/x.js", b"code")],
    );
    let instance = tempfile::tempdir().unwrap();
    let parsed = read_pack(&p.path, true).unwrap();
    assert!(parsed.skipped.is_empty());
    let fake = FakeModrinth::default();
    install_pack_contents(&fake, &parsed, &p.path, instance.path(), true, &|_, _| {})
        .await
        .unwrap();
    assert!(instance.path().join("kubejs/server_scripts/x.js").exists());
}

#[tokio::test]
async fn damaged_downloads_fail_the_install() {
    let files = vec![file_entry(
        "mods/a.jar",
        b"expected",
        None,
        "https://cdn.modrinth.com/a.jar",
    )];
    let p = pack(index(files, fabric()), &[]);
    let instance = tempfile::tempdir().unwrap();
    let fake = FakeModrinth::default();
    fake.add_file("https://cdn.modrinth.com/a.jar", b"something else".to_vec());
    let parsed = read_pack(&p.path, false).unwrap();
    let err = install_pack_contents(&fake, &parsed, &p.path, instance.path(), false, &|_, _| {})
        .await
        .unwrap_err();
    assert!(err.source.to_string().contains("damaged"), "{}", err.source);
    assert!(!instance.path().join("mods/a.jar").exists());
    let leftovers: Vec<_> = std::fs::read_dir(instance.path().join("mods"))
        .unwrap()
        .collect();
    assert!(leftovers.is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn does_not_write_through_links_in_the_instance() {
    let p = pack(
        index(vec![], fabric()),
        &[Entry::File("overrides/config/x.txt", b"x")],
    );
    let instance = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), instance.path().join("config")).unwrap();
    let parsed = read_pack(&p.path, false).unwrap();
    let fake = FakeModrinth::default();
    let err = install_pack_contents(&fake, &parsed, &p.path, instance.path(), false, &|_, _| {})
        .await
        .unwrap_err();
    assert!(err.source.to_string().contains("leaves the server folder"));
    assert!(!outside.path().join("x.txt").exists());
}

#[test]
fn skipped_message_is_short() {
    let skipped: Vec<SkippedPath> = (0..15)
        .map(|i| SkippedPath {
            path: format!("f{i}"),
            reason: "r".into(),
        })
        .collect();
    let message = describe_skipped(&skipped).unwrap();
    assert!(message.contains("and 5 more"));
    assert_eq!(describe_skipped(&[]), None);
}

#[tokio::test]
async fn origin_from_a_modrinth_pack_keeps_cdn_icons_only() {
    use crate::modrinth::fake::FakeVersion;
    let fake = FakeModrinth::default();
    fake.add(FakeVersion::new("pack", "pv1", "pack.mrpack"));
    fake.set_icon_url("pack", "https://cdn.modrinth.com/data/pack/icon.png");
    let origin = ModpackOrigin::from_modrinth(&fake, "pv1").await.unwrap();
    assert_eq!(
        origin,
        ModpackOrigin {
            project_id: "pack".into(),
            version_id: "pv1".into(),
            name: "Title of pack".into(),
            version_number: Some("pv1-number".into()),
            icon_url: Some("https://cdn.modrinth.com/data/pack/icon.png".into()),
        }
    );

    for icon in [
        "https://example.com/icon.png",
        "http://cdn.modrinth.com/data/pack/icon.png",
        "https://cdn.modrinth.com.evil.com/icon.png",
        "https://cdn.modrinth.com:8443/icon.png",
        "https://user@cdn.modrinth.com/icon.png",
        "javascript:alert(1)",
        "",
    ] {
        fake.set_icon_url("pack", icon);
        let origin = ModpackOrigin::from_modrinth(&fake, "pv1").await.unwrap();
        assert_eq!(origin.icon_url, None, "{icon}");
    }
    assert!(ModpackOrigin::from_modrinth(&fake, "missing")
        .await
        .is_err());
}

#[test]
fn origin_from_an_uploaded_pack() {
    let p = pack(index(vec![], fabric()), &[]);
    let info = read_pack(&p.path, false).unwrap().info;
    let origin = ModpackOrigin::from_upload(&info);
    assert_eq!(origin.project_id, "");
    assert_eq!(origin.version_id, "");
    assert_eq!(origin.name, "Test Pack");
    assert_eq!(origin.version_number.as_deref(), Some("1.2.3"));
    assert_eq!(origin.icon_url, None);
}
