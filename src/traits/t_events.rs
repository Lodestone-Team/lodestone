use rocket::serde::json::serde_json;

use super::MaybeUnsupported;

pub trait TEventProcessing {
    fn event_stream(&self) -> MaybeUnsupported<Box<dyn Iterator<Item = String>>> {
        MaybeUnsupported::Unsupported
    }
    fn notify_event(&self, event: serde_json::Value) -> MaybeUnsupported<Result<(), super::Error>> {
        MaybeUnsupported::Unsupported
    }
}