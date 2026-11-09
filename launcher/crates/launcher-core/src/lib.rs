//! Lodestone Launcher's accounts, Microsoft sign-in, instances, data storage, and
//! the installing and launching of Minecraft.
//!
//! This crate has no GPUI dependency so it can be unit-tested on its own. The app
//! crate supplies the platform pieces: the keychain-backed [`SecretStore`], the
//! browser and the executor.
//!
//! Secrets (refresh and access tokens) are wrapped in [`Secret`], which never prints
//! its value. They are kept in the [`SecretStore`] or in memory only, and never
//! written to the JSON files in the data directory.

pub mod accounts;
pub mod auth;
pub mod content;
pub mod cores;
pub mod disk;
pub mod export;
pub mod game;
pub mod http;
pub mod import;
pub mod instances;
pub mod markdown;
pub mod modpack;
pub mod offline;
pub mod paths;
pub mod screenshots;
pub mod secret;
pub mod settings;
pub mod shared_options;
pub mod skin;
pub mod storage;
pub mod update;
pub mod wording;
pub mod worlds;

pub use secret::{MemorySecretStore, Secret, SecretStore};

/// The User-Agent sent with every request. It identifies the app and its version
/// only, and carries no personal data.
pub const USER_AGENT: &str = concat!(
    "LodestoneLauncher/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/Lodestone-Team/lodestone)"
);

/// Offline accounts are a development aid: they are available in debug builds, or
/// in any build with `LODESTONE_LAUNCHER_OFFLINE=1`.
pub fn offline_mode_enabled() -> bool {
    offline_mode_enabled_with(
        cfg!(debug_assertions),
        std::env::var_os("LODESTONE_LAUNCHER_OFFLINE").as_deref(),
    )
}

/// Random bytes from the operating system's generator.
pub(crate) fn random_bytes<const N: usize>() -> [u8; N] {
    use rand::TryRngCore as _;
    let mut bytes = [0; N];
    rand::rngs::OsRng
        .try_fill_bytes(&mut bytes)
        .expect("the operating system's random number generator failed");
    bytes
}

fn offline_mode_enabled_with(debug_build: bool, env: Option<&std::ffi::OsStr>) -> bool {
    debug_build || env.is_some_and(|value| value == "1")
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;

    #[test]
    fn user_agent_names_the_app_and_version_only() {
        assert_eq!(
            USER_AGENT,
            format!(
                "LodestoneLauncher/{} (+https://github.com/Lodestone-Team/lodestone)",
                env!("CARGO_PKG_VERSION")
            )
        );
    }

    #[test]
    fn offline_mode_needs_a_debug_build_or_the_env_var() {
        assert!(offline_mode_enabled_with(true, None));
        assert!(offline_mode_enabled_with(false, Some(OsStr::new("1"))));
        assert!(!offline_mode_enabled_with(false, None));
        assert!(!offline_mode_enabled_with(false, Some(OsStr::new("0"))));
        assert!(!offline_mode_enabled_with(false, Some(OsStr::new("true"))));
    }
}
