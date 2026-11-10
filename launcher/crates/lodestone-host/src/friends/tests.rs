use launcher_core::friend_list::{
    Book, Friend, FriendStatus,
    core_sync::{CoreFriend, CoreFriends},
};
use lodestone_connect::Service;

use super::*;

fn friend(key: char, name: &str, status: FriendStatus) -> Friend {
    Friend {
        key: key.to_string().repeat(64),
        name: name.into(),
        status,
        ..Default::default()
    }
}

fn book(name: &str, friends: Vec<Friend>) -> Book {
    Book {
        name: name.into(),
        friends,
        ..Default::default()
    }
}

fn core(friends: Vec<(char, Vec<&str>, bool)>) -> CoreFriends {
    CoreFriends {
        fingerprint: "0".repeat(64),
        version: 1,
        friends: friends
            .into_iter()
            .map(|(key, shared, connected)| CoreFriend {
                key: key.to_string().repeat(64),
                name: String::new(),
                shared: shared.into_iter().map(String::from).collect(),
                connected,
            })
            .collect(),
        host: None,
    }
}

fn key(c: char) -> String {
    c.to_string().repeat(64)
}

#[test]
fn only_active_visible_friends_are_listed_by_name() {
    let mut sam = friend('c', "sam", FriendStatus::Active);
    sam.verified = true;
    let mut alex = friend('a', "Alex", FriendStatus::Active);
    alex.nickname = Some("Zed".into());
    let mut hidden = friend('d', "Hidden", FriendStatus::Active);
    hidden.hidden = true;
    let book = book(
        "Me",
        vec![
            sam,
            alex,
            hidden,
            friend('e', "Pending", FriendStatus::Pending),
            friend('f', "Retired", FriendStatus::Retired),
            friend('b', "bo", FriendStatus::Active),
        ],
    );
    let rows = listed(&book);
    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["bo", "sam", "Zed"], "by name, the nickname first");
    assert!(rows[1].verified && !rows[0].verified);
    assert_eq!(rows[2].key, key('a'));
}

#[test]
fn the_launcher_is_needed_without_a_name_or_active_friends() {
    assert!(needs_launcher(&Book::default()));
    let active = || friend('a', "Alex", FriendStatus::Active);
    assert!(needs_launcher(&book("  ", vec![active()])), "no name");
    assert!(
        needs_launcher(&book(
            "Me",
            vec![friend('a', "Alex", FriendStatus::Pending)]
        )),
        "only pending friends"
    );
    let mut hidden = active();
    hidden.hidden = true;
    assert!(needs_launcher(&book("Me", vec![hidden])), "only hidden");
    assert!(!needs_launcher(&book("Me", vec![active()])));
}

#[test]
fn sharing_adds_or_removes_one_instance_and_keeps_the_rest() {
    let current = core(vec![('a', vec!["one", "two"], false), ('b', vec![], false)]);
    assert_eq!(
        shared_after(&current, &key('a'), "three", true),
        ["one", "two", "three"]
    );
    assert_eq!(
        shared_after(&current, &key('a'), "two", true),
        ["one", "two"],
        "already shared: once"
    );
    assert_eq!(shared_after(&current, &key('a'), "one", false), ["two"]);
    assert_eq!(
        shared_after(&current, &key('a'), "nine", false),
        ["one", "two"],
        "not shared: nothing to take out"
    );
    assert_eq!(shared_after(&current, &key('b'), "one", true), ["one"]);
    // a friend the core doesn't have yet starts with nothing shared
    assert_eq!(shared_after(&current, &key('z'), "one", true), ["one"]);
    assert!(shared_after(&current, &key('z'), "one", false).is_empty());
}

