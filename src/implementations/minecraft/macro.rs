use crate::traits::t_macro::TMacro;

use super::Instance;

impl TMacro for Instance {
    fn get_macro_list(&self) -> Vec<String> {
        todo!()
    }

    fn delete_macro(&mut self, _name: &str) -> Result<(), crate::traits::Error> {
        todo!()
    }

    fn create_macro(&mut self, _name: &str, _content: &str) -> Result<(), crate::traits::Error> {
        todo!()
    }

    fn run_macro(
        &mut self,
        _name: &str,
        _args: Vec<String>,
        _executor: Option<&str>,
    ) -> crate::traits::MaybeUnsupported<Result<String, crate::traits::Error>> {
        todo!()
    }
}