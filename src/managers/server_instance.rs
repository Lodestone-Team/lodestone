use crate::instance_manager::InstanceManager;
use chashmap::CHashMap;
use mongodb::{bson::doc, options::ClientOptions, sync::Client};
use regex::internal::Input;
use serde::{Deserialize, Serialize};
use std::char::UNICODE_VERSION;
use std::env;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fmt, thread};

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct InstanceConfig {
    pub name: String,
    pub version: String,
    pub flavour: String,
    /// url to download the server.jar file from upon setup
    pub url: Option<String>,
    pub port: Option<u32>,
    pub uuid: Option<String>,
    pub min_ram: Option<u32>,
    pub max_ram: Option<u32>,
}

#[derive(Clone, PartialEq)]
enum BroadcastCommand {
    Terminate,
    Continue,
}
#[derive(Debug, Clone, Copy)]
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
pub struct ServerInstance {
    pub name: String,
    jvm_args: Vec<String>,
    path: String,
    pub uuid: String,
    pub stdin: Option<Arc<Mutex<ChildStdin>>>,
    status: Arc<Mutex<Status>>,
    process: Arc<Mutex<Option<Child>>>,
    player_online: Arc<Mutex<Vec<String>>>,
}
/// Instance specific events,
/// Ex. Player joining, leaving, dying
pub enum InstanceEvent {
    Joined,
    Left,
}

impl ServerInstance {
    pub fn new(config: &InstanceConfig, path: String) -> ServerInstance {
        let mut jvm_args: Vec<String> = vec![];
        match config.min_ram {
            Some(min_ram) => jvm_args.push(format!("-Xms{}M", min_ram)),
            None => (),
        }
        match config.max_ram {
            Some(max_ram) => jvm_args.push(format!("-Xmx{}M", max_ram)),
            None => (),
        }
        jvm_args.push("-jar".to_string());
        jvm_args.push("server.jar".to_string());
        jvm_args.push("nogui".to_string());

        ServerInstance {
            status: Arc::new(Mutex::new(Status::Stopped)),
            name: config.name.clone(),
            stdin: None,
            jvm_args,
            process: Arc::new(Mutex::new(None)),
            path,
            uuid: config.uuid.as_ref().unwrap().clone(),
            player_online: Arc::new(Mutex::new(vec![])),
        }
    }

