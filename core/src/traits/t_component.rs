use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use ts_rs::TS;

use crate::{error::Error, traits::GameInstance};

use crate::traits::GenericInstance;
use crate::traits::MinecraftInstance;

/// Component as part of any instance that can be configured
///
/// A component can be any type of file or directory that
/// should be managed by the instance (in the case of
/// cross-checking versioning, how to handle updates, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Component {
    component_id: String,
    name: String,
    description: Option<String>,
    version: Option<String>,
    updateable: bool,
}

#[async_trait]
#[enum_dispatch::enum_dispatch]
pub trait TComponent {
    async fn get_component_list(&self) -> Result<Vec<Component>, Error>;

    /// Creates a new component and adds to the configuration
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the component to add
    /// * `content` - The content to add
    async fn create_component(
        &self,
        name: &str,
        description: Option<&str>,
        content: &str,
    ) -> Result<(), Error>;

    /// Updates a component in the configuration
    ///
    /// Returns `Ok()` on success, otherwise an Error.
    ///
    /// # Arguments
    ///
    /// * `id` - The index of the component to update (searches the list of components for the
    /// corresponding index)
    /// * `content` - How the component should be updated
    async fn update_component(&self, index: &str, content: &str) -> Result<(), Error>;

    /// Deletes a component from the configuration
    ///
    /// # Arguments
    ///
    /// * `index` - The index of the component to delete
    async fn delete_component(&self, index: &str) -> Result<(), Error>;
}
