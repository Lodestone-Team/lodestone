use std::{
    fs::File,
    io::{BufRead, Write},
};

use rocket::serde::{self, json::serde_json::json};

use crate::traits::{t_configurable::TConfigurable, ErrorInner};

use crate::traits::Error;

use super::Instance;

impl TConfigurable for Instance {
    fn uuid(&self) -> String {
        self.config.uuid.clone()
    }

    fn name(&self) -> String {
        self.config.name.clone()
    }

    fn description(&self) -> String {
        self.config.description.clone()
    }

    fn port(&self) -> u32 {
        self.config.port
    }

    fn min_ram(&self) -> crate::traits::MaybeUnsupported<u32> {
        crate::traits::MaybeUnsupported::Supported(self.config.min_ram)
    }

    fn max_ram(&self) -> crate::traits::MaybeUnsupported<u32> {
        crate::traits::MaybeUnsupported::Supported(self.config.max_ram)
    }

    fn creation_time(&self) -> u64 {
        self.config.creation_time
    }

    fn path(&self) -> std::path::PathBuf {
        self.config.path.clone()
    }

    fn auto_start(&self) -> bool {
        self.config.auto_start
    }

    fn set_name(&mut self, name: String) {
        self.config.name = name;
    }

    fn set_description(&mut self, description: String) {
        self.config.description = description;
    }

    fn set_auto_start(&mut self, auto_start: bool) {
        self.config.auto_start = auto_start;
        *self.auto_start.lock().unwrap() = auto_start;
    }

    fn set_field(&mut self, field: &str, value: String) -> Result<(), Error> {
        let properties_file = File::open(&self.path_to_properties).map_err(|_| Error {
            inner: ErrorInner::FailedToReadFileOrDir,
            detail: String::new(),
        })?;
        let buf_reader = std::io::BufReader::new(properties_file);
        let stream = buf_reader.lines().filter(Result::is_ok).map(Result::unwrap);

        // vector of key value pairs
        let mut key_value_pairs = Vec::new();

        for line in stream {
            // if a line starts with '#', it is a comment, skip it
            if line.starts_with('#') {
                continue;
            }
            // split the line into key and value
            let mut split = line.split('=');
            let key = split
                .next()
                .ok_or(Error {
                    inner: ErrorInner::MalformedFile,
                    detail: String::new(),
                })?
                .trim();
            let value = split
                .next()
                .ok_or(Error {
                    inner: ErrorInner::MalformedFile,
                    detail: String::new(),
                })?
                .trim();
            key_value_pairs.push((key.to_string(), value.to_string()));
        }

        // loop through the key value pairs and replace the value if the key matches
        for (key, _value) in key_value_pairs.iter_mut() {
            if key == field {
                *_value = value;
                // write the new key value pairs to the properties file
                let file = File::open(&self.path_to_properties).map_err(|_| Error {
                    inner: ErrorInner::FailedToWriteFileOrDir,
                    detail: String::new(),
                })?;
                let mut file_writer = std::io::BufWriter::new(file);
                for (key, value) in key_value_pairs.iter() {
                    file_writer
                        .write_all(format!("{}={}", key, value).as_bytes())
                        .map_err(|_| Error {
                            inner: ErrorInner::FailedToWriteFileOrDir,
                            detail: String::new(),
                        })?;
                }
                return Ok(());
            }
        }
        Err(Error {
            inner: ErrorInner::FieldNotFound,
            detail: format!("Field {} not found", field),
        })
    }

    fn get_field(&self, field: &str) -> Result<String, Error> {
        let properties_file = File::open(&self.path_to_properties).map_err(|_| Error {
            inner: ErrorInner::FailedToReadFileOrDir,
            detail: String::new(),
        })?;
        let buf_reader = std::io::BufReader::new(properties_file);
        let stream = buf_reader.lines().filter(Result::is_ok).map(Result::unwrap);

        for line in stream {
            // if a line starts with '#', it is a comment, skip it
            if line.starts_with('#') {
                continue;
            }
            // split the line into key and value
            let mut split = line.split('=');
            let key = split
                .next()
                .ok_or(Error {
                    inner: ErrorInner::MalformedFile,
                    detail: String::new(),
                })?
                .trim();
            let value = split
                .next()
                .ok_or(Error {
                    inner: ErrorInner::MalformedFile,
                    detail: String::new(),
                })?
                .trim();
            if key == field {
                return Ok(value.to_string());
            }
        }
        Err(Error {
            inner: ErrorInner::FieldNotFound,
            detail: format!("Field {} not found", field),
        })
    }

    fn setup_params(&self) -> serde::json::Value {
        json!({
            "version" : "string"
        })
    }
}
