//! This core's own UUID, kept in the data directory so it stays the same across
//! restarts. Launchers recognise a core, and keep what they trust about it, by this
//! UUID; a new one on every start made each restart look like a different core.

use std::path::Path;

use tracing::warn;
use uuid::Uuid;

use crate::persist::write_atomic_blocking;

/// The UUID stored at `path`, or a new one that is saved there for next time.
///
/// A missing, unreadable or malformed file gets a new UUID. If it can't be saved, the
/// core still starts with it, and the next start makes another.
pub fn load_or_create(path: &Path) -> String {
    if let Ok(text) = std::fs::read_to_string(path) {
        match Uuid::parse_str(text.trim()) {
            Ok(uuid) => return uuid.to_string(),
            Err(e) => warn!("Ignoring the core UUID in {}: {e}", path.display()),
        }
    }
    let uuid = Uuid::new_v4().to_string();
    if let Err(e) = write_atomic_blocking(path, uuid.as_bytes(), None) {
        warn!(
            "Couldn't save the core UUID to {}, so it will change on the next start: {e}",
            path.display()
        );
    }
    uuid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_uuid_survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("core_uuid");
        let first = load_or_create(&path);
        assert!(Uuid::parse_str(&first).is_ok());
        assert_eq!(load_or_create(&path), first);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), first);
    }

    #[test]
    fn a_damaged_file_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("core_uuid");
        std::fs::write(&path, "not a uuid").unwrap();
        let uuid = load_or_create(&path);
        assert!(Uuid::parse_str(&uuid).is_ok());
        assert_eq!(load_or_create(&path), uuid);
    }

    #[test]
    fn surrounding_whitespace_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("core_uuid");
        let uuid = Uuid::new_v4().to_string();
        std::fs::write(&path, format!("{uuid}\r\n")).unwrap();
        assert_eq!(load_or_create(&path), uuid);
    }
}
