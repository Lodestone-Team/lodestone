//! Which instance folder owns an instance UUID.
//!
//! Every instance folder has a `.lodestone_config` with the instance's UUID, and
//! permissions and other settings refer to instances by that UUID. Copying a folder
//! copies its UUID, so two folders can claim the same instance. The core records the
//! folder each UUID was last loaded or created in (`stores/instance_folders.json`), and
//! that folder keeps the UUID: every other folder with it gets a new UUID, and with it
//! nothing the original was granted. When the record names none of the folders, none of
//! them is loaded, because nothing on disk proves which one is the original.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::path::{Component, Path, PathBuf};

use color_eyre::eyre::Context;
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use crate::{error::Error, types::DotLodestoneConfig, types::InstanceUuid};

/// The folder (its name in the instances folder) each instance UUID was last loaded or
/// created in.
pub(crate) struct InstanceFolders {
    path: PathBuf,
    folders: tokio::sync::Mutex<HashMap<InstanceUuid, String>>,
}

#[derive(Default, Serialize, Deserialize)]
struct StoredFolders {
    folders: BTreeMap<String, String>,
}

impl InstanceFolders {
    /// Loads the record from `path`. A missing, unreadable or corrupt file is an empty
    /// record: instances still load, but folders sharing a UUID do not.
    pub(crate) async fn load(path: PathBuf) -> Self {
        let folders = match tokio::fs::read(&path).await {
            Ok(data) => match serde_json::from_slice::<StoredFolders>(&data) {
                Ok(stored) => stored
                    .folders
                    .into_iter()
                    .filter(|(_, name)| is_folder_name(name))
                    .map(|(uuid, name)| (InstanceUuid::from(uuid), name))
                    .collect(),
                Err(e) => {
                    error!(
                        "{} is corrupt ({e}); folders that share an instance UUID will not be loaded",
                        path.display()
                    );
                    HashMap::new()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(e) => {
                error!(
                    "Failed to read {} ({e}); folders that share an instance UUID will not be loaded",
                    path.display()
                );
                HashMap::new()
            }
        };
        Self {
            path,
            folders: tokio::sync::Mutex::new(folders),
        }
    }

    pub(crate) async fn snapshot(&self) -> HashMap<InstanceUuid, String> {
        self.folders.lock().await.clone()
    }

    /// Records that each UUID was loaded or created in its folder. The record in memory
    /// only changes once the file is saved.
    pub(crate) async fn record(
        &self,
        loaded: impl IntoIterator<Item = (InstanceUuid, PathBuf)>,
    ) -> Result<(), Error> {
        let mut folders = self.folders.lock().await;
        let mut updated = folders.clone();
        for (uuid, path) in loaded {
            match path.file_name().and_then(|name| name.to_str()) {
                Some(name) if is_folder_name(name) => {
                    updated.insert(uuid, name.to_string());
                }
                _ => warn!(
                    "Not recording the folder of instance {uuid}: {} has no usable folder name",
                    path.display()
                ),
            }
        }
        if updated != *folders {
            self.save(&updated).await?;
            *folders = updated;
        }
        Ok(())
    }

    /// Forgets the folder of a deleted instance.
    pub(crate) async fn forget(&self, uuid: &InstanceUuid) -> Result<(), Error> {
        let mut folders = self.folders.lock().await;
        if folders.contains_key(uuid) {
            let mut updated = folders.clone();
            updated.remove(uuid);
            self.save(&updated).await?;
            *folders = updated;
        }
        Ok(())
    }

    async fn save(&self, folders: &HashMap<InstanceUuid, String>) -> Result<(), Error> {
        let stored = StoredFolders {
            folders: folders
                .iter()
                .map(|(uuid, name)| (uuid.to_string(), name.clone()))
                .collect(),
        };
        let data = serde_json::to_vec_pretty(&stored)
            .context("Failed to serialize the instance folders")?;
        crate::persist::write_atomic(&self.path, data, None)
            .await
            .context(format!("Failed to write {}", self.path.display()))?;
        Ok(())
    }
}

/// A single plain folder name, so a record can only name a folder directly in the
/// instances folder.
fn is_folder_name(name: &str) -> bool {
    let mut components = Path::new(name).components();
    matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(only)), None) if only == name
    )
}

