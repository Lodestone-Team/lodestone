use async_trait::async_trait;

use crate::traits::GameInstance;
#[async_trait]
#[enum_dispatch::enum_dispatch]
pub trait TMacro {
    async fn get_macro_list(&self) -> Vec<String>;
    async fn delete_macro(&mut self, name: &str) -> Result<(), super::Error>;
    async fn create_macro(&mut self, name: &str, content: &str) -> Result<(), super::Error>;
    async fn run_macro(
        &mut self,
        _name: &str,
        _args: Vec<String>,
        _executor: Option<&str>,
        _is_in_game: bool,
    ) -> Result<(), super::Error> {
        Err(super::Error {
            inner: super::ErrorInner::UnsupportedOperation,
            detail: "This instance does not support running macros".to_string(),
        })
    }
}
