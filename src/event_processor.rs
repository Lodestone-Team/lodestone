use regex::Regex;
use serde::Serialize;
use std::{process::{Child, Command, Stdio}, thread, sync::{Arc, Mutex}};

use crate::managers::server_instance::ServerInstance;

use self::parser::{parse, parse_player_event};

#[derive(Clone, Serialize)]
pub enum Signal {
    Info,
    Warn,
    Error,
}
#[derive(Clone, Serialize)]
pub struct ServerMessage {
    pub timestamp: String,
    pub signal: Signal,
    pub message: String,
    pub player_event: Option<PlayerEvent>,
}
#[derive(Clone, Serialize)]
pub struct PlayerEvent {
    pub player: String,
    pub event: PlayerEventVarient,
}
#[derive(Clone, Serialize)]
pub enum PlayerEventVarient {
    Joined,
    Left,
    Chat(String),
    Died(String),
    IllegalMove(String),
    Advancement(String),
    Command(String)
}

pub struct EventProcessor {
    pub on_server_message: Vec<Arc<dyn Fn(ServerMessage) + Send + Sync>>,
    pub on_player_event: Vec<Arc<dyn Fn(PlayerEvent) + Send + Sync>>,
    pub on_player_joined: Vec<Arc<dyn Fn(String) + Send + Sync>>,
    pub on_player_left: Vec<Arc<dyn Fn(String) + Send + Sync>>,
    pub on_chat: Vec<Arc<dyn Fn(String, String) + Send + Sync>>,
    pub on_player_died: Vec<Arc<dyn Fn(String, String) + Send + Sync>>,
    pub on_player_illegal_moved: Vec<Arc<dyn Fn(String, String) + Send + Sync>>,
    pub on_player_advancement: Vec<Arc<dyn Fn(String, String) + Send + Sync>>,
    pub on_player_send_command: Vec<Arc<dyn Fn(String, String) + Send + Sync>>,
    pub on_server_startup: Vec<Arc<dyn Fn() + Send + Sync>>,
    pub on_server_shutdown: Vec<Arc<dyn Fn() + Send + Sync>>,
    pub on_custom_event: Vec<Arc<dyn Fn(String) + Send + Sync>>,
}

impl EventProcessor {
    pub fn new() -> EventProcessor {
        EventProcessor {
            on_server_message: vec![],
            on_player_event: vec![],
            on_player_joined: vec![],
            on_player_left: vec![],
            on_chat: vec![],
            on_player_died: vec![],
            on_player_illegal_moved: vec![],
            on_player_advancement: vec![],
            on_player_send_command: vec![],
            on_server_startup: vec![],
            on_server_shutdown: vec![],
            on_custom_event: vec![],
        }
    }


    pub fn process(&self, line: &String) {
        if let Some(msg) = parse(&line) {
            for f in &self.on_server_message {
                let f = f.clone();
                let msg = msg.clone();
                thread::spawn(move || f(msg));
            }
            if let Some(player_event) = parse_player_event(&msg.message) {
                for f in &self.on_player_event {
                    let f = f.clone();
                    let player = player_event.clone();
                    thread::spawn(move || f(player));
                }
                match player_event.event {
                    PlayerEventVarient::Joined => {
                        for f in &self.on_player_joined {
                            let f = f.clone();
                            let player = player_event.player.clone();
                            thread::spawn(move || f(player));
                        }
                    }
                    PlayerEventVarient::Left => {
                        for f in &self.on_player_left {
                            let f = f.clone();
                            let player = player_event.player.clone();
                            thread::spawn(move || f(player));
                        }
                    }
                    PlayerEventVarient::Chat(s) => {
                        for f in &self.on_chat {
                            let f = f.clone();
                            let s = s.clone();
                            let player = player_event.player.clone();
                            thread::spawn(move || f(player, s));
                        }
                    }
                    PlayerEventVarient::Died(s) => {
                        for f in &self.on_player_died {
                            let f = f.clone();
                            let s = s.clone();
                            let player = player_event.player.clone();
                            thread::spawn(move || f(player, s));
                        }
                    }
                    PlayerEventVarient::IllegalMove(s) => {
                        for f in &self.on_player_illegal_moved {
                            let f = f.clone();
                            let s = s.clone();
                            let player = player_event.player.clone();
                            thread::spawn(move || f(player, s));
                        }
                    }
                    PlayerEventVarient::Advancement(s) => {
                        for f in &self.on_player_advancement {
                            let f = f.clone();
                            let s = s.clone();
                            let player = player_event.player.clone();
                            thread::spawn(move || f(player, s));
                        }
                    }
                    PlayerEventVarient::Command(cmd) => {
                        for f in &self.on_player_send_command {
                            let f = f.clone();
                            let cmd = cmd.clone();
                            let player = player_event.player.clone();
                            thread::spawn(move || f(player, cmd));
                        }
                    },
                }
            } else {
                let re = Regex::new(r"Done .+! For help, type").unwrap();
                if re.is_match(line) {
                    for f in &self.on_server_startup {
                        let f = f.clone();
            thread::spawn(move || f());
                    }
                }
            }
        }
    }

