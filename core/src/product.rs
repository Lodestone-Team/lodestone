//! What a core is (the standalone core, or the one inside Lodestone Host) and what
//! started it.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Which program this core is, reported in `/info` (and by `POST /local_claim/start`) as
/// `product`, so clients can tell Lodestone Host, the desktop app with its own window,
/// from a bare backend.
///
/// Cores from before this have no `product`: clients should treat a missing field as
/// [`CoreProduct::Core`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum CoreProduct {
    /// Running inside Lodestone Host, the desktop app (`dashboard/src-tauri`).
    Host,
    /// The standalone core binary, run directly, by Lodestone CLI or in Docker.
    Core,
}

impl CoreProduct {
    /// The product for a core started with `--is-host` (`is_host`) or without.
    pub fn from_is_host(is_host: bool) -> Self {
        if is_host {
            Self::Host
        } else {
            Self::Core
        }
    }
}

/// What started this core, reported in `/info` (and by `POST /local_claim/start`) as
/// `launched_by`. Lodestone CLI is the intended way to run a standalone core.
///
/// Cores from before this have no `launched_by`: clients should treat a missing field as
/// unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum LaunchedBy {
    /// Lodestone Host, which runs the core in its own process (`--is-host`).
    Host,
    /// Lodestone CLI (`lodestone_cli`, which passes `--is-cli`).
    LodestoneCli,
    /// The core's Docker image (`LODESTONE_DOCKER`, or `/.dockerenv`).
    Docker,
    /// Anything else: the binary run by hand, or by another program.
    Direct,
}

impl LaunchedBy {
    /// What started the core, from its flags and whether it runs in Docker. Lodestone
    /// Host first, then Lodestone CLI, then Docker.
    pub fn from_flags(is_host: bool, is_cli: bool, in_docker: bool) -> Self {
        if is_host {
            Self::Host
        } else if is_cli {
            Self::LodestoneCli
        } else if in_docker {
            Self::Docker
        } else {
            Self::Direct
        }
    }
}

/// Whether the core runs in a Docker container: `LODESTONE_DOCKER` set to 1, true or yes
/// (the core's image sets it), or else Docker's `/.dockerenv` file.
pub fn in_docker() -> bool {
    docker_env_says(std::env::var("LODESTONE_DOCKER").ok().as_deref())
        || std::path::Path::new("/.dockerenv").exists()
}

fn docker_env_says(value: Option<&str>) -> bool {
    value.is_some_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launched_by_precedence() {
        use LaunchedBy::*;
        for (is_host, is_cli, docker, expected) in [
            (true, true, true, Host),
            (true, false, false, Host),
            (false, true, true, LodestoneCli),
            (false, true, false, LodestoneCli),
            (false, false, true, Docker),
            (false, false, false, Direct),
        ] {
            assert_eq!(
                LaunchedBy::from_flags(is_host, is_cli, docker),
                expected,
                "host {is_host}, cli {is_cli}, docker {docker}"
            );
        }
        assert_eq!(
            serde_json::to_value(LodestoneCli).unwrap(),
            serde_json::json!("lodestone_cli")
        );
        assert_eq!(
            serde_json::to_value(Direct).unwrap(),
            serde_json::json!("direct")
        );
    }

    #[test]
    fn docker_variable() {
        assert!(docker_env_says(Some("1")));
        assert!(docker_env_says(Some(" TRUE ")));
        assert!(docker_env_says(Some("yes")));
        assert!(!docker_env_says(Some("0")));
        assert!(!docker_env_says(Some("")));
        assert!(!docker_env_says(None));
    }

    #[test]
    fn serializes_as_lowercase_names() {
        assert_eq!(
            serde_json::to_value(CoreProduct::Host).unwrap(),
            serde_json::json!("host")
        );
        assert_eq!(
            serde_json::to_value(CoreProduct::Core).unwrap(),
            serde_json::json!("core")
        );
        assert_eq!(CoreProduct::from_is_host(true), CoreProduct::Host);
        assert_eq!(CoreProduct::from_is_host(false), CoreProduct::Core);
    }
}
