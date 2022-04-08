use regex::Regex;
use serde::Serialize;
use std::process::{Child, Command, Stdio};


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
}

pub struct EventProcessor {
    on_server_message: Vec<Box<dyn Fn(ServerMessage) + Send>>,

    on_player_event: Vec<Box<dyn Fn(PlayerEvent) + Send>>,
    on_player_joined: Vec<Box<dyn Fn(String) + Send>>,
    on_player_left: Vec<Box<dyn Fn(String) + Send>>,
    on_chat: Vec<Box<dyn Fn(String, String) + Send>>,
    on_player_died: Vec<Box<dyn Fn(String, String) + Send>>,
    on_player_illegal_moved: Vec<Box<dyn Fn(String, String) + Send>>,
    on_player_advancement: Vec<Box<dyn Fn(String, String) + Send>>,
    on_server_startup: Vec<Box<dyn Fn() + Send>>,
    on_server_shutdown: Vec<Box<dyn Fn() + Send>>,
    on_custom_event: Vec<Box<dyn Fn(String) + Send>>,

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
            on_server_startup: vec![],
            on_server_shutdown: vec![],
            on_custom_event: vec![],
        }
    }
    pub fn process(&self, line: &String) {
        if let Some(msg) = parse(&line) {
            for f in &self.on_server_message {
                f(msg.clone());
            }
            if let Some(player_event) = parse_player_event(&msg.message) {
                for f in &self.on_player_event {
                    f(player_event.clone());
                }
                match player_event.event {
                    PlayerEventVarient::Joined => {
                        for f in &self.on_player_joined {
                            f(player_event.player.clone());
                        }
                    }
                    PlayerEventVarient::Left => {
                        for f in &self.on_player_left {
                            f(player_event.player.clone());
                        }
                    }
                    PlayerEventVarient::Chat(s) => {
                        for f in &self.on_chat {
                            f(player_event.player.clone(), s.clone());
                        }
                    }
                    PlayerEventVarient::Died(s) => {
                        for f in &self.on_player_died {
                            f(player_event.player.clone(), s.clone());
                        }
                    }
                    PlayerEventVarient::IllegalMove(s) => {
                        for f in &self.on_player_illegal_moved {
                            f(player_event.player.clone(), s.clone());
                        }
                    }
                    PlayerEventVarient::Advancement(s) => {
                        for f in &self.on_player_advancement {
                            f(player_event.player.clone(), s.clone());
                        }
                    }
                }
            } else {
                let re = Regex::new(r"Done .+! For help, type").unwrap();
                if re.is_match(line) {
                    for f in &self.on_server_startup {
                        f();
                    }
                }
            }
        }
    }

    pub fn on_server_message(&mut self, callback: Box<dyn Fn(ServerMessage) + Send>) {
        self.on_server_message.push(callback);
    }
    pub fn on_player_event(&mut self, callback: Box<dyn Fn(PlayerEvent) + Send>) {
        self.on_player_event.push(callback);
    }

    pub fn on_player_joined(&mut self, callback: Box<dyn Fn(String) + Send>) {
        self.on_player_joined.push(callback);
    }
    pub fn on_player_left(&mut self, callback: Box<dyn Fn(String) + Send>) {
        self.on_player_left.push(callback);
    }
    pub fn on_chat(&mut self, callback: Box<dyn Fn(String, String) + Send>) {
        self.on_chat.push(callback);
    }
    pub fn on_player_died(&mut self, callback: Box<dyn Fn(String, String) + Send>) {
        self.on_player_died.push(callback);
    }
    pub fn on_player_illegal_moved(&mut self, callback: Box<dyn Fn(String, String) + Send>) {
        self.on_player_illegal_moved.push(callback);
    }
    pub fn on_player_advancement(&mut self, callback: Box<dyn Fn(String, String) + Send>) {
        self.on_player_advancement.push(callback);
    }
    pub fn on_server_startup(&mut self, callback: Box<dyn Fn() + Send>) {
        self.on_server_startup.push(callback);
    }
    pub fn on_server_shutdown(&mut self, callback: Box<dyn Fn() + Send>) {
        self.on_server_shutdown.push(callback);
    }
    pub fn notify_server_shutdown(&self) {
        for f in &self.on_server_shutdown {
            f();
        }
    }

    pub fn on_custom_event(&mut self, callback: Box<dyn Fn(String) + Send>) {
        self.on_custom_event.push(callback);
    }
    pub fn notify_custom_event(&self, event: String) {
        for f in &self.on_custom_event {
            f(event.clone());
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
        let spigot_regex =
            Regex::new(r"^\[([0-9][0-9]:[0-9][0-9]:[0-9][0-9]) (.+)\]: (.+)").unwrap();

        if vanilla_regex.is_match(s.as_str()) {
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
        // if the first char is [, it is a /say message
        if s.chars().next().unwrap() == '[' {
            None
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
