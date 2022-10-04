use async_trait::async_trait;

use crate::traits::t_resource::TResourceManagement;

use super::Instance;

#[async_trait]
impl TResourceManagement for Instance {
    async fn list(&self) -> Vec<serde_json::Value> {
        todo!()
    }

    async fn load(
        &mut self,
        _resource: &str,
    ) -> crate::traits::MaybeUnsupported<Result<(), crate::traits::Error>> {
        todo!()
    }

    async fn unload(
        &mut self,
        _resource: &str,
    ) -> crate::traits::MaybeUnsupported<Result<(), crate::traits::Error>> {
        todo!()
    }

    async fn delete(
        &mut self,
        _resource: &str,
    ) -> crate::traits::MaybeUnsupported<Result<(), crate::traits::Error>> {
        todo!()
    }
}
