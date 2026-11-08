//! Secrets and where they are kept.

use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, Mutex},
};

use futures::future::{self, LocalBoxFuture};

/// A token or other secret. Its `Debug` and `Display` never show the value, and it
/// does not implement `Serialize`, so it cannot end up in a log line or a JSON file
/// by accident.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The secret's value, for the one place that has to send it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// Where long-lived secrets (the Microsoft refresh tokens) are kept.
///
/// The app implements it with GPUI's credentials API, which is the macOS Keychain.
/// [`MemorySecretStore`] is the implementation for tests. The futures need not be
/// `Send`: the app runs them on GPUI's main thread, where the credentials API lives.
pub trait SecretStore {
    fn read(&self, key: &str) -> LocalBoxFuture<'_, anyhow::Result<Option<Secret>>>;
    fn write(&self, key: &str, secret: &Secret) -> LocalBoxFuture<'_, anyhow::Result<()>>;
    /// Deletes the secret. Deleting a secret that does not exist is not an error.
    fn delete(&self, key: &str) -> LocalBoxFuture<'_, anyhow::Result<()>>;
}

/// A [`SecretStore`] in memory, for tests and the offscreen screenshots.
#[derive(Clone, Default)]
pub struct MemorySecretStore {
    secrets: Arc<Mutex<HashMap<String, Secret>>>,
}

impl MemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// The keys currently stored, sorted.
    pub fn keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.lock().keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn get(&self, key: &str) -> Option<Secret> {
        self.lock().get(key).cloned()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Secret>> {
        self.secrets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl SecretStore for MemorySecretStore {
    fn read(&self, key: &str) -> LocalBoxFuture<'_, anyhow::Result<Option<Secret>>> {
        Box::pin(future::ready(Ok(self.get(key))))
    }

    fn write(&self, key: &str, secret: &Secret) -> LocalBoxFuture<'_, anyhow::Result<()>> {
        self.lock().insert(key.to_string(), secret.clone());
        Box::pin(future::ready(Ok(())))
    }

    fn delete(&self, key: &str) -> LocalBoxFuture<'_, anyhow::Result<()>> {
        self.lock().remove(key);
        Box::pin(future::ready(Ok(())))
    }
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;

    use super::*;

    #[test]
    fn debug_and_display_hide_the_value() {
        let secret = Secret::new("hunter2");
        assert_eq!(format!("{secret:?}"), "Secret(<redacted>)");
        assert_eq!(secret.to_string(), "<redacted>");
        assert_eq!(format!("{:?}", Some(&secret)), "Some(Secret(<redacted>))");
        assert_eq!(secret.expose(), "hunter2");
    }

    #[test]
    fn memory_store_round_trip() {
        let store = MemorySecretStore::new();
        block_on(async {
            assert_eq!(store.read("a").await.unwrap(), None);
            store.write("a", &Secret::new("1")).await.unwrap();
            store.write("a", &Secret::new("2")).await.unwrap();
            assert_eq!(store.read("a").await.unwrap(), Some(Secret::new("2")));
            store.delete("a").await.unwrap();
            store.delete("a").await.unwrap();
            assert_eq!(store.read("a").await.unwrap(), None);
        });
        assert!(store.keys().is_empty());
    }
}
