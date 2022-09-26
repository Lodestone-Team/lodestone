

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
    fn list(&self) -> Vec<serde_json::Value> {
        vec![]
    }

    fn load(&mut self, _resource: &str) -> MaybeUnsupported<Result<(), super::Error>> {
        Unsupported
    }

    fn unload(&mut self, _resource: &str) -> MaybeUnsupported<Result<(), super::Error>> {
        Unsupported
    }


    fn delete(&mut self, _resource: &str) -> MaybeUnsupported<Result<(), super::Error>> {
        Unsupported
    }
}