    pub fn start(&mut self, mongodb_client: Client) -> Result<(), String> {
        let mut status = self.status.lock().unwrap();
        env::set_current_dir(self.path.as_str()).unwrap();
        match *status {
            Status::Starting => {
                return Err("cannot start, instance is already starting".to_string())
            }
            Status::Stopping => return Err("cannot start, instance is stopping".to_string()),
            Status::Running => return Err("cannot start, instance is already running".to_string()),
            Status::Stopped => (),
        }
        *status = Status::Starting;
        let mut command = Command::new("java");
        command
            .args(&self.jvm_args)
            .stdout(Stdio::piped())
            .stdin(Stdio::piped());
        match command.spawn() {
            Ok(proc) => {
                env::set_current_dir("../..").unwrap();

                let stdin = proc.stdin.ok_or("failed to open stdin of child process")?;
                self.stdin = Some(Arc::new(Mutex::new(stdin)));
                let stdout = proc.stdout.ok_or("failed to open stdin of child process")?;
                let reader = BufReader::new(stdout);
                let uuid_closure = self.uuid.clone();
                let status_closure = self.status.clone();
                let players_closure = self.player_online.clone();
                thread::spawn(move || {
                    use regex::Regex;
                    let re = Regex::new(r"\[Server thread/INFO\]: Done").unwrap();
                    for line_result in reader.lines() {
                        let mut status = status_closure.lock().unwrap();
                        let line = line_result.unwrap();
                        let time128 = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_millis();
                        let time = i64::try_from(time128).unwrap();
                        if re.is_match(line.as_str()) {
                            *status = Status::Running;
                        }
                        drop(status);
                        println!("Server said: {}", line);
                        mongodb_client
                            .database(uuid_closure.as_str())
                            .collection("logs")
                            .insert_one(
                                doc! {
                                    "time": time,
                                    "log": line.clone()
                                },
                                None,
                            )
                            .unwrap();
                        if let Some(event) = instance_event_parser(&line) {
                            match event.1 {
                                InstanceEvent::Joined => {
                                    players_closure.lock().unwrap().push(event.0.clone());
                                    mongodb_client
                                        .database(uuid_closure.as_str())
                                        .collection("events")
                                        .insert_one(
                                            doc! {
                                                "time": time,
                                                "player": event.0.clone(),
                                                "eventMsg": "joined the instance"
                                            },
                                            None,
                                        )
                                        .unwrap();
                                }
                                InstanceEvent::Left => {
                                    let mut players = players_closure.lock().unwrap();
                                    if let Some(index) =
                                        players.iter().position(|x| *x == event.0.clone())
                                    {
                                        players.swap_remove(index);
                                    }
                                    drop(players);
                                    mongodb_client
                                        .database(uuid_closure.as_str())
                                        .collection("events")
                                        .insert_one(
                                            doc! {
                                                "time": time,
                                                "player": event.0.clone(),
                                                "eventMsg": "left the instance"
                                            },
                                            None,
                                        )
                                        .unwrap();
                                }
                            }
                        }
                    }
                    let mut status = status_closure.lock().unwrap();
                    println!("program exiting as reader thread is terminating...");
                    match *status {
                        Status::Starting => println!("instance failed to start"),
                        Status::Stopping => {
                            println!("instance is already stopping, this is not ok")
                        }
                        Status::Running => println!("instance exited unexpectedly, restarting..."), //TODO: Restart thru localhost
                        Status::Stopped => println!("instance stopped properly, exiting..."),
                    }
                    *status = Status::Stopped;
                });
                return Ok(());
            }
            Err(_) => {
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
            Status::Running => println!("stopping instance"),
        }
        *status = Status::Stopping;
        self.send_stdin("stop".to_string())?;
        *status = Status::Stopped;
        self.player_online.lock().unwrap().clear();
        Ok(())
    }

    pub fn send_stdin(&self, line: String) -> Result<(), String> {
        match self.stdin.clone().as_mut() {
            Some(stdin_mutex) => match stdin_mutex.lock() {
                Ok(mut stdin) => Ok(stdin
                    .write_all(line.as_bytes())
                    .map_err(|_| "failed to write to process's stdin".to_string())?),
                Err(_) => Err("failed to acquire lock on stdin".to_string()),
            },
            None => Err("could not find stdin of process".to_string()),
        }
    }

    pub fn get_status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    pub fn get_process(&self) -> Arc<Mutex<Option<Child>>> {
        self.process.clone()
    }

    pub fn get_player_list(&self) -> Vec<String> {
        self.player_online.lock().unwrap().clone()
    }

    pub fn get_player_num(&self) -> u32 {
        self.player_online.lock().unwrap().len().try_into().unwrap()
    }

    pub fn get_path(&self) -> String {
        self.path.clone()
    }
}

fn instance_event_parser(line: &String) -> Option<(String, InstanceEvent)> {
    // match if its a server message
    if line.matches("[").count() == 2
        && line.matches("<").count() == 0
        && line.matches(":").count() == 3
        && line.matches("/").count() == 1
    {
        if line.contains("joined the game") || line.contains("left the game") {
            let i1 = line.find("]:").unwrap();
            let tmp = &line.as_str()[i1 + 3..];
            let i2 = tmp.find(char::is_whitespace).unwrap();

            let tmp_name = &line.as_str()[i1 + 3..i2 + line.len() - tmp.len()];
            let player_name = String::from(tmp_name);
            if line.contains("joined the game") {
                return Some((player_name, InstanceEvent::Joined));
            }
            if line.contains("left the game") {
                return Some((player_name, InstanceEvent::Left));
            }
        }
    }
    None
}
