use crate::managers::server_instance::event_parser::{dispatch_macro, is_player_message};
use mongodb::{bson::doc, sync::Client};
use serde::{Deserialize, Serialize};
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fmt, thread, time};

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
                    // TODO: generalize parser
                    let re = Regex::new(r": Done").unwrap();
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
                        let path_closure = path_closure.clone();
                        let stdin_sender_closure = stdin_sender_closure.clone();
                        if is_player_message(&line, flavour_closure) {
                            println!("player msg: {}", line);
                            // TODO: use green thread instead
                            thread::spawn(move || {
                                dispatch_macro(
                                    &line,
                                    flavour_closure,
                                    path_closure
                                        .clone()
                                        .parent()
                                        .unwrap()
                                        .parent()
                                        .unwrap()
                                        .join("macros/"),
                                    stdin_sender_closure.clone(),
                                );
                            });
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
        collections::HashMap,
        fs::File,
        io::{self, BufRead},
        path::PathBuf,
        sync::mpsc::Sender,
        thread, time,
    };

    use super::Flavour;

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
            Flavour::Paper => false,
            Flavour::Spigot => todo!(),
        }
    }

    pub fn is_player_message(line: &String, flavour: Flavour) -> bool {
        match flavour {
            Flavour::Vanilla | Flavour::Fabric => {
                // [*:*:*] [*/* */]: *
                line.matches("<").count() >= 1 && line.matches(">").count() >= 1
            }
            Flavour::Paper => true,
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
        flavour: Flavour,
        path: PathBuf,
        stdin_sender: Sender<String>,
    ) {
        match flavour {
            Flavour::Vanilla | Flavour::Fabric => {
                let i = line.find(">").unwrap();
                let tmp = line.as_str()[i + 2..].to_string();
                // println!("tmp: {}", tmp);
                let iterator = tmp.split_whitespace();
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
                            stdin_sender.send(format!("/say macro {} does no exist\n", token));
                            return;
                        }
                    }
                    if iter >= 2 {
                        match token.parse::<i32>() {
                            Ok(n) => {
                                sym_table.insert((iter - 2).to_string(), Data::Int(n));
                            }
                            Err(_) => {
                                sym_table.insert(
                                    (iter - 2).to_string(),
                                    Data::String(token.to_string()),
                                );
                            }
                        }
                    }
                    iter = iter + 1;
                }
                if iter == 1 {
                    stdin_sender.send("/say Usage: .macro [macro file] args..\n".to_string());
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
                                            stdin_sender.send(format!("/say {}\n", e));
                                        })
                                        .unwrap_or(0),
                                ));
                                pc = pc + 1;
                                continue;
                            }
                            "goto" => {
                                pc = eval(tokens.get(2).unwrap(), &sym_table).unwrap().get_int().unwrap() as usize;
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
                                sym_table.insert(
                                    "31".to_string(),
                                    Data::Int((pc + 1) as i32),
                                );
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
                                            stdin_sender
                                                .send(format!("/say {} is not defined\n", sym));
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
                            stdin_sender.send(format!("{}\n", line));
                        }
                    }
                    pc = pc + 1;
                }
            }

            Flavour::Paper => {},
            Flavour::Spigot => todo!(),
        }
    }
}
