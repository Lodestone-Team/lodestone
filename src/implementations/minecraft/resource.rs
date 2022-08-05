use crate::traits::t_resource::TResourceManagement;

use super::Instance;

impl TResourceManagement for Instance {
    fn list(&self) -> Vec<serde_json::Value> {
        todo!()
    }

    fn load(
        &mut self,
        resource: &str,
    ) -> crate::traits::MaybeUnsupported<Result<(), crate::traits::Error>> {
        todo!()
    }

    fn unload(
        &mut self,
        resource: &str,
    ) -> crate::traits::MaybeUnsupported<Result<(), crate::traits::Error>> {
        todo!()
    }

    fn delete(
        &mut self,
        resource: &str,
    ) -> crate::traits::MaybeUnsupported<Result<(), crate::traits::Error>> {
        todo!()
    }
}
