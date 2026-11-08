use std::{collections::HashSet, path::PathBuf};

use crate::{error::Error, types::DotLodestoneConfig};
use color_eyre::eyre::Context;
use tracing::{error, warn};

pub(crate) async fn load_unique_configs(
    instances_path: &std::path::Path,
) -> Result<Vec<(PathBuf, DotLodestoneConfig)>, Error> {
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
    deduplicate(configs).await
}

async fn deduplicate(
    mut configs: Vec<(PathBuf, DotLodestoneConfig)>,
) -> Result<Vec<(PathBuf, DotLodestoneConfig)>, Error> {
    // Oldest creation time, then lexical path order owns the original UUID, even if
    // restoring that instance later fails. Directory enumeration order is unspecified.
    configs.sort_by(|(a_path, a), (b_path, b)| {
        a.creation_time()
            .cmp(&b.creation_time())
            .then_with(|| a_path.cmp(b_path))
    });
    // Reserve every on-disk identity, including identities whose restore may fail.
    let mut reserved: HashSet<_> = configs.iter().map(|(_, c)| c.uuid().clone()).collect();
    let mut seen = HashSet::new();
    let mut ready = Vec::new();
    for (path, mut config) in configs {
        if !seen.insert(config.uuid().clone()) {
            let original = config.uuid().clone();
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
                error!("Skipping duplicate instance {} (UUID {}): failed to persist replacement identity: {error}", path.display(), original);
                continue;
            }
            warn!("Duplicate instance {}: UUID {} replaced with {}. Per-instance permissions and other UUID-linked references remain with the original instance; no grants are transferred", path.display(), original, config.uuid());
        }
        ready.push((path, config));
    }
    Ok(ready)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::t_configurable::GameType;
    use crate::types::InstanceUuid;

    fn config(uuid: &InstanceUuid, creation_time: i64) -> DotLodestoneConfig {
        let mut value = serde_json::to_value(DotLodestoneConfig::new(
            uuid.clone(),
            GameType::MinecraftJava,
        ))
        .unwrap();
        value["creation_time"] = creation_time.into();
        serde_json::from_value(value).unwrap()
    }

    fn save(path: &std::path::Path, config: &DotLodestoneConfig) {
        std::fs::create_dir_all(path).unwrap();
        std::fs::write(
            path.join(".lodestone_config"),
            serde_json::to_vec(config).unwrap(),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn deterministic_survivor_metadata_and_restart_stability() {
        let dir = tempfile::tempdir().unwrap();
        let uuid = InstanceUuid::default();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        let c = dir.path().join("c");
        let original = config(&uuid, 20);
        save(&c, &original);
        save(&b, &config(&uuid, 10));
        save(&a, &config(&uuid, 10));
        let first = load_unique_configs(dir.path()).await.unwrap();
        assert_eq!(first.len(), 3);
        assert_eq!(first[0], (a.clone(), config(&uuid, 10)));
        let ids: HashSet<_> = first.iter().map(|(_, c)| c.uuid()).collect();
        assert_eq!(ids.len(), 3);
        let updated = &first.iter().find(|(p, _)| p == &c).unwrap().1;
        assert_ne!(updated.uuid(), &uuid);
        let mut expected = serde_json::to_value(&original).unwrap();
        expected["uuid"] = serde_json::to_value(updated.uuid()).unwrap();
        assert_eq!(serde_json::to_value(updated).unwrap(), expected);
        assert_eq!(load_unique_configs(dir.path()).await.unwrap(), first);
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
        let ready = deduplicate(vec![
            (b.clone(), config.clone()),
            (a.clone(), config.clone()),
        ])
        .await
        .unwrap();
        assert_eq!(ready, vec![(a.clone(), config.clone())]);
        assert!(b.join(".lodestone_config").is_dir());
        assert_eq!(
            load_unique_configs(dir.path()).await.unwrap(),
            vec![(a, config)]
        );
    }
}
