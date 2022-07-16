use std::{collections::HashMap, path::PathBuf, sync::atomic::AtomicU64};

use serde::Serialize;

use super::MaybeUnsupported;

pub enum Resource<T>
where
    T: Serialize,
{
    Mod(Vec<T>),
    World(Vec<T>),
}

pub struct DownloadProgress {
    pub total: u64,
    pub downloaded: AtomicU64,
}
#[async_trait]
pub trait TResourceManagement {
    fn list<T>(&self) -> MaybeUnsupported<Resource<T>>
    where
        T: Serialize,
    {
        MaybeUnsupported::Unsupported
    }

    fn load(&mut self, resource: &str) -> MaybeUnsupported<(Result<(), super::Error>)> {
        MaybeUnsupported::Unsupported
    }

    fn unload(&mut self, resource: &str) -> MaybeUnsupported<(Result<(), super::Error>)> {
        MaybeUnsupported::Unsupported
    }

    async fn download_resource(
        &mut self,
        override_name: &str,
        path: PathBuf,
    ) -> MaybeUnsupported<(Result<DownloadProgress, super::Error>)> {
        MaybeUnsupported::Unsupported
    }

    fn delete(&mut self, resource: &str) -> MaybeUnsupported<(Result<(), super::Error>)> {
        MaybeUnsupported::Unsupported
    }
}
