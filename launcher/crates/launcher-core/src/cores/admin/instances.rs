//! Instances: the list, starting and stopping. Work package A adds creating (games,
//! setup manifests, modpacks), deleting, renaming and the Overview's routes here
//! (see `launcher/docs/host-parity.md`).

use http_client::Method;

use super::{AdminClient, models::InstanceInfo, segment};
use crate::cores::CoreError;

/// What the power menu does to an instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Power {
    Start,
    Stop,
    Restart,
    Kill,
}

impl Power {
    fn route(self) -> &'static str {
        match self {
            Power::Start => "start",
            Power::Stop => "stop",
            Power::Restart => "restart",
            Power::Kill => "kill",
        }
    }
}

impl AdminClient {
    /// `GET /instance/list`: the instances the user may see, in the core's order.
    pub async fn instances(&self) -> Result<Vec<InstanceInfo>, CoreError> {
        self.get("instance/list").await
    }

    /// `PUT /instance/{uuid}/{start|stop|restart|kill}`. The new state arrives as a
    /// `StateTransition` event.
    pub async fn power(&self, uuid: &str, power: Power) -> Result<(), CoreError> {
        self.call(
            Method::PUT,
            &format!("instance/{}/{}", segment(uuid), power.route()),
            None::<()>,
        )
        .await
    }
}
