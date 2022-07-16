use super::MaybeUnsupported;

pub trait TMacro {
    fn get_macro_list(&self) -> Vec<String>;
    fn delete_macro(&mut self, name: &str) -> Result<(), super::Error>;
    fn create_macro(&mut self, name: &str, content: &str) -> Result<(), super::Error>;
    fn run_macro(
        &mut self,
        name: &str,
        args: Vec<String>,
        executor: Option<&str>,
    ) -> MaybeUnsupported<Result<String, super::Error>>;
}
