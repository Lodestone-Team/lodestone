
use tokio;

use super::MaybeUnsupported;

pub trait TEventProcessing {
    fn subscribe(&self, _event : String) -> MaybeUnsupported<tokio::sync::broadcast::Receiver<String>> {
        MaybeUnsupported::Unsupported
    }
    fn notify_event(&self, _event: String) -> MaybeUnsupported<Result<(), super::Error>> {
        MaybeUnsupported::Unsupported
    }
}