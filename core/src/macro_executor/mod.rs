//! Runs macros, instance scripts and atom instances.
//!
//! With the `deno` feature (the default) they run on the Deno runtime, see [`deno`]. Without
//! it [`unavailable`] provides a [`MacroExecutor`] with the same API that refuses to start
//! anything, so the code that holds an executor needs no feature checks of its own.

use std::{fmt::Display, future::Future, pin::Pin};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{error::Error, traits::t_macro::ExitStatus};

#[cfg(feature = "deno")]
mod deno;
#[cfg(feature = "deno")]
pub use deno::*;

#[cfg(not(feature = "deno"))]
mod unavailable;
#[cfg(not(feature = "deno"))]
pub use unavailable::*;

#[derive(Copy, Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash, TS)]
#[serde(transparent)]
#[ts(export)]
pub struct MacroPID(pub usize); // todo remove pub

impl From<MacroPID> for usize {
    fn from(uid: MacroPID) -> Self {
        uid.0
    }
}

impl From<&MacroPID> for usize {
    fn from(uid: &MacroPID) -> Self {
        uid.0
    }
}

impl AsRef<usize> for MacroPID {
    fn as_ref(&self) -> &usize {
        &self.0
    }
}

impl Display for MacroPID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MacroPID({})", self.0)
    }
}

pub struct SpawnResult {
    pub macro_pid: MacroPID,
    pub detach_future: Pin<Box<dyn Future<Output = ()> + Send>>,
    pub exit_future: Pin<Box<dyn Future<Output = Result<ExitStatus, Error>> + Send>>,
}
