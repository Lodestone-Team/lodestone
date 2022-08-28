use crate::traits::t_events::TEventProcessing;

use super::Instance;
impl TEventProcessing for Instance {


    fn notify_event(&self, _event: String) -> crate::traits::MaybeUnsupported<Result<(), crate::traits::Error>> {
        todo!()
    }

    fn subscribe(&self, _event : String) -> crate::traits::MaybeUnsupported<tokio::sync::broadcast::Receiver<String>> {
        todo!()
    }
}
