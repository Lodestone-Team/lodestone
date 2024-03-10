use crate::error::{Error, ErrorKind};

use async_trait::async_trait;
use color_eyre::eyre::eyre;

use crate::traits::t_component::{Component, TComponent};

use super::GenericInstance;

#[async_trait]
impl TComponent for GenericInstance {
    async fn get_component_list(&self) -> Result<Vec<Component>, Error> {
        Ok(vec![])
    }

    async fn create_component(
        &self,
        name: &str,
        description: Option<&str>,
        content: &str,
    ) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("Cannot add component"),
        })
    }

    async fn update_component(&self, index: &str, content: &str) -> Result<(), Error> {
        Ok(())
    }
    async fn delete_component(&self, index: &str) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("Cannot delete component"),
        })
    }
}
