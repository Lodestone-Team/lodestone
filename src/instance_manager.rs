use std::fs;
use std::collections::HashMap;

pub use crate::instance::*;
pub struct InstanceManager{
    instance_collection : HashMap<String, ServerInstance>,
    path : String, // must end with /
}

impl InstanceManager {
    pub fn new(path : String) -> InstanceManager {
        InstanceManager{instance_collection : HashMap::new(), path}
    }
    pub fn add(& mut self, name : String, config : Option<InstanceConfig>) -> Result<(), String> {
        let instance = ServerInstance::new(config, format!("{}{}", self.path, name), name);
        self.instance_collection.insert(instance.uuid.clone(), instance);
        fs::create_dir(self.path.as_str()).map_err(|e| e.to_string())
    }

}
