use std::path::PathBuf;
use std::{collections::HashMap, fs::File, io::BufReader};
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
            let mut properties_file = File::create(&path).map_err(|e| e.to_string())?;
            properties_file.write_all(
                b"enable-jmx-monitoring=false\nrcon.port=25575\nenable-command-block=false\ngamemode=survival\nenable-query=false\nlevel-name=world\nmotd=AMinecraftServer\nquery.port=25565\npvp=true\ndifficulty=easy\nnetwork-compression-threshold=256\nmax-tick-time=60000\nrequire-resource-pack=false\nmax-players=20\nuse-native-transport=true\nonline-mode=true\nenable-status=true\nallow-flight=false\nvbroadcast-rcon-to-ops=true\nview-distance=10\nserver-ip=\nresource-pack-prompt=\nallow-nether=true\nserver-port=25565\nenable-rcon=false\nsync-chunk-writes=true\nop-permission-level=4\nprevent-proxy-connections=false\nhide-online-players=false\nresource-pack=\nentity-broadcast-range-percentage=100\nsimulation-distance=10\nrcon.password=\nplayer-idle-timeout=0\nforce-gamemode=false\nrate-limit=0\nhardcore=false\nwhite-list=false\nbroadcast-console-to-ops=true\nspawn-npcs=true\nspawn-animals=true\nfunction-permission-level=2\ntext-filtering-config=\nspawn-monsters=true\nenforce-whitelist=false\nresource-pack-sha1=\nspawn-protection=16\nmax-world-size=29999984\n").map_err(|e| e.to_string())?;
        }
        // this unwrap is safe because we just created the file
        let file = File::open(&path).unwrap();
        let buf_reader = BufReader::new(file);
        let mut properties = HashMap::new();
        let line_num = 0;
        for line in buf_reader.lines() {
            // if a line has a comment, remove it
            let line = line.unwrap();
            let line = line.split("#").next().unwrap();
            if line.is_empty() {
                continue;
            }
            let res: Vec<String> = line.split("=").map(|s| s.to_string()).collect();
            properties.insert(res.get(0).unwrap().clone(), (line_num, res.get(1).unwrap().clone(), res.get(1).unwrap().clone()));
        }
        Ok(PropertiesManager {
           properties,
           path_to_properties : path,
        })
    }

    pub fn edit_field(&mut self, field : &String, value : String) -> Result<(), String> {
        let line = self.properties.get_mut(field).ok_or("property does not exist".to_string())?;
        line.1 = value;
        Ok(())
    }

    pub fn get_field(&self, field : &String) -> Result<String, String> {
        let line = self.properties.get(field).ok_or("property does not exist".to_string())?;
        Ok(line.2.clone())
    }

    /// flush the internal properties buffer to file,
    /// this function is expensive so repeated calling is discouraged.
    pub fn write_to_file(&self) -> Result<(), String> {
        let file = File::create(&self.path_to_properties).map_err(|e| e.to_string())?;
        let mut line_writer = LineWriter::new(file);
        for entry in &self.properties {
            line_writer.write_all(format!("{}={}\n", entry.0, entry.1.1).as_bytes()).unwrap();
        }
        line_writer.flush().unwrap();
        Ok(())
    }

}

