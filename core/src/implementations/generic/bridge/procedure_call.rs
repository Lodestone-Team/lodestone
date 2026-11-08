use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use color_eyre::eyre::eyre;
use deno_core::anyhow::anyhow;
use deno_core::{op2, OpState};

use crate::deno_ops::{op_try, op_try_sync, OpError};
use crate::traits::t_macro::ExitStatus;
use enum_kinds::EnumKind;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot, watch, Mutex};
use tracing::{error, warn};
use ts_rs::TS;

use crate::error::{Error, ErrorKind};
use crate::events::CausedBy;

use crate::implementations::generic::player::GenericPlayer;

use crate::traits::t_configurable::manifest::{
    ConfigurableManifest, ConfigurableValue, SetupManifest, SetupValue,
};
use crate::traits::t_configurable::Game;
use crate::traits::t_player::Player;
use crate::traits::t_server::State;
use crate::types::{DotLodestoneConfig, Snowflake};
use crate::MonitorReport;

#[derive(Debug, Clone, Serialize, Deserialize, TS, EnumKind)]
#[serde(tag = "type")]
// #[ts(export_to = "src/implementations/generic/js/main/libs/bindings/ProcedureCallInner.ts")]
// #[ts(export)]
#[enum_kind(ProcedureCallKind, derive(Serialize, Deserialize, TS))]
pub enum ProcedureCallInner {
    SetupInstance {
        dot_lodestone_config: DotLodestoneConfig,
        setup_value: SetupValue,
        // ProgressionEventID is non-clone
        progression_event_id: Snowflake,
        path: PathBuf,
    },
    RestoreInstance {
        dot_lodestone_config: DotLodestoneConfig,
        path: PathBuf,
    },
    DestructInstance,
    GetSetupManifest,
    // start of TConfigurable
    GetName,
    GetDescription,
    GetVersion,
    GetGame,
    GetPort,
    GetAutoStart,
    GetRestartOnCrash,
    SetName {
        new_name: String,
    },
    SetDescription {
        new_description: String,
    },
    SetPort {
        new_port: u32,
    },
    SetAutoStart {
        new_auto_start: bool,
    },
    SetRestartOnCrash {
        new_restart_on_crash: bool,
    },
    GetConfigurableManifest,
    UpdateConfigurable {
        section_id: String,
        setting_id: String,
        new_value: ConfigurableValue,
    },
    // end of TConfigurable
    // start of TServer
    StartInstance {
        caused_by: CausedBy,
        block: bool,
    },
    StopInstance {
        caused_by: CausedBy,
        block: bool,
    },
    RestartInstance {
        caused_by: CausedBy,
        block: bool,
    },
    KillInstance {
        caused_by: CausedBy,
    },
    GetState,
    SendCommand {
        command: String,
        caused_by: CausedBy,
    },
    Monitor,
    // end of TServer
    // start of TPlayerManagement
    GetPlayerCount,
    GetMaxPlayerCount,
    GetPlayerList,
    // end of TPlayerManagement
    // start of TMacro
    GetMacroList,
    GetTaskList,
    GetHistoryList,
    DeleteMacro {
        name: String,
    },
    CreateMacro {
        name: String,
        content: String,
    },
    RunMacro {
        name: String,
        args: Vec<String>,
        caused_by: CausedBy,
    }, // end of TMacro
}

#[test]
fn export_procedure_call_kind() {
    let _ = ProcedureCallKind::export(&ts_rs::Config::from_env());
}

// sent to TS side to call a TS procedure
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProcedureCall {
    id: u64,
    inner: ProcedureCallInner,
}

#[derive(Debug, Clone, TS, Deserialize)]
// #[ts(export_to = "src/implementations/generic/js/main/libs/bindings/ProcedureCallResultInner.ts")]
// #[ts(export)]
pub enum ProcedureCallResultInner {
    String(String),
    Monitor(MonitorReport),
    State(State),
    Num(u32),
    Game(Game),
    Bool(bool),
    ConfigurableManifest(ConfigurableManifest),
    Player(HashSet<GenericPlayer>),
    SetupManifest(SetupManifest),
    Void,
}

