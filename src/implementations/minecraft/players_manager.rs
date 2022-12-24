use std::collections::HashSet;

use serde_json::Value;
use tokio::sync::broadcast::Sender;

use crate::{
    events::{CausedBy, Event, EventInner, InstanceEvent, InstanceEventInner},
    prelude::get_snowflake,
    traits::t_player::Player,
};

use super::player::MinecraftPlayer;

#[derive(Clone)]
pub struct PlayersManager {
    players: HashSet<MinecraftPlayer>,
    event_broadcaster: Sender<Event>,
    instance_uuid: String,
}

impl PlayersManager {
    pub fn new(event_broadcaster: Sender<Event>, instance_uuid: String) -> Self {
        Self {
            players: HashSet::new(),
            event_broadcaster,
            instance_uuid,
        }
    }

    pub fn add_player(&mut self, player: MinecraftPlayer, instance_name: String) {
        self.players.insert(player.clone());
        let _ = self.event_broadcaster.send(Event {
            event_inner: EventInner::InstanceEvent(InstanceEvent {
                instance_uuid: self.instance_uuid.clone(),
                instance_name,
                instance_event_inner: InstanceEventInner::PlayerChange {
                    player_list: self.players.iter().map(|p| p.name.to_owned()).collect(),
                    players_joined: HashSet::from([player.name]),
                    players_left: HashSet::new(),
                },
            }),
            details: "".to_string(),
            snowflake: get_snowflake(),
            caused_by: CausedBy::Instance {
                instance_uuid: self.instance_uuid.clone(),
            },
        });
    }

    pub fn remove_player(&mut self, player: MinecraftPlayer, instance_name: String) {
        if self.players.remove(&player) {
            let _ = self.event_broadcaster.send(Event {
                event_inner: EventInner::InstanceEvent(InstanceEvent {
                    instance_uuid: self.instance_uuid.clone(),
                    instance_name,
                    instance_event_inner: InstanceEventInner::PlayerChange {
                        player_list: self.players.iter().map(|p| p.name.to_owned()).collect(),
                        players_joined: HashSet::new(),
                        players_left: HashSet::from([player.name]),
                    },
                }),
                details: "".to_string(),
                snowflake: get_snowflake(),
                caused_by: CausedBy::Instance {
                    instance_uuid: self.instance_uuid.clone(),
                },
            });
        }
    }

    pub fn remove_by_name(&mut self, player_name: impl AsRef<str>, instance_name: String) {
        if let Some(player) = self.players.iter().find(|p| p.name == player_name.as_ref()).cloned() {
            self.remove_player(player, instance_name);
        }
    }

    pub fn count(&self) -> u32 {
        self.players.len() as u32
    }

    pub fn clear(&mut self, instance_name: String) {
        let _ = self.event_broadcaster.send(Event {
            event_inner: EventInner::InstanceEvent(InstanceEvent {
                instance_uuid: self.instance_uuid.clone(),
                instance_name,
                instance_event_inner: InstanceEventInner::PlayerChange {
                    player_list: HashSet::new(),
                    players_joined: HashSet::new(),
                    players_left: self.players.iter().map(|p| p.name.to_owned()).collect(),
                },
            }),
            details: "".to_string(),
            snowflake: get_snowflake(),
            caused_by: CausedBy::Instance {
                instance_uuid: self.instance_uuid.clone(),
            },
        });
        self.players.clear();
    }

    
}

impl AsRef<HashSet<MinecraftPlayer>> for PlayersManager {
    fn as_ref(&self) -> &HashSet<MinecraftPlayer> {
        &self.players
    }
}

impl Into<HashSet<Player>> for PlayersManager {
    fn into(self) -> HashSet<Player> {
        self.players
            .into_iter()
            .map(Player::MinecraftPlayer)
            .collect()
    }
}
