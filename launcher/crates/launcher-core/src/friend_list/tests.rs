use futures::executor::block_on;
use lodestone_connect::friend::state::StateCore;

use super::*;
use crate::MemorySecretStore;

fn test_service() -> Service {
    Service::parse("http://127.0.0.1:9").unwrap()
}

/// Starts (or opens) the friend list in `dir`, as the Launcher does.
fn open(dir: &Path, store: &MemorySecretStore) -> Friends {
    block_on(Friends::create(&DataDir::at(dir), store, test_service())).unwrap()
}

/// Opens the list in `dir` as Lodestone Host does: never making anything.
fn open_existing(dir: &Path, store: &MemorySecretStore) -> Opened {
    block_on(Friends::open_existing(
        &DataDir::at(dir),
        store,
        test_service(),
    ))
    .unwrap()
}

fn ben() -> Friend {
    Friend {
        key: "ab".repeat(32),
        name: "Ben".into(),
        pair: "cd".repeat(32),
        status: FriendStatus::Active,
        ..Friend::default()
    }
}

#[test]
fn the_book_is_sealed_and_survives_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let store = MemorySecretStore::new();
    let mut friends = open(dir.path(), &store);
    let me = friends.me();
    let data = DataDir::at(dir.path());
    let mut names = vec![
        secret_name(IDENTITY_SECRET, &data),
        secret_name(FILE_KEY_SECRET, &data),
    ];
    names.sort();
    assert_eq!(store.keys(), names);
    assert!(
        names.iter().all(|n| n.contains('@')),
        "a test folder never uses the real items"
    );
    friends.set_name("  Ana \u{202E} ").unwrap();
    assert_eq!(friends.book.name, "Ana");
    friends
        .modify(|book| {
            book.friends.push(ben());
            Ok(())
        })
        .unwrap();

    // nothing readable on disk
    let text = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
    assert!(!text.contains("Ben"), "{text}");
    assert!(!text.contains(&"cd".repeat(32)));

    // the same person and book after a restart
    let again = open(dir.path(), &store);
    assert_eq!(again.me(), me);
    assert_eq!(again.book, friends.book);

    // another computer's (or a wiped) Keychain can't open it, and makes nothing
    let elsewhere = MemorySecretStore::new();
    let failed = block_on(Friends::open_existing(
        &DataDir::at(dir.path()),
        &elsewhere,
        test_service(),
    ));
    assert!(matches!(failed, Err(FriendsError::Damaged(_))));
    assert!(elsewhere.keys().is_empty());
    let failed = block_on(Friends::create(
        &DataDir::at(dir.path()),
        &elsewhere,
        test_service(),
    ));
    assert!(
        matches!(failed, Err(FriendsError::Damaged(_))),
        "a damaged list is never replaced"
    );
}

#[test]
fn opening_an_existing_list_makes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = MemorySecretStore::new();
    assert!(matches!(open_existing(dir.path(), &store), Opened::None));
    assert!(store.keys().is_empty(), "no key made");
    assert!(!dir.path().join(FILE_NAME).exists(), "no file made");
    open(dir.path(), &store);
    assert!(matches!(
        open_existing(dir.path(), &store),
        Opened::Ready(_)
    ));
}

#[test]
fn a_missing_file_stops_changes_and_is_never_written_anew() {
    let dir = tempfile::tempdir().unwrap();
    let store = MemorySecretStore::new();
    let mut launcher = open(dir.path(), &store);
    launcher.set_name("Ana").unwrap();
    launcher
        .modify(|book| {
            book.friends.push(ben());
            Ok(())
        })
        .unwrap();
    let mut host = launcher.clone();
    // the file goes (the data folder was moved, say)
    std::fs::remove_file(dir.path().join(FILE_NAME)).unwrap();
    assert!(matches!(host.reload(), Err(FriendsError::Missing)));
    assert!(matches!(
        host.set_verified(&"ab".repeat(32), true),
        Err(FriendsError::Missing)
    ));
    assert!(matches!(
        host.add_my_core(&"11".repeat(32), "PC", None),
        Err(FriendsError::Missing)
    ));
    assert!(
        !dir.path().join(FILE_NAME).exists(),
        "a change never writes a new file with the old keys"
    );
    assert!(matches!(
        read_book(&dir.path().join(FILE_NAME), &launcher.file_key),
        Err(FriendsError::Missing)
    ));
}

