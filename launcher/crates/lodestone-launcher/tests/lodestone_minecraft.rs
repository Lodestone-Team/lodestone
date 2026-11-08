//! The launcher reads Minecraft and Modrinth data through `lodestone_minecraft`, the crate
//! it shares with Lodestone Core. These tests parse that crate's recorded API responses.

use lodestone_minecraft::modrinth::links::parse_modrinth_url;
use lodestone_minecraft::versions::fabric::{FabricLoaderEntry, select_fabric_loader};
use lodestone_minecraft::versions::mojang::{MojangVersionManifest, find_mojang_version};

const MANIFEST: &str =
    include_str!("../../../../crates/lodestone-minecraft/testdata/mojang_version_manifest.json");
const FABRIC_LOADERS: &str =
    include_str!("../../../../crates/lodestone-minecraft/testdata/fabric_loader_1.19.json");

#[test]
fn reads_the_mojang_version_manifest() {
    let manifest: MojangVersionManifest = serde_json::from_str(MANIFEST).unwrap();
    assert_eq!(manifest.versions[0].id, "26.4-snapshot-7");
    // Forge's '_' names find Mojang's '-' versions
    let version = find_mojang_version(&manifest.versions, "1.7.10_pre4").unwrap();
    assert_eq!(version.id, "1.7.10-pre4");
    assert!(find_mojang_version(&manifest.versions, "1.99.99").is_err());
}

#[test]
fn picks_the_newest_stable_fabric_loader() {
    let loaders: Vec<FabricLoaderEntry> = serde_json::from_str(FABRIC_LOADERS).unwrap();
    assert_eq!(select_fabric_loader(&loaders, "1.19").unwrap(), "0.20.1");
}

#[test]
fn parses_modrinth_links() {
    let link =
        parse_modrinth_url("https://modrinth.com/mod/sodium/version/mc1.20.1-0.5.3").unwrap();
    assert_eq!(link.project, "sodium");
    assert_eq!(link.version.as_deref(), Some("mc1.20.1-0.5.3"));
}
