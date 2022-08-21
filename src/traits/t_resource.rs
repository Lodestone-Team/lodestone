use std::sync::{atomic::AtomicU64, Arc, RwLock};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json;

use crate::util::DownloadProgress;

use super::MaybeUnsupported;

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

    fn load(&mut self, resource: &str) -> MaybeUnsupported<Result<(), super::Error>> {
        MaybeUnsupported::Unsupported
    }

    fn unload(&mut self, resource: &str) -> MaybeUnsupported<Result<(), super::Error>> {
        MaybeUnsupported::Unsupported
    }

    async fn download_file(
        &mut self,
        override_name: Option<&str>,
        resource_type: ResourceType,
    ) -> MaybeUnsupported<Result<DownloadProgress, super::Error>> {
        MaybeUnsupported::Unsupported
    }

    fn delete(&mut self, resource: &str) -> MaybeUnsupported<Result<(), super::Error>> {
        MaybeUnsupported::Unsupported
    }
}