#[test]
fn an_unchanged_book_is_not_written_again() {
    let dir = tempfile::tempdir().unwrap();
    let store = MemorySecretStore::new();
    let mut friends = open(dir.path(), &store);
    friends.set_name("Ana").unwrap();
    let generation = friends.book.generation;
    let stamp = file_stamp(&DataDir::at(dir.path()));
    friends.set_name("Ana").unwrap();
    friends.set_verified("nobody", true).unwrap();
    assert_eq!(friends.book.generation, generation);
    assert_eq!(
        file_stamp(&DataDir::at(dir.path())),
        stamp,
        "the file wasn't touched"
    );
    // a real change is written, with the next generation, and a new stamp (a new
    // nonce) even within the same second
    friends.set_name("Ana B").unwrap();
    assert_eq!(friends.book.generation, generation + 1);
    assert_ne!(file_stamp(&DataDir::at(dir.path())), stamp);
}

#[test]
fn names_cores_and_flags() {
    let dir = tempfile::tempdir().unwrap();
    let store = MemorySecretStore::new();
    let mut friends = open(dir.path(), &store);
    assert!(friends.set_name("\u{200B}").is_err());
    friends.set_name("Ana").unwrap();
    let seq = friends.book.state_seq;

    let fp = "11".repeat(32);
    assert!(friends.add_my_core(&fp, "Ana's PC", None).unwrap());
    assert!(
        !friends.add_my_core(&fp, "Ana's PC", None).unwrap(),
        "no change"
    );
    assert!(
        friends.add_my_core(&fp, "Ana's server", None).unwrap(),
        "renamed"
    );
    assert!(friends.book.state_seq > seq);
    assert_eq!(
        friends.book.state_cores(),
        vec![StateCore {
            fingerprint: [0x11; 32],
            name: "Ana's server".into()
        }]
    );
    assert!(friends.retire_my_core(&fp).unwrap());
    assert!(friends.book.state_cores().is_empty());

    friends
        .modify(|book| {
            book.friends.push(Friend {
                status: FriendStatus::Pending,
                ..ben()
            });
            Ok(())
        })
        .unwrap();
    friends
        .set_nickname(&"ab".repeat(32), Some("Benny"))
        .unwrap();
    friends.set_verified(&"ab".repeat(32), true).unwrap();
    let ben = friends.book.friend(&"ab".repeat(32)).unwrap();
    assert_eq!(ben.display_name(), "Benny");
    assert!(ben.verified);
    assert_eq!(friends.book.same_name("benny", "zz").len(), 1);
    assert_eq!(friends.book.same_name("BEN", "zz").len(), 1);
    assert_eq!(
        friends.book.same_name("Веn", "zz").len(),
        1,
        "a Cyrillic look-alike"
    );
    assert!(friends.book.same_name("Ben", &"ab".repeat(32)).is_empty());
}

