use tokio::fs::{create_dir_all, remove_file, File};
use std::path::PathBuf;

use rocket::fs::{NamedFile, TempFile};
use zip::ZipArchive;

use crate::services::file_service::{save_temp_file, zipping};

use super::types::ResourceType;

pub struct ResourceManager {
    path_to_instance: PathBuf,
    path_to_lodestone_resources: PathBuf,
}

impl ResourceManager {
    pub fn new(path_to_instance: PathBuf) -> ResourceManager {
        let path_to_lodestone_resources = path_to_instance.join(".lodestone_resources");
        if !path_to_lodestone_resources.is_dir() {
            create_dir_all(&path_to_lodestone_resources);
        }
        return ResourceManager {
            path_to_instance,
            path_to_lodestone_resources,
        };
    }

    pub async fn save_resource(
        &self,
        data: TempFile<'_>,
        resource_type: ResourceType,
    ) -> Result<(), String> {
        let path_to_folder = self.path_to_lodestone_resources.join(match resource_type {
            ResourceType::Mod => "mods",
            ResourceType::World => "worlds",
        });
        create_dir_all(&path_to_folder);
        let mut path_to_file = path_to_folder.join(data.name().unwrap());
        path_to_file.set_extension(match resource_type {
            ResourceType::Mod => "jar",
            ResourceType::World => "zip",
        });
        save_temp_file(&path_to_file, data).await?;
        match resource_type {
            ResourceType::World => {
                let mut zipped_world = ZipArchive::new(File::open(&path_to_file).unwrap()).unwrap();
                // create_dir_all(&path_to_extract);
                zipped_world.extract(&path_to_folder);
                remove_file(&path_to_file);
            }
            _ => {}
        }
        return Ok(());
    }

    pub async fn get_mod(&self, name: &String) -> Result<File, std::io::Error> {
        let mut path_to_file = self.path_to_lodestone_resources.join("mods").join(name);
        path_to_file.set_extension("jar");
        if !path_to_file.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "File not found",
            ));
        }
        return File::open(path_to_file);
    }

    pub async fn get_world(&self, name: &String) -> Result<File, std::io::Error> {
        let path_to_worlds = self.path_to_lodestone_resources.join("worlds");
        let path_to_world = path_to_worlds.join(name);
        let mut path_to_world = self.path_to_lodestone_resources.join("worlds").join(name);
        if !path_to_world.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "World folder not found",
            ));
        }
        
        match zipping(path_to_world, path_to_worlds.clone(), name) {
            Ok(file) => Ok(file),
            Err(err) => Err(err),
        }

    }
}