    pub fn on_server_message(&mut self, callback: Arc<dyn Fn(ServerMessage) + Send + Sync>) {
        self.on_server_message.push(callback);
    }
    pub fn on_player_event(&mut self, callback: Arc<dyn Fn(PlayerEvent) + Send + Sync>) {
        self.on_player_event.push(callback);
    }

    pub fn on_player_joined(&mut self, callback: Arc<dyn Fn(String) + Send + Sync>) {
        self.on_player_joined.push(callback);
    }
    pub fn on_player_left(&mut self, callback: Arc<dyn Fn(String) + Send + Sync>) {
        self.on_player_left.push(callback);
    }
    pub fn on_chat(&mut self, callback: Arc<dyn Fn(String, String) + Send + Sync>) {
        self.on_chat.push(callback);
    }
    pub fn on_player_died(&mut self, callback: Arc<dyn Fn(String, String) + Send + Sync>) {
        self.on_player_died.push(callback);
    }
    pub fn on_player_illegal_moved(&mut self, callback: Arc<dyn Fn(String, String) + Send + Sync>) {
        self.on_player_illegal_moved.push(callback);
    }
    pub fn on_player_advancement(&mut self, callback: Arc<dyn Fn(String, String) + Send + Sync>) {
        self.on_player_advancement.push(callback);
    }
    pub fn on_server_startup(&mut self, callback: Arc<dyn Fn() + Send + Sync>) {
        self.on_server_startup.push(callback);
    }
    /// triggers ONLY when the subprocess exists, NOT when a shutdown command is sent
    pub fn on_server_shutdown(&mut self, callback: Arc<dyn Fn() + Send + Sync>) {
        self.on_server_shutdown.push(callback);
    }
    pub fn notify_server_shutdown(&mut self) {
        for f in &self.on_server_shutdown {
            let f = f.clone();
            thread::spawn(move || f());
        }
        // self.on_chat.clear();
        // self.on_player_advancement.clear();
        // self.on_player_died.clear();
        // self.on_player_illegal_moved.clear();
        // self.on_player_joined.clear();
        // self.on_player_left.clear();
        // self.on_player_send_command.clear();
        // self.on_player_event.clear();
        // self.on_server_message.clear();
        // self.on_server_startup.clear();
        // self.on_server_shutdown.clear();
        

    }

    pub fn on_player_send_command(&mut self, callback: Arc<dyn Fn(String, String) + Send + Sync>) {
        self.on_player_send_command.push(callback);
    }

    // pub fn on_custom_event(&mut self, callback: Box<dyn Fn(String) + Send + Sync>) {
    //     self.on_custom_event.push(callback);
    // }
    pub fn notify_custom_event(&self, event: String) {
        for f in &self.on_custom_event {
            let f = f.clone();
            let event = event.clone();
            thread::spawn(move || f(event));
    }
}
}

pub mod parser {

    use std::str::FromStr;

    use regex::Regex;

    use super::{PlayerEvent, PlayerEventVarient, ServerMessage, Signal};

    impl FromStr for Signal {
        type Err = ();
        fn from_str(input: &str) -> Result<Signal, Self::Err> {
            let input_lower = input.to_lowercase();
            match input_lower.as_str() {
                "info" => Ok(Signal::Info),
                "warn" => Ok(Signal::Warn),
                "error" => Ok(Signal::Error),
                _ => Err(()),
            }
        }
    }