#[test]
fn a_retired_core_is_never_listed_again_and_a_new_certificate_replaces_the_old() {
    let dir = tempfile::tempdir().unwrap();
    let store = MemorySecretStore::new();
    let mut friends = open(dir.path(), &store);
    let (old, new, elsewhere) = ("11".repeat(32), "22".repeat(32), "33".repeat(32));
    friends.add_my_core(&old, "PC", Some("host:a")).unwrap();
    friends
        .add_my_core(&elsewhere, "Laptop", Some("host:b"))
        .unwrap();
    // the same core comes back with another certificate: the old one goes, the other
    // core stays
    assert!(friends.add_my_core(&new, "PC", Some("host:a")).unwrap());
    let fingerprints: Vec<&str> = friends
        .book
        .my_cores
        .iter()
        .map(|c| c.fingerprint.as_str())
        .collect();
    assert_eq!(fingerprints, [elsewhere.as_str(), new.as_str()]);
    // retired: gone, and refused when it syncs again
    let seq = friends.book.state_seq;
    assert!(friends.retire_my_core(&new.to_uppercase()).unwrap());
    assert!(friends.book.state_seq > seq, "friends get a new state");
    assert!(friends.book.my_cores.iter().all(|c| c.fingerprint != new));
    assert!(matches!(
        friends.add_my_core(&new, "PC", Some("host:a")),
        Err(FriendsError::CoreRetired)
    ));
    assert_eq!(friends.book.my_cores.len(), 1);
}

#[test]
fn a_friend_renamed_like_another_is_flagged() {
    let mut book = Book {
        friends: vec![
            ben(),
            Friend {
                key: "cd".repeat(32),
                name: "Sam".into(),
                ..ben()
            },
        ],
        ..Book::default()
    };
    assert!(book.name_clashes().is_empty());
    // Sam's next state calls them "Bеn" (with a Cyrillic е)
    book.friends[1].name = "B\u{0435}n".into();
    let clashes = book.name_clashes();
    assert_eq!(
        clashes.get(&"cd".repeat(32)).unwrap(),
        &vec!["Ben".to_string()]
    );
    assert_eq!(
        clashes.get(&"ab".repeat(32)).unwrap(),
        &vec!["B\u{0435}n".to_string()]
    );
}

fn player(name: &str, byte: u8, offline: bool) -> StatePlayer {
    StatePlayer {
        name: name.into(),
        uuid: [byte; 16],
        offline,
    }
}

#[test]
fn the_name_is_the_minecraft_name_until_one_is_chosen() {
    let dir = tempfile::tempdir().unwrap();
    let store = MemorySecretStore::new();
    let mut friends = open(dir.path(), &store);
    assert!(friends.book.name.is_empty());
    assert!(friends.book.profile().is_none());

    // an offline account alone gives no name: the person is asked for one
    assert!(friends.set_players(vec![player("Dev_1", 1, true)]).unwrap());
    assert!(friends.book.name.is_empty());
    assert!(friends.book.profile().is_none());
    assert!(friends.use_profile_name().is_err());

    // signed in with Microsoft: their Minecraft name, without asking
    let seq = friends.book.state_seq;
    friends
        .set_players(vec![player("Steve_1", 2, false), player("Dev_1", 1, true)])
        .unwrap();
    assert_eq!(friends.book.name, "Steve_1");
    assert!(friends.book.name_from_profile);
    assert_eq!(friends.book.profile(), Some(&player("Steve_1", 2, false)));
    assert!(friends.book.state_seq > seq, "friends get the name");
    // it follows the profile's name (renamed on minecraft.net, or another account
    // first)
    friends
        .set_players(vec![player("Steve_2", 2, false)])
        .unwrap();
    assert_eq!(friends.book.name, "Steve_2");
    assert!(
        !friends
            .set_players(vec![player("Steve_2", 2, false)])
            .unwrap(),
        "no change"
    );
    // signed out: the name stays
    friends.set_players(Vec::new()).unwrap();
    assert_eq!(friends.book.name, "Steve_2");
    friends
        .set_players(vec![player("Steve_2", 2, false)])
        .unwrap();

    // a name of their own wins, and stays
    friends.set_name("Ana").unwrap();
    assert!(!friends.book.name_from_profile);
    friends.set_players(vec![player("Alex", 3, false)]).unwrap();
    assert_eq!(friends.book.name, "Ana");
    // until they go back to the Minecraft name
    friends.use_profile_name().unwrap();
    assert_eq!(friends.book.name, "Alex");
    assert!(friends.book.name_from_profile);

    // kept across a restart
    let again = open(dir.path(), &store);
    assert_eq!(again.book, friends.book);
}

