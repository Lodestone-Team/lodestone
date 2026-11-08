//! The [`SecretStore`] on GPUI's credentials API, which is the macOS Keychain.

use futures::{FutureExt as _, future::LocalBoxFuture};
use gpui::AsyncApp;
use launcher_core::{Secret, SecretStore};

/// The Keychain item's account field. The item is found by its server field, the
/// key from `launcher-core`.
const KEYCHAIN_ACCOUNT: &str = "Lodestone Launcher";

/// Keeps secrets as Keychain internet passwords. Each call goes through GPUI's
/// main thread, which then runs the Keychain call on its background executor.
pub struct KeychainStore {
    cx: AsyncApp,
}

impl KeychainStore {
    pub fn new(cx: AsyncApp) -> Self {
        Self { cx }
    }
}

impl SecretStore for KeychainStore {
    fn read(&self, key: &str) -> LocalBoxFuture<'_, anyhow::Result<Option<Secret>>> {
        let task = self.cx.update(|cx| cx.read_credentials(key));
        async move {
            let Some((_, bytes)) = task.await? else {
                return Ok(None);
            };
            let value = String::from_utf8(bytes)
                .map_err(|_| anyhow::anyhow!("the saved sign-in is not valid text"))?;
            Ok(Some(Secret::new(value)))
        }
        .boxed_local()
    }

    fn write(&self, key: &str, secret: &Secret) -> LocalBoxFuture<'_, anyhow::Result<()>> {
        let task = self
            .cx
            .update(|cx| cx.write_credentials(key, KEYCHAIN_ACCOUNT, secret.expose().as_bytes()));
        task.boxed_local()
    }

    fn delete(&self, key: &str) -> LocalBoxFuture<'_, anyhow::Result<()>> {
        let key = key.to_string();
        async move {
            // Deleting a missing Keychain item is an error, but not for us.
            let read = self.cx.update(|cx| cx.read_credentials(&key));
            if read.await?.is_none() {
                return Ok(());
            }
            self.cx.update(|cx| cx.delete_credentials(&key)).await
        }
        .boxed_local()
    }
}
