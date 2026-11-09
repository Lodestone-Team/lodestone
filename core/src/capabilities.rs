//! What this core can do, reported to clients in `/info` as `capabilities`.
//!
//! Most capabilities depend on the cargo features the core was built with; see the
//! `deno` feature in `Cargo.toml`. The routes of a capability the build lacks answer
//! 501 Not Implemented with [`Capability::unavailable`].

use color_eyre::eyre::eyre;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{Error, ErrorKind};

/// The optional features of a core. A client talking to an older core, which sends no
/// `capabilities`, should treat every capability as available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CoreCapabilities {
    /// Macros, tasks, instance scripts (prelaunch) and extensions, which run on the Deno
    /// runtime. False when the core was built without the `deno` feature. Whether code
    /// may run right now is the separate code execution setting.
    pub macros: bool,
    /// Atom (generic TypeScript) instances, which also run on Deno. False when the core
    /// was built without the `deno` feature. Docker instances do not need it.
    pub generic_instances: bool,
    /// Whether a Docker daemon was reachable when the core started, so Docker containers
    /// can be managed as instances.
    pub docker: bool,
}

impl CoreCapabilities {
    /// The capabilities the build has, with `docker` from the running core.
    pub fn new(docker: bool) -> Self {
        CoreCapabilities {
            macros: Capability::Macros.is_built(),
            generic_instances: Capability::GenericInstances.is_built(),
            docker,
        }
    }
}

/// A capability that is compiled in or out with a cargo feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    Macros,
    GenericInstances,
}

impl Capability {
    /// Its field in [`CoreCapabilities`].
    pub fn name(self) -> &'static str {
        match self {
            Capability::Macros => "macros",
            Capability::GenericInstances => "generic_instances",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Capability::Macros => "Macros, tasks, server scripts and extensions are",
            Capability::GenericInstances => "Atom (generic) servers are",
        }
    }

    /// Whether this build includes it.
    pub fn is_built(self) -> bool {
        match self {
            Capability::Macros | Capability::GenericInstances => cfg!(feature = "deno"),
        }
    }

    /// The 501 error for using it on a core built without it.
    pub fn unavailable(self) -> Error {
        Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!(
                "{} not available: this Lodestone Core was built without scripting support \
                 (the `deno` feature). Missing capability: {}",
                self.description(),
                self.name()
            ),
        }
    }
}

/// Answers every request to the routes of `router` with [`Capability::unavailable`], so a
/// capability that is not built keeps its routes, with a clear error instead of a 404.
#[cfg(not(feature = "deno"))]
pub fn unavailable_routes<S>(router: axum::Router<S>, capability: Capability) -> axum::Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    use axum::response::IntoResponse;
    router.route_layer(axum::middleware::from_fn(
        move |_request: axum::extract::Request, _next: axum::middleware::Next| async move {
            capability.unavailable().into_response()
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_follow_the_deno_feature() {
        let capabilities = CoreCapabilities::new(false);
        assert_eq!(capabilities.macros, cfg!(feature = "deno"));
        assert_eq!(capabilities.generic_instances, cfg!(feature = "deno"));
        assert!(!capabilities.docker);
        assert!(CoreCapabilities::new(true).docker);
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
                "macros": cfg!(feature = "deno"),
                "generic_instances": cfg!(feature = "deno"),
                // the test state has no Docker daemon
                "docker": false,
            })
        );
    }

    /// Every route of a capability the build lacks answers 501 with the capability's name,
    /// before authentication, instead of a 404.
    #[cfg(not(feature = "deno"))]
    #[tokio::test]
    async fn compiled_out_routes_answer_501() {
        use reqwest::{Method, StatusCode};

        let dir = tempfile::tempdir().unwrap();
        let state = crate::test_util::test_app_state(dir.path(), Default::default()).await;
        let base = crate::test_util::serve_api(state).await;
        let client = reqwest::Client::new();
        for (method, path, capability) in [
            (Method::GET, "/instance/x/macro/list", Capability::Macros),
            (
                Method::PUT,
                "/instance/x/macro/run/hello",
                Capability::Macros,
            ),
            (Method::PUT, "/instance/x/macro/kill/1", Capability::Macros),
            (
                Method::GET,
                "/instance/x/macro/config/get/hello",
                Capability::Macros,
            ),
            (
                Method::POST,
                "/instance/x/macro/config/store/hello",
                Capability::Macros,
            ),
            (Method::GET, "/instance/x/task/list", Capability::Macros),
            (Method::GET, "/instance/x/history/list", Capability::Macros),
            (Method::GET, "/extension/gitstatus", Capability::Macros),
            (Method::PUT, "/extension/install", Capability::Macros),
            (
                Method::POST,
                "/instance/create_generic",
                Capability::GenericInstances,
            ),
        ] {
            let response = client
                .request(method.clone(), format!("{base}{path}"))
                .send()
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::NOT_IMPLEMENTED,
                "{method} {path}"
            );
            let body: serde_json::Value = response.json().await.unwrap();
            assert_eq!(body["kind"], "UnsupportedOperation", "{method} {path}");
            let message = body["causes"][0].as_str().unwrap();
            assert!(
                message.contains(&format!("Missing capability: {}", capability.name())),
                "{method} {path}: {message}"
            );
        }
        // the rest of the API is still there
        let response = client.get(format!("{base}/info")).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn unavailable_error_names_the_capability() {
        for capability in [Capability::Macros, Capability::GenericInstances] {
            let error = capability.unavailable();
            assert!(matches!(error.kind, ErrorKind::UnsupportedOperation));
            let message = error.source.to_string();
            assert!(message.contains(capability.name()), "{message}");
            assert!(message.contains("`deno` feature"), "{message}");
        }
    }
}