#[test]
fn a_name_chosen_before_profiles_is_kept() {
    // a friend list from before profiles: a chosen name, and no `name_from_profile`
    let old: Book = serde_json::from_value(serde_json::json!({
        "name": "Ana",
        "friends": [{ "key": "ab".repeat(32), "name": "Ben", "pair": "cd".repeat(32) }],
    }))
    .unwrap();
    assert!(!old.name_from_profile);
    assert!(old.friends[0].player.is_none());
    assert!(old.friends[0].minecraft().is_none());
    let dir = tempfile::tempdir().unwrap();
    let store = MemorySecretStore::new();
    let mut friends = open(dir.path(), &store);
    friends
        .modify(|book| {
            *book = Book {
                generation: book.generation,
                ..old.clone()
            };
            Ok(())
        })
        .unwrap();
    friends
        .set_players(vec![player("Steve_1", 2, false)])
        .unwrap();
    assert_eq!(friends.book.name, "Ana");
    assert!(!friends.book.name_from_profile);
    // what's left out of the file keeps it as before
    let json = serde_json::to_value(&old).unwrap();
    assert!(json.get("name_from_profile").is_none());
    assert!(json["friends"][0].get("player").is_none());
}

#[test]
fn a_friend_s_minecraft_profile_survives_and_their_states_take_over() {
    let ana = Friend {
        player: Some(player("Ana", 7, false)),
        ..ben()
    };
    let json = serde_json::to_string(&ana).unwrap();
    let back: Friend = serde_json::from_str(&json).unwrap();
    assert_eq!(back, ana);
    assert_eq!(back.minecraft(), Some(&player("Ana", 7, false)));
    // an offline profile has no skin to show
    let offline = Friend {
        player: Some(player("Ana", 7, true)),
        ..ben()
    };
    assert!(offline.minecraft().is_none());
    // once a state arrives, its first Microsoft account is theirs
    let with_state = Friend {
        state: Some(FriendState {
            players: vec![player("Dev", 1, true), player("Ana_2", 8, false)],
            ..FriendState::default()
        }),
        ..ana.clone()
    };
    assert_eq!(with_state.minecraft(), Some(&player("Ana_2", 8, false)));
    let signed_out = Friend {
        state: Some(FriendState::default()),
        ..ana
    };
    assert!(signed_out.minecraft().is_none());
}

#[test]
fn someone_playing_as_a_friend_s_profile_is_flagged() {
    let mut ana = Friend {
        key: "ef".repeat(32),
        name: "Ana".into(),
        player: Some(player("AnaPlays", 7, false)),
        ..ben()
    };
    let book = Book {
        friends: vec![ben(), ana.clone()],
        ..Book::default()
    };
    // another name, but Ana's Minecraft profile (and so her skin)
    let same_uuid = player("Whoever", 7, false);
    let names: Vec<&str> = book
        .lookalikes("Mallory", Some(&same_uuid), "zz")
        .iter()
        .map(|f| f.name.as_str())
        .collect();
    assert_eq!(names, ["Ana"]);
    // a Minecraft name like a friend's name, or like a friend's Minecraft name
    assert_eq!(
        book.lookalikes("Mallory", Some(&player("Ben", 9, false)), "zz")
            .len(),
        1
    );
    assert_eq!(book.lookalikes("anaplays", None, "zz").len(), 1);
    assert!(
        book.lookalikes("Mallory", Some(&player("Mallory", 9, false)), "zz")
            .is_empty()
    );
    // an offline profile isn't anyone's: its UUID isn't compared
    assert!(
        book.lookalikes("Mallory", Some(&player("Whoever", 7, true)), "zz")
            .is_empty()
    );

    // in the list: a friend who plays as another's profile is flagged, both ways
    ana.player = Some(player("Mallory", 7, false));
    let mallory = Friend {
        key: "12".repeat(32),
        name: "Mallory".into(),
        player: Some(player("Mallory", 7, false)),
        ..ben()
    };
    let book = Book {
        friends: vec![ben(), ana, mallory],
        ..Book::default()
    };
    let clashes = book.name_clashes();
    assert_eq!(
        clashes.get(&"12".repeat(32)).unwrap(),
        &vec!["Ana".to_string()]
    );
    assert_eq!(
        clashes.get(&"ef".repeat(32)).unwrap(),
        &vec!["Mallory".to_string()]
    );
    assert!(!clashes.contains_key(&"ab".repeat(32)));
}

