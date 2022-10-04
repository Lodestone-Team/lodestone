use async_trait::async_trait;

use crate::traits::t_macro::TMacro;

use super::Instance;

#[async_trait]
impl TMacro for Instance {
    async fn get_macro_list(&self) -> Vec<String> {
        todo!()
    }

    async fn delete_macro(&mut self, _name: &str) -> Result<(), crate::traits::Error> {
        todo!()
    }

    async fn create_macro(&mut self, _name: &str, _content: &str) -> Result<(), crate::traits::Error> {
        todo!()
    }

    async fn run_macro(
        &mut self,
        _name: &str,
        _args: Vec<String>,
        _executor: Option<&str>,
    ) -> crate::traits::MaybeUnsupported<Result<String, crate::traits::Error>> {
        todo!()
    }
}