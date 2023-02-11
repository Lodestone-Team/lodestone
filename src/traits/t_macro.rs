use async_trait::async_trait;
use color_eyre::eyre::eyre;

use crate::{
    error::{Error, ErrorKind},
    events::CausedBy,
    traits::GameInstance,
};
#[async_trait]
#[enum_dispatch::enum_dispatch]
pub trait TMacro {
    async fn get_macro_list(&self) -> Vec<String>;
    async fn delete_macro(&mut self, name: &str) -> Result<(), Error>;
    async fn create_macro(&mut self, name: &str, content: &str) -> Result<(), Error>;
    async fn run_macro(
        &mut self,
        _name: &str,
        _args: Vec<String>,
        _caused_by: CausedBy,
        _is_in_game: bool,
    ) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support running macro"),
        })
    }
}