/// Folders that share a UUID, none of which was loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Conflict {
    pub uuid: InstanceUuid,
    pub paths: Vec<PathBuf>,
    /// The folder the core recorded for the UUID, which is none of `paths`.
    pub recorded: Option<String>,
}

impl fmt::Display for Conflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let paths: Vec<_> = self
            .paths
            .iter()
            .map(|path| path.display().to_string())
            .collect();
        write!(
            f,
            "Instance UUID {} is used by {} folders: {}. None of them was loaded, because ",
            self.uuid,
            paths.len(),
            paths.join(", ")
        )?;
        match &self.recorded {
            Some(name) => write!(
                f,
                "the folder Lodestone recorded for this UUID ({name}) is not one of them"
            )?,
            None => write!(f, "Lodestone has no record of which folder owns this UUID")?,
        }
        write!(
            f,
            ". Remove every copy but one from the instances folder, then restart Lodestone Core"
        )
    }
}

/// The instances to restore, and the folders left out because they share a UUID.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Loaded {
    pub ready: Vec<(PathBuf, DotLodestoneConfig)>,
    pub conflicts: Vec<Conflict>,
}

struct Candidate {
    path: PathBuf,
    canonical: PathBuf,
    config: DotLodestoneConfig,
}

/// Reads every instance's `.lodestone_config` in `instances_path`, and resolves folders
/// that share a UUID with `recorded` (see the module documentation). Paths that resolve
/// to the same folder (links, junctions) are one instance.
pub(crate) async fn load_unique_configs(
    instances_path: &Path,
    recorded: &HashMap<InstanceUuid, String>,
) -> Result<Loaded, Error> {
    let mut configs = Vec::new();
    let mut entries = tokio::fs::read_dir(instances_path)
        .await
        .context("Failed to read instances directory")?;
    while let Some(entry) = entries
        .next_entry()
        .await
        .context("Failed to read instances directory")?
    {
        let path = entry.path();
        let config = async {
            let bytes = tokio::fs::read(path.join(".lodestone_config"))
                .await
                .context("failed to read .lodestone_config")?;
            Ok::<DotLodestoneConfig, Error>(
                serde_json::from_slice(&bytes).context("failed to parse .lodestone_config")?,
            )
        }
        .await;
        match config {
            Ok(config) => configs.push((path, config)),
            Err(error) => error!(
                "Skipping instance {}: cannot read config: {error}",
                path.display()
            ),
        }
    }
    configs.sort_by(|(a, _), (b, _)| a.cmp(b));
    let mut directories = HashSet::new();
    let mut candidates = Vec::new();
    for (path, config) in configs {
        match tokio::fs::canonicalize(&path).await {
            Ok(canonical) => {
                if directories.insert(canonical.clone()) {
                    candidates.push(Candidate {
                        path,
                        canonical,
                        config,
                    });
                } else {
                    warn!("Skipping duplicate path to instance {}", path.display());
                }
            }
            Err(error) => error!(
                "Skipping instance {}: cannot resolve path: {error}",
                path.display()
            ),
        }
    }
    resolve(instances_path, candidates, recorded).await
}