#[test]
fn only_active_friends_with_a_live_state_can_be_joined() {
    let core = StateCore {
        fingerprint: [1; 32],
        name: "PC".into(),
    };
    let mut friend = Friend {
        state: Some(FriendState {
            launchers: Vec::new(),
            players: Vec::new(),
            seq: 1,
            expires: 1000,
            cores: vec![core],
        }),
        ..Friend::default()
    };
    assert!(friend.joinable_cores(10).is_empty(), "pending");
    friend.status = FriendStatus::Active;
    assert_eq!(friend.joinable_cores(10).len(), 1);
    assert!(friend.joinable_cores(1000).is_empty(), "expired");
    friend.status = FriendStatus::Retired;
    assert!(friend.joinable_cores(10).is_empty());
}

#[test]
fn states_numbered_far_ahead_are_refused() {
    use lodestone_connect::friend::state::State;
    let now = 1_800_000_000;
    let state = |seq: u64, issued: u64| State {
        seq,
        issued,
        expires: issued + 1000,
        name: "Ana".into(),
        cores: Vec::new(),
        retired: false,
        players: Vec::new(),
        launchers: Vec::new(),
    };
    assert_eq!(ops::implausible_state(&state(now, now), now), None);
    // a few changes within a second put the number a little ahead
    assert_eq!(ops::implausible_state(&state(now + 5, now), now), None);
    // a stolen key numbering a state to block the owner's later ones
    assert!(ops::implausible_state(&state(u64::MAX, now), now).is_some());
    assert!(ops::implausible_state(&state(now + 3 * 86_400, now), now).is_some());
    // or claiming to be issued far in the future
    assert!(ops::implausible_state(&state(now + 3 * 86_400, now + 3 * 86_400), now).is_some());
}

#[test]
fn a_state_made_while_the_clock_was_days_fast_is_reissued_once_it_is_right() {
    let dir = tempfile::tempdir().unwrap();
    let store = MemorySecretStore::new();
    let mut friends = open(dir.path(), &store);
    friends.set_name("Ana").unwrap();
    // a change made while the clock read two days ahead, as the book saved it
    let fast = now() + 2 * 86_400;
    friends
        .modify(|book| {
            book.state_seq = fast;
            book.state_issued = fast;
            Ok(())
        })
        .unwrap();

    // The clock is right again: the state is reissued, dated and numbered by it,
    // so friends take it.
    friends.freshen_state().unwrap();
    let state = friends.my_state();
    assert!(state.issued <= now());
    assert_eq!(ops::implausible_state(&state, now()), None);

    // and so are the changes after it, each numbered higher
    let seq = state.seq;
    friends.set_name("Ana B").unwrap();
    friends.freshen_state().unwrap();
    let state = friends.my_state();
    assert!(state.seq > seq);
    assert_eq!(ops::implausible_state(&state, now()), None);
}

