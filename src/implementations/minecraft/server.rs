use crate::traits::t_server::{State, TServer};

use crate::traits::{Error, ErrorInner, MaybeUnsupported};

use super::Instance;
use rocket::tokio;

impl TServer for Instance {
    fn start(&mut self) -> Result<(), Error> {
        unimplemented!()
    }
    fn stop(&mut self) -> Result<(), Error> {
        unimplemented!()
    }
    fn state(&self) -> State {
        self.state.read().unwrap().clone()
    }

    fn get_stdout(&self) -> Result<tokio::sync::broadcast::Receiver<std::string::String>, Error> {
        Ok(self.stdin_broadcast.as_ref().ok_or(Error {
                    inner: ErrorInner::InstanceStopped,
                    detail: "Stdout is not opened".to_string(),
                })?.subscribe())
    }

    fn send_command(&self, command: &str) -> MaybeUnsupported<Result<(), Error>> {
        todo!()
    }
}
