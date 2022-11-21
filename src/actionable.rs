use futures::Future;
use tokio::sync::broadcast::{self, error::RecvError, Sender};

struct Actionable<S: Send + Sync + Clone, A: Clone> {
    state: S,
    action_sender: Sender<A>,
}

impl<S: Send + Sync + Clone + 'static, A: Clone + Send + 'static + core::fmt::Debug>
    Actionable<S, A>
{
    pub fn new<
        F: FnMut((S, A)) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + Sync,
    >(
        state: S,
        mut action_handler: F,
    ) -> Actionable<S, A> {
        let (tx, mut rx) = broadcast::channel(16);
        tokio::task::spawn({
            let state = state.clone();
            async move {
                loop {
                    match rx.recv().await {
                        Ok(action) => {
                            action_handler((state.clone(), action)).await;
                        }
                        Err(RecvError::Closed) => break,
                        Err(RecvError::Lagged(_)) => continue,
                    }
                }
            }
        });
        Actionable {
            state,
            action_sender: tx,
        }
    }
    pub fn send(&self, action: A) {
        self.action_sender
            .send(action)
            .expect("action channel closed, this should never happen");
    }
}
