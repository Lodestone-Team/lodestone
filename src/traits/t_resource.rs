use async_trait::async_trait;
use color_eyre::eyre::eyre;
use serde::Serialize;
use serde_json;

pub enum ResourceType {
    Unknown,
    Mod,
    World,
    Executable,
    Runtime,
}

pub struct Resource<T>
where
    T: Serialize,
{
    r#mod: Vec<T>,
    world: Vec<T>,
    executable: Vec<T>,
    runtime: Vec<T>,
}

use crate::{
    error::{Error, ErrorKind},
    traits::GameInstance,
};
#[async_trait]
#[enum_dispatch::enum_dispatch]
pub trait TResourceManagement {
    async fn list(&self) -> Vec<serde_json::Value>
    where
        Self: Sized,
    {
        vec![]
    }

    async fn load(&mut self, _resource: &str) -> Result<(), Error>
    where
        Self: Sized,
    {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support loading resources"),
        })
    }

    async fn unload(&mut self, _resource: &str) -> Result<(), Error>
    where
        Self: Sized,
    {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support unloading resources"),
        })
    }

    async fn delete(&mut self, _resource: &str) -> Result<(), Error>
    where
        Self: Sized,
    {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support deleting resources"),
        })
    }
}