#[test]
fn shared_and_connected_come_from_the_core() {
    let current = core(vec![('a', vec!["one"], true), ('b', vec!["two"], false)]);
    assert!(is_shared(Some(&current), &key('a'), "one"));
    assert!(!is_shared(Some(&current), &key('a'), "two"));
    assert!(!is_shared(Some(&current), &key('z'), "one"));
    assert!(
        !is_shared(None, &key('a'), "one"),
        "nothing shared by default"
    );
    assert!(is_connected(Some(&current), &key('a')));
    assert!(!is_connected(Some(&current), &key('b')));
    assert!(!is_connected(None, &key('a')));
}

#[test]
fn the_service_comes_from_the_environment() {
    assert_eq!(service_from(None), Service::default());
    assert_eq!(service_from(Some(" ")), Service::default());
    let local = service_from(Some("http://127.0.0.1:18791"));
    assert_eq!(
        (local.host.as_str(), local.port, local.tls),
        ("127.0.0.1", 18791, false)
    );
    assert_eq!(
        service_from(Some("https://a b")),
        Service::default(),
        "not an address: the default"
    );
}

/// End to end, as the Host runs it ([`run`]), against the real `lodestone_core`
/// program and an introduction service: two people become friends (as two launchers
/// would), then the Host puts the friend on its core, shares an instance, the friend
/// sees the core in the person's state, and removing the friend drops them from the
/// core. Needs the core's program (`cargo build -p lodestone_core
/// --no-default-features`) and `LODESTONE_HOST_FRIENDS_SERVICE` (a local service:
/// `wrangler dev --local` in services/connect); without them it says so and passes.
/// The Keychain isn't used (an in-memory store), and no server is started.
#[test]
fn the_host_keeps_its_core_in_step_with_the_friend_list() {
    use std::time::{Duration, Instant};

    use futures::executor::block_on;
    use launcher_core::{
        MemorySecretStore, Secret,
        cores::{CoreClient, local_claim::OwnerCredentials},
        friend_list::now,
        paths::DataDir,
    };

    use crate::core_process::{self, CoreConfig, CoreProcess};

    let Some(service) = std::env::var("LODESTONE_HOST_FRIENDS_SERVICE")
        .ok()
        .and_then(|s| Service::parse(&s))
    else {
        eprintln!("skipped: set LODESTONE_HOST_FRIENDS_SERVICE");
        return;
    };
    let program = match core_process::find_core() {
        Ok(program) => program,
        Err(why) => {
            eprintln!("skipped: {why}");
            return;
        }
    };

    // a core with one (never started) instance
    let root = tempfile::tempdir().unwrap();
    let data_dir = root.path().join("core");
    let instance = "INSTANCE_6f1c2b9e-0d7a-4c55-9a51-3f0e2a7b8c11";
    let folder = data_dir.join("instances").join("survival");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join(".lodestone_config"),
        format!(
            r#"{{"game_type":"MinecraftJava","uuid":"{instance}","creation_time":1762700000}}"#
        ),
    )
    .unwrap();
    std::fs::write(
        folder.join(".lodestone_minecraft_config.json"),
        r#"{"name":"Survival","version":"1.21.4","flavour":"vanilla","description":"","cmd_args":[],"java_cmd":"java","port":25591,"min_ram":1024,"max_ram":2048,"auto_start":false,"restart_on_crash":false,"backup_period":null,"jre_major_version":21,"has_started":false}"#,
    )
    .unwrap();
    std::fs::write(folder.join("server.properties"), "server-port=25591\n").unwrap();
    let config = CoreConfig {
        program,
        data_dir: data_dir.clone(),
        port: (17_400..17_700)
            .find(|&port| core_process::port_is_free(port))
            .expect("a free port"),
        log_file: root.path().join("core.log"),
    };
    let mut process = CoreProcess::spawn(&config).expect("the core starts");
    let http = launcher_core::http::default_client().unwrap();
    let local = CoreClient::new(http.clone(), config.local_url());
    let started = Instant::now();
    while block_on(local.info()).is_err() {
        assert!(
            started.elapsed() < Duration::from_secs(90),
            "the core didn't answer"
        );
        std::thread::sleep(Duration::from_millis(250));
    }
    let sleep: launcher_core::game::download::Sleep =
        Arc::new(|duration| Box::pin(async move { std::thread::sleep(duration) }));
    let session = block_on(crate::session::claim_owner(
        http,
        config.local_url(),
        &data_dir,
        Some(OwnerCredentials {
            username: "friends_owner".into(),
            password: Secret::new(String::from("a long test password 456")),
        }),
        sleep,
    ))
    .expect("signed in");
    let admin = session.admin.clone();

    // two people become friends, as their launchers would
    let open = |dir: &std::path::Path, store: &MemorySecretStore| {
        block_on(Friends::open(&DataDir::at(dir), store, service.clone())).unwrap()
    };
    let (dir_a, dir_b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (store_a, store_b) = (MemorySecretStore::new(), MemorySecretStore::new());
    let mut ana = open(dir_a.path(), &store_a);
    let mut ben = open(dir_b.path(), &store_b);
    assert!(needs_launcher(&ana.book), "no name, no friends");
    ana.set_name("Ana").unwrap();
    ben.set_name("Ben").unwrap();
    let link = block_on(ana.create_link()).unwrap();
    block_on(ben.answer_link(&link)).unwrap();
    let replies = block_on(ana.link_replies()).unwrap();
    assert_eq!(replies.len(), 1);
    block_on(ana.accept_reply(&replies[0].code, &replies[0].key)).unwrap();
    block_on(ben.refresh()).unwrap();
    let ana_key = ana.me().to_hex();
    let ben_key = ben.me().to_hex();
    assert_eq!(listed(&ana.book).len(), 1);
    let ana = Arc::new(Mutex::new(ana));

    // the Host's first sync: Ben is on the core, with nothing shared
    let synced = block_on(run(
        ana.clone(),
        admin.clone(),
        "Ana's core".into(),
        Job::Sync { reload: false },
    ));
    let core = synced.core.unwrap().expect("synced");
    assert!(!synced.needs_launcher);
    assert!(synced.problems.is_empty(), "{:?}", synced.problems);
    assert_eq!(core.friends.len(), 1);
    assert_eq!(core.friends[0].key, ben_key);
    assert!(
        !is_shared(Some(&core), &ben_key, instance),
        "nothing shared by default"
    );

    // a tick shares the instance with Ben
    let shared = block_on(run(
        ana.clone(),
        admin.clone(),
        "Ana's core".into(),
        Job::Share {
            key: ben_key.clone(),
            instance: instance.into(),
            share: true,
        },
    ));
    let core = shared.core.unwrap().expect("shared");
    assert!(is_shared(Some(&core), &ben_key, instance));
    let fingerprint = core.fingerprint.clone();

    // Ben finds the core in Ana's state
    block_on(ben.refresh()).unwrap();
    let cores = ben.book.friend(&ana_key).unwrap().joinable_cores(now());
    assert!(
        cores.iter().any(|c| c
            .fingerprint
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
            == fingerprint),
        "{cores:?}"
    );

    // unticking keeps Ben on the core, with nothing shared
    let unshared = block_on(run(
        ana.clone(),
        admin.clone(),
        "Ana's core".into(),
        Job::Share {
            key: ben_key.clone(),
            instance: instance.into(),
            share: false,
        },
    ));
    let core = unshared.core.unwrap().expect("unshared");
    assert_eq!(core.friends.len(), 1);
    assert!(!is_shared(Some(&core), &ben_key, instance));

    // removing Ben (in the launcher) drops him from the core at the next sync, though
    // there's no one left to share with
    block_on(async { ana.lock().await.remove(&ben_key).await }).unwrap();
    let removed = block_on(run(
        ana.clone(),
        admin.clone(),
        "Ana's core".into(),
        Job::Sync { reload: true },
    ));
    assert!(removed.needs_launcher);
    let core = removed.core.unwrap().expect("synced after the removal");
    assert!(core.friends.is_empty(), "{core:?}");

    let status = process.stop(Duration::from_secs(30));
    assert!(status.is_some_and(|s| s.success()), "{status:?}");
}
