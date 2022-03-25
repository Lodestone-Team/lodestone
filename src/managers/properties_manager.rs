use std::path::PathBuf;
use std::{collections::HashMap, path::Path, fs::File, io::BufReader};
use std::io::{prelude::*, LineWriter};
use std::result::Result;
pub struct PropertiesManager {
    /// (line number, current value, default value)
    properties : HashMap<String, (i32, String, String)>,
    path_to_properties : PathBuf
}

impl PropertiesManager {
    pub fn new(path : PathBuf) -> Result<PropertiesManager, String> {
        if !path.exists() {
            return Err("server.properties not found".to_string());
        }
        let file = File::open(path.clone()).unwrap();
        let buf_reader = BufReader::new(file);
        let mut properties = HashMap::new();
        let line_num = 0;
        for line in buf_reader.lines() {
            let res: Vec<String> = line.unwrap().split("=").map(|s| s.to_string()).collect();
            properties.insert(res.get(0).unwrap().clone(), (line_num, res.get(1).unwrap().clone(), res.get(1).unwrap().clone()));
        }
        Ok(PropertiesManager {
           properties,
           path_to_properties : path,
        })
    }

    pub fn edit_field(&mut self, field : String, value : String) -> Result<(), String> {
        let line = self.properties.get_mut(&field).ok_or("property does not exist".to_string())?;
        line.1 = value;
        Ok(())
    }

    pub fn get_field(&mut self, field : String) -> Result<String, String> {
        let line = self.properties.get_mut(&field).ok_or("property does not exist".to_string())?;
        Ok(line.2.clone())
    }

    /// flush the internal properties buffer to file,
    /// this function is expensive so repeated calling is discouraged.
    pub fn write_to_file(self) -> Result<(), String> {
        let file = File::create(self.path_to_properties).map_err(|e| e.to_string())?;
        let mut line_writer = LineWriter::new(file);
        for entry in self.properties {
            line_writer.write_all(format!("{}={}\n", entry.0, entry.1.1).as_bytes()).unwrap();
        }
        line_writer.flush().unwrap();
        Ok(())
    }

}

