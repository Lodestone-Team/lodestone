use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Arc;

use color_eyre::eyre::eyre;
use deno_core::anyhow::anyhow;
use deno_core::{anyhow, op, OpState};
use enum_kinds::EnumKind;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::info;
use ts_rs::TS;

use crate::error::{Error, ErrorKind};
use crate::events::{CausedBy, Event};

use crate::implementations::generic::player::GenericPlayer;
use crate::implementations::generic::GenericInstance;

use crate::traits::t_configurable::manifest::{
    ConfigurableManifest, ConfigurableValue, ManifestValue, SetupManifest,
};
use crate::traits::t_configurable::{Game, TConfigurable};
use crate::traits::t_player::Player;
use crate::traits::t_server::State;
use crate::types::DotLodestoneConfig;
use crate::MonitorReport;

#[derive(Debug, Clone, Serialize, Deserialize, TS, EnumKind)]
#[serde(tag = "type")]
// #[ts(export_to = "src/implementations/generic/js/main/libs/bindings/ProcedureCallInner.ts")]
// #[ts(export)]
#[enum_kind(ProcedureCallKind, derive(Serialize, Deserialize, TS))]
pub enum ProcedureCallInner {
    SetupInstance {
        dot_lodestone_config: DotLodestoneConfig,
        setup_value: ManifestValue,
        path: PathBuf,
    },
    RestoreInstance {
        dot_lodestone_config: DotLodestoneConfig,
        path: PathBuf,
    },
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
    let _ = ProcedureCallKind::export();
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
    procedure_call_kind: ProcedureCallKind,
    /// MUST be None if success is false
    /// MUST be Some if success is true
    inner: Option<ProcedureCallResultInner>,
    /// MUST be None if success is true
    /// MUST be Some if success is false
    error: Option<ErrorIR>,
}

#[op]
async fn on_procedure(state: Rc<RefCell<OpState>>) -> Result<ProcedureCall, anyhow::Error> {
    let bridge = state.borrow().borrow::<ProcedureBridge>().clone();
    let mut rx = bridge.procedure_tx.lock().await.subscribe();
    rx.recv()
        .await
        .map_err(|_| anyhow!("ProcedureBridge::on_procedure: procedure_tx closed"))
}

#[op]
fn proc_bridge_ready(state: Rc<RefCell<OpState>>) -> Result<String, anyhow::Error> {
    let bridge = state.borrow().borrow::<ProcedureBridge>().clone();
    // if already ready, return error
    if bridge.ready.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(anyhow!("ProcedureBridge::proc_bridge_ready: already ready"));
    }
    bridge
        .ready
        .store(true, std::sync::atomic::Ordering::SeqCst);
    Ok("".to_string())
}

#[op]
fn emit_result(
    state: Rc<RefCell<OpState>>,
    result: ProcedureCallResultIR,
) -> Result<(), anyhow::Error> {
    let bridge = state.borrow().borrow::<ProcedureBridge>().clone();
    bridge
        .procedure_result_tx
        .send(result)
        .map_err(|_| anyhow!("ProcedureBridge::emit_result: procedure_result_tx closed"))?;
    Ok(())
}

#[op]
async fn emit_console_out(state: Rc<RefCell<OpState>>, out: String) -> Result<(), anyhow::Error> {
    let instance = state.borrow().borrow::<GenericInstance>().clone();
    instance
        .event_broadcaster
        .as_ref()
        .send(Event::new_instance_output(
            instance.dot_lodestone_config.uuid().clone(),
            instance.name().await,
            out,
        ))
        .map_err(|_| {
            anyhow!("ProcedureBridge::emit_console_out: global_event_broadcast_tx closed")
        })?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct ProcedureBridge {
    ready: Arc<AtomicBool>,
    procedure_call_id: Arc<AtomicU64>,
    pub procedure_tx: Arc<Mutex<tokio::sync::broadcast::Sender<ProcedureCall>>>,
    pub procedure_result_tx: tokio::sync::broadcast::Sender<ProcedureCallResultIR>,
}

impl ProcedureBridge {
    pub fn new() -> Self {
        Self {
            ready: Arc::new(AtomicBool::new(false)),
            procedure_call_id: Arc::new(AtomicU64::new(0)),
            procedure_tx: Arc::new(Mutex::new(tokio::sync::broadcast::channel(100).0)),
            procedure_result_tx: tokio::sync::broadcast::channel(100).0,
        }
    }

    pub async fn call(&self, inner: ProcedureCallInner) -> Result<ProcedureCallResultInner, Error> {
        // wait until TS side is ready
        if !self.ready.load(std::sync::atomic::Ordering::SeqCst) {
            // 5 retries with exponential backoff
            let mut flag = false;
            for i in 0..5 {
                info!(
                    "ProcedureBridge::call: TS side not ready, retrying in {}ms",
                    2u64.pow(i) * 100
                );
                tokio::time::sleep(std::time::Duration::from_millis(2u64.pow(i) * 100)).await;
                if self.ready.load(std::sync::atomic::Ordering::SeqCst) {
                    flag = true;
                    break;
                }
            }
            if !flag {
                return Err(eyre!(
                    "ProcedureBridge::call: TS side not ready, macro most likely crashed"
                )
                .into());
            }
        }

        let id = self
            .procedure_call_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let mut rx = self.procedure_result_tx.subscribe();
        self.procedure_tx
            .lock()
            .await
            .send(ProcedureCall { id, inner })
            .unwrap();
        loop {
            match rx.recv().await {
                Ok(result) => {
                    if result.id == id {
                        return match result.success {
                            true => Ok(result.inner.unwrap()),
                            false => Err(result.error.unwrap().into()),
                        };
                    }
                }
                Err(_) => {
                    Err(eyre!("ProcedureBridge::call: procedure_result_tx closed"))?;
                    unreachable!()
                }
            }
        }
    }
}
