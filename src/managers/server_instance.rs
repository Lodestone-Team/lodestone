use crate::event_processor::{EventProcessor, PlayerEventVarient};
use mongodb::{bson::doc, sync::Client};
use rocket::tokio;
use serde::{Deserialize, Serialize};
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fmt, thread, time};

use self::macro_code::dispatch_macro;

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

// impl fmt::Display for Flavour {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         match self {
//             Flavour::Vanilla => write!(f, "vanilla"),
//             Flavour::Fabric => write!(f, "fabric"),
//             Flavour::Paper => write!(f, "paper"),
//             Flavour::Spigot => write!(f, "spigot"),
//         }
//     }
// }

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
    port: u32,
    pub stdin: Option<Arc<Mutex<ChildStdin>>>,
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
            port: config.port.expect("no port provided"),
            kill_tx: None,
            uuid: config.uuid.as_ref().unwrap().clone(),
            player_online: Arc::new(Mutex::new(vec![])),
            instance_config: config.clone(),
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
        Command::new("bash")
            .arg(&self.path.join("prelaunch.sh"))
            .output()
            .map_err(|e| println!("{}", e.to_string()));
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
                let stdin = Arc::new(Mutex::new(stdin));
                let event_processor = Arc::new(Mutex::new(EventProcessor::new()));
                self.stdin = Some(stdin.clone());
                let stdout = proc
                    .stdout
                    .ok_or("failed to open stdout of child process")?;
                let reader = BufReader::new(stdout);
                let path_closure = self.path.clone();

                let players_closure = self.player_online.clone();
                let uuid_closure = self.uuid.clone();
                let event_processor_closure = event_processor.clone();
                let status_closure = self.status.clone();
                thread::spawn(move || {
                    for line_result in reader.lines() {
                        let line = line_result.unwrap();
                        println!("server said: {}", line);
                        event_processor_closure.lock().unwrap().process(&line);
                    }
                    event_processor_closure.lock().unwrap().notify_server_shutdown();
                    let mut status = status_closure.lock().unwrap();
                    players_closure.lock().unwrap().clear();
                    println!("program exiting as reader thread is terminating...");
                    match *status {
                        Status::Starting => println!("instance failed to start"),
                        Status::Stopping => println!("instance stopped properly"),
                        Status::Running => println!("instance exited unexpectedly, restarting..."), //TODO: Restart thru localhost
                        Status::Stopped => println!("instance already stopped"),
                    }
                    *status = Status::Stopped;
                });

                let mongodb_client_closure = mongodb_client.clone();
                event_processor.lock().unwrap().on_server_message(Box::new(
                    move |server_message| {
                        mongodb_client_closure
                            .database(uuid_closure.as_str())
                            .collection("logs")
                            .insert_one(server_message, None)
                            .unwrap();
                    },
                ));

                let status_closure = self.status.clone();
                event_processor
                    .lock()
                    .unwrap()
                    .on_server_startup(Box::new(move || {
                        *status_closure.lock().unwrap() = Status::Running;
                    }));
                let uuid_closure = self.uuid.clone();
                let mongodb_client_closure = mongodb_client.clone();
                let event_processor_closure = event_processor.clone();
                event_processor
                    .lock()
                    .unwrap()
                    .on_player_event(Box::new(move |player_event| {
                        mongodb_client_closure
                            .database(uuid_closure.as_str())
                            .collection("events")
                            .insert_one(player_event.clone(), None);
                    }));
                let players_closure = self.player_online.clone();
                event_processor
                    .lock()
                    .unwrap()
                    .on_player_joined(Box::new(move |player| {
                        players_closure.lock().unwrap().push(player);
                    }));

                let players_closure = self.player_online.clone();
                event_processor
                    .lock()
                    .unwrap()
                    .on_player_left(Box::new(move |player| {
                        // remove player from players_closur
                        players_closure.lock().unwrap().retain(|p| p != &player);

                    }));

                event_processor
                    .lock()
                    .unwrap()
                    .on_chat(Box::new(move |player, msg| {
                        let a = stdin.clone();
                        let event_processor_closure = event_processor_closure.clone();
                        let path_closure = path_closure.clone();
                        thread::spawn(move || {
                            dispatch_macro(
                                &msg,
                                path_closure
                                    .clone()
                                    .parent()
                                    .unwrap()
                                    .parent()
                                    .unwrap()
                                    .join("macros/"),
                                a,
                                event_processor_closure.clone(),
                            );
                        });
                    }));

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
        *status = Status::Stopping;
        self.send_stdin("stop".to_string())?;
        self.player_online.lock().unwrap().clear();
        Ok(())
    }

    pub fn send_stdin(&self, line: String) -> Result<(), String> {
        match self.stdin.clone().as_mut() {
            Some(stdin_mutex) => match stdin_mutex.lock() {
                Ok(mut stdin) => Ok(stdin
                    .write_all(format!("{}\n", line).as_bytes())
                    .map_err(|_| "failed to write to process's stdin".to_string())?),
                Err(_) => Err("failed to acquire lock on stdin".to_string()),
            },
            None => Err("could not find stdin of process".to_string()),
        }
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
mod macro_code {
    use std::{
        collections::HashMap,
        fs::File,
        io::{self, BufRead, Write},
        path::PathBuf,
        process::ChildStdin,
        sync::{
            mpsc::{self, Sender},
            Arc, Mutex, MutexGuard,
        },
        thread, time,
    };

    use crate::event_processor::EventProcessor;
    enum Data {
        Int(i32),
        String(String),
    }

    impl Data {
        fn get_string(&self) -> Option<String> {
            match self {
                Data::Int(i) => Some(i.to_string()),
                Data::String(s) => None,
            }
        }
        fn get_int(&self) -> Option<i32> {
            match self {
                Data::Int(i) => Some(*i),
                Data::String(s) => None,
            }
        }
    }

    fn eval(exp: &String, sym_table: &HashMap<String, Data>) -> Option<Data> {
        if exp.chars().next().unwrap() == '$' {
            let data = sym_table.get(&exp.as_str()[1..].to_string()).unwrap();
            match data {
                Data::Int(n) => Some(Data::Int(n.clone())),
                Data::String(s) => Some(Data::String(s.clone())),
            }
        } else {
            Some(Data::Int(exp.parse::<i32>().unwrap()))
        }
    }

    pub fn dispatch_macro(
        line: &String,
        path: PathBuf,
        stdin_sender: Arc<Mutex<ChildStdin>>,
        event_processor: Arc<Mutex<EventProcessor>>,
    ) {
        let iterator = line.split_whitespace();
        let mut iter = 0;
        let mut path_to_macro = path.clone();
        let mut sym_table: HashMap<String, Data> = HashMap::new();
        for token in iterator.clone() {
            if iter == 0 {
                if token != ".macro" {
                    return;
                }
            }
            if iter == 1 {
                path_to_macro.push(token);
                println!("path_to_macro: {}", path_to_macro.to_str().unwrap());
                if !path_to_macro.exists() {
                    stdin_sender
                        .lock()
                        .as_mut()
                        .unwrap()
                        .write_all(format!("/say macro {} does no exist\n", token).as_bytes());
                    return;
                }
            }
            if iter >= 2 {
                match token.parse::<i32>() {
                    Ok(n) => {
                        sym_table.insert((iter - 2).to_string(), Data::Int(n));
                    }
                    Err(_) => {
                        sym_table.insert((iter - 2).to_string(), Data::String(token.to_string()));
                    }
                }
            }
            iter = iter + 1;
        }
        if iter == 1 {
            stdin_sender
                .lock()
                .as_mut()
                .unwrap()
                .write_all("/say Usage: .macro [macro file] args..\n".as_bytes());
            return;
        }

        let mut lines: Vec<String> = vec![];

        for line_result in io::BufReader::new(File::open(path_to_macro).unwrap()).lines() {
            lines.push(line_result.unwrap());
        }
        let mut pc = 0;
        while pc < lines.len() {
            let mut line = lines[pc].clone();
            let mut tokens: Vec<String> = vec![];
            for token in line.split_whitespace() {
                tokens.push(token.to_string());
            }
            if tokens.len() == 0 {
                continue;
            }
            match tokens.first().unwrap().as_str() {
                "[" => match tokens.get(1).unwrap().as_str() {
                    "delay" => {
                        thread::sleep(time::Duration::from_secs(
                            tokens
                                .get(2)
                                .unwrap()
                                .parse::<u64>()
                                .map_err(|e| {
                                    stdin_sender
                                        .lock()
                                        .as_mut()
                                        .unwrap()
                                        .write_all((format!("/say {}\n", e)).as_bytes());
                                })
                                .unwrap_or(0),
                        ));
                        pc = pc + 1;
                        continue;
                    }

                    "event" => match tokens.get(2).unwrap().as_str() {
                        "player_joined" => {
                            let (tx, rx) = mpsc::channel();
                            event_processor.lock().unwrap().on_player_joined(Box::new(
                                move |player| {
                                    tx.send(player);
                                },
                            ));
                            sym_table
                                .insert("PLAYER_NAME".to_string(), Data::String(rx.recv().unwrap()));
                        }
                        "player_left" => {
                            let (tx, rx) = mpsc::channel();
                            event_processor.lock().unwrap().on_player_left(Box::new(
                                move |player| {
                                    tx.send(player);
                                },
                            ));
                            sym_table
                                .insert("PLAYER_NAME".to_string(), Data::String(rx.recv().unwrap()));
                        }
                        "player_chat" => {
                            let (tx, rx) = mpsc::channel();
                            event_processor.lock().unwrap().on_chat(Box::new(
                                move |player, msg| {
                                    tx.send((player, msg));
                                },
                            ));
                            let (player, msg) = rx.recv().unwrap();
                            sym_table
                                .insert("PLAYER_NAME".to_string(), Data::String(player));
                            sym_table
                                .insert("CHAT_MSG".to_string(), Data::String(msg));
                        }
                        _ => {
                            stdin_sender.lock().as_mut().unwrap().write_all(
                                format!("/say event {} not implemented\n", tokens.get(2).unwrap())
                                    .as_bytes(),
                            );
                            pc = pc + 1;
                            continue;
                        }
                    },

                    "goto" => {
                        pc = eval(tokens.get(2).unwrap(), &sym_table)
                            .unwrap()
                            .get_int()
                            .unwrap() as usize;
                        continue;
                    }
                    "let" => {
                        let mut var_name = tokens.get(2).unwrap().clone();
                        if var_name.chars().next().unwrap() == '$' {
                            var_name.remove(0);
                        }
                        let var_value = tokens.get(4).unwrap();
                        match var_value.parse::<i32>() {
                            Ok(n) => {
                                sym_table.insert(var_name.to_string(), Data::Int(n));
                            }
                            Err(_) => {
                                sym_table.insert(
                                    var_name.to_string(),
                                    Data::String(var_value.to_string()),
                                );
                            }
                        }
                        pc = pc + 1;

                        continue;
                    }
                    "add" => {
                        let mut var_name = tokens.get(2).unwrap().clone();
                        if var_name.chars().next().unwrap() == '$' {
                            var_name.remove(0);
                        }
                        let op_1 = tokens.get(3).unwrap();
                        let op_2 = tokens.get(4).unwrap();
                        sym_table.insert(
                            var_name.to_string(),
                            Data::Int(
                                eval(&op_1, &sym_table).unwrap().get_int().unwrap()
                                    + eval(&op_2, &sym_table).unwrap().get_int().unwrap(),
                            ),
                        );
                        pc = pc + 1;

                        continue;
                    }
                    "sub" => {
                        let mut var_name = tokens.get(2).unwrap().clone();
                        if var_name.chars().next().unwrap() == '$' {
                            var_name.remove(0);
                        }
                        let op_1 = tokens.get(3).unwrap();
                        let op_2 = tokens.get(4).unwrap();
                        sym_table.insert(
                            var_name.to_string(),
                            Data::Int(
                                eval(&op_1, &sym_table).unwrap().get_int().unwrap()
                                    - eval(&op_2, &sym_table).unwrap().get_int().unwrap(),
                            ),
                        );
                        pc = pc + 1;

                        continue;
                    }

                    "mult" => {
                        let mut var_name = tokens.get(2).unwrap().clone();
                        if var_name.chars().next().unwrap() == '$' {
                            var_name.remove(0);
                        }
                        let op_1 = tokens.get(3).unwrap();
                        let op_2 = tokens.get(4).unwrap();
                        sym_table.insert(
                            var_name.to_string(),
                            Data::Int(
                                eval(&op_1, &sym_table).unwrap().get_int().unwrap()
                                    * eval(&op_2, &sym_table).unwrap().get_int().unwrap(),
                            ),
                        );
                        pc = pc + 1;

                        continue;
                    }

                    "div" => {
                        let mut var_name = tokens.get(2).unwrap().clone();
                        if var_name.chars().next().unwrap() == '$' {
                            var_name.remove(0);
                        }
                        let op_1 = tokens.get(3).unwrap();
                        let op_2 = tokens.get(4).unwrap();
                        sym_table.insert(
                            var_name.to_string(),
                            Data::Int(
                                eval(&op_1, &sym_table).unwrap().get_int().unwrap()
                                    / eval(&op_2, &sym_table).unwrap().get_int().unwrap(),
                            ),
                        );
                        pc = pc + 1;
                        continue;
                    }

                    "mod" => {
                        let mut var_name = tokens.get(2).unwrap().clone();
                        if var_name.chars().next().unwrap() == '$' {
                            var_name.remove(0);
                        }
                        let op_1 = tokens.get(3).unwrap();
                        let op_2 = tokens.get(4).unwrap();
                        sym_table.insert(
                            var_name.to_string(),
                            Data::Int(
                                eval(&op_1, &sym_table).unwrap().get_int().unwrap()
                                    % eval(&op_2, &sym_table).unwrap().get_int().unwrap(),
                            ),
                        );
                        pc = pc + 1;
                        continue;
                    }

                    "beq" => {
                        let op_1 = tokens.get(2).unwrap();
                        let op_2 = tokens.get(3).unwrap();
                        let op_3 = tokens.get(4).unwrap();
                        let op_1_data = eval(&op_1, &sym_table).unwrap().get_int().unwrap();
                        let op_2_data = eval(&op_2, &sym_table).unwrap().get_int().unwrap();
                        let op_3_data = eval(&op_3, &sym_table).unwrap().get_int().unwrap();
                        if op_1_data == op_2_data {
                            pc = op_3_data as usize;
                            continue;
                        }
                        pc = pc + 1;
                        continue;
                    }
                    "bne" => {
                        let op_1 = tokens.get(2).unwrap();
                        let op_2 = tokens.get(3).unwrap();
                        let op_3 = tokens.get(4).unwrap();
                        let op_1_data = eval(&op_1, &sym_table).unwrap().get_int().unwrap();
                        let op_2_data = eval(&op_2, &sym_table).unwrap().get_int().unwrap();
                        let op_3_data = eval(&op_3, &sym_table).unwrap().get_int().unwrap();
                        if op_1_data != op_2_data {
                            pc = op_3_data as usize;
                            continue;
                        }
                        pc = pc + 1;
                        continue;
                    }
                    "bge" => {
                        let op_1 = tokens.get(2).unwrap();
                        let op_2 = tokens.get(3).unwrap();
                        let op_3 = tokens.get(4).unwrap();
                        let op_1_data = eval(&op_1, &sym_table).unwrap().get_int().unwrap();
                        let op_2_data = eval(&op_2, &sym_table).unwrap().get_int().unwrap();
                        let op_3_data = eval(&op_3, &sym_table).unwrap().get_int().unwrap();
                        if op_1_data >= op_2_data {
                            pc = op_3_data as usize;
                            continue;
                        }
                        pc = pc + 1;
                    }
                    "ble" => {
                        let op_1 = tokens.get(2).unwrap();
                        let op_2 = tokens.get(3).unwrap();
                        let op_3 = tokens.get(4).unwrap();
                        let op_1_data = eval(&op_1, &sym_table).unwrap().get_int().unwrap();
                        let op_2_data = eval(&op_2, &sym_table).unwrap().get_int().unwrap();
                        let op_3_data = eval(&op_3, &sym_table).unwrap().get_int().unwrap();
                        if op_1_data <= op_2_data {
                            pc = op_3_data as usize;
                            continue;
                        }
                        pc = pc + 1;
                    }
                    "bgt" => {
                        let op_1 = tokens.get(2).unwrap();
                        let op_2 = tokens.get(3).unwrap();
                        let op_3 = tokens.get(4).unwrap();
                        let op_1_data = eval(&op_1, &sym_table).unwrap().get_int().unwrap();
                        let op_2_data = eval(&op_2, &sym_table).unwrap().get_int().unwrap();
                        let op_3_data = eval(&op_3, &sym_table).unwrap().get_int().unwrap();
                        if op_1_data > op_2_data {
                            pc = op_3_data as usize;
                            continue;
                        }
                        pc = pc + 1;
                        continue;
                    }

                    "blt" => {
                        let op_1 = tokens.get(2).unwrap();
                        let op_2 = tokens.get(3).unwrap();
                        let op_3 = tokens.get(4).unwrap();
                        let op_1_data = eval(&op_1, &sym_table).unwrap().get_int().unwrap();
                        let op_2_data = eval(&op_2, &sym_table).unwrap().get_int().unwrap();
                        let op_3_data = eval(&op_3, &sym_table).unwrap().get_int().unwrap();
                        if op_1_data < op_2_data {
                            pc = op_3_data as usize;
                            continue;
                        }
                        pc = pc + 1;
                        continue;
                    }
                    "jalr" => {
                        let op_1 = tokens.get(2).unwrap();
                        let op_1_data = eval(&op_1, &sym_table).unwrap().get_int().unwrap();
                        sym_table.insert("31".to_string(), Data::Int((pc + 1) as i32));
                        pc = op_1_data as usize;
                        continue;
                    }

                    _ => panic!("Unknown instruction {}", tokens.get(1).unwrap()),
                },

                _ => {
                    for token in tokens {
                        if token.chars().next().unwrap() == '$' {
                            let sym = token.as_str()[1..].to_string();
                            let data = sym_table
                                .get(&sym)
                                .ok_or_else(|| {
                                    stdin_sender.lock().as_mut().unwrap().write_all(
                                        format!("/say {} is not defined\n", sym).as_bytes(),
                                    );
                                })
                                .unwrap();
                            match data {
                                Data::Int(n) => {
                                    line = line.replace(&token, &n.to_string());
                                }
                                Data::String(s) => {
                                    line = line.replace(&token, s.as_str());
                                }
                            }
                        }
                    }
                    stdin_sender
                        .lock()
                        .as_mut()
                        .unwrap()
                        .write_all(format!("{}\n", line).as_bytes());
                }
            }
            pc = pc + 1;
        }
    }
}
