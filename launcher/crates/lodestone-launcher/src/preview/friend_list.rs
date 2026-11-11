//! The friend list with made-up friends, for the offscreen screenshots. Nothing is
//! opened, read or sent, and no key is made.

use gpui::Context;
use launcher_core::{
    cores::{connect::SharedServer, lan_open::OpenWorld},
    friend_list::{
        Book, Friend, FriendState, FriendStatus, MyCore, PendingLink, StatePlayer,
        lan::{LAN_CORE_NAME, LAN_SOURCE},
        now,
        ops::{LinkPreview, LinkReply},
    },
};

use crate::servers::{
    Servers,
    friend_list::{CoreServers, MadeLink, PreviewAddFriend, PreviewFriendList, Snapshot, hex},
};

const ME: &str = "K7Q2-M9XD-4TRW-H3BE";

fn key(byte: u8) -> String {
    hex(&[byte; 32])
}

fn core(byte: u8, name: &str) -> launcher_core::friend_list::FriendState {
    FriendState {
        launchers: Vec::new(),
        players: Vec::new(),
        seq: 3,
        expires: now() + 30 * 86_400,
        cores: vec![
            serde_json::from_value(
                serde_json::json!({ "fingerprint": hex(&[byte; 32]), "name": name }),
            )
            .expect("a made-up core"),
        ],
    }
}

/// A made-up Microsoft account's profile.
fn player(name: &str, byte: u8) -> StatePlayer {
    StatePlayer {
        name: name.into(),
        uuid: [byte; 16],
        offline: false,
    }
}

fn friend(byte: u8, name: &str, status: FriendStatus, verified: bool) -> Friend {
    Friend {
        key: key(byte),
        name: name.into(),
        verified,
        status,
        added_at: 1_700_000_000,
        ..Friend::default()
    }
}

fn shared(
    name: &str,
    loader: &str,
    version: Option<&str>,
    modpack: bool,
    state: &str,
) -> SharedServer {
    SharedServer {
        core_name: "Alex's PC".into(),
        core_uuid: "alex-core".into(),
        instance_uuid: name.to_lowercase().replace(' ', "-"),
        name: name.into(),
        game_version: "1.21.4".into(),
        loader: loader.into(),
        loader_version: version.map(str::to_string),
        modpack,
        state: Some(state.into()),
        player_count: (state == "Running").then_some(2),
        max_player_count: Some(20),
        online_mode: None,
    }
}

/// Alex (verified, online, expanded, with a skin), Sam (not verified, offline,
/// nicknamed, with a skin), Jo (waiting to confirm, with the skin their link's card
/// named) and Kim (hidden, sharing nothing, no Minecraft profile, and calling
/// themselves "Alex": flagged).
fn friends() -> Vec<Friend> {
    let mut alex = friend(0x11, "Alex", FriendStatus::Active, true);
    alex.state = Some(core(0xa1, "Alex's PC"));
    if let Some(state) = &mut alex.state {
        state.players = vec![player("Alex", 0xc1)];
    }
    let mut sam = friend(0x22, "Samantha", FriendStatus::Active, false);
    sam.nickname = Some("Sam".into());
    sam.state = Some(core(0xb2, "Sam's laptop"));
    if let Some(state) = &mut sam.state {
        state.players = vec![player("Sam_Builds", 0xc2)];
    }
    let mut jo = friend(0x33, "Jo", FriendStatus::Pending, false);
    jo.player = Some(player("JoJo", 0xc3));
    let mut kim = friend(0x44, "Alex", FriendStatus::Active, false);
    kim.nickname = Some("Kim".into());
    kim.hidden = true;
    kim.state = Some(FriendState {
        launchers: Vec::new(),
        players: Vec::new(),
        seq: 1,
        expires: now() + 86_400,
        cores: Vec::new(),
    });
    vec![alex, sam, jo, kim]
}

