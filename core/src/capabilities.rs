//! What this core can do, reported to clients in `/info` as `capabilities`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The optional features of a core. A client talking to an older core, which sends no
/// `capabilities`, should treat every capability as available. This core has none of them.
// `Default` is this core's: every capability false.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CoreCapabilities {
    /// Macros, tasks, instance scripts (prelaunch) and extensions. They ran on the Deno
    /// runtime, which Lodestone Core no longer has, so this is always false. Kept so that
    /// clients that read it hide those features instead of assuming an older core.
    pub macros: bool,
    /// Atom (generic TypeScript) instances, which also ran on Deno. Always false, like
    /// `macros`.
    pub generic_instances: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macros_and_atoms_are_never_available() {
        let capabilities = CoreCapabilities::default();
        assert!(!capabilities.macros);
        assert!(!capabilities.generic_instances);
    }

    #[tokio::test]
    async fn info_reports_the_capabilities() {
        let dir = tempfile::tempdir().unwrap();
        let state = crate::test_util::test_app_state(dir.path(), Default::default()).await;
        let base = crate::test_util::serve_api(state).await;
        let info: serde_json::Value = reqwest::get(format!("{base}/info"))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            info["capabilities"],
            serde_json::json!({
                "macros": false,
                "generic_instances": false,
            })
        );
    }

    /// The routes of the removed features are gone; the rest of the API is still there.
    #[tokio::test]
    async fn removed_routes_are_not_found() {
        use reqwest::{Method, StatusCode};

        let dir = tempfile::tempdir().unwrap();
        let state = crate::test_util::test_app_state(dir.path(), Default::default()).await;
        let base = crate::test_util::serve_api(state).await;
        let client = reqwest::Client::new();
        for (method, path) in [
            (Method::GET, "/instance/x/macro/list"),
            (Method::PUT, "/instance/x/macro/run/hello"),
            (Method::GET, "/instance/x/task/list"),
            (Method::GET, "/instance/x/history/list"),
            (Method::GET, "/extension/gitstatus"),
            (Method::PUT, "/extension/install"),
            (Method::POST, "/instance/create_generic"),
            (Method::PUT, "/generic_setup_manifest"),
        ] {
            let response = client
                .request(method.clone(), format!("{base}{path}"))
                .send()
                .await
                .unwrap();
            assert!(
                matches!(
                    response.status(),
                    StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
                ),
                "{method} {path}: {}",
                response.status()
            );
        }
        let response = client.get(format!("{base}/info")).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
