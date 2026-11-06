use std::{borrow::Cow, cell::RefCell, future::Future, rc::Rc, sync::Arc};

use deno_core::{anyhow, Extension, ExtensionFileSource, OpState};
use serde::Deserialize;

pub mod events;
pub mod instance_control;
pub mod prelude;

/// Error type returned by Lodestone ops.
///
/// `deno_core` ops must return an error implementing `deno_error::JsErrorClass`,
/// which `anyhow::Error` does not. This wraps an `anyhow::Error` and surfaces it to
/// JavaScript as a plain `Error` whose message includes the full context chain.
#[derive(Debug, deno_error::JsError)]
#[class(generic)]
pub struct OpError(anyhow::Error);

impl std::fmt::Display for OpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#}", self.0)
    }
}

impl std::error::Error for OpError {}

impl From<anyhow::Error> for OpError {
    fn from(e: anyhow::Error) -> Self {
        Self(e)
    }
}

/// Runs an op body written against `anyhow::Result` and converts its error into an [`OpError`].
pub(crate) async fn op_try<T>(body: impl Future<Output = anyhow::Result<T>>) -> Result<T, OpError> {
    body.await.map_err(OpError::from)
}

/// Synchronous counterpart of [`op_try`].
pub(crate) fn op_try_sync<T>(body: impl FnOnce() -> anyhow::Result<T>) -> Result<T, OpError> {
    body().map_err(OpError::from)
}

/// Handle to Lodestone's main tokio runtime, stored in the `OpState` of every macro.
///
/// Each macro runs on its own single-threaded runtime (deno_core requires one), which is
/// torn down when the macro exits. Op bodies that act on instances must run on the main
/// runtime instead, so tasks they spawn (e.g. a server's stdout reader) outlive the macro.
#[derive(Clone)]
pub struct MainRuntime(pub tokio::runtime::Handle);

/// Runs an op body on the main runtime (see [`MainRuntime`]).
///
/// The body runs as an independent task: if the macro is killed while awaiting the op,
/// only the wait is cancelled, so a half-finished side effect (starting a server, an RCON
/// exchange) still runs to completion.
pub(crate) async fn op_on_main<T: Send + 'static>(
    state: &Rc<RefCell<OpState>>,
    body: impl Future<Output = anyhow::Result<T>> + Send + 'static,
) -> Result<T, OpError> {
    let handle = state.borrow().borrow::<MainRuntime>().0.clone();
    match handle.spawn(body).await {
        Ok(result) => result.map_err(OpError::from),
        Err(e) => Err(anyhow::anyhow!("Op task failed: {e}").into()),
    }
}

/// Like [`op_on_main`], but the task is aborted if the macro stops waiting for it (e.g. it
/// is killed). Only for bodies without side effects that could be left half done, such as
/// waits that may never finish.
pub(crate) async fn op_on_main_abortable<T: Send + 'static>(
    state: &Rc<RefCell<OpState>>,
    body: impl Future<Output = anyhow::Result<T>> + Send + 'static,
) -> Result<T, OpError> {
    struct AbortOnDrop(tokio::task::AbortHandle);
    impl Drop for AbortOnDrop {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let handle = state.borrow().borrow::<MainRuntime>().0.clone();
    let task = handle.spawn(body);
    let _guard = AbortOnDrop(task.abort_handle());
    match task.await {
        Ok(result) => result.map_err(OpError::from),
        Err(e) => Err(anyhow::anyhow!("Op task failed: {e}").into()),
    }
}

/// An op argument deserialized through serde, so a JS value of the wrong type is rejected
/// with a `TypeError` instead of being coerced (`#[string]` turns non-strings into `""`
/// and plain integer arguments wrap out-of-range numbers).
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct Strict<T>(pub T);

const COMPAT_SPECIFIER: &str = "ext:lodestone_compat/compat.js";

/// Restores the op calling convention Lodestone macros rely on.
///
/// Macros (and the remote `lodestone-macro-lib`, which imports the `deno_ops/**/*.ts`
/// wrappers from this repository) call ops through `Deno[Deno.internal].core`:
/// sync ops via `core.ops.<name>(...)` and async ops via `core.opAsync("<name>", ...)`.
///
/// Since deno_core 0.2xx, `core.opAsync` no longer exists, and deno_runtime's bootstrap
/// removes every op not on its own allow-list from `core.ops`. This extension rebuilds
/// `Deno[Deno.internal].core` with all non-runtime ops registered on the worker
/// (i.e. Lodestone's ops) plus `opAsync`/`opSync` shims.
///
/// MUST be registered after every extension whose ops should be exposed.
pub fn register_compat_shim(worker_options: &mut deno_runtime::worker::WorkerOptions) {
    let names: Vec<&'static str> = worker_options
        .extensions
        .iter()
        .flat_map(|ext| ext.ops.iter().map(|decl| decl.name))
        .collect();
    let code = include_str!("compat.js").replacen(
        "__LODESTONE_OP_NAMES__",
        &serde_json::to_string(&names).expect("op names are serializable"),
        1,
    );
    worker_options.extensions.push(Extension {
        name: "lodestone_compat",
        esm_files: Cow::Owned(vec![ExtensionFileSource::new_computed(
            COMPAT_SPECIFIER,
            Arc::from(code),
        )]),
        esm_entry_point: Some(COMPAT_SPECIFIER),
        ..Default::default()
    });
}
