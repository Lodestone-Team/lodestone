use std::{collections::HashMap, path::Path, fs::File, io::BufReader};
use std::io::{self, prelude::*};
struct PropertiesManager {
    properties : HashMap<String, String>,
}

impl PropertiesManager {
    pub fn new(path : String) -> Result<PropertiesManager, String> {
        if !Path::new(path.as_str()).exists() {
            return Err("server.properties not found".to_string());
        }
        let file = File::open(path.as_str()).unwrap();
        let buf_reader = BufReader::new(file);
        let mut properties = HashMap::new();
        for line in buf_reader.lines() {
            let res: Vec<String> = line.unwrap().split("=").map(|s| s.to_string()).collect();
            properties.insert(res.get(0).unwrap().clone(), res.get(1).unwrap().clone());
        }
        Ok(PropertiesManager {
           properties
        })
    }

    pub fn edit_property(&mut self, property : String) -> Result<(), String> {
        *self.properties.get_mut(&property).ok_or("property does not exist".to_string()).unwrap() = property.clone();
        Ok(())
    }

}

