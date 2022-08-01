use crate::traits::t_events::TEventProcessing;

use super::Instance;
impl TEventProcessing for Instance {


    fn notify_event(&self, event: String) -> crate::traits::MaybeUnsupported<Result<(), crate::traits::Error>> {
        todo!()
    }

    fn subscribe(&self, event : String) -> crate::traits::MaybeUnsupported<rocket::tokio::sync::broadcast::Receiver<String>> {
        todo!()
    }
}
