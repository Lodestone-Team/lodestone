use async_trait::async_trait;

use crate::traits::{
    t_configurable::TConfigurable,
    t_manifest::{Manifest, Operation, TManifest},
};

use super::Instance;

#[async_trait]
impl TManifest for Instance {
    async fn get_manifest(&self) -> Manifest {
        Manifest {
            supported_operations: Operation::all(),
            settings: self.settings().await.unwrap().keys().cloned().collect(),
        }
    }
}
