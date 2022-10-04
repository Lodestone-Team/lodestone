

use async_trait::async_trait;
use serde::{Serialize};
use serde_json;



use super::{MaybeUnsupported, Unsupported};

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

#[async_trait]
pub trait TResourceManagement {
    async fn list(&self) -> Vec<serde_json::Value> where Self: Sized {
        vec![]
    }

    async fn load(&mut self, _resource: &str) -> MaybeUnsupported<Result<(), super::Error>> where Self: Sized {
        Unsupported
    }

    async fn unload(&mut self, _resource: &str) -> MaybeUnsupported<Result<(), super::Error>> where Self: Sized {
        Unsupported
    }


    async fn delete(&mut self, _resource: &str) -> MaybeUnsupported<Result<(), super::Error>> where Self: Sized {
        Unsupported
    }
}