#[test]
fn friends_sent_a_state_while_the_clock_was_days_fast_are_sent_the_next_one() {
    let dir = tempfile::tempdir().unwrap();
    let store = MemorySecretStore::new();
    let mut friends = open(dir.path(), &store);
    friends.set_name("Ana").unwrap();
    // Ben was sent a state made while the clock read two days ahead (the write
    // worked; he refused it when he read it)
    let fast = now() + 2 * 86_400;
    friends
        .modify(|book| {
            book.state_seq = fast;
            book.state_issued = fast;
            book.friends.push(Friend {
                sent_seq: fast,
                ..ben()
            });
            Ok(())
        })
        .unwrap();

    // With the clock right, a change is numbered by it, and is sent to Ben again:
    // he can't be reached here, so trying shows up as a problem.
    friends.set_name("Ana B").unwrap();
    let problems = block_on(friends.publish_states()).unwrap();
    assert_eq!(problems.len(), 1, "Ben is sent the new state");
    let seq = friends.my_state().seq;
    assert!(seq <= now());
    let ben = friends.book.friend(&ben().key).unwrap();
    assert!(ben.sent_seq < seq, "Ben isn't marked as having it");
}

/// The whole flow between two people, against the introduction service at
/// CONNECT_TEST_SERVICE (`npm run dev` in services/connect). Skipped without it.
#[test]
fn two_people_become_friends_through_the_service() {
    let Some(url) = std::env::var("CONNECT_TEST_SERVICE").ok() else {
        return;
    };
    let service = Service::parse(&url).unwrap();
    let (ana_dir, ben_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (ana_keys, ben_keys) = (MemorySecretStore::new(), MemorySecretStore::new());
    let mut ana = block_on(Friends::create(
        &DataDir::at(ana_dir.path()),
        &ana_keys,
        service.clone(),
    ))
    .unwrap();
    let mut ben = block_on(Friends::create(
        &DataDir::at(ben_dir.path()),
        &ben_keys,
        service.clone(),
    ))
    .unwrap();
    // Ana is signed in with Microsoft: her name is her Minecraft name, and her card
    // says which profile she plays as. Ben has no Microsoft account: he chose a name.
    ana.set_players(vec![player("Ana", 0xa5, false)]).unwrap();
    assert_eq!(ana.book.name, "Ana");
    ben.set_name("Ben").unwrap();

    let link = block_on(ana.create_link()).unwrap();
    // someone without a friend list can look at the link, and nothing is made
    let nobody = block_on(ops::preview_link(&service, None, &link)).unwrap();
    assert_eq!(nobody.safety_code, ana.safety_code());
    let preview = block_on(ben.preview_link(&link)).unwrap();
    assert_eq!(preview.name, "Ana");
    assert_eq!(preview.safety_code, ana.safety_code());
    assert_eq!(preview.player, Some(player("Ana", 0xa5, false)));
    assert_eq!(nobody.player, preview.player);
    let ana_key = block_on(ben.answer_link(&link)).unwrap();
    assert_eq!(
        ben.book.friend(&ana_key).unwrap().minecraft(),
        Some(&player("Ana", 0xa5, false)),
        "her skin shows while she hasn't confirmed"
    );
    assert_eq!(
        ben.book.friend(&ana_key).unwrap().status,
        FriendStatus::Pending
    );
    // pending until Ana confirms
    let refreshed = block_on(ben.refresh()).unwrap();
    assert!(refreshed.confirmed.is_empty());

    let replies = block_on(ana.link_replies()).unwrap();
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].name, "Ben");
    assert_eq!(replies[0].safety_code, ben.safety_code());
    assert_eq!(replies[0].player, None, "Ben has no Minecraft profile");
    block_on(ana.accept_reply(&replies[0].code, &replies[0].key)).unwrap();
    assert!(ana.book.links.is_empty(), "the link and its keys are gone");
    // the link can't be used again
    assert!(block_on(ben.preview_link(&link)).is_err());

    let refreshed = block_on(ben.refresh()).unwrap();
    assert_eq!(refreshed.confirmed, vec![ana_key.clone()]);
    // her state's accounts take over from the card
    assert_eq!(
        ben.book.friend(&ana_key).unwrap().minecraft(),
        Some(&player("Ana", 0xa5, false))
    );
    let ben_key = ben.me().to_hex();
    block_on(ana.refresh()).unwrap();
    assert_eq!(
        ana.book.friend(&ben_key).unwrap().status,
        FriendStatus::Active
    );
    assert_eq!(
        ana.book.friend(&ben_key).unwrap().pair,
        ben.book.friend(&ana_key).unwrap().pair
    );

    // Ana adds a core: Ben sees it; she retires it: Ben drops it
    let fp = "22".repeat(32);
    ana.add_my_core(&fp, "Ana's PC", None).unwrap();
    block_on(ana.publish_states()).unwrap();
    block_on(ben.refresh()).unwrap();
    let cores = ben
        .book
        .friend(&ana_key)
        .unwrap()
        .joinable_cores(now())
        .to_vec();
    assert_eq!(cores.len(), 1);
    assert_eq!(cores[0].name, "Ana's PC");
    assert!(block_on(ana.retire_core(&fp)).unwrap().is_empty());
    let refreshed = block_on(ben.refresh()).unwrap();
    assert_eq!(refreshed.dropped_cores, vec![fp]);

    // The Launcher and the Host each send a state: neither rolls the other's back.
    // The Host's copy sends an older state after the Launcher sent a newer one; it
    // sees that under the lock and sends the newer one again.
    let mut host = ana.clone();
    ana.set_name("Ana B").unwrap();
    block_on(ana.publish_state_to(&ben_key)).unwrap();
    let newest = ana.book.state_seq;
    host.book.state_seq -= 1; // what the Host had before
    block_on(host.publish_state_to(&ben_key)).unwrap();
    block_on(ben.refresh()).unwrap();
    assert_eq!(ben.book.friend(&ana_key).unwrap().name, "Ana B");
    assert_eq!(
        ben.book
            .friend(&ana_key)
            .unwrap()
            .state
            .as_ref()
            .unwrap()
            .seq,
        newest
    );

    // Ana retires her identity: Ben sees it, whatever its number (as if a thief had
    // sent him a state numbered as high as can be first)
    ben.modify(|book| {
        book.friend_mut(&ana_key)
            .unwrap()
            .state
            .as_mut()
            .unwrap()
            .seq = u64::MAX;
        Ok(())
    })
    .unwrap();
    block_on(ana.retire_identity()).unwrap();
    assert!(
        ana.book.retired && ana.book.friends.is_empty(),
        "nothing secret kept"
    );
    let refreshed = block_on(ben.refresh()).unwrap();
    assert_eq!(refreshed.retired, vec![ana_key.clone()]);
    assert_eq!(
        ben.book.friend(&ana_key).unwrap().status,
        FriendStatus::Retired
    );
    // nothing uses the retired identity again
    assert!(matches!(
        block_on(ana.create_link()),
        Err(FriendsError::Retired)
    ));

    // Ben removes Ana: his slot for her goes
    block_on(ben.remove(&ana_key)).unwrap();
    assert!(ben.book.friend(&ana_key).is_none());
}

