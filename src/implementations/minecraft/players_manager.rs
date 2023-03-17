use std::collections::HashSet;

use crate::{
    event_broadcaster::EventBroadcaster,
    events::{CausedBy, Event, EventInner, InstanceEvent, InstanceEventInner},
    traits::t_player::Player,
    types::{InstanceUuid, Snowflake},
};

use super::player::MinecraftPlayer;

#[derive(Clone)]
pub struct PlayersManager {
    players: HashSet<MinecraftPlayer>,
    event_broadcaster: EventBroadcaster,
    instance_uuid: InstanceUuid,
}

impl PlayersManager {
    pub fn new(event_broadcaster: EventBroadcaster, instance_uuid: InstanceUuid) -> Self {
        Self {
            players: HashSet::new(),
            event_broadcaster,
            instance_uuid,
        }
    }

    pub fn add_player(&mut self, player: MinecraftPlayer, instance_name: String) {
        self.players.insert(player.clone());
        self.event_broadcaster.send(Event {
            event_inner: EventInner::InstanceEvent(InstanceEvent {
                instance_uuid: self.instance_uuid.clone(),
                instance_name,
                instance_event_inner: InstanceEventInner::PlayerChange {
                    player_list: self.players.iter().map(|p| p.clone().into()).collect(),
                    players_joined: HashSet::from([player.into()]),
                    players_left: HashSet::new(),
                },
            }),
            details: "".to_string(),
            snowflake: Snowflake::default(),
            caused_by: CausedBy::Instance {
                instance_uuid: self.instance_uuid.clone(),
            },
        });
    }

    pub fn remove_player(&mut self, player: MinecraftPlayer, instance_name: String) {
        if self.players.remove(&player) {
            self.event_broadcaster.send(Event {
                event_inner: EventInner::InstanceEvent(InstanceEvent {
                    instance_uuid: self.instance_uuid.clone(),
                    instance_name,
                    instance_event_inner: InstanceEventInner::PlayerChange {
                        player_list: self.players.iter().map(|p| p.clone().into()).collect(),
                        players_joined: HashSet::new(),
                        players_left: HashSet::from([player.into()]),
                    },
                }),
                details: "".to_string(),
                snowflake: Snowflake::default(),
                caused_by: CausedBy::Instance {
                    instance_uuid: self.instance_uuid.clone(),
                },
            });
        }
    }

    pub fn remove_by_name(&mut self, player_name: impl AsRef<str>, instance_name: String) {
        if let Some(player) = self
            .players
            .iter()
            .find(|p| p.name == player_name.as_ref())
            .cloned()
        {
            self.remove_player(player, instance_name);
        }
    }

    pub fn count(&self) -> u32 {
        self.players.len() as u32
    }

