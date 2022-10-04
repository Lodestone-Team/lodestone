use crate::event_processor::{self, EventProcessor};
use crate::managers::types::ResourceType;
use log::warn;
use rocket::fs::{TempFile, NamedFile};
use rocket::serde::json::serde_json;
use serde::{Deserialize, Serialize};
use std::env;
use tokio::fs::{File};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use tokio::process::{Child, ChildStdin, Command, Stdio};
use systemstat::Duration;
// use std::sync::mpsc::{self, Receiver, Sender};
use crossbeam_channel::{bounded, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fmt, thread};

// use self::macro_code::dispatch_macro;

use super::macro_manager::MacroManager;
use super::properties_manager::PropertiesManager;
use super::resource_manager::ResourceManager;

#[derive(Debug, Clone, Copy)]
pub enum Flavour {
    Vanilla,
    Fabric,
    Paper,
    Spigot,
}

impl<'de> Deserialize<'de> for Flavour {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "vanilla" => Ok(Flavour::Vanilla),
            "fabric" => Ok(Flavour::Fabric),
            "paper" => Ok(Flavour::Paper),
            "spigot" => Ok(Flavour::Spigot),
            _ => Err(serde::de::Error::custom(format!("Unknown flavour: {}", s))),
        }
    }
}
impl Serialize for Flavour {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Flavour::Vanilla => serializer.serialize_str("vanilla"),
            Flavour::Fabric => serializer.serialize_str("fabric"),
            Flavour::Paper => serializer.serialize_str("paper"),
            Flavour::Spigot => serializer.serialize_str("spigot"),
        }
    }
}