impl TryFrom<ProcedureCallResultInner> for String {
    type Error = Error;
    fn try_from(value: ProcedureCallResultInner) -> Result<Self, Self::Error> {
        match value {
            ProcedureCallResultInner::String(s) => Ok(s),
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("ProcedureCallResultInner::String expected, got {:?}", value),
            }),
        }
    }
}

impl TryFrom<ProcedureCallResultInner> for u32 {
    type Error = Error;
    fn try_from(value: ProcedureCallResultInner) -> Result<Self, Self::Error> {
        match value {
            ProcedureCallResultInner::Num(n) => Ok(n),
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("ProcedureCallResultInner::Num expected, got {:?}", value),
            }),
        }
    }
}

impl TryFrom<ProcedureCallResultInner> for bool {
    type Error = Error;
    fn try_from(value: ProcedureCallResultInner) -> Result<Self, Self::Error> {
        match value {
            ProcedureCallResultInner::Bool(b) => Ok(b),
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("ProcedureCallResultInner::Bool expected, got {:?}", value),
            }),
        }
    }
}

impl TryFrom<ProcedureCallResultInner> for ConfigurableManifest {
    type Error = Error;
    fn try_from(value: ProcedureCallResultInner) -> Result<Self, Self::Error> {
        match value {
            ProcedureCallResultInner::ConfigurableManifest(m) => Ok(m),
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!(
                    "ProcedureCallResultInner::ConfigurableManifest expected, got {:?}",
                    value
                ),
            }),
        }
    }
}

impl TryFrom<ProcedureCallResultInner> for MonitorReport {
    type Error = Error;
    fn try_from(value: ProcedureCallResultInner) -> Result<Self, Self::Error> {
        match value {
            ProcedureCallResultInner::Monitor(m) => Ok(m),
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!(
                    "ProcedureCallResultInner::Monitor expected, got {:?}",
                    value
                ),
            }),
        }
    }
}

impl TryFrom<ProcedureCallResultInner> for State {
    type Error = Error;
    fn try_from(value: ProcedureCallResultInner) -> Result<Self, Error> {
        match value {
            ProcedureCallResultInner::State(s) => Ok(s),
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("ProcedureCallResultInner::State expected, got {:?}", value),
            }),
        }
    }
}

impl TryFrom<ProcedureCallResultInner> for Game {
    type Error = Error;
    fn try_from(value: ProcedureCallResultInner) -> Result<Self, Self::Error> {
        match value {
            ProcedureCallResultInner::Game(g) => Ok(g),
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("ProcedureCallResultInner::Game expected, got {:?}", value),
            }),
        }
    }
}

impl TryFrom<ProcedureCallResultInner> for HashSet<Player> {
    type Error = Error;
    fn try_from(value: ProcedureCallResultInner) -> Result<Self, Self::Error> {
        match value {
            ProcedureCallResultInner::Player(p) => Ok(p.into_iter().map(|p| p.into()).collect()),
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("ProcedureCallResultInner::Player expected, got {:?}", value),
            }),
        }
    }
}

impl TryFrom<ProcedureCallResultInner> for SetupManifest {
    type Error = Error;
    fn try_from(value: ProcedureCallResultInner) -> Result<Self, Self::Error> {
        match value {
            ProcedureCallResultInner::SetupManifest(m) => Ok(m),
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!(
                    "ProcedureCallResultInner::SetupManifest expected, got {:?}",
                    value
                ),
            }),
        }
    }
}