    pub fn parse(s: &String) -> Option<ServerMessage> {
        let vanilla_regex =
            Regex::new(r"^\[([0-9][0-9]:[0-9][0-9]:[0-9][0-9])\] \[(.+)/(\w+)\]: (.+)").unwrap();

        let fabric_regex =
            Regex::new(r"^\[([0-9][0-9]:[0-9][0-9]:[0-9][0-9])\] \[(.+)\] \[(.+)/(.+)\]: (.+)")
                .unwrap();
        let spigot_regex =
            Regex::new(r"^\[([0-9][0-9]:[0-9][0-9]:[0-9][0-9]) (.+)\]: (.+)").unwrap();
        if fabric_regex.is_match(s.as_str()) {
            let cap = fabric_regex.captures(s.as_str()).unwrap();
            let message = cap.get(5).unwrap().as_str().to_string();
            Some(ServerMessage {
                timestamp: cap.get(1).unwrap().as_str().to_string(),
                signal: Signal::from_str(cap.get(2).unwrap().as_str()).unwrap(),
                message: message.clone(),
                player_event: parse_player_event(&message),
            })
        } else if vanilla_regex.is_match(s.as_str()) {
            let cap = vanilla_regex.captures(s.as_str()).unwrap();
            let message = cap.get(4).unwrap().as_str().to_string();
            Some(ServerMessage {
                timestamp: cap.get(1).unwrap().as_str().to_string(),
                signal: Signal::from_str(cap.get(3).unwrap().as_str()).unwrap(),
                message: message.clone(),
                player_event: parse_player_event(&message),
            })
        } else if spigot_regex.is_match(s.as_str()) {
            let cap = spigot_regex.captures(s.as_str()).unwrap();
            let message = cap.get(3).unwrap().as_str().to_string();
            Some(ServerMessage {
                timestamp: cap.get(1).unwrap().as_str().to_string(),
                signal: Signal::from_str(cap.get(2).unwrap().as_str()).unwrap(),
                message: message.clone(),
                player_event: parse_player_event(&message),
            })
        } else {
            None
        }
    }

    pub fn parse_player_event(s: &String) -> Option<PlayerEvent> {
        let s_vec: Vec<&str> = s.split(" ").collect();
        let re_player_command = Regex::new(r"\[(\w+): (.+)\]").unwrap();
        // if the first char is [, it is a command
        if re_player_command.is_match(s) {
            let cap = re_player_command.captures(s).unwrap();
            Some(PlayerEvent {
                event: PlayerEventVarient::Command(cap.get(2).unwrap().as_str().to_string()),
                player: cap.get(1).unwrap().as_str().to_string(),
            })
        } else if is_player_message(&s) {
            let re = Regex::new(r"^<(.+)> (.+)").unwrap();
            let cap = re.captures(s.as_str()).unwrap();
            Some(PlayerEvent {
                player: cap.get(1).unwrap().as_str().to_string(),
                event: PlayerEventVarient::Chat(cap.get(2).unwrap().as_str().to_string()),
            })
        } else if s.contains("joined the game") {
            Some(PlayerEvent {
                player: s_vec[0].to_string(),
                event: PlayerEventVarient::Joined,
            })
        } else if s.contains("left the game") {
            let s_vec: Vec<&str> = s.split(" ").collect();

            Some(PlayerEvent {
                player: s_vec[0].to_string(),
                event: PlayerEventVarient::Left,
            })
        } else {
            // match for advancement
            let re_chanllenge = Regex::new(r"completed the challenge \[(.+)\]").unwrap();
            let re_advancement = Regex::new(r"has made the advancement \[(.+)\]").unwrap();
            if re_advancement.is_match(s) || re_chanllenge.is_match(s) {
                Some(PlayerEvent {
                    player: s_vec[0].to_string(),
                    event: PlayerEventVarient::Advancement(
                        re_advancement
                            .captures(s)
                            .unwrap()
                            .get(1)
                            .unwrap()
                            .as_str()
                            .to_string(),
                    ),
                })
            } else {
                // match for illegal movement
                let re_moved_too_quick = Regex::new(r"moved too quickly! (.+)").unwrap();
                let re_moved_wrongly = Regex::new(r"moved wrongly!").unwrap();

                if re_moved_too_quick.is_match(s) {
                    Some(PlayerEvent {
                        player: s_vec[0].to_string(),
                        event: PlayerEventVarient::IllegalMove(
                            re_moved_too_quick
                                .captures(s)
                                .unwrap()
                                .get(1)
                                .unwrap()
                                .as_str()
                                .to_string(),
                        ),
                    })
                } else if re_moved_wrongly.is_match(s) {
                    Some(PlayerEvent {
                        player: s_vec[0].to_string(),
                        event: PlayerEventVarient::IllegalMove("moved wrongly".to_string()),
                    })
                } else {
                    None
                }
            }
        }
    }

    pub fn is_player_message(s: &String) -> bool {
        return s.chars().next().unwrap() == '<';
    }
}