    pub fn clear(&mut self, instance_name: String) {
        self.event_broadcaster.send(Event {
            event_inner: EventInner::InstanceEvent(InstanceEvent {
                instance_uuid: self.instance_uuid.clone(),
                instance_name,
                instance_event_inner: InstanceEventInner::PlayerChange {
                    player_list: HashSet::new(),
                    players_joined: HashSet::new(),
                    players_left: self.players.iter().map(|p| p.clone().into()).collect(),
                },
            }),
            details: "".to_string(),
            snowflake: Snowflake::default(),
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

impl From<PlayersManager> for HashSet<Player> {
    fn from(val: PlayersManager) -> Self {
        val.players
            .into_iter()
            .map(Player::MinecraftPlayer)
            .collect()
    }
}

#[cfg(test)]
mod tests {

    use tokio;

    use crate::event_broadcaster::EventBroadcaster;

    #[tokio::test]
    async fn test_players_manager() {
        use crate::types::InstanceUuid;
        use crate::{events::InstanceEventInner, traits::t_player::Player};
        use std::collections::HashSet;

        let mock_instance = (InstanceUuid::default(), "mock_instance".to_string());

        let (tx, mut rx) = EventBroadcaster::new(10);
        let mut players_manager = super::PlayersManager::new(tx, mock_instance.0.clone());

        players_manager.add_player(
            super::MinecraftPlayer {
                name: "player1".to_string(),
                uuid: Some("uuid1".to_string()),
            },
            mock_instance.1.clone(),
        );

        players_manager.add_player(
            super::MinecraftPlayer {
                name: "player2".to_string(),
                uuid: Some("uuid2".to_string()),
            },
            mock_instance.1.clone(),
        );

        players_manager.add_player(
            super::MinecraftPlayer {
                name: "player3".to_string(),
                uuid: Some("uuid3".to_string()),
            },
            mock_instance.1.clone(),
        );

        players_manager.remove_by_name("player2", mock_instance.1.clone());

        players_manager.remove_by_name("player3", mock_instance.1.clone());

        players_manager.clear(mock_instance.1.clone());

        let expected = vec![
            InstanceEventInner::PlayerChange {
                player_list: HashSet::from([Player::MinecraftPlayer(super::MinecraftPlayer {
                    name: "player1".to_string(),
                    uuid: Some("uuid1".to_string()),
                })]),
                players_joined: HashSet::from([Player::MinecraftPlayer(super::MinecraftPlayer {
                    name: "player1".to_string(),
                    uuid: Some("uuid1".to_string()),
                })]),
                players_left: HashSet::new(),
            },
            InstanceEventInner::PlayerChange {
                player_list: HashSet::from([
                    Player::MinecraftPlayer(super::MinecraftPlayer {
                        name: "player1".to_string(),
                        uuid: Some("uuid1".to_string()),
                    }),
                    Player::MinecraftPlayer(super::MinecraftPlayer {
                        name: "player2".to_string(),
                        uuid: Some("uuid2".to_string()),
                    }),
                ]),
                players_joined: HashSet::from([Player::MinecraftPlayer(super::MinecraftPlayer {
                    name: "player2".to_string(),
                    uuid: Some("uuid2".to_string()),
                })]),
                players_left: HashSet::new(),
            },
            InstanceEventInner::PlayerChange {
                player_list: HashSet::from([
                    Player::MinecraftPlayer(super::MinecraftPlayer {
                        name: "player1".to_string(),
                        uuid: Some("uuid1".to_string()),
                    }),
                    Player::MinecraftPlayer(super::MinecraftPlayer {
                        name: "player2".to_string(),
                        uuid: Some("uuid2".to_string()),
                    }),
                    Player::MinecraftPlayer(super::MinecraftPlayer {
                        name: "player3".to_string(),
                        uuid: Some("uuid3".to_string()),
                    }),
                ]),
                players_joined: HashSet::from([Player::MinecraftPlayer(super::MinecraftPlayer {
                    name: "player3".to_string(),
                    uuid: Some("uuid3".to_string()),
                })]),
                players_left: HashSet::new(),
            },
            InstanceEventInner::PlayerChange {
                player_list: HashSet::from([
                    Player::MinecraftPlayer(super::MinecraftPlayer {
                        name: "player1".to_string(),
                        uuid: Some("uuid1".to_string()),
                    }),
                    Player::MinecraftPlayer(super::MinecraftPlayer {
                        name: "player3".to_string(),
                        uuid: Some("uuid3".to_string()),
                    }),
                ]),
                players_joined: HashSet::new(),
                players_left: HashSet::from([Player::MinecraftPlayer(super::MinecraftPlayer {
                    name: "player2".to_string(),
                    uuid: Some("uuid2".to_string()),
                })]),
            },
            InstanceEventInner::PlayerChange {
                player_list: HashSet::from([Player::MinecraftPlayer(super::MinecraftPlayer {
                    name: "player1".to_string(),
                    uuid: Some("uuid1".to_string()),
                })]),
                players_joined: HashSet::new(),
                players_left: HashSet::from([Player::MinecraftPlayer(super::MinecraftPlayer {
                    name: "player3".to_string(),
                    uuid: Some("uuid3".to_string()),
                })]),
            },
            InstanceEventInner::PlayerChange {
                player_list: HashSet::new(),
                players_joined: HashSet::new(),
                players_left: HashSet::from([Player::MinecraftPlayer(super::MinecraftPlayer {
                    name: "player1".to_string(),
                    uuid: Some("uuid1".to_string()),
                })]),
            },
        ];

        for expected in expected {
            let event = rx.recv().await.unwrap();
            match event.event_inner {
                crate::events::EventInner::InstanceEvent(instance_event) => {
                    assert_eq!(instance_event.instance_uuid, mock_instance.0);
                    assert_eq!(instance_event.instance_name, mock_instance.1);
                    assert_eq!(instance_event.instance_event_inner, expected);
                }
                _ => panic!("Unexpected event"),
            }
        }
    }
}
