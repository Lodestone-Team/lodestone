use regex::Regex;
use rocket::tokio::sync::broadcast::{channel, Receiver, Sender};
use serde::Serialize;

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

pub enum SubscribeType {
    OnPlayerJoined,
    OnPlayerLeft,
    OnPlayerChat,
    OnPlayerDied,
    OnPlayerIllegalMoved,
    OnPlayerAdvancement,
}

#[derive(Clone)]
pub enum BroadcastMessage<T> {
    Message(T),
    Kill,
}
use BroadcastMessage::{Kill, Message};
pub struct EventProcessor {
    server_msg_tx: Sender<BroadcastMessage<ServerMessage>>,
    player_all_event_tx: Sender<BroadcastMessage<PlayerEvent>>,
    player_joined_tx: Sender<BroadcastMessage<(String, String)>>,
    player_left_tx: Sender<BroadcastMessage<(String, String)>>,
    player_chat_tx: Sender<BroadcastMessage<(String, String)>>,
    player_died_tx: Sender<BroadcastMessage<(String, String)>>,
    player_illegal_moved_tx: Sender<BroadcastMessage<(String, String)>>,
    player_advancement_tx: Sender<BroadcastMessage<(String, String)>>,
    server_finished_setup_tx: Sender<BroadcastMessage<()>>,
}

impl EventProcessor {
    pub fn new() -> EventProcessor {
        let (server_msg_tx, _) = channel(16);
        let (player_all_event_tx, _) = channel(16);
        let (player_joined_tx, _) = channel(16);
        let (player_left_tx, _) = channel(16);
        let (player_chat_tx, _) = channel(16);
        let (player_died_tx, _) = channel(16);
        let (player_illegal_moved_tx, _) = channel(16);
        let (player_advancement_tx, _) = channel(16);
        let (server_finished_setup_tx, _) = channel(16);

        EventProcessor {
            server_msg_tx,
            player_all_event_tx,
            player_joined_tx,
            player_left_tx,
            player_chat_tx,
            player_died_tx,
            player_illegal_moved_tx,
            player_advancement_tx,
            server_finished_setup_tx,
        }
    }
    pub fn process(&self, line: &String) {
        if let Some(msg) = parse(&line) {
            self.server_msg_tx.send(Message(msg.clone()));
            if let Some(player_event) = parse_player_event(&msg.message) {
                self.player_all_event_tx.send(Message(player_event.clone()));
                match player_event.event {
                    PlayerEventVarient::Joined => {
                        self.player_joined_tx
                            .send(Message((player_event.player.clone(), msg.message.clone())));
                    }
                    PlayerEventVarient::Left => {
                        self.player_left_tx
                            .send(Message((player_event.player.clone(), msg.message.clone())));
                    }
                    PlayerEventVarient::Chat(s) => {
                        self.player_chat_tx
                            .send(Message((player_event.player.clone(), s)));
                    }
                    PlayerEventVarient::Died(s) => {
                        self.player_died_tx
                            .send(Message((player_event.player.clone(), s)));
                    }
                    PlayerEventVarient::IllegalMove(s) => {
                        self.player_illegal_moved_tx
                            .send(Message((player_event.player.clone(), s)));
                    }
                    PlayerEventVarient::Advancement(s) => {
                        self.player_advancement_tx
                            .send(Message((player_event.player.clone(), s)));
                    }
                }
            } else {
                let re = Regex::new(r"Done .+! For help, type").unwrap();
                if re.is_match(line) {
                    self.server_finished_setup_tx.send(Message(()));
                }
            }
        }
    }

    pub fn kill(&self) {
        self.player_advancement_tx.send(Kill);
        self.player_all_event_tx.send(Kill);
        self.player_chat_tx.send(Kill);
        self.player_died_tx.send(Kill);
        self.player_illegal_moved_tx.send(Kill);
        self.player_joined_tx.send(Kill);
        self.player_left_tx.send(Kill);
        self.server_msg_tx.send(Kill);
    }

    pub fn subscribe_msg(&self) -> Receiver<BroadcastMessage<ServerMessage>> {
        self.server_msg_tx.subscribe()
    }
    pub fn subscribe_all_event(&self) -> Receiver<BroadcastMessage<PlayerEvent>> {
        self.player_all_event_tx.subscribe()
    }
    pub fn subscribe_event(
        &self,
        stype: SubscribeType,
    ) -> Receiver<BroadcastMessage<(String, String)>> {
        match stype {
            SubscribeType::OnPlayerJoined => self.player_joined_tx.subscribe(),
            SubscribeType::OnPlayerLeft => self.player_left_tx.subscribe(),
            SubscribeType::OnPlayerChat => self.player_chat_tx.subscribe(),
            SubscribeType::OnPlayerDied => self.player_died_tx.subscribe(),
            SubscribeType::OnPlayerIllegalMoved => self.player_illegal_moved_tx.subscribe(),
            SubscribeType::OnPlayerAdvancement => self.player_advancement_tx.subscribe(),
        }
    }
    pub fn subscribe_server_finished_setup(&self) -> Receiver<BroadcastMessage<()>> {
        self.server_finished_setup_tx.subscribe()
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
                let re_illegal_movement = Regex::new(r"moved too quickly! (.+)").unwrap();
                if re_illegal_movement.is_match(s) {
                    Some(PlayerEvent {
                        player: s_vec[0].to_string(),
                        event: PlayerEventVarient::IllegalMove(
                            re_illegal_movement
                                .captures(s)
                                .unwrap()
                                .get(1)
                                .unwrap()
                                .as_str()
                                .to_string(),
                        ),
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