async fn resolve(
    instances_path: &Path,
    candidates: Vec<Candidate>,
    recorded: &HashMap<InstanceUuid, String>,
) -> Result<Loaded, Error> {
    // Every on-disk identity is taken, including those of folders that are not loaded.
    let mut reserved: HashSet<_> = candidates.iter().map(|c| c.config.uuid().clone()).collect();
    let mut groups: Vec<Vec<Candidate>> = Vec::new();
    let mut group_of: HashMap<InstanceUuid, usize> = HashMap::new();
    for candidate in candidates {
        match group_of.get(candidate.config.uuid()) {
            Some(&i) => groups[i].push(candidate),
            None => {
                group_of.insert(candidate.config.uuid().clone(), groups.len());
                groups.push(vec![candidate]);
            }
        }
    }

    let mut loaded = Loaded::default();
    for mut group in groups {
        if group.len() == 1 {
            let only = group.remove(0);
            loaded.ready.push((only.path, only.config));
            continue;
        }
        let uuid = group[0].config.uuid().clone();
        let recorded_name = recorded.get(&uuid).filter(|name| is_folder_name(name));
        let owner = match recorded_name {
            Some(name) => match tokio::fs::canonicalize(instances_path.join(name)).await {
                Ok(canonical) => group.iter().position(|c| c.canonical == canonical),
                Err(_) => None,
            },
            None => None,
        };
        let Some(owner) = owner else {
            loaded.conflicts.push(Conflict {
                uuid,
                paths: group.into_iter().map(|c| c.path).collect(),
                recorded: recorded_name.cloned(),
            });
            continue;
        };
        let owner = group.remove(owner);
        info!(
            "Instance UUID {uuid} is used by {} folders; {} keeps it, as the folder Lodestone recorded for it",
            group.len() + 1,
            owner.path.display()
        );
        loaded.ready.push((owner.path, owner.config));
        for copy in group {
            let Candidate {
                path, mut config, ..
            } = copy;
            loop {
                config.regenerate_uuid();
                if reserved.insert(config.uuid().clone()) {
                    break;
                }
            }
            let bytes = serde_json::to_vec_pretty(&config)
                .context("Failed to serialize .lodestone_config")?;
            if let Err(error) =
                crate::persist::write_atomic(path.join(".lodestone_config"), bytes, None).await
            {
                error!(
                    "Skipping instance {} (a copy of instance {uuid}): failed to save its new UUID: {error}",
                    path.display()
                );
                continue;
            }
            warn!(
                "Instance {} is a copy of instance {uuid}: it now has the new UUID {}, with none of the original's permissions",
                path.display(),
                config.uuid()
            );
            loaded.ready.push((path, config));
        }
    }
    loaded.ready.sort_by(|(a, _), (b, _)| a.cmp(b));
    Ok(loaded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::permission::UserPermission;
    use crate::auth::user::{User, UserAction};
    use crate::traits::t_configurable::GameType;

    fn config(uuid: &InstanceUuid, creation_time: i64) -> DotLodestoneConfig {
        let mut value = serde_json::to_value(DotLodestoneConfig::new(
            uuid.clone(),
            GameType::MinecraftJava,
        ))
        .unwrap();
        value["creation_time"] = creation_time.into();
        serde_json::from_value(value).unwrap()
    }

    fn save(path: &Path, config: &DotLodestoneConfig) {
        std::fs::create_dir_all(path).unwrap();
        std::fs::write(
            path.join(".lodestone_config"),
            serde_json::to_vec(config).unwrap(),
        )
        .unwrap();
    }

    fn on_disk(path: &Path) -> DotLodestoneConfig {
        serde_json::from_slice(&std::fs::read(path.join(".lodestone_config")).unwrap()).unwrap()
    }

    fn record(uuid: &InstanceUuid, name: &str) -> HashMap<InstanceUuid, String> {
        HashMap::from([(uuid.clone(), name.to_string())])
    }

    #[cfg(windows)]
    fn junction(link: &Path, target: &Path) {
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
    }

    /// A copy made with its creation time, listed before the original: the recorded
    /// folder keeps the UUID, the copy gets a new one saved with its other metadata, and
    /// the next start changes nothing.
    #[tokio::test]
    async fn recorded_folder_keeps_uuid_of_identical_copy() {
        let dir = tempfile::tempdir().unwrap();
        let uuid = InstanceUuid::default();
        let original = dir.path().join("server-b");
        let copy = dir.path().join("server-a");
        let config = config(&uuid, 10);
        save(&original, &config);
        save(&copy, &config);
        let recorded = record(&uuid, "server-b");

        let first = load_unique_configs(dir.path(), &recorded).await.unwrap();
        assert!(first.conflicts.is_empty());
        assert_eq!(first.ready.len(), 2);
        assert!(first.ready.contains(&(original.clone(), config.clone())));
        let (_, renamed) = first.ready.iter().find(|(p, _)| p == &copy).unwrap();
        assert_ne!(renamed.uuid(), &uuid);
        let mut expected = serde_json::to_value(&config).unwrap();
        expected["uuid"] = serde_json::to_value(renamed.uuid()).unwrap();
        assert_eq!(serde_json::to_value(renamed).unwrap(), expected);
        assert_eq!(&on_disk(&copy), renamed);
        assert_eq!(on_disk(&original), config);

        let second = load_unique_configs(dir.path(), &recorded).await.unwrap();
        assert_eq!(second, first);
    }

    #[tokio::test]
    async fn copies_get_uuids_of_their_own() {
        let dir = tempfile::tempdir().unwrap();
        let uuid = InstanceUuid::default();
        let config = config(&uuid, 10);
        for name in ["a", "b", "c"] {
            save(&dir.path().join(name), &config);
        }
        let loaded = load_unique_configs(dir.path(), &record(&uuid, "c"))
            .await
            .unwrap();
        assert_eq!(loaded.ready.len(), 3);
        let ids: HashSet<_> = loaded.ready.iter().map(|(_, c)| c.uuid()).collect();
        assert_eq!(ids.len(), 3);
        assert_eq!(on_disk(&dir.path().join("c")).uuid(), &uuid);
    }

    #[tokio::test]
    async fn no_record_loads_none_of_the_copies() {
        let dir = tempfile::tempdir().unwrap();
        let uuid = InstanceUuid::default();
        let other = InstanceUuid::default();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        save(&a, &config(&uuid, 10));
        save(&b, &config(&uuid, 20));
        save(&dir.path().join("c"), &config(&other, 10));
        // a record of another UUID says nothing about this one
        let loaded = load_unique_configs(dir.path(), &record(&other, "a"))
            .await
            .unwrap();
        assert_eq!(
            loaded.ready,
            vec![(dir.path().join("c"), config(&other, 10))]
        );
        assert_eq!(
            loaded.conflicts,
            vec![Conflict {
                uuid: uuid.clone(),
                paths: vec![a.clone(), b.clone()],
                recorded: None,
            }]
        );
        let message = loaded.conflicts[0].to_string();
        assert!(message.contains(&uuid.to_string()), "{message}");
        assert!(message.contains(&a.display().to_string()), "{message}");
        assert!(message.contains(&b.display().to_string()), "{message}");
        assert!(message.contains("no record"), "{message}");
        // nothing on disk changes
        assert_eq!(on_disk(&a), config(&uuid, 10));
        assert_eq!(on_disk(&b), config(&uuid, 20));
    }

    #[tokio::test]
    async fn recorded_folder_outside_the_copies_loads_none_of_them() {
        let dir = tempfile::tempdir().unwrap();
        let uuid = InstanceUuid::default();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        save(&a, &config(&uuid, 10));
        save(&b, &config(&uuid, 10));
        // a folder that is gone, an existing folder without the UUID, paths out of or
        // through the instances folder
        std::fs::create_dir_all(dir.path().join("unrelated")).unwrap();
        for name in ["gone", "unrelated", "../a", "a/..", "..", "./a"] {
            let loaded = load_unique_configs(dir.path(), &record(&uuid, name))
                .await
                .unwrap();
            assert!(loaded.ready.is_empty(), "{name}");
            assert_eq!(loaded.conflicts.len(), 1, "{name}");
            assert_eq!(loaded.conflicts[0].paths, vec![a.clone(), b.clone()]);
            assert_eq!(on_disk(&a).uuid(), &uuid);
            assert_eq!(on_disk(&b).uuid(), &uuid);
        }
        let loaded = load_unique_configs(dir.path(), &record(&uuid, "gone"))
            .await
            .unwrap();
        let message = loaded.conflicts[0].to_string();
        assert!(message.contains("(gone) is not one of them"), "{message}");
    }

    /// The copy's new UUID is in none of the permissions granted on the original.
    #[tokio::test]
    async fn copy_gets_no_permissions_of_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let uuid = InstanceUuid::default();
        let config = config(&uuid, 10);
        save(&dir.path().join("original"), &config);
        save(&dir.path().join("copy"), &config);
        let mut granted = UserPermission::default();
        granted.can_view_instance.insert(uuid.clone());
        granted.can_start_instance.insert(uuid.clone());
        granted.can_stop_instance.insert(uuid.clone());
        granted.can_access_instance_console.insert(uuid.clone());
        granted.can_access_instance_setting.insert(uuid.clone());
        granted.can_read_instance_resource.insert(uuid.clone());
        granted.can_write_instance_resource.insert(uuid.clone());
        granted.can_read_instance_file.insert(uuid.clone());
        granted.can_write_instance_file.insert(uuid.clone());
        let user = User::new("member".to_string(), "pw", false, false, granted);

        let loaded = load_unique_configs(dir.path(), &record(&uuid, "original"))
            .await
            .unwrap();
        let (_, copy) = loaded
            .ready
            .iter()
            .find(|(p, _)| p.ends_with("copy"))
            .unwrap();
        let actions = |id: &InstanceUuid| {
            vec![
                UserAction::ViewInstance(id.clone()),
                UserAction::StartInstance(id.clone()),
                UserAction::StopInstance(id.clone()),
                UserAction::AccessConsole(id.clone()),
                UserAction::AccessSetting(id.clone()),
                UserAction::ReadResource(id.clone()),
                UserAction::WriteResource(id.clone()),
                UserAction::ReadInstanceFile(id.clone()),
                UserAction::WriteInstanceFile(id.clone()),
            ]
        };
        for (i, action) in actions(&uuid).iter().enumerate() {
            assert!(user.can_perform_action(action), "action {i}");
        }
        for (i, action) in actions(copy.uuid()).iter().enumerate() {
            assert!(!user.can_perform_action(action), "action {i}");
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn directory_junction_does_not_regenerate_same_instance() {
        let dir = tempfile::tempdir().unwrap();
        let uuid = InstanceUuid::default();
        let real = dir.path().join("a");
        let alias = dir.path().join("b");
        save(&real, &config(&uuid, 10));
        junction(&alias, &real);
        let loaded = load_unique_configs(dir.path(), &HashMap::new())
            .await
            .unwrap();
        assert!(loaded.conflicts.is_empty());
        assert_eq!(loaded.ready.len(), 1);
        assert_eq!(loaded.ready[0].1.uuid(), &uuid);
        assert_eq!(on_disk(&real).uuid(), &uuid);
    }

    /// A junction counts only as the folder it resolves to: a junction to the recorded
    /// folder is that folder, not a copy, and a recorded junction to a folder that is not
    /// among the copies matches none of them.
    #[cfg(windows)]
    #[tokio::test]
    async fn junction_counts_only_as_the_folder_it_resolves_to() {
        let dir = tempfile::tempdir().unwrap();
        let uuid = InstanceUuid::default();
        let original = dir.path().join("b-original");
        let copy = dir.path().join("c-copy");
        let alias = dir.path().join("a-alias");
        save(&original, &config(&uuid, 10));
        save(&copy, &config(&uuid, 10));
        junction(&alias, &original);
        let loaded = load_unique_configs(dir.path(), &record(&uuid, "b-original"))
            .await
            .unwrap();
        assert!(loaded.conflicts.is_empty());
        assert_eq!(loaded.ready.len(), 2);
        assert_eq!(loaded.ready[0].1.uuid(), &uuid);
        assert_eq!(on_disk(&original).uuid(), &uuid);
        assert_ne!(on_disk(&copy).uuid(), &uuid);

        save(&copy, &config(&uuid, 10));
        let elsewhere = tempfile::tempdir().unwrap();
        let outside = elsewhere.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::remove_dir(&alias).unwrap();
        junction(&alias, &outside);
        let loaded = load_unique_configs(dir.path(), &record(&uuid, "a-alias"))
            .await
            .unwrap();
        assert!(loaded.ready.is_empty());
        assert_eq!(loaded.conflicts[0].paths, vec![original, copy]);
    }

    #[tokio::test]
    async fn failed_atomic_write_skips_unpersisted_identity() {
        let dir = tempfile::tempdir().unwrap();
        let uuid = InstanceUuid::default();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        let config = config(&uuid, 10);
        save(&a, &config);
        // A directory cannot be atomically replaced by a file, on Windows or Unix.
        std::fs::create_dir_all(b.join(".lodestone_config")).unwrap();
        let candidates = vec![
            Candidate {
                path: a.clone(),
                canonical: std::fs::canonicalize(&a).unwrap(),
                config: config.clone(),
            },
            Candidate {
                path: b.clone(),
                canonical: std::fs::canonicalize(&b).unwrap(),
                config: config.clone(),
            },
        ];
        let loaded = resolve(dir.path(), candidates, &record(&uuid, "a"))
            .await
            .unwrap();
        assert_eq!(loaded.ready, vec![(a.clone(), config.clone())]);
        assert!(loaded.conflicts.is_empty());
        assert!(b.join(".lodestone_config").is_dir());
    }

    #[tokio::test]
    async fn record_is_saved_and_reloaded() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("instance_folders.json");
        let instances = dir.path().join("instances");
        let a = InstanceUuid::default();
        let b = InstanceUuid::default();
        let folders = InstanceFolders::load(file.clone()).await;
        assert!(folders.snapshot().await.is_empty());
        folders
            .record([
                (a.clone(), instances.join("a")),
                (b.clone(), instances.join("b")),
            ])
            .await
            .unwrap();
        // moved while the core was stopped
        folders
            .record([(a.clone(), instances.join("moved"))])
            .await
            .unwrap();
        folders.forget(&b).await.unwrap();
        let reloaded = InstanceFolders::load(file.clone()).await;
        assert_eq!(reloaded.snapshot().await, record(&a, "moved"));

        // entries that are not plain folder names are dropped
        std::fs::write(
            &file,
            serde_json::to_vec(&serde_json::json!({
                "folders": { a.to_string(): "../x", b.to_string(): "b" }
            }))
            .unwrap(),
        )
        .unwrap();
        let reloaded = InstanceFolders::load(file.clone()).await;
        assert_eq!(reloaded.snapshot().await, record(&b, "b"));

        // a corrupt record is an empty one
        std::fs::write(&file, b"{").unwrap();
        let corrupt = InstanceFolders::load(file.clone()).await;
        assert!(corrupt.snapshot().await.is_empty());
    }

    /// A record that cannot be saved is not changed in memory either.
    #[tokio::test]
    async fn failed_save_keeps_the_record() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("instance_folders.json");
        let a = InstanceUuid::default();
        let folders = InstanceFolders::load(file.clone()).await;
        folders
            .record([(a.clone(), dir.path().join("a"))])
            .await
            .unwrap();
        std::fs::remove_file(&file).unwrap();
        std::fs::create_dir_all(&file).unwrap();
        assert!(folders
            .record([(InstanceUuid::default(), dir.path().join("b"))])
            .await
            .is_err());
        assert!(folders.forget(&a).await.is_err());
        assert_eq!(folders.snapshot().await, record(&a, "a"));
    }
}
