use crate::traits::t_server::{TServer, State};

use crate::traits::{Error, MaybeUnsupported};

use super::Instance;

impl TServer for Instance {
    fn start(&mut self) -> Result<(), Error> {
        unimplemented!()
    }
    fn stop(&mut self) -> Result<(), Error> {
        unimplemented!()
    }
    fn state(&self) -> State {
        self.state.clone()
    }

    fn get_stdout(&self) -> Box<dyn Iterator<Item = String>> {
        unimplemented!()
    }

    fn send_command(&self, command: &str) -> MaybeUnsupported<Result<(), Error>> {
        todo!()
    }
}