impl TryFrom<ProcedureCallResultInner> for () {
    type Error = Error;
    fn try_from(value: ProcedureCallResultInner) -> Result<Self, Self::Error> {
        match value {
            ProcedureCallResultInner::Void => Ok(()),
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("ProcedureCallResultInner::Void expected, got {:?}", value),
            }),
        }
    }
}
#[derive(Debug, Clone, TS, Deserialize)]
// #[ts(export_to = "src/implementations/generic/js/main/libs/bindings/ErrorKindIR.ts")]
// #[ts(export)]
pub enum ErrorKindIR {
    NotFound,
    UnsupportedOperation,
    BadRequest,
    Internal,
}

impl From<ErrorKindIR> for ErrorKind {
    fn from(ir: ErrorKindIR) -> Self {
        match ir {
            ErrorKindIR::NotFound => Self::NotFound,
            ErrorKindIR::UnsupportedOperation => Self::UnsupportedOperation,
            ErrorKindIR::BadRequest => Self::BadRequest,
            ErrorKindIR::Internal => Self::Internal,
        }
    }
}

#[derive(Debug, Clone, TS, Deserialize)]
// #[ts(export_to = "src/implementation/generic/js/main/libs")]
// #[ts(export)]
pub struct ErrorIR {
    kind: ErrorKindIR,
    source: String,
}

impl From<ErrorIR> for Error {
    fn from(ir: ErrorIR) -> Self {
        Self {
            kind: ir.kind.into(),
            source: eyre!(ir.source),
        }
    }
}

#[derive(Debug, Clone, TS, Deserialize)]
// #[ts(export_to = "src/implementation/generic/js/main/libs")]
// #[ts(export)]
pub struct ProcedureCallResultIR {
    id: u64,
    success: bool,
    /// MUST be None if success is false
    /// MUST be Some if success is true
    inner: Option<ProcedureCallResultInner>,
    /// MUST be None if success is true
    /// MUST be Some if success is false
    error: Option<ErrorIR>,
}

#[op2]
#[serde]
pub(crate) async fn next_procedure(state: Rc<RefCell<OpState>>) -> Result<ProcedureCall, OpError> {
    let bridge = state.borrow().borrow::<ProcedureBridge>().clone();
    op_try(async move {
        let mut rx = bridge.procedure_rx.lock().await;
        // the bridge holds a sender itself, so the channel never closes while it is alive
        rx.recv()
            .await
            .ok_or_else(|| anyhow!("ProcedureBridge::next_procedure: channel closed"))
    })
    .await
}

/// Called once by the atom's procedure bridge loop, right before it starts taking calls.
#[op2]
#[string]
pub(crate) fn proc_bridge_ready(state: Rc<RefCell<OpState>>) -> Result<String, OpError> {
    let bridge = state.borrow().borrow::<ProcedureBridge>().clone();
    op_try_sync(|| {
        let became_ready = bridge.state.send_if_modified(|state| {
            let starting = *state == BridgeState::Starting;
            if starting {
                *state = BridgeState::Ready;
            }
            starting
        });
        if !became_ready {
            return Err(anyhow!("ProcedureBridge::proc_bridge_ready: already ready"));
        }
        Ok("".to_string())
    })
}

#[op2]
pub(crate) fn emit_result(
    state: Rc<RefCell<OpState>>,
    #[serde] result: ProcedureCallResultIR,
) -> Result<(), OpError> {
    let bridge = state.borrow().borrow::<ProcedureBridge>().clone();
    // Never throw here: the atom's bridge loop does not catch errors from `emit_result`,
    // so a late reply (after the caller timed out) would otherwise stop the atom.
    bridge.complete(result);
    Ok(())
}

/// Builds the deno extension exposing the procedure bridge ops to the generic instance runtime.
pub(crate) fn procedure_bridge_extension(bridge: ProcedureBridge) -> deno_core::Extension {
    deno_core::Extension {
        name: "generic_deno_extension_builder",
        ops: std::borrow::Cow::Owned(vec![next_procedure(), emit_result(), proc_bridge_ready()]),
        op_state_fn: Some(Box::new(move |state| {
            state.put(bridge);
        })),
        ..Default::default()
    }
}

