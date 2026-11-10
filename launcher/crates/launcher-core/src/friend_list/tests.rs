use futures::executor::block_on;
use lodestone_connect::friend::state::StateCore;

use super::*;
use crate::MemorySecretStore;

fn open(dir: &Path, store: &MemorySecretStore) -> Friends {
    block_on(Friends::open(
        &DataDir::at(dir),
        store,
        Service::parse("http://127.0.0.1:9").unwrap(),
    ))
    .unwrap()
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
            book.friends.push(Friend {
                key: "ab".repeat(32),
                name: "Ben".into(),
                pair: "cd".repeat(32),
                status: FriendStatus::Active,
                ..Friend::default()
            });
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

    // another computer's (or a wiped) Keychain can't open it
    let elsewhere = MemorySecretStore::new();
    let failed = block_on(Friends::open(
        &DataDir::at(dir.path()),
        &elsewhere,
        Service::parse("http://127.0.0.1:9").unwrap(),
    ));
    assert!(matches!(failed, Err(FriendsError::Damaged(_))));
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
    assert!(friends.add_my_core(&fp, "Ana's PC").unwrap());
    assert!(!friends.add_my_core(&fp, "Ana's PC").unwrap(), "no change");
    assert!(friends.add_my_core(&fp, "Ana's server").unwrap(), "renamed");
    assert!(friends.book.state_seq > seq);
    assert_eq!(
        friends.book.state_cores(),
        vec![StateCore {
            fingerprint: [0x11; 32],
            name: "Ana's server".into()
        }]
    );
    assert!(friends.remove_my_core(&fp).unwrap());
    assert!(friends.book.state_cores().is_empty());

    friends
        .modify(|book| {
            book.friends.push(Friend {
                key: "ab".repeat(32),
                name: "Ben".into(),
                pair: "cd".repeat(32),
                ..Friend::default()
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
fn only_active_friends_with_a_live_state_can_be_joined() {
    let core = StateCore {
        fingerprint: [1; 32],
        name: "PC".into(),
    };
    let mut friend = Friend {
        state: Some(FriendState {
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
    let mut ana = block_on(Friends::open(
        &DataDir::at(ana_dir.path()),
        &ana_keys,
        service.clone(),
    ))
    .unwrap();
    let mut ben = block_on(Friends::open(
        &DataDir::at(ben_dir.path()),
        &ben_keys,
        service,
    ))
    .unwrap();
    ana.set_name("Ana").unwrap();
    ben.set_name("Ben").unwrap();

    let link = block_on(ana.create_link()).unwrap();
    let preview = block_on(ben.preview_link(&link)).unwrap();
    assert_eq!(preview.name, "Ana");
    assert_eq!(preview.safety_code, ana.safety_code());
    let ana_key = block_on(ben.answer_link(&link)).unwrap();
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
    block_on(ana.accept_reply(&replies[0].code, &replies[0].key)).unwrap();
    assert!(ana.book.links.is_empty(), "the link and its keys are gone");
    // the link can't be used again
    assert!(block_on(ben.preview_link(&link)).is_err());

    let refreshed = block_on(ben.refresh()).unwrap();
    assert_eq!(refreshed.confirmed, vec![ana_key.clone()]);
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
    ana.add_my_core(&fp, "Ana's PC").unwrap();
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
    ana.remove_my_core(&fp).unwrap();
    block_on(ana.publish_states()).unwrap();
    let refreshed = block_on(ben.refresh()).unwrap();
    assert_eq!(refreshed.dropped_cores, vec![fp]);

    // Ben removes Ana: his slot for her goes
    block_on(ben.remove(&ana_key)).unwrap();
    assert!(ben.book.friend(&ana_key).is_none());
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
                key: "ab".repeat(32),
                name: "Ben".into(),
                pair: "cd".repeat(32),
                ..Friend::default()
            });
            Ok(())
        })
        .unwrap();
    // the Host's copy is stale, but its change starts from the file
    host.add_my_core(&"11".repeat(32), "PC").unwrap();
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
    // a stale lock (an app that died) is taken over
    std::fs::write(dir.path().join(FILE_NAME).with_extension("lock"), b"").unwrap();
    let old = std::time::SystemTime::now() - Duration::from_secs(60);
    std::fs::File::options()
        .write(true)
        .open(dir.path().join(FILE_NAME).with_extension("lock"))
        .unwrap()
        .set_modified(old)
        .unwrap();
    launcher.set_name("Ana B").unwrap();
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