#[test]
fn a_retired_identity_is_never_used_and_starting_again_makes_a_new_one() {
    let dir = tempfile::tempdir().unwrap();
    let store = MemorySecretStore::new();
    let data = DataDir::at(dir.path());
    let mut friends = open(dir.path(), &store);
    friends.set_name("Ana").unwrap();
    friends
        .modify(|book| {
            book.friends.push(ben());
            Ok(())
        })
        .unwrap();
    let old = friends.me();
    let mut host = friends.clone();
    // Ben can't be reached (no service here), but the identity is retired anyway
    let problems = block_on(friends.retire_identity()).unwrap();
    assert_eq!(problems.len(), 1);
    assert!(friends.book.retired);
    assert!(friends.book.friends.is_empty(), "no pair secrets kept");
    // the Host sees it, and changes nothing
    assert!(matches!(host.reload(), Err(FriendsError::Retired)));
    assert!(matches!(
        host.add_my_core(&"11".repeat(32), "PC", None),
        Err(FriendsError::Retired)
    ));
    // the person key is still in the Keychain (deleting it failed, say), but the
    // retired list is never opened for use
    assert!(matches!(open_existing(dir.path(), &store), Opened::Retired));
    block_on(forget_identity(&data, &store)).unwrap();
    assert!(matches!(open_existing(dir.path(), &store), Opened::Retired));
    // starting again deletes the old list and makes a new identity
    let again = open(dir.path(), &store);
    assert_ne!(again.me(), old);
    assert!(!again.book.retired);
    assert!(again.book.friends.is_empty());
}

