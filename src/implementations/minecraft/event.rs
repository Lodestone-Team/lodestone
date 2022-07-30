use crate::traits::t_events::TEventProcessing;

use super::Instance;
impl TEventProcessing for Instance {
    fn event_stream(&self) -> crate::traits::MaybeUnsupported<Box<dyn Iterator<Item = String>>> {
        todo!()
    }

    fn notify_event(&self, event: rocket::serde::json::serde_json::Value) -> crate::traits::MaybeUnsupported<Result<(), crate::traits::Error>> {
        todo!()
    }
}
