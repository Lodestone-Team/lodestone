//! Which server a launcher instance was made for: `<instance>/lodestone_server.json`.
//! The pack's files themselves are recorded by the modpack import, in `modpack.json`
//! (see [`crate::modpack`]); the link names the core and server, and the ETag of the
//! client pack the instance has, so "keep mods in step" can ask the core whether it
//! changed.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::storage::{self, Document, Migration, StorageError};

/// The link file in an instance folder.
pub const LINK_FILE: &str = "lodestone_server.json";

/// Which server an instance was made for.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerLink {
    pub core_uuid: String,
    pub core_url: String,
    pub server_uuid: String,
    pub server_name: String,
    /// The ETag of the client pack the instance's mods came from.
    #[serde(default)]
    pub pack_etag: Option<String>,
}

impl Document for ServerLink {
    const VERSION: u32 = 1;
    /// Version 0 is a file without a `"version"` key; its fields carry over. Files of
    /// milestone 8a also list the pack's mods (`files`), which `modpack.json` records now;
    /// they are ignored.
    const MIGRATIONS: &'static [Migration] = &[Ok];
}

pub fn link_path(instance_dir: &Path) -> PathBuf {
    instance_dir.join(LINK_FILE)
}

/// The instance's server link, if it has a readable one.
pub fn load_link(instance_dir: &Path) -> Option<ServerLink> {
    let path = link_path(instance_dir);
    if !path.is_file() {
        return None;
    }
    storage::load::<ServerLink>(&path)
        .ok()
        .map(|loaded| loaded.document)
        .filter(|link| !link.core_uuid.is_empty() && !link.server_uuid.is_empty())
}

pub fn save_link(instance_dir: &Path, link: &ServerLink) -> Result<(), StorageError> {
    storage::save(&link_path(instance_dir), link)
}

/// Every instance under `instances_root` with a server link: `(instance id, link)`.
pub fn linked_instances(instances_root: &Path) -> Vec<(String, ServerLink)> {
    let Ok(entries) = fs::read_dir(instances_root) else {
        return Vec::new();
    };
    let mut linked: Vec<(String, ServerLink)> = entries
        .flatten()
        .filter_map(|entry| {
            let id = entry.file_name().to_str()?.to_string();
            if id.starts_with('.') {
                return None;
            }
            Some((id, load_link(&entry.path())?))
        })
        .collect();
    linked.sort_by(|a, b| a.0.cmp(&b.0));
    linked
}

/// Points the links made for the core `from` at `to`, at `core_url`: the same core under
/// the id it has now (a core makes a new id each time it starts), or the entry another
/// was merged into. Returns the instances changed; one that can't be saved keeps its
/// old link.
pub fn relink(instances_root: &Path, from: &str, to: &str, core_url: &str) -> Vec<String> {
    if from == to {
        return Vec::new();
    }
    linked_instances(instances_root)
        .into_iter()
        .filter(|(_, link)| link.core_uuid == from)
        .filter_map(|(id, mut link)| {
            link.core_uuid = to.to_string();
            link.core_url = core_url.to_string();
            save_link(&instances_root.join(&id), &link).ok()?;
            Some(id)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relink_moves_only_the_old_cores_links() {
        let dir = tempfile::tempdir().unwrap();
        let link = |core: &str, server: &str| ServerLink {
            core_uuid: core.into(),
            core_url: "http://localhost:16662".into(),
            server_uuid: server.into(),
            server_name: "SMP".into(),
            pack_etag: Some("\"e\"".into()),
        };
        for (id, core) in [("a", "old"), ("b", "other"), ("c", "old")] {
            let instance = dir.path().join(id);
            fs::create_dir_all(&instance).unwrap();
            save_link(&instance, &link(core, id)).unwrap();
        }
        assert_eq!(
            relink(dir.path(), "old", "new", "http://127.0.0.1:16662"),
            ["a", "c"]
        );
        let a = load_link(&dir.path().join("a")).unwrap();
        assert_eq!(a.core_uuid, "new");
        assert_eq!(a.core_url, "http://127.0.0.1:16662");
        // the rest of the link is kept
        assert_eq!(a.server_uuid, "a");
        assert_eq!(a.pack_etag.as_deref(), Some("\"e\""));
        assert_eq!(
            load_link(&dir.path().join("b")).unwrap(),
            link("other", "b")
        );
        assert!(relink(dir.path(), "old", "new", "u").is_empty());
        assert!(relink(dir.path(), "new", "new", "u").is_empty());
    }

    #[test]
    fn links_are_found_and_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let instance = dir.path().join("smp");
        fs::create_dir_all(&instance).unwrap();
        fs::create_dir_all(dir.path().join("plain")).unwrap();
        assert!(load_link(&instance).is_none());
        let link = ServerLink {
            core_uuid: "c".into(),
            core_url: "http://localhost:16662".into(),
            server_uuid: "s".into(),
            server_name: "SMP".into(),
            pack_etag: Some("\"e\"".into()),
        };
        save_link(&instance, &link).unwrap();
        assert_eq!(load_link(&instance), Some(link.clone()));
        assert_eq!(
            linked_instances(dir.path()),
            vec![("smp".to_string(), link)]
        );
        fs::write(link_path(&instance), "{broken").unwrap();
        assert!(linked_instances(dir.path()).is_empty());
    }

    #[test]
    fn reads_links_of_milestone_8a() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            link_path(dir.path()),
            r#"{"version":1,"core_uuid":"c","core_url":"u","server_uuid":"s","server_name":"SMP","pack_etag":"\"e\"","files":[{"name":"a.jar","sha1":"00"}]}"#,
        )
        .unwrap();
        let link = load_link(dir.path()).unwrap();
        assert_eq!(link.pack_etag.as_deref(), Some("\"e\""));
    }
}
