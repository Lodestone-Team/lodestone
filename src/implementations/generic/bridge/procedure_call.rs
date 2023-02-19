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
use ts_rs::TS;

use crate::error::{Error, ErrorKind};
use crate::events::{CausedBy, Event};

use crate::implementations::generic::player::GenericPlayer;
use crate::implementations::generic::{GenericInstance, SetupConfig};

use crate::traits::t_server::State;
use crate::MonitorReport;

#[derive(Debug, Clone, Serialize, Deserialize, TS, EnumKind)]
#[serde(tag = "type")]
#[ts(export)]
#[enum_kind(ProcedureCallKind, derive(Serialize, Deserialize, TS))]
pub enum ProcedureCallInner {
    SetupInstance {
        config: SetupConfig,
        path: PathBuf,
    },
    StartInstance {
        caused_by: CausedBy,
    },
    StopInstance {
        caused_by: CausedBy,
    },
    RestartInstance {
        caused_by: CausedBy,
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
    GetPlayerCount,
    GetPlayerList,
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
#[ts(export)]
pub enum ProcedureCallResultInner {
    String(String),
    Monitor(MonitorReport),
    State(State),
    Num(u32),
    Player(HashSet<GenericPlayer>),
    Void,
}

#[derive(Debug, Clone, TS, Deserialize)]
#[ts(export)]
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
#[ts(export)]
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
#[ts(export)]
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

pub struct ProcedureCallResult {
    id: u64,
    result: Result<ProcedureCallResultInner, Error>,
}

impl From<ProcedureCallResultIR> for ProcedureCallResult {
    fn from(ir: ProcedureCallResultIR) -> Self {
        Self {
            id: ir.id,
            result: match ir.success {
                true => Ok(ir.inner.unwrap()),
                false => Err(ir.error.unwrap().into()),
            },
        }
    }
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
fn emit_console_out(state: Rc<RefCell<OpState>>, out: String) -> Result<(), anyhow::Error> {
    let instance = state.borrow().borrow::<GenericInstance>().clone();
    instance
        .global_event_broadcaster
        .send(crate::events::Event::new_instance_output(
            instance.config.uuid,
            instance.config.name,
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
    global_event_broadcast_tx: tokio::sync::broadcast::Sender<Event>,
    procedure_call_id: Arc<AtomicU64>,
    pub procedure_tx: Arc<Mutex<tokio::sync::broadcast::Sender<ProcedureCall>>>,
    pub procedure_result_tx: tokio::sync::broadcast::Sender<ProcedureCallResultIR>,
}

impl ProcedureBridge {
    pub fn new(global_event_broadcast_tx: tokio::sync::broadcast::Sender<Event>) -> Self {
        Self {
            ready: Arc::new(AtomicBool::new(false)),
            global_event_broadcast_tx,
            procedure_call_id: Arc::new(AtomicU64::new(0)),
            procedure_tx: Arc::new(Mutex::new(tokio::sync::broadcast::channel(100).0)),
            procedure_result_tx: tokio::sync::broadcast::channel(100).0,
        }
    }

    pub async fn call(&self, inner: ProcedureCallInner) -> Result<ProcedureCallResultInner, Error> {
        let id = self
            .procedure_call_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        // wait until TS side is ready
        while !self.ready.load(std::sync::atomic::Ordering::SeqCst) {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

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
