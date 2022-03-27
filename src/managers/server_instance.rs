use mongodb::{bson::doc, sync::Client};
use serde::{Deserialize, Serialize};
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fmt, thread};

use crate::managers::server_instance::event_parser::{dispatch_macro, is_player_message};

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
}
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum Flavour {
    Vanilla,
    Fabric,
    Paper,
    Spigot,
}

impl fmt::Display for Flavour {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Flavour::Vanilla => write!(f, "Vanilla"),
            Flavour::Fabric => write!(f, "Fabric"),
            Flavour::Paper => write!(f, "Paper"),
            Flavour::Spigot => write!(f, "Spigot"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
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
    name: String,
    flavour: Flavour,
    jvm_args: Vec<String>,
    path: PathBuf,
    uuid: String,
    port : u32,
    pub stdin: Option<Sender<String>>,
    status: Arc<Mutex<Status>>,
    kill_tx: Option<Sender<()>>,
    process: Arc<Mutex<Option<Child>>>,
    player_online: Arc<Mutex<Vec<String>>>,
    // used to reconstruct the server instance from the database
    instance_config: InstanceConfig,
}
/// Instance specific events,
/// Ex. Player joining, leaving, dying

impl ServerInstance {
    pub fn new(config: &InstanceConfig, path: PathBuf) -> ServerInstance {
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
        println!("jvm_args: {:?}", jvm_args);

        ServerInstance {
            status: Arc::new(Mutex::new(Status::Stopped)),
            flavour: config.flavour,
            name: config.name.clone(),
            stdin: None,
            jvm_args,
            process: Arc::new(Mutex::new(None)),
            path,
            port : config.port.expect("no port provided"),
            kill_tx: None,
            uuid: config.uuid.as_ref().unwrap().clone(),
            player_online: Arc::new(Mutex::new(vec![])),
            instance_config : config.clone()
        }
    }