#[test]
fn two_apps_writing_the_file_never_undo_each_other() {
    let dir = tempfile::tempdir().unwrap();
    let store = MemorySecretStore::new();
    let mut launcher = open(dir.path(), &store);
    let mut host = open(dir.path(), &store);
    launcher.set_name("Ana").unwrap();
    launcher
        .modify(|book| {
            book.friends.push(Friend {
                status: FriendStatus::Pending,
                ..ben()
            });
            Ok(())
        })
        .unwrap();
    // the Host's copy is stale, but its change starts from the file
    host.add_my_core(&"11".repeat(32), "PC", None).unwrap();
    assert_eq!(host.book.friends.len(), 1, "the Host saw Ben");
    launcher.reload().unwrap();
    assert_eq!(launcher.book.my_cores.len(), 1);
    // the launcher removes Ben; the Host, still holding Ben, changes something else
    futures::executor::block_on(launcher.remove(&"ab".repeat(32))).unwrap();
    host.set_verified(&"ab".repeat(32), true).unwrap();
    assert!(host.book.friends.is_empty(), "Ben stays removed");
    let seq = host.book.state_seq;
    launcher.reload().unwrap();
    assert!(launcher.book.state_seq >= seq);
    assert!(seq >= now() - 5, "state numbers never fall behind the time");
    // a lock file left behind (by an app that died) doesn't block: only a held lock
    // does, and the system lets go of it when its app exits
    std::fs::write(dir.path().join(FILE_NAME).with_extension("lock"), b"").unwrap();
    launcher.set_name("Ana B").unwrap();
    // while one app holds the lock, the other waits
    let held = FileLock::acquire(&dir.path().join(FILE_NAME)).unwrap();
    let started = std::time::Instant::now();
    let waiter = std::thread::spawn({
        let mut host = host.clone();
        move || host.set_name("Ana C")
    });
    std::thread::sleep(Duration::from_millis(200));
    drop(held);
    waiter.join().unwrap().unwrap();
    assert!(started.elapsed() >= Duration::from_millis(200));
    launcher.reload().unwrap();
    assert_eq!(launcher.book.name, "Ana C");
}

#[test]
fn nothing_secret_is_printed() {
    let friend = Friend {
        key: "ab".repeat(32),
        pair: "cd".repeat(32),
        ..Friend::default()
    };
    assert!(!format!("{friend:?}").contains(&"cd".repeat(32)));
    let link = PendingLink {
        code: "X".into(),
        invitation: "secret-stuff".into(),
        expires: 1,
    };
    assert!(!format!("{link:?}").contains("secret-stuff"));
}

#[test]
fn the_service_comes_from_the_environment_when_set() {
    // the friend list reads the apps' one source ([`crate::cores::connect_service`])
    use crate::cores::connect_service::resolve;
    let service_from = |env: Option<&str>| resolve(env, None).service;
    assert_eq!(service_from(None), Service::default());
    assert_eq!(service_from(Some("  ")), Service::default());
    let local = service_from(Some("http://127.0.0.1:18790"));
    assert_eq!(local.host, "127.0.0.1");
    assert_eq!(local.port, 18790);
    assert!(!local.tls);
    // nonsense falls back to the real service rather than nowhere
    assert_eq!(service_from(Some("http://")), Service::default());
}

#[test]
fn hex_round_trips() {
    let bytes = [0xab; 32];
    assert_eq!(unhex32(&hex(&bytes)), Some(bytes));
    assert_eq!(unhex32("ab"), None);
    assert_eq!(unhex32(&"zz".repeat(32)), None);
}
