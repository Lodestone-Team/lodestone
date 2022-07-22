use std::{
    path::PathBuf,
    sync::{atomic::AtomicU64, Arc, Mutex},
};

use rocket::serde::json::serde_json;
use serde::Serialize;

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

pub struct DownloadReport {
    pub total: AtomicU64,
    pub downloaded: AtomicU64,
    pub download_name: Arc<Mutex<String>>,
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

    async fn download_resource(
        &mut self,
        override_name: Option<&str>,
        resource_type: ResourceType,
    ) -> MaybeUnsupported<Result<DownloadReport, super::Error>> {
        MaybeUnsupported::Unsupported
    }

    fn delete(&mut self, resource: &str) -> MaybeUnsupported<Result<(), super::Error>> {
        MaybeUnsupported::Unsupported
    }
}