/// How long a normal procedure call may take before the caller gives up.
const DEFAULT_CALL_TIMEOUT: Duration = Duration::from_secs(30);
/// Restoring an atom runs on every core start and may read or check its files.
const RESTORE_CALL_TIMEOUT: Duration = Duration::from_secs(120);

/// Timeout for a procedure call; `None` waits until the atom answers or exits.
fn call_timeout(inner: &ProcedureCallInner) -> Option<Duration> {
    match inner {
        // Setup may download and unpack a whole game server, which can take far longer than
        // any fixed limit. It reports progress through its progression event, and the call
        // still fails at once if the atom macro exits.
        ProcedureCallInner::SetupInstance { .. } => None,
        ProcedureCallInner::RestoreInstance { .. } => Some(RESTORE_CALL_TIMEOUT),
        _ => Some(DEFAULT_CALL_TIMEOUT),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BridgeState {
    /// The atom macro has not started its procedure bridge loop yet.
    Starting,
    /// The atom macro is taking procedure calls.
    Ready,
    /// The atom macro has exited; no call can be answered any more.
    Closed(String),
}

type PendingCalls = Arc<std::sync::Mutex<HashMap<u64, oneshot::Sender<ProcedureCallResultIR>>>>;

fn lock_pending(
    pending: &PendingCalls,
) -> std::sync::MutexGuard<'_, HashMap<u64, oneshot::Sender<ProcedureCallResultIR>>> {
    // the map stays consistent even if a holder panicked
    pending
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Removes a call from the pending table when its caller stops waiting.
struct PendingGuard<'a> {
    pending: &'a PendingCalls,
    id: u64,
}

impl Drop for PendingGuard<'_> {
    fn drop(&mut self) {
        lock_pending(self.pending).remove(&self.id);
    }
}

/// Request/response channel between Rust and an atom's TypeScript implementation.
#[derive(Debug, Clone)]
pub struct ProcedureBridge {
    state: watch::Sender<BridgeState>,
    procedure_call_id: Arc<AtomicU64>,
    procedure_tx: mpsc::UnboundedSender<ProcedureCall>,
    // shared so a call queued before the atom polls is not lost
    procedure_rx: Arc<Mutex<mpsc::UnboundedReceiver<ProcedureCall>>>,
    /// Callers waiting for a result, by call id.
    pending: PendingCalls,
}

