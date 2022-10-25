use async_trait::async_trait;

use super::MaybeUnsupported;

#[async_trait]
pub trait TMacro {
    async fn get_macro_list(&self) -> Vec<String>;
    async fn delete_macro(&mut self, name: &str) -> Result<(), super::Error>;
    async fn create_macro(&mut self, name: &str, content: &str) -> Result<(), super::Error>;
    async fn run_macro(
        &mut self,
        name: &str,
        args: Vec<String>,
        executor: Option<&str>,
    ) -> MaybeUnsupported<Result<String, super::Error>>;
}
