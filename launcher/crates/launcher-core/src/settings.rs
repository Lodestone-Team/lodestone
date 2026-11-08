//! `settings.json`: the launcher's settings. It holds no secrets.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::storage::{Document, Migration};

/// The launcher's settings.
///
/// Milestone 2 has no settings of its own yet; the file exists so later milestones
/// add fields through migrations. Keys this build does not know are kept, so a file
/// edited by a newer build at the same version survives a round trip.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(flatten)]
    pub other: Map<String, Value>,
}

impl Document for Settings {
    const VERSION: u32 = 1;
    /// Version 0 is a file without a `"version"` key; its fields carry over.
    const MIGRATIONS: &'static [Migration] = &[Ok];
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::storage;

    #[test]
    fn round_trips_unknown_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"version": 1, "future_key": [1, 2]}"#).unwrap();
        let loaded = storage::load::<Settings>(&path).unwrap();
        storage::save(&path, &loaded.document).unwrap();
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw, json!({"version": 1, "future_key": [1, 2]}));
    }

    #[test]
    fn migrates_an_unversioned_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"kept": true}"#).unwrap();
        let loaded = storage::load::<Settings>(&path).unwrap();
        assert!(loaded.migrated(Settings::VERSION));
        assert_eq!(loaded.document.other.get("kept"), Some(&json!(true)));
    }
}
