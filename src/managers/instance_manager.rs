use crate::managers::server_instance::{InstanceConfig, ServerInstance};
use crate::properties_manager::PropertiesManager;
use crate::util::db_util::mongo_schema::*;
use crate::util::{self};
use crate::MyManagedState;
use rocket::fairing::Result;
use rocket::State;
use rocket::serde::json::serde_json::{from_str, to_string_pretty};
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::{fs, fs::File};

pub struct InstanceManager {
    instance_collection: HashMap<String, ServerInstance>,
    taken_ports: HashSet<u32>,
    /// path to lodestone installation directory
    path: PathBuf,
}

// TODO: DB IO
// TODO : should prob change parameter String to &str
impl InstanceManager {
    pub fn new(path: PathBuf) -> Result<InstanceManager, String> {
        let path_to_config = path.join(".lodestone/");
        fs::create_dir_all(path_to_config.as_path()).map_err(|e| e.to_string())?;
        let path_to_instances = path.join("instances/");
        fs::create_dir_all(path_to_instances.as_path()).map_err(|e| e.to_string())?;
        if !Path::exists(path_to_config.join("server.properties").as_path()) {
            let mut properties_file =
                File::create(path_to_config.join("server.properties")).unwrap();
            properties_file.write_all(
        b"enable-jmx-monitoring=false\nrcon.port=25575\nenable-command-block=false\ngamemode=survival\nenable-query=false\nlevel-name=world\nmotd=AMinecraftServer\nquery.port=25565\npvp=true\ndifficulty=easy\nnetwork-compression-threshold=256\nmax-tick-time=60000\nrequire-resource-pack=false\nmax-players=20\nuse-native-transport=true\nonline-mode=true\nenable-status=true\nallow-flight=false\nvbroadcast-rcon-to-ops=true\nview-distance=10\nserver-ip=\nresource-pack-prompt=\nallow-nether=true\nserver-port=25565\nenable-rcon=false\nsync-chunk-writes=true\nop-permission-level=4\nprevent-proxy-connections=false\nhide-online-players=false\nresource-pack=\nentity-broadcast-range-percentage=100\nsimulation-distance=10\nrcon.password=\nplayer-idle-timeout=0\nforce-gamemode=false\nrate-limit=0\nhardcore=false\nwhite-list=false\nbroadcast-console-to-ops=true\nspawn-npcs=true\nspawn-animals=true\nfunction-permission-level=2\ntext-filtering-config=\nspawn-monsters=true\nenforce-whitelist=false\nresource-pack-sha1=\nspawn-protection=16\nmax-world-size=29999984\n").unwrap();
        }
        let mut instance_collection: HashMap<String, ServerInstance> = HashMap::new();
        let mut taken_ports = HashSet::new();
        // get all directories in instances folder
        for entry in fs::read_dir(path_to_instances.as_path()).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                if entry.path().join(".lodestone_config").exists() {
                    // open config file
                    let mut config_file = File::open(entry.path().join(".lodestone_config")).unwrap();
                    // read config file
                    let mut config_file_contents = String::new();
                    config_file.read_to_string(&mut config_file_contents).unwrap();
                    let instance_config : InstanceConfig = from_str(str::replace(&config_file_contents, "\r\n", "\n").as_str()).unwrap();
                    instance_collection.insert(instance_config.uuid.clone().unwrap(), ServerInstance::new(&instance_config, path.join("instances").join(instance_config.name.clone())));
                    taken_ports.insert(instance_config.port.unwrap());
                }
            }
        }

        Ok(InstanceManager {
            instance_collection,
            path,
            taken_ports,
        })
    }

    pub fn list_instances(&self) -> Vec<InstanceConfig> {
        let mut instances: Vec<InstanceConfig> = Vec::new();
        for (_, instance) in self.instance_collection.iter() {
            instances.push(instance.get_instance_config().clone());
        }
        instances
    }

    pub async fn create_instance(
        &mut self,
        mut config: InstanceConfig,
        state: &State<MyManagedState>,
    ) -> Result<String, String> {
        config.name = sanitize_filename::sanitize(config.name);

        config.uuid.clone().ok_or("uuid not found")?;
        if !config.uuid.clone().unwrap().contains("-") {
            return Err("uuid format error".to_string());
        }
        if self.check_if_name_exists(&config.name) {
            return Err(format!("{} already exists as an instance", &config.name));
        }
        //check if uuid already exists in instance_collection
        if self
            .instance_collection
            .contains_key(&config.uuid.clone().unwrap())
        {
            return Err(format!(
                "{} already exists as an instance",
                &config.uuid.unwrap()
            ));
        }

        fs::create_dir_all("tmp").map_err(|_| "couldn't create tmp folder".to_string())?;
        util::download_file(
            &config.url.clone().unwrap(),
            format!("tmp/{}", &config.uuid.clone().unwrap()).as_str(),
            state,
            config.uuid.clone().unwrap().as_str(),
        )
        .await?; // TODO: get rid of await

        let path_to_instance = self.path.join("instances").join(config.name.clone());
        fs::create_dir_all(path_to_instance.clone()).map_err(|e| e.to_string())?;
        fs::rename(
            self.path.join("tmp").join(config.uuid.clone().unwrap()),
            path_to_instance.join("server.jar"),
        )
        .map_err(|_| "failed to copy file".to_string())?;
        let path_to_eula = path_to_instance.join("eula.txt");
        let mut eula_file =
            File::create(path_to_eula).map_err(|_| "failed to create eula.txt".to_string())?;
        eula_file
            .write_all(b"#generated by Lodestone\neula=true\n")
            .map_err(|_| "failed to write to eula,txt".to_string())?;

        let path_to_properties = path_to_instance.join("server.properties");
        fs::copy(
            ".lodestone/server.properties",
            path_to_properties.clone(),
        )
        .unwrap();
        if let None = config.port {
            for port in 25565..26000 {
                if !self.taken_ports.contains(&port) {
                    self.taken_ports.insert(port);
                    println!("using port {}", port);
                    let mut pm = PropertiesManager::new(path_to_properties).unwrap();
                    pm.edit_field(&"server-port".to_string(), port.to_string())
                        .unwrap();
                    pm.write_to_file().unwrap();
                    config.port = Some(port);
                    break;
                }
            }
        }
        let instance = ServerInstance::new(&config, path_to_instance.clone());
        self.instance_collection
            .insert(config.uuid.clone().unwrap(), instance);

        // create the config file
        let mut config_file = File::create(path_to_instance.join(".lodestone_config"))
            .map_err(|_| "failed to create config file".to_string())?;
        // serialize the config and write it to the file
        let config_string = to_string_pretty(&config).unwrap();
        config_file
            .write_all(config_string.as_bytes())
            .map_err(|_| "failed to write to config file".to_string())?;
        

        Ok(config.uuid.unwrap())
    }

    pub fn get_status(&self, uuid: &String) -> Result<String, String> {
        let instance = self
            .instance_collection
            .get(uuid)
            .ok_or("instance does not exist".to_string())?;
        Ok(instance.get_status().to_string())
    }

    // TODO: basically drop database
    pub fn delete_instance(&mut self, uuid: &String) -> Result<(), String> {
        use crate::server_instance::Status;

        match self
            .instance_collection
            .get(uuid)
            .ok_or("instance does not exist".to_string())?
            .get_status()
        {
            Status::Stopped => {
                let name = self.instance_collection.get(uuid).unwrap().get_name();
                fs::remove_dir_all(format!("instances/{}", name)).map_err(|e| e.to_string())?;

                self.taken_ports
                    .remove(&self.instance_collection.get(uuid).unwrap().get_port());
                self.instance_collection.remove(uuid);
                return Ok(());
            }
            _ => return Err("instance is running".to_string()),
        }
    }

    pub fn clone_instance(&mut self, uuid: &String) -> Result<(), String> {
        for pair in &self.instance_collection {
            if pair.0 == uuid {
                if self.check_if_name_exists(&format!("{}_copy", &pair.1.get_name())) {
                    return Err(format!(
                        "{}_copy already exists as an instance",
                        &pair.1.get_name()
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn player_list(&self, uuid: &String) -> Result<Vec<String>, String> {
        let ins = self
            .instance_collection
            .get(uuid)
            .ok_or("instance does not exist".to_string())?;
        Ok(ins.get_player_list())
    }

    pub fn player_num(&self, uuid: &String) -> Result<u32, String> {
        let ins = self
            .instance_collection
            .get(uuid)
            .ok_or("instance does not exist".to_string())?;
        Ok(ins.get_player_num())
    }

    pub fn send_command(&self, uuid: &String, command: String) -> Result<(), String> {
        let instance = self
            .instance_collection
            .get(uuid)
            .ok_or("cannot send command to instance as it does not exist".to_string())?;
        instance.send_stdin(command).map_err(|e| {
            format!(
                "failed to send command to instance {} : {}",
                instance.get_uuid(),
                e
            )
        })?;
        Ok(())
    }

    pub fn start_instance(&mut self, uuid: &String) -> Result<(), String> {
        let instance = self
            .instance_collection
            .get_mut(uuid)
            .ok_or("instance cannot be started as it does not exist".to_string())?;
        instance.start()
    }

    pub fn stop_instance(&mut self, uuid: &String) -> Result<(), String> {
        let instance = self
            .instance_collection
            .get_mut(uuid)
            .ok_or("instance cannot be stopped as it does not exist".to_string())?;
        instance.stop()
    }

    pub fn check_if_name_exists(&self, name: &String) -> bool {
        // TODO: DB IO
        let mut ret = false;
        for pair in &self.instance_collection {
            if &pair.1.get_name() == name {
                ret = true;
                break;
            }
        }
        ret
    }

    pub fn name_to_uuid(&self, name: &String) -> Result<String, String> {
        for pair in &self.instance_collection {
            if &pair.1.get_name() == name {
                return Ok(pair.0.clone());
            }
        }
        Err("instance does not exist".to_string())
    }
}
pub mod resource_management {
    use std::fs::{self, create_dir_all};

    use rocket::request::FromParam;

    use crate::{
        managers::{properties_manager::PropertiesManager, server_instance::Status},
        util::list_dir,
    };

    use super::InstanceManager;

    pub enum ResourceType {
        World,
        Mod,
    }

    impl<'r> FromParam<'r> for ResourceType {
        type Error = &'static str;

        fn from_param(param: &'r str) -> Result<Self, Self::Error> {
            match param {
                "world" => Ok(ResourceType::World),
                "mod" => Ok(ResourceType::Mod),
                _ => Err("invalid resource type"),
            }
        }
    }
    impl InstanceManager {
        /// this code is completely garbage, someone should fix it
        /// first is loaded resource, second is unloaded
        pub fn list_resource(
            &self,
            uuid: &String,
            resource_type: ResourceType,
        ) -> Result<(Vec<String>, Vec<String>), String> {
            let path_to_instance = self.path.join("instances").join(
                self.instance_collection
                    .get(uuid)
                    .ok_or("instance does not exist".to_string())?
                    .get_name(),
            );
            match resource_type {
                ResourceType::World => {
                    create_dir_all(path_to_instance.join("lodestone_resources/").join("worlds"))
                        .map_err(|_| {
                            "failed to create worlds directory in the resource directory"
                                .to_string()
                        })?;
                    let pm =
                        PropertiesManager::new(path_to_instance.clone().join("server.properties"))
                            .map_err(|e| {
                                format!("{}: {}", "failed to create properties manager", e)
                            })?;
                    let world_name = pm.get_field(&"level-name".to_string()).unwrap();
                    Ok((
                        if path_to_instance.join(&world_name).is_dir() {
                            vec![world_name]
                        } else {
                            vec![]
                        },
                        list_dir(
                            path_to_instance.join("lodestone_resources").join("worlds"),
                            false,
                        )
                        .unwrap(),
                    ))
                }
                ResourceType::Mod => {
                    create_dir_all(path_to_instance.join("lodestone_resources/").join("mods"))
                        .map_err(|_| {
                            "failed to create mods directory in the resource directory".to_string()
                        })?;
                    Ok((
                        list_dir(path_to_instance.join("mods"), false).unwrap(),
                        list_dir(
                            path_to_instance.join("lodestone_resources/").join("mods"),
                            false,
                        )
                        .unwrap(),
                    ))
                }
            }
        }

        pub fn load(
            &self,
            uuid: &String,
            resource_type: ResourceType,
            resource_name: &String,
        ) -> Result<(), String> {
            let path_to_instance = self.path.join("instances").join(
                self.instance_collection
                    .get(uuid)
                    .ok_or("instance does not exist".to_string())?
                    .get_name(),
            );
            match resource_type {
                ResourceType::World => {
                    if self.instance_collection.get(uuid).unwrap().get_status() != Status::Stopped {
                        return Err("instance is not stopped".to_string());
                    }
                    let mut pm =
                        PropertiesManager::new(path_to_instance.clone().join("server.properties"))
                            .map_err(|e| {
                                format!("{}: {}", "failed to create properties manager", e)
                            })?;
                    let world_name = pm.get_field(&"level-name".to_string()).unwrap();

                    self.unload(&uuid, resource_type, &world_name)
                        .map_err(|e| format!("{}: {}", "failed to unload world", e))?;
                    if !path_to_instance
                        .join("lodestone_resources")
                        .join("worlds")
                        .join(resource_name)
                        .is_dir()
                    {
                        return Err(format!("world {} does not exist", resource_name));
                    }
                    fs_extra::dir::move_dir(
                        path_to_instance
                            .join("lodestone_resources")
                            .join("worlds")
                            .join(resource_name),
                        path_to_instance,
                        &fs_extra::dir::CopyOptions::new(),
                    )
                    .map_err(|e| format!("{}: {}", "failed to move world", e))?;
                    pm.edit_field(&"level-name".to_string(), resource_name.clone())
                        .map_err(|e| format!("{}: {}", "failed to edit level-name", e))?;
                    pm.write_to_file()
                        .map_err(|e| format!("{}: {}", "failed to write to file", e))?;
                    Ok(())
                }
                ResourceType::Mod => {
                    self.unload(&uuid, resource_type, &resource_name)
                        .map_err(|e| format!("{}: {}", "failed to unload mod", e))?;
                    if !path_to_instance
                        .join("lodestone_resources")
                        .join("mods")
                        .join(resource_name)
                        .is_file()
                    {
                        return Err(format!("mod {} does not exist", resource_name));
                    }
                    fs_extra::file::move_file(
                        path_to_instance
                            .join("lodestone_resources")
                            .join("mods")
                            .join(resource_name),
                        path_to_instance.join("mods").join(resource_name),
                        &fs_extra::file::CopyOptions::new(),
                    )
                    .map_err(|e| format!("{}: {}", "failed to move mod", e))?;

                    Ok(())
                }
            }
        }
        pub fn unload(
            &self,
            uuid: &String,
            resource_type: ResourceType,
            resource_name: &String,
        ) -> Result<(), String> {
            let path_to_instance = self.path.join("instances").join(
                self.instance_collection
                    .get(uuid)
                    .ok_or("instance does not exist".to_string())?
                    .get_name(),
            );

            match resource_type {
                ResourceType::World => {
                    if self.instance_collection.get(uuid).unwrap().get_status() != Status::Stopped {
                        return Err("instance is not stopped".to_string());
                    }
                    match (
                        path_to_instance.join(resource_name).is_dir(),
                        path_to_instance
                            .join("lodestone_resources")
                            .join("worlds")
                            .join(resource_name)
                            .is_dir(),
                    ) {
                        // if there is already a world loaded, and if the world is in the resource already
                        // simply delete the world in the instance directory? //!warn: may not be the best solution?
                        (true, true) => Ok(fs::remove_dir_all(
                            path_to_instance.join(resource_name),
                        )
                        .map_err(|e| format!("failed to remove directory: {}", e.to_string()))?),
                        // if there is already a world loaded, and if the world is NOT in the resource already
                        // move the world from instance directory to resource directory
                        (true, false) => {
                            fs_extra::dir::move_dir(
                                path_to_instance.join(resource_name),
                                path_to_instance.join("lodestone_resources").join("worlds"),
                                &fs_extra::dir::CopyOptions::new(),
                            )
                            .map_err(|e| format!("failed to move directory: {}", e.to_string()))?;
                            Ok(())
                        }
                        // maybe should error
                        (false, true) => Ok(()),
                        (false, false) => Err("resource does not exist".to_string()),
                    }
                }
                ResourceType::Mod => {
                    match (
                        path_to_instance.join("mods").join(resource_name).is_file(),
                        path_to_instance
                            .join("lodestone_resources")
                            .join("mods")
                            .join(resource_name)
                            .is_file(),
                    ) {
                        (true, true) => Ok(fs::remove_file(
                            path_to_instance.join("mods").join(resource_name),
                        )
                        .map_err(|e| format!("failed to remove file: {}", e.to_string()))?),
                        (true, false) => {
                            fs_extra::file::move_file(
                                path_to_instance.join("mods").join(resource_name),
                                path_to_instance
                                    .join("lodestone_resources")
                                    .join("mods")
                                    .join(resource_name),
                                &fs_extra::file::CopyOptions::new(),
                            )
                            .map_err(|e| format!("failed to move file: {}", e.to_string()))?;
                            Ok(())
                        }
                        (false, true) => Ok(()),
                        (false, false) => Err("resource does not exist".to_string()),
                    }
                }
            }
        }
    }
}
