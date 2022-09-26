use std::{
    fs::File,
    io::{BufRead, Write},
    sync::atomic,
};



use serde_json::json;

use crate::traits::{
    self, t_configurable::TConfigurable, ErrorInner, MaybeUnsupported, Supported,
};

use crate::traits::Error;

use super::Instance;

impl TConfigurable for Instance {
    fn uuid(&self) -> String {
        self.config.uuid.clone()
    }

    fn name(&self) -> String {
        self.config.name.clone()
    }

    fn game_type(&self) -> String {
        self.config.r#type.clone()
    }

    fn flavour(&self) -> String {
        self.config.flavour.to_string()
    }

    fn cmd_args(&self) -> Vec<String> {
        self.config.jvm_args.clone()
    }

    fn description(&self) -> String {
        self.config.description.clone()
    }

    fn port(&self) -> u32 {
        self.config.port
    }

    fn min_ram(&self) -> MaybeUnsupported<u32> {
        Supported(self.config.min_ram)
    }

    fn max_ram(&self) -> MaybeUnsupported<u32> {
        Supported(self.config.max_ram)
    }

    fn creation_time(&self) -> i64 {
        self.config.creation_time
    }

    fn path(&self) -> std::path::PathBuf {
        self.config.path.clone()
    }

    fn auto_start(&self) -> bool {
        self.config.auto_start
    }

    fn restart_on_crash(&self) -> MaybeUnsupported<bool> {
        Supported(self.config.restart_on_crash)
    }

    fn timeout_last_left(&self) -> MaybeUnsupported<Option<u32>> {
        Supported(self.config.timeout_last_left)
    }

    fn timeout_no_activity(&self) -> MaybeUnsupported<Option<u32>> {
        Supported(self.config.timeout_no_activity)
    }

    fn start_on_connection(&self) -> MaybeUnsupported<bool> {
        Supported(self.config.start_on_connection)
    }

    fn backup_period(&self) -> MaybeUnsupported<Option<u32>> {
        Supported(self.config.backup_period)
    }

    fn get_flavours(&self) -> Vec<String> {
        vec![
            "vanilla".to_string(),
            "fabric".to_string(),
            "paper".to_string(),
        ]
    }
    fn get_info(&self) -> serde_json::Value {
        json!(self.config)
    }

    fn set_name(&mut self, name: String) -> Result<(), traits::Error> {
        self.config.name = name;
        self.write_config_to_file()?;
        Ok(())
    }

    fn set_description(&mut self, description: String) -> Result<(), traits::Error> {
        self.config.description = description;
        self.write_config_to_file()?;
        Ok(())
    }

    fn set_jvm_args(
        &mut self,
        jvm_args: Vec<String>,
    ) -> MaybeUnsupported<Result<(), traits::Error>> {
        self.config.jvm_args = jvm_args;
        self.write_config_to_file()
            .map_or_else(|e| Supported(Err(e)), |_| Supported(Ok(())))
    }

    fn set_min_ram(&mut self, min_ram: u32) -> MaybeUnsupported<Result<(), traits::Error>> {
        self.config.min_ram = min_ram;
        self.write_config_to_file()
            .map_or_else(|e| Supported(Err(e)), |_| Supported(Ok(())))
    }

    fn set_max_ram(&mut self, max_ram: u32) -> MaybeUnsupported<Result<(), traits::Error>> {
        self.config.min_ram = max_ram;
        self.write_config_to_file()
            .map_or_else(|e| Supported(Err(e)), |_| Supported(Ok(())))
    }

    fn set_auto_start(&mut self, auto_start: bool) -> MaybeUnsupported<Result<(), traits::Error>> {
        self.config.auto_start = auto_start;
        self.auto_start.store(auto_start, atomic::Ordering::Relaxed);
        self.write_config_to_file()
            .map_or_else(|e| Supported(Err(e)), |_| Supported(Ok(())))
    }

    fn set_restart_on_crash(
        &mut self,
        restart_on_crash: bool,
    ) -> MaybeUnsupported<Result<(), traits::Error>> {
        self.config.restart_on_crash = restart_on_crash;
        self.auto_start
            .store(restart_on_crash, atomic::Ordering::Relaxed);
        self.write_config_to_file()
            .map_or_else(|e| Supported(Err(e)), |_| Supported(Ok(())))
    }

    fn set_timeout_last_left(
        &mut self,
        timeout_last_left: Option<u32>,
    ) -> MaybeUnsupported<Result<(), traits::Error>> {
        match self.timeout_last_left.lock() {
            Ok(mut v) => *v = timeout_last_left,
            Err(_) => {
                return Supported(Err(Error {
                    inner: ErrorInner::FailedToAcquireLock,
                    detail: "Uh oh! Thread poisoned! This is not good.".to_string(),
                }));
            }
        }
        self.config.timeout_last_left = timeout_last_left;
        self.write_config_to_file()
            .map_or_else(|e| Supported(Err(e)), |_| Supported(Ok(())))
    }

    fn set_timeout_no_activity(
        &mut self,
        timeout_no_activity: Option<u32>,
    ) -> MaybeUnsupported<Result<(), traits::Error>> {
        match self.timeout_no_activity.lock() {
            Ok(mut v) => *v = timeout_no_activity,
            Err(_) => {
                return Supported(Err(Error {
                    inner: ErrorInner::FailedToAcquireLock,
                    detail: "Uh oh! Thread poisoned! This is not good.".to_string(),
                }));
            }
        }
        self.config.timeout_no_activity = timeout_no_activity;
        self.write_config_to_file()
            .map_or_else(|e| Supported(Err(e)), |_| Supported(Ok(())))
    }

    fn set_start_on_connection(
        &mut self,
        start_on_connection: bool,
    ) -> MaybeUnsupported<Result<(), traits::Error>> {
        self.config.start_on_connection = start_on_connection;
        self.auto_start
            .store(start_on_connection, atomic::Ordering::Relaxed);
        self.write_config_to_file()
            .map_or_else(|e| Supported(Err(e)), |_| Supported(Ok(())))
    }

    fn set_backup_period(
        &mut self,
        backup_period: Option<u32>,
    ) -> MaybeUnsupported<Result<(), traits::Error>> {
        match self.backup_period.lock() {
            Ok(mut v) => *v = backup_period,
            Err(_) => {
                return Supported(Err(Error {
                    inner: ErrorInner::FailedToAcquireLock,
                    detail: "Uh oh! Thread poisoned! This is not good.".to_string(),
                }));
            }
        }
        self.config.timeout_no_activity = backup_period;
        self.write_config_to_file()
            .map_or_else(|e| Supported(Err(e)), |_| Supported(Ok(())))
    }

    fn set_field(&mut self, field: &str, value: String) -> Result<(), Error> {
        let properties_file = File::open(&self.path_to_properties).map_err(|_| Error {
            inner: ErrorInner::FailedToReadFileOrDir,
            detail: String::new(),
        })?;
        let buf_reader = std::io::BufReader::new(properties_file);
        let stream = buf_reader
            .lines()
            .filter(Result::is_ok)
            // this unwrap is safe because we filtered all the ok values
            .map(Result::unwrap);

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
        let stream = buf_reader
            .lines()
            .filter(Result::is_ok)
            // this unwrap is safe because we filtered all the ok values
            .map(Result::unwrap);

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

    fn setup_params(&self) -> serde_json::Value {
        serde_json::json!({
            "version" : "string"
        })
    }
}
