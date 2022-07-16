use std::iter::Iterator;

pub enum State {
    Starting,
    Running,
    Stopping,
    Stopped,
    Error,
}

pub trait TServer {
    fn start(&mut self) -> Result<(), super::Error>;
    fn stop(&mut self) -> Result<(), super::Error>;
    fn state(&self) -> State;
    fn send_stdin(&self, command: &str) -> Result<(), super::Error>;
    fn get_stdout(&self) -> Box<dyn Iterator<Item = String>>;
}