fn base(name: &str, friends: Vec<Friend>) -> PreviewFriendList {
    let book = Book {
        name: name.into(),
        friends,
        // this person shares from one core
        my_cores: if name.is_empty() {
            Vec::new()
        } else {
            vec![MyCore {
                fingerprint: key(0xa5),
                name: format!("{name}'s PC"),
                source: None,
            }]
        },
        ..Book::default()
    };
    PreviewFriendList {
        snapshot: Snapshot {
            name_clashes: book.name_clashes(),
            book,
            safety_code: if name.is_empty() {
                String::new()
            } else {
                ME.into()
            },
            key: key(0x99),
        },
        replies: Vec::new(),
        presence: Vec::new(),
        expanded: Vec::new(),
        made: None,
        opened: !name.is_empty(),
        heads: vec![
            ([0xc1; 16], true),
            ([0xc2; 16], false),
            ([0xc3; 16], true),
            ([0xc6; 16], false),
            ([0xc7; 16], true),
        ],
    }
}

fn link(code: &str) -> PendingLink {
    PendingLink {
        code: code.into(),
        invitation: String::new(),
        expires: now() + 20 * 3600,
    }
}

pub fn apply(scene: &str, servers: &mut Servers, cx: &mut Context<Servers>) {
    let made_link = || {
        MadeLink {
        link: "https://connect.lodestone.cc/f/7KQ2M9XD4T#3f9a0c11d2e84b7756a1c0ffee42b19d.8e7a6c5b4d3e2f1a0b9c8d7e6f5a4b3c3f9a0c11d2e84b7756a1c0ffee42b19d"
            .into(),
        expires: now() + 24 * 3600,
    }
    };
    match scene {
        "servers-friend-list-empty" => {
            servers.preview_friend_list(base("", Vec::new()));
        }
        "servers-friend-list" | "servers-friend-list-verify" | "servers-friend-list-remove" => {
            let mut list = base("Peter", friends());
            list.presence = vec![
                ((key(0x11), hex(&[0xa1; 32])), true),
                ((key(0x22), hex(&[0xb2; 32])), false),
            ];
            list.expanded = vec![
                (
                    key(0x11),
                    vec![(
                        hex(&[0xa1; 32]),
                        CoreServers::Loaded(vec![
                            shared("Survival SMP", "fabric", Some("0.16.14"), true, "Running"),
                            shared("Creative build", "vanilla", None, false, "Stopped"),
                        ]),
                    )],
                ),
                // Sam's core doesn't answer, and Jo hasn't confirmed: their status
                // isn't repeated in the open panel
                (key(0x22), Vec::new()),
                (key(0x33), Vec::new()),
            ];
            servers.preview_friend_list(list);
            match scene {
                "servers-friend-list-verify" => {
                    // Sam, who isn't verified yet
                    servers.preview_friend_dialog_for("verify", &key(0x22), cx)
                }
                "servers-friend-list-remove" => {
                    servers.preview_friend_dialog_for("remove", &key(0x22), cx)
                }
                _ => {}
            }
        }
        // someone without a Microsoft account: asked to choose a name
        "servers-friend-list-offline" => {
            servers.preview_friend_list(base("", Vec::new()));
        }
        // a friend link this person answered: waiting for them to confirm
        "servers-friend-list-pending" => {
            servers.preview_friend_list(base("Peter", vec![friends().remove(2)]));
        }
        "servers-friend-list-reply" => {
            let mut list = base(
                "Peter",
                vec![friend(0x11, "Alex", FriendStatus::Active, true)],
            );
            list.snapshot.book.links = vec![link("7KQ2M9XD4T")];
            list.presence = vec![];
            list.replies = vec![
                LinkReply {
                    code: "7KQ2M9XD4T".into(),
                    key: key(0x55),
                    name: "Alex".into(),
                    safety_code: "M3PD-91XQ-ZT7R-K2HA".into(),
                    answers: 2,
                    same_name_as: vec!["Alex".into()],
                    player: None,
                },
                LinkReply {
                    code: "7KQ2M9XD4T".into(),
                    key: key(0x66),
                    name: "Robin".into(),
                    safety_code: "Q8WE-4RTY-7UIO-P2AS".into(),
                    answers: 2,
                    same_name_as: Vec::new(),
                    player: Some(player("RobinHood_7", 0xc6)),
                },
            ];
            servers.preview_friend_list(list);
        }
        "servers-friend-list-add" | "servers-friend-list-add-link" => {
            let mut list = base("Peter", friends());
            if scene == "servers-friend-list-add-link" {
                list.made = Some(made_link());
                list.snapshot.book.links = vec![link("7KQ2M9XD4T")];
            }
            servers.preview_friend_list(list);
            let add = if scene == "servers-friend-list-add" {
                PreviewAddFriend::Preview {
                    link: "https://connect.lodestone.cc/f/QW3RT8YP2M#a1b2c3d4e5f60718293a4b5c6d7e8f90.112233445566778899aabbccddeeff00112233445566778899aabbccddeeff00".into(),
                    preview: LinkPreview {
                        name: "Sam".into(),
                        safety_code: "H4ND-7KQ2-W9XZ-3TRB".into(),
                        key: key(0x77),
                        already_friends: false,
                        same_name_as: vec!["Sam".into()],
                        player: Some(player("Sam_Plays", 0xc7)),
                    },
                }
            } else {
                PreviewAddFriend::Empty
            };
            servers.preview_friend_dialog("add", add, cx);
        }
        "servers-friend-list-retire" => {
            servers.preview_friend_list(base("Peter", friends()));
            servers.preview_friend_dialog("retire", PreviewAddFriend::Empty, cx);
        }
        // "Share LAN worlds with friends": Peter shares a world he opened, and Alex
        // has one open (his launcher, beside his PC); off, the switch explains itself
        "servers-friend-list-lan" | "servers-friend-list-lan-off" => {
            let mut friends = friends();
            let alex = &mut friends[0];
            if let Some(state) = &mut alex.state {
                state.cores.push(
                    serde_json::from_value(serde_json::json!({
                        "fingerprint": hex(&[0xa2; 32]),
                        "name": LAN_CORE_NAME,
                    }))
                    .expect("a made-up core"),
                );
                state.launchers = vec![hex(&[0xa2; 32])];
            }
            let mut list = base("Peter", friends);
            if scene == "servers-friend-list-lan" {
                list.snapshot.book.my_cores.push(MyCore {
                    fingerprint: key(0xa6),
                    name: LAN_CORE_NAME.into(),
                    source: Some(LAN_SOURCE.into()),
                });
            }
            list.presence = vec![
                ((key(0x11), hex(&[0xa1; 32])), false),
                ((key(0x11), hex(&[0xa2; 32])), true),
                ((key(0x22), hex(&[0xb2; 32])), false),
            ];
            let mut world = shared("Alex - Island", "vanilla", None, false, "Running");
            world.instance_uuid = "lan-3f9a0c11d2e8".into();
            world.online_mode = Some(true);
            world.player_count = Some(1);
            world.max_player_count = Some(8);
            list.expanded = vec![(
                key(0x11),
                vec![(hex(&[0xa2; 32]), CoreServers::Loaded(vec![world]))],
            )];
            servers.preview_friend_list(list);
            if scene == "servers-friend-list-lan" {
                servers.preview_lan_share(
                    vec![OpenWorld {
                        id: "lan-7c1d9e0a4b52".into(),
                        name: "Peter - New World".into(),
                        port: 51234,
                        game_version: "1.21.4".into(),
                        loader: "vanilla".into(),
                        loader_version: None,
                        online_mode: Some(true),
                        player_count: Some(2),
                        max_player_count: Some(8),
                        instance: None,
                    }],
                    1,
                );
            }
        }
        "servers-friend-list-retire-core" => {
            servers.preview_friend_list(base("Peter", friends()));
            servers.open_friend_retire_core(&key(0xa5), cx);
        }
        _ => {}
    }
}
