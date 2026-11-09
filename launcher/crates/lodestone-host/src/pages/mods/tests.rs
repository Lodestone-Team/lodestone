use launcher_core::cores::admin::mods::{
    InstalledModKind, InstalledModLoader, ModInstallResult, ModrinthVersion,
};

use super::{ModsPage, Target, project::listed_versions};

fn version(id: &str, game: &str, loader: &str) -> ModrinthVersion {
    serde_json::from_value(serde_json::json!({
        "id": id, "project_id": "p", "name": "", "version_number": id,
        "version_type": "release", "game_versions": [game], "loaders": [loader],
        "date_published": "2026-01-02T00:00:00Z", "downloads": 0, "files": [], "dependencies": [],
    }))
    .unwrap()
}

fn fabric() -> Target {
    Target {
        kind: InstalledModKind::Mod,
        loader: InstalledModLoader::Fabric,
        game_version: "1.21.1".into(),
    }
}

#[test]
fn lists_compatible_versions_and_the_linked_one_first() {
    let versions = [
        version("a", "1.21.1", "fabric"),
        version("b", "1.20.1", "fabric"),
        version("c", "1.21.1", "forge"),
    ];
    let ids = |list: Vec<ModrinthVersion>| list.into_iter().map(|v| v.id).collect::<Vec<_>>();
    assert_eq!(
        ids(listed_versions(&versions, None, "p", &fabric(), false)),
        ["a"]
    );
    assert_eq!(
        ids(listed_versions(&versions, None, "p", &fabric(), true)),
        ["a", "b", "c"]
    );
    // a linked version comes first, even one for something else
    let linked = version("b", "1.20.1", "fabric");
    assert_eq!(
        ids(listed_versions(
            &versions,
            Some(&linked),
            "p",
            &fabric(),
            false
        )),
        ["b", "a"]
    );
    // never another project's version
    let mut other = linked.clone();
    other.project_id = "q".into();
    assert_eq!(
        ids(listed_versions(
            &versions,
            Some(&other),
            "p",
            &fabric(),
            false
        )),
        ["a"]
    );
}

#[test]
fn the_install_toast_says_what_happened() {
    let result: ModInstallResult = serde_json::from_value(serde_json::json!({
        "installed": [{
            "file_name": "sodium.jar", "enabled": true, "size": 1, "sha512": "",
            "metadata": null,
            "modrinth": {"project_id": "p", "version_id": "v", "slug": "sodium", "title": "Sodium",
                "icon_url": null, "version_number": "0.6", "update": null,
                "client_side": "required", "server_side": "optional"},
            "client_requirement": "unknown", "required_by": [],
        }],
        "removed": ["old.jar"],
        "skipped": [{"project_id": "fabric-api", "reason": "already installed"}],
    }))
    .unwrap();
    let toast = ModsPage::result_toast("Installed Sodium 0.6".into(), &result);
    let text = toast.text.to_string();
    assert!(text.contains("Sodium 0.6"), "{text}");
    assert!(text.contains("old.jar"), "{text}");
    assert!(text.contains("fabric-api (already installed)"), "{text}");
    assert!(text.contains("Players will need Sodium too"), "{text}");
}