impl ToString for Flavour {
    fn to_string(&self) -> String {
        match self {
            Flavour::Vanilla => "vanilla".to_string(),
            Flavour::Fabric => "fabric".to_string(),
            Flavour::Paper => "paper".to_string(),
            Flavour::Spigot => "spigot".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum Status {
    Starting,
    Stopping,
    Running,
    Stopped,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Status::Starting => write!(f, "Starting"),
            Status::Stopping => write!(f, "Stopping"),
            Status::Running => write!(f, "Running"),
            Status::Stopped => write!(f, "Stopped"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]

// this struct is neccessary to set up a server instance
pub struct InstanceConfig {
    pub name: String,
    pub version: String,
    pub flavour: Flavour,
    /// url to download the server.jar file from upon setup
    pub url: Option<String>,
    pub port: Option<u32>,
    pub uuid: Option<String>,
    pub min_ram: Option<u32>,
    pub max_ram: Option<u32>,
    pub creation_time: Option<u64>,
    pub auto_start: Option<bool>,
    pub restart_on_crash: Option<bool>,
    pub timeout_last_left: Option<i32>,
    pub timeout_no_activity: Option<i32>,
    pub start_on_connection: Option<bool>,
    pub backup_period: Option<i32>,
}

impl InstanceConfig {
    fn fill_default(&self) -> InstanceConfig {
        let mut config_override = self.clone();
        if self.auto_start == None {
            config_override.auto_start = Some(false);
        }
        if self.restart_on_crash == None {
            config_override.restart_on_crash = Some(false);
        }
        if self.creation_time == None {
            config_override.creation_time = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            );
        }
        if self.timeout_last_left == None {
            config_override.timeout_last_left = Some(-1);
        }
        if self.timeout_no_activity == None {
            config_override.timeout_no_activity = Some(-1);
        }
        if self.start_on_connection == None {
            config_override.start_on_connection = Some(false);
        }
        if self.max_ram == None {
            config_override.max_ram = Some(3000);
        }
        if self.min_ram == None {
            config_override.min_ram = Some(1000);
        }
        if self.backup_period == None {
            config_override.backup_period = Some(-1);
        }
        config_override
    }
}

pub struct ServerInstance {
    name: String,
    version: String,
    flavour: Flavour,
    port: u32,
    uuid: String,
    min_ram: u32,
    max_ram: u32,
    creation_time: u64,
    auto_start: Arc<Mutex<bool>>,
    restart_on_crash: Arc<Mutex<bool>>,
    timeout_last_left: Arc<Mutex<i32>>,
    timeout_no_activity: Arc<Mutex<i32>>,
    start_on_connection: Arc<Mutex<bool>>,
    backup_period: Arc<Mutex<i32>>,
    cmd_argss: Vec<String>,
    path: PathBuf,
    pub stdin: Arc<Mutex<Option<ChildStdin>>>,
    status: Arc<Mutex<Status>>,
    process: Option<Arc<Mutex<Child>>>,
    player_online: Arc<Mutex<Vec<String>>>,
    pub event_processor: Arc<Mutex<EventProcessor>>,
    properties_manager: PropertiesManager,
    resource_manager: ResourceManager,
    macro_manager: MacroManager,
    proxy_kill_tx: Sender<()>,
    proxy_kill_rx: Receiver<()>,
    /// used to reconstruct the server instance from the database
    /// this field MUST be synced to the main object
    pub instance_config: InstanceConfig,
}

impl ServerInstance {
    pub fn new(config: &InstanceConfig, path: PathBuf) -> ServerInstance {
        let mut cmd_argss: Vec<String> = vec![];
        let config_override = config.fill_default();
        // this unwrap is safe because we just filled it in
        cmd_argss.push(format!("-Xms{}M", config_override.min_ram.unwrap()));
        cmd_argss.push(format!("-Xmx{}M", config_override.max_ram.unwrap()));
        cmd_argss.push("-jar".to_string());
        cmd_argss.push("server.jar".to_string());
        cmd_argss.push("nogui".to_string());
        info!("cmd_argss: {:?}", cmd_argss);

        let properties_manager = PropertiesManager::new(path.join("server.properties")).unwrap();
        let resource_manager = ResourceManager::new(path.clone());

        let event_processor = Arc::new(Mutex::new(EventProcessor::new()));
        let stdin: Arc<Mutex<Option<ChildStdin>>> = Arc::new(Mutex::new(None));
        let player_online = Arc::new(Mutex::new(vec![]));
        let status = Arc::new(Mutex::new(Status::Stopped));
        let macro_manager = MacroManager::new(
            path.join("macros/"),
            path.clone(),
            stdin.clone(),
            event_processor.clone(),
            player_online.clone(),
            status.clone(),
        );

        let (proxy_kill_tx, proxy_kill_rx): (Sender<()>, Receiver<()>) = bounded(1);

        if let Some(true) = config.start_on_connection {
            let listener =
                TcpListener::bind(format!("127.0.0.1:{}", config.port.unwrap())).unwrap();
            listener.set_nonblocking(true).unwrap();
            let uuid = config.uuid.clone().unwrap();
            let proxy_kill_rx = proxy_kill_rx.clone();
            thread::spawn(move || {
                let mut kill = false;
                while !kill {
                    if let Ok(_) = proxy_kill_rx.try_recv() {
                        info!("Proxy kill received");
                        kill = true;
                        break;
                    }
                    if let Ok((_, _)) = listener.accept() {
                        break;
                    }
                }
                if !kill {
                    info!("got tcp connection");
                    reqwest::blocking::Client::new()
                        .post(format!(
                            "http://127.0.0.1:8001/api/v1/instance/{}/start",
                            uuid
                        ))
                        .send()
                        .unwrap();
                    info!("end")
                }
            });
        }

        // serilize config_override to a file
        let mut file = File::create(path.join(".lodestone_config")).unwrap();
        let config_override_string = serde_json::to_string_pretty(&config_override).unwrap();
        file.write_all(config_override_string.as_bytes()).unwrap();

        let mut server_instance = ServerInstance {
            status,
            flavour: config.flavour,
            name: config.name.clone(),
            stdin,
            cmd_argss,
            process: None,
            path: path.clone(),
            port: config.port.expect("no port provided"),
            uuid: config.uuid.as_ref().unwrap().clone(),
            player_online,
            event_processor,
            proxy_kill_tx,
            properties_manager,
            resource_manager,
            macro_manager,
            proxy_kill_rx,
            version: config_override.version.clone(),
            min_ram: config_override.min_ram.unwrap(),
            max_ram: config_override.max_ram.unwrap(),
            creation_time: config_override.creation_time.unwrap(),
            auto_start: Arc::new(Mutex::new(config_override.auto_start.unwrap())),
            restart_on_crash: Arc::new(Mutex::new(config_override.restart_on_crash.unwrap())),
            timeout_last_left: Arc::new(Mutex::new(config_override.timeout_last_left.unwrap())),
            timeout_no_activity: Arc::new(Mutex::new(config_override.timeout_no_activity.unwrap())),
            start_on_connection: Arc::new(Mutex::new(config_override.start_on_connection.unwrap())),
            instance_config: config_override,
            backup_period: Arc::new(Mutex::new(-1)),
        };

        server_instance.setup_event_processor();
        server_instance
    }

    fn setup_event_processor(&mut self) {
        let mut event_processor = self.event_processor.lock().unwrap();
        let player_online = self.player_online.clone();
        event_processor.on_player_joined(Arc::new(move |player| {
            player_online.lock().unwrap().push(player);
        }));

        let timeout_last_left = self.timeout_last_left.clone();
        let player_online = self.player_online.clone();
        let status = self.status.clone();
        let stdin = self.stdin.clone();
        let instance_name = self.name.clone();
        event_processor.on_player_left(Arc::new(move |player| {
            player_online.lock().unwrap().retain(|p| p != &player);

            let timeout = timeout_last_left.lock().unwrap().to_owned();
            if timeout > 0 {
                let mut i = timeout;
                while i > 0 {
                    thread::sleep(Duration::from_secs(1));
                    i -= 1;
                    if player_online.lock().unwrap().len() > 0
                        || status.lock().unwrap().to_owned() != Status::Running
                    {
                        i = timeout;
                        continue;
                    }
                    if i < 10 {
                        info!(
                            "[{}] [EventProcessor] Last player left the server, shutting down in {} seconds",instance_name,
                            i
                        );
                    }
                }
                stdin
                    .lock()
                    .unwrap()
                    .as_mut()
                    .unwrap()
                    .write_all(b"stop\n")
                    .unwrap();
            }
        }));

        let timeout_no_activity = self.timeout_no_activity.clone();
        let player_online = self.player_online.clone();
        let status = self.status.clone();
        let stdin = self.stdin.clone();
        let instance_name = self.name.clone();
        event_processor.on_server_startup(Arc::new(move || {
            *status.lock().unwrap() = Status::Running;
            // sets up timeout no activity
            let timeout = timeout_no_activity.lock().unwrap().to_owned();
            if timeout > 0 {
                let mut i = timeout;
                while i > 0 {
                    thread::sleep(Duration::from_secs(1));
                    i -= 1;
                    if player_online.lock().unwrap().len() > 0
                        || status.lock().unwrap().to_owned() != Status::Running
                    {
                        i = timeout;
                        continue;
                    }
                    if i < 10 {
                        info!(
                            "[{}] [EventProcessor] No activity on server, shutting down in {} seconds",instance_name,
                            i
                        );
                    }
                }

                stdin
                    .lock()
                    .unwrap()
                    .as_mut()
                    .unwrap()
                    .write_all(b"stop\n")
                    .unwrap();
            }
        }));
        let name = self.name.clone();
        event_processor.on_server_message(Arc::new(move |message| match message.signal {
            crate::event_processor::Signal::Info => {
                info!("[{}] [{}] {}", name, message.timestamp, message.message)
            }
            crate::event_processor::Signal::Warn => {
                warn!("[{}] [{}] {}", name, message.timestamp, message.message)
            }
            crate::event_processor::Signal::Error => {
                error!("[{}] [{}] {}", name, message.timestamp, message.message)
            }
        }));

        let player_online = self.player_online.clone();
        event_processor.on_server_shutdown(Arc::new(move || {
            player_online.lock().unwrap().clear();
        }));
        let status = self.status.clone();
        event_processor.on_server_message(Arc::new(move |msg| {
            if msg.message.contains("Stopping server") {
                *status.lock().unwrap() = Status::Stopping;
            }
        }));

        let macro_manager = self.macro_manager.clone();
        let stdin = self.stdin.clone();
        event_processor.on_chat(Arc::new(move |player, msg| {
            let macro_manager = macro_manager.clone();
            if msg.starts_with(".macro") {
                let mut macro_name = String::new();

                let mut args = msg.split_whitespace();
                // if there is a second argument
                if let Some(name) = args.nth(1) {
                    macro_name = name.to_string();
                } else {
                    stdin
                        .lock()
                        .unwrap()
                        .as_ref()
                        .unwrap()
                        .write_all(b"say Usage: .macro [file] [args..]\n")
                        .unwrap();
                    return;
                }
                // collect the string into a vec<String>
                let mut vec_string = vec![];
                for token in args {
                    vec_string.push(token.to_owned())
                }
                thread::spawn(move || {
                    macro_manager
                        .run(macro_name, vec_string, Some(player))
                        .unwrap();
                });
            }
        }));
        let start_on_connection = self.start_on_connection.clone();
        let proxy_kill_rx = self.proxy_kill_rx.clone();
        let port = self.port;
        let uuid = self.uuid.clone();
        event_processor.on_server_shutdown(Arc::new(move || {
            if start_on_connection.lock().unwrap().to_owned() {
                let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
                info!("awaiting connection on port {}", port);
                listener.set_nonblocking(true).unwrap();
                let mut kill = false;
                proxy_kill_rx.try_recv();
                while !kill {
                    if let Ok(_) = proxy_kill_rx.try_recv() {
                        info!("Proxy kill received");
                        kill = true;
                        break;
                    }
                    if let Ok((_, _)) = listener.accept() {
                        break;
                    }
                }
                if !kill {
                    info!("got tcp connection");
                    reqwest::blocking::Client::new()
                        .post(format!(
                            "http://127.0.0.1:8001/api/v1/instance/{}/start",
                            uuid
                        ))
                        .send()
                        .unwrap();
                }
            }
        }));

        let macro_manager = self.macro_manager.clone();
        let name = self.name.clone();
        event_processor.on_server_startup(Arc::new(move || {
            macro_manager
                .run("on_startup".to_owned(), vec![], None)
                .map_err(|_| {
                    warn!(
                        "[{}] [MacroManager] no macro named \"on_startup\" found, skipping",
                        name
                    );
                });
        }));
        let macro_manager = self.macro_manager.clone();
        let name = self.name.clone();

        event_processor.on_server_shutdown(Arc::new(move || {
            macro_manager
                .run("on_shutdown".to_owned(), vec![], None)
                .map_err(|_| {
                    warn!(
                        "[{}] [MacroManager] no macro named \"on_shutdown\" found, skipping",
                        name
                    );
                });
        }));
        let name = self.name.clone();
        let macro_manager = self.macro_manager.clone();
        event_processor.on_player_joined(Arc::new(move |player| {
            macro_manager
                .run("on_player_joined".to_owned(), vec![player], None)
                .map_err(|_| {
                    warn!(
                        "[{}] [MacroManager] no macro named \"on_player_joined\" found, skipping",
                        name
                    );
                });
        }));
        let name = self.name.clone();
        let macro_manager = self.macro_manager.clone();
        event_processor.on_player_left(Arc::new(move |player| {
            macro_manager
                .run("on_player_left".to_owned(), vec![player], None)
                .map_err(|_| {
                    warn!(
                        "[{}] [MacroManager] no macro named \"on_player_left\" found, skipping",
                        name
                    );
                });
        }));
    }

    pub fn start(&mut self) -> Result<(), String> {
        let status = self.status.lock().unwrap().clone();
        env::set_current_dir(&self.path).unwrap();
        if self.instance_config.start_on_connection.unwrap() == true {
            self.proxy_kill_tx.send(()).unwrap();
        }

        match status {
            Status::Starting => {
                return Err("cannot start, instance is already starting".to_string())
            }
            Status::Stopping => return Err("cannot start, instance is stopping".to_string()),
            Status::Running => return Err("cannot start, instance is already running".to_string()),
            _ => (),
        }
        Command::new("bash")
            .arg(&self.path.join("prelaunch.sh"))
            .output()
            .map_err(|e| error!("{}", e.to_string()));
        *self.status.lock().unwrap() = Status::Starting;
        let mut command = Command::new("java");
        command
            .args(&self.cmd_argss)
            .stdout(Stdio::piped())
            .stdin(Stdio::piped());
        match command.spawn() {
            Ok(mut proc) => {
                env::set_current_dir("../..").unwrap();
                let stdin = proc
                    .stdin
                    .take()
                    .ok_or("failed to open stdin of child process")?;
                *self.stdin.lock().unwrap() = Some(stdin);
                let stdout = proc
                    .stdout
                    .take()
                    .ok_or("failed to open stdout of child process")?;
                let reader = BufReader::new(stdout);
                self.macro_manager
                    .set_event_processor(self.event_processor.clone());
                self.macro_manager.set_stdin_sender(self.stdin.clone());

                let players_closure = self.player_online.clone();
                let event_processor_closure = self.event_processor.clone();
                let status_closure = self.status.clone();
                let uuid_closure = self.uuid.clone();
                let restart_on_crash = Arc::new(self.instance_config.restart_on_crash);
                let name_closure = self.instance_config.name.clone();
                thread::spawn(move || {
                    for line_result in reader.lines() {
                        let line = line_result.unwrap();
                        event_processor_closure.lock().unwrap().process(&line);
                    }

                    let status = status_closure.lock().unwrap().clone();
                    players_closure.lock().unwrap().clear();
                    info!(
                        "[{}] program exiting as reader thread is terminating...",
                        name_closure
                    );
                    match status {
                        Status::Stopping => {
                            info!("[{}] instance stopped properly", name_closure);
                        }
                        Status::Running => {
                            if let Some(true) = *restart_on_crash {
                                info!("restarting instance");
                                // make a post request to localhost
                                let client = reqwest::blocking::Client::new();
                                client
                                    .post(format!(
                                        "http://localhost:8001/api/v1/instance/{}/start",
                                        uuid_closure
                                    ))
                                    .send()
                                    .unwrap();
                            }
                        }
                        Status::Starting => {
                            error!(
                                "[{}] instance crashed while attemping to start",
                                name_closure
                            );
                        }
                        _ => {
                            error!("this is a really weird bug");
                        }
                    }
                    *status_closure.lock().unwrap() = Status::Stopped;

                    event_processor_closure
                        .lock()
                        .unwrap()
                        .notify_server_shutdown();
                });
                self.process = Some(Arc::new(Mutex::new(proc)));

                return Ok(());
            }
            Err(_) => {
                *self.status.lock().unwrap() = Status::Stopped;
                env::set_current_dir("../..").unwrap();
                return Err("failed to open child process".to_string());
            }
        };
    }
    pub fn stop(&mut self) -> Result<(), String> {
        let mut status = self.status.lock().unwrap();
        match *status {
            Status::Starting => return Err("cannot stop, instance is starting".to_string()),
            Status::Stopping => return Err("cannot stop, instance is already stopping".to_string()),
            Status::Stopped => return Err("cannot stop, instance is already stopped".to_string()),
            Status::Running => info!("[{}] stopping instance", self.instance_config.name),
        }
        *status = Status::Stopping;
        self.send_stdin("stop".to_string())?;
        self.player_online.lock().unwrap().clear();
        Ok(())
    }

    pub fn send_stdin(&self, line: String) -> Result<(), String> {
        match self.stdin.lock() {
            Ok(stdin_option) => {
                if (*stdin_option).is_none() {
                    return Err("stdin is not open".to_string());
                }
                return (*stdin_option)
                    .as_ref()
                    .unwrap()
                    .write_all(format!("{}\n", line).as_bytes())
                    .map_err(|e| format!("failed to write to stdin: {}", e));
            }
            Err(_) => Err("failed to aquire lock on stdin".to_string()),
        }
    }

    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    pub fn get_status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    pub fn get_process(&self) -> Option<Arc<Mutex<Child>>> {
        self.process.clone()
    }

    pub fn get_player_list(&self) -> Vec<String> {
        self.player_online.lock().unwrap().clone()
    }

    pub fn get_player_num(&self) -> u32 {
        self.player_online.lock().unwrap().len().try_into().unwrap()
    }

    pub fn get_path(&self) -> PathBuf {
        self.path.clone()
    }

    pub fn get_uuid(&self) -> String {
        self.uuid.clone()
    }

    pub fn get_flavour(&self) -> Flavour {
        self.flavour
    }
    pub fn get_port(&self) -> u32 {
        self.port
    }

    pub fn get_instance_config(&self) -> &InstanceConfig {
        &self.instance_config
    }

    /// Get the server instance's creation time.
    #[must_use]
    pub fn creation_time(&self) -> u64 {
        self.instance_config.creation_time.unwrap()
    }

    /// Get a reference to the server instance's instance config.
    #[must_use]
    pub fn instance_config(&self) -> &InstanceConfig {
        &self.instance_config
    }
}

impl ServerInstance {
    pub async fn upload(
        &self,
        data: TempFile<'_>,
        resource_type: ResourceType,
    ) -> Result<(), String> {
        self.resource_manager.save_resource(data, resource_type).await
    }

    pub async fn get_mod(
        &self,
        name: &String,
    ) -> Result<File, std::io::Error> {
        self.resource_manager.get_mod(name).await
    }

    pub async fn get_world(
        &self,
        name: &String, 
    ) -> Result<File, std::io::Error> {
        self.resource_manager.get_world(name).await
    }
}