impl ProcedureBridge {
    pub fn new() -> Self {
        let (procedure_tx, procedure_rx) = mpsc::unbounded_channel();
        Self {
            state: watch::Sender::new(BridgeState::Starting),
            procedure_call_id: Arc::new(AtomicU64::new(0)),
            procedure_tx,
            procedure_rx: Arc::new(Mutex::new(procedure_rx)),
            pending: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Marks the bridge as dead once the atom macro exits, so pending and later calls fail
    /// at once instead of waiting for an answer that can never come.
    pub fn close_on_exit(
        &self,
        exit_future: impl Future<Output = Result<ExitStatus, Error>> + Send + 'static,
    ) {
        let bridge = self.clone();
        tokio::spawn(async move {
            let reason = match exit_future.await {
                Ok(ExitStatus::Error { error_msg, .. }) => {
                    format!("the atom macro exited with an error: {error_msg}")
                }
                Ok(ExitStatus::Killed { .. }) => "the atom macro was killed".to_string(),
                Ok(ExitStatus::Success { .. }) => "the atom macro exited".to_string(),
                Err(e) => format!("the atom macro's exit status is unknown: {e}"),
            };
            bridge.close(reason);
        });
    }

    fn close(&self, reason: String) {
        self.state.send_replace(BridgeState::Closed(reason));
        // waiting callers see the state change; drop their senders as well
        lock_pending(&self.pending).clear();
    }

    /// Waits until the atom starts taking calls. Fails if it exits first.
    pub async fn wait_ready(&self) -> Result<(), Error> {
        let mut state_rx = self.state.subscribe();
        let state = match state_rx
            .wait_for(|state| *state != BridgeState::Starting)
            .await
        {
            Ok(state) => state.clone(),
            // unreachable: `self` holds the sender
            Err(_) => BridgeState::Closed("the bridge was dropped".to_string()),
        };
        match state {
            BridgeState::Closed(reason) => Err(Error {
                kind: ErrorKind::Internal,
                source: eyre!("The atom did not start: {reason}"),
            }),
            _ => Ok(()),
        }
    }

    /// Resolves with the reason once the bridge is closed.
    async fn closed(&self) -> String {
        let mut state_rx = self.state.subscribe();
        let state = state_rx
            .wait_for(|state| matches!(state, BridgeState::Closed(_)))
            .await
            .map(|state| state.clone());
        match state {
            Ok(BridgeState::Closed(reason)) => reason,
            Ok(_) => "the bridge was closed".to_string(),
            // unreachable: `self` holds the sender
            Err(_) => std::future::pending().await,
        }
    }

    /// Delivers a result sent by the atom to the caller waiting for it.
    fn complete(&self, result: ProcedureCallResultIR) {
        let waiting = lock_pending(&self.pending).remove(&result.id);
        match waiting {
            Some(tx) => {
                // the caller may have stopped waiting in the meantime
                let _ = tx.send(result);
            }
            None => warn!(
                "Atom sent a result for procedure call {} that nobody is waiting for (it may have timed out)",
                result.id
            ),
        }
    }

    pub async fn call(&self, inner: ProcedureCallInner) -> Result<ProcedureCallResultInner, Error> {
        let timeout = call_timeout(&inner);
        self.call_with_timeout(inner, timeout).await
    }

    async fn call_with_timeout(
        &self,
        inner: ProcedureCallInner,
        timeout: Option<Duration>,
    ) -> Result<ProcedureCallResultInner, Error> {
        let kind = ProcedureCallKind::from(&inner);
        let failed = |reason: String| Error {
            kind: ErrorKind::Internal,
            source: eyre!("Atom procedure call {kind:?} failed: {reason}"),
        };
        if let BridgeState::Closed(reason) = &*self.state.borrow() {
            return Err(failed(reason.clone()));
        }

        let id = self.procedure_call_id.fetch_add(1, Ordering::SeqCst);
        let (result_tx, result_rx) = oneshot::channel();
        lock_pending(&self.pending).insert(id, result_tx);
        let _guard = PendingGuard {
            pending: &self.pending,
            id,
        };
        self.procedure_tx
            .send(ProcedureCall { id, inner })
            .map_err(|_| failed("the procedure channel is closed".to_string()))?;

        let timed_out = async {
            match timeout {
                Some(timeout) => tokio::time::sleep(timeout).await,
                None => std::future::pending().await,
            }
        };
        let result = tokio::select! {
            result = result_rx => match result {
                Ok(result) => result,
                // the sender is only dropped when the bridge closes
                Err(_) => return Err(failed(self.closed().await)),
            },
            reason = self.closed() => return Err(failed(reason)),
            _ = timed_out => {
                let timeout = timeout.unwrap_or_default();
                error!("Atom procedure call {kind:?} got no answer within {timeout:?}");
                return Err(failed(format!("no answer within {timeout:?}")));
            }
        };

        // the TypeScript side is not trusted to keep the success/inner/error contract
        match result {
            ProcedureCallResultIR {
                success: true,
                inner: Some(inner),
                ..
            } => Ok(inner),
            ProcedureCallResultIR {
                success: false,
                error: Some(error),
                ..
            } => Err(error.into()),
            ProcedureCallResultIR { success: true, .. } => Err(failed(
                "the atom reported success without a result".to_string(),
            )),
            ProcedureCallResultIR { success: false, .. } => Err(failed(
                "the atom reported a failure without an error".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plays the TypeScript side: answers each call with `reply(id)`.
    fn serve(bridge: &ProcedureBridge, reply: fn(u64) -> Option<ProcedureCallResultIR>) {
        let bridge = bridge.clone();
        tokio::spawn(async move {
            loop {
                let call = bridge.procedure_rx.lock().await.recv().await.unwrap();
                if let Some(result) = reply(call.id) {
                    bridge.complete(result);
                }
            }
        });
    }

    fn ok(id: u64, inner: Option<ProcedureCallResultInner>) -> Option<ProcedureCallResultIR> {
        Some(ProcedureCallResultIR {
            id,
            success: true,
            inner,
            error: None,
        })
    }

    #[tokio::test]
    async fn call_returns_the_matching_result() {
        let bridge = ProcedureBridge::new();
        serve(&bridge, |id| {
            ok(
                id,
                Some(ProcedureCallResultInner::String(format!("name {id}"))),
            )
        });
        let (a, b) = tokio::join!(
            bridge.call(ProcedureCallInner::GetName),
            bridge.call(ProcedureCallInner::GetName)
        );
        let a: String = a.unwrap().try_into().unwrap();
        let b: String = b.unwrap().try_into().unwrap();
        assert_ne!(a, b);
        assert!(lock_pending(&bridge.pending).is_empty());
    }

    #[tokio::test]
    async fn broken_reply_contract_is_an_error() {
        let bridge = ProcedureBridge::new();
        serve(&bridge, |id| {
            if id == 0 {
                ok(id, None)
            } else {
                Some(ProcedureCallResultIR {
                    id,
                    success: false,
                    inner: None,
                    error: None,
                })
            }
        });
        assert!(bridge.call(ProcedureCallInner::GetName).await.is_err());
        assert!(bridge.call(ProcedureCallInner::GetName).await.is_err());
    }

    #[tokio::test]
    async fn call_fails_when_the_atom_exits() {
        let bridge = ProcedureBridge::new();
        // never answers
        serve(&bridge, |_| None);
        let (exit_tx, exit_rx) = oneshot::channel::<()>();
        bridge.close_on_exit(async move {
            let _ = exit_rx.await;
            Ok(ExitStatus::Killed { time: 0 })
        });
        let call = tokio::spawn({
            let bridge = bridge.clone();
            async move { bridge.call(ProcedureCallInner::GetState).await }
        });
        // the atom exits while the call waits for its answer
        crate::test_util::wait_until("the call is pending", || {
            let pending = !lock_pending(&bridge.pending).is_empty();
            async move { pending }
        })
        .await;
        exit_tx.send(()).unwrap();
        let result = tokio::time::timeout(Duration::from_secs(5), call)
            .await
            .expect("call did not fail after the atom exited")
            .unwrap();
        assert!(format!("{:#}", result.unwrap_err().source).contains("killed"));
        // later calls fail at once, and so does waiting for readiness
        assert!(bridge.call(ProcedureCallInner::GetState).await.is_err());
        assert!(bridge.wait_ready().await.is_err());
    }

    #[tokio::test]
    async fn call_times_out_and_a_late_result_is_ignored() {
        let bridge = ProcedureBridge::new();
        serve(&bridge, |_| None);
        let result = bridge
            .call_with_timeout(ProcedureCallInner::GetName, Some(Duration::from_millis(50)))
            .await;
        assert!(result.is_err());
        assert!(lock_pending(&bridge.pending).is_empty());
        // the atom answering after the timeout must not panic
        bridge.complete(ProcedureCallResultIR {
            id: 0,
            success: true,
            inner: Some(ProcedureCallResultInner::Void),
            error: None,
        });
    }

    #[tokio::test]
    async fn wait_ready_resolves_once_the_atom_is_ready() {
        let bridge = ProcedureBridge::new();
        let waiter = tokio::spawn({
            let bridge = bridge.clone();
            async move { bridge.wait_ready().await }
        });
        // let the waiter start waiting before the atom becomes ready
        tokio::task::yield_now().await;
        bridge.state.send_replace(BridgeState::Ready);
        waiter.await.unwrap().unwrap();
    }
}