    pub fn start(&mut self, mongodb_client: Client) -> Result<(), String> {
        let mut status = self.status.lock().unwrap();
        env::set_current_dir(&self.path).unwrap();
        match *status {
            Status::Starting => {
                return Err("cannot start, instance is already starting".to_string())
            }
            Status::Stopping => return Err("cannot start, instance is stopping".to_string()),
            Status::Running => return Err("cannot start, instance is already running".to_string()),
            Status::Stopped => (),
        }
        Command::new("bash").arg(&self.path.join("prelaunch.sh")).output().map_err(|e| println!("{}", e.to_string()));
        *status = Status::Starting;
        let mut command = Command::new("java");
        command
            .args(&self.jvm_args)
            .stdout(Stdio::piped())
            .stdin(Stdio::piped());
        match command.spawn() {
            Ok(proc) => {
                env::set_current_dir("../..").unwrap();

                let (stdin_sender, stdin_receiver): (Sender<String>, Receiver<String>) =
                    mpsc::channel();
                let (kill_tx, kill_rx): (Sender<()>, Receiver<()>) = mpsc::channel();
                let mut stdin_writer = proc.stdin.ok_or("failed to open stdin of child process")?;
                self.stdin = Some(stdin_sender.clone());
                self.kill_tx = Some(kill_tx);
                // self.stdin = Some(Arc::new(Mutex::new(stdin_writer.)));
                let stdout = proc
                    .stdout
                    .ok_or("failed to open stdout of child process")?;
                let reader = BufReader::new(stdout);
                thread::spawn(move || {
                    let stdin_receiver = stdin_receiver;
                    loop {
                        if kill_rx.try_recv().is_ok() {
                            break;
                        }
                        let rec = stdin_receiver.recv().unwrap();
                        println!("writing to stdin: {}", rec);
                        stdin_writer.write_all(rec.as_bytes()).unwrap();
                        stdin_writer.flush().unwrap();
                    }
                    println!("writer thread terminating");
                });
                let uuid_closure = self.uuid.clone();
                let status_closure = self.status.clone();
                let players_closure = self.player_online.clone();
                let flavour_closure = self.flavour.clone();
                let path_closure = self.path.clone();
                let stdin_sender_closure = stdin_sender.clone();
                thread::spawn(move || {
                    use event_parser::parse;
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
                        if let Some(event) = parse(&line, flavour_closure) {
                            match event.1 {
                                event_parser::InstanceEvent::Joined => {
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
                                event_parser::InstanceEvent::Left => {
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
                        if is_player_message(&line, flavour_closure) {
                            println!("player msg: {}", line);
                            match dispatch_macro(
                                &line,
                                flavour_closure,
                                path_closure
                                    .clone()
                                    .parent()
                                    .unwrap()
                                    .parent()
                                    .unwrap()
                                    .join("macros/"),
                            ) {
                                (None, Ok(_)) => {}
                                (None, Err(e)) => {
                                    stdin_sender_closure.send(format!("say {}\n", e)).unwrap()
                                }
                                (Some(vec), Ok(_)) => {
                                    for cmd in vec {
                                        stdin_sender_closure.send(format!("{}\n", cmd)).unwrap();
                                    }
                                }
                                (Some(_), Err(_)) => todo!(),
                            }
                        }
                    }
                    let mut status = status_closure.lock().unwrap();
                    println!("program exiting as reader thread is terminating...");
                    match *status {
                        Status::Starting => println!("instance failed to start"),
                        Status::Stopping => println!("instance stopped properly"),
                        Status::Running => println!("instance exited unexpectedly, restarting..."), //TODO: Restart thru localhost
                        Status::Stopped => println!("instance already stopped"),
                    }
                    *status = Status::Stopped;
                });
                return Ok(());
            }
            Err(_) => {
                *status = Status::Stopped;
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
        self.kill_tx.as_mut().unwrap().send(()).unwrap();
        *status = Status::Stopping;
        self.send_stdin("stop".to_string())?;
        self.player_online.lock().unwrap().clear();
        Ok(())
    }

    pub fn send_stdin(&self, line: String) -> Result<(), String> {
        self.stdin
            .as_ref()
            .unwrap()
            .send(format!("{}\n", line))
            .unwrap();
        Ok(())
    }

    pub fn get_name(&self) -> String {
        self.name.clone()
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
}
mod event_parser {
    use std::{
        fs::File,
        io::{self, BufRead},
        path::PathBuf,
    };

    use super::{Flavour};

    pub enum InstanceEvent {
        Joined,
        Left,
    }

    pub enum Level {
        Info,
        Warn,
        Error,
    }

    pub enum PlayerSpecificEvent {
        // SentCommand {
        //     msg : String
        // },
        ConnectionEvent,
        Said { msg: String },
    }
    pub enum EventVariants {
        PlayerSpecificEvent(PlayerSpecificEvent),
        InstanceEvent,
    }

    pub struct Event {
        time: String,
        level: Level,
        even_variant: EventVariants,
    }
    pub fn is_server_message(line: &String, flavour: Flavour) -> bool {
        match flavour {
            Flavour::Vanilla | Flavour::Fabric => {
                // [*:*:*] [*/* */]: *
                line.matches("[").count() == 2 // fabric and vanilla has two sets of square brackets
                        && line.matches("<").count() == 0 // filter out play sent messages
                        && line.matches(":").count() == 3 // 2 for timestamp + 1 for final
                        && line.matches("/").count() == 1
            }
            Flavour::Paper => todo!(),
            Flavour::Spigot => todo!(),
        }
    }

    pub fn is_player_message(line: &String, flavour: Flavour) -> bool {
        match flavour {
            Flavour::Vanilla | Flavour::Fabric => {
                // [*:*:*] [*/* */]: *
                line.matches("<").count() >= 1 && line.matches(">").count() >= 1
            }
            Flavour::Paper => todo!(),
            Flavour::Spigot => todo!(),
        }
    }
    pub fn parse(line: &String, flavour: Flavour) -> Option<(String, InstanceEvent)> {
        if is_server_message(line, flavour) {
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
    pub fn dispatch_macro(
        line: &String,
        flavour: Flavour,
        path: PathBuf,
    ) -> (Option<Vec<String>>, Result<(), String>) {
        match flavour {
            Flavour::Vanilla | Flavour::Fabric => {
                let i = line.find(">").unwrap();
                let tmp = line.as_str()[i + 2..].to_string();
                // println!("tmp: {}", tmp);
                let iterator = tmp.split_whitespace();
                let mut iter = 0;
                let mut args: Vec<String> = vec![];
                let mut ret: Vec<String> = vec![];
                let mut path_to_macro = path.clone();

                for token in iterator.clone() {
                    if iter == 0 {
                        if token != ".macro" {
                            return (None, Ok(()));
                        }
                    }
                    if iter == 1 {
                        path_to_macro.push(token);
                        println!("path_to_macro: {}", path_to_macro.to_str().unwrap());
                        if !path_to_macro.exists() {
                            return (None, Err("macro does not exist".to_string()));
                        }
                    }
                    if iter >= 2 {
                        args.push(token.to_string());
                    }
                    iter = iter + 1;
                }
                if iter == 1 {
                    return (None, Err("Usage: .macro [macro file] args..".to_string()));
                }
                for line_result in io::BufReader::new(File::open(path_to_macro).unwrap()).lines() {
                    let mut line = line_result.unwrap();
                    let iterline = line.clone();
                    let iter = iterline.split_ascii_whitespace();
                    for token in iter {
                        let mut token_num = token.to_string();
                        if token.chars().next().unwrap() == '$' {
                            token_num.remove(0);
                            match token_num.parse::<usize>() {
                                Ok(num) => {
                                    if num >= args.len() {
                                        return (
                                            None,
                                            Err(format!("token: {} out of range", token)),
                                        );
                                    }
                                    line = line.replace(token, args.get(num).unwrap());
                                    println!("line: {}", line.replace(token, args.get(num).unwrap()));
                                    
                                }
                                Err(_) => {
                                    return (None, Err(format!("token: {} is not valid", token)))
                                }
                            }
                        }
                    }
                    ret.push(line);
                }
                // iter = 0;
                // for mut token in iterator {
                //     let mut token_num = token.to_string();
                //     if token.chars().next().unwrap() == '$' {
                //         token_num.remove(0);
                //         match token_num.parse::<usize>() {
                //             Ok(num) => {
                //                 if num >= args.len() {
                //                     return (None, Err(format!("token: {} out of range", token)));
                //                 }
                //                 token = args.get(num).unwrap().as_str();
                //             }
                //             Err(_) => return (None, Err(format!("token: {} is not valid", token))),
                //         }
                //     };

                //     iter = iter + 1;
                // }
                (Some(ret), Ok(()))
            }

            Flavour::Paper => todo!(),
            Flavour::Spigot => todo!(),
        }
    }
}
