use super::*;
use launcher_core::friend_list::{
    Book, Friend, FriendStatus,
    core_sync::{CoreFriend, CoreFriends},
};

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
                hidden: false,
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
fn a_test_copy_never_shares_with_the_real_friend_list() {
    let home = std::path::Path::new("/Users/ana");
    let real = home.join(".lodestone");
    let test = std::path::Path::new("/tmp/host-dev/core");
    // the real core with the real friend list
    assert!(sharing_allowed(&real, Some(home), false));
    // a dev bundle or test: a test core, but the real friend list
    assert!(!sharing_allowed(test, Some(home), false));
    // a debug build's own folder isn't the real one either
    assert!(!sharing_allowed(
        &home.join(".lodestone-host-dev"),
        Some(home),
        false
    ));
    // both test folders: allowed, for testing
    assert!(sharing_allowed(test, Some(home), true));
    // a test friend list must not replace the real core's friends
    assert!(!sharing_allowed(&real, Some(home), true));
}

#[test]
fn each_core_folder_is_its_own_source() {
    let a = core_source(std::path::Path::new("/Users/ana/.lodestone"));
    assert!(a.starts_with("host:") && a.len() == 21, "{a}");
    assert_eq!(
        a,
        core_source(std::path::Path::new("/Users/ana/.lodestone"))
    );
    assert_ne!(a, core_source(std::path::Path::new("/tmp/core")));
}

#[test]
fn only_changes_the_core_cares_about_sync_again() {
    let mut book = book("Me", vec![friend('a', "Alex", FriendStatus::Active)]);
    let view = SyncView::of(&book);
    // the Host's own writes: state numbers, sent states, the generation
    book.state_seq += 5;
    book.generation += 3;
    book.friends[0].sent_seq = 9;
    assert_eq!(SyncView::of(&book), view);
    // the Launcher's: hiding, renaming, verifying, retiring, a retired core
    let changes: [fn(&mut Book); 5] = [
        |b| b.friends[0].hidden = true,
        |b| b.friends[0].nickname = Some("Al".into()),
        |b| b.friends[0].verified = true,
        |b| b.friends[0].status = FriendStatus::Retired,
        |b| b.retired_cores.push("11".repeat(32)),
    ];
    for change in changes {
        let mut changed = book.clone();
        change(&mut changed);
        assert_ne!(SyncView::of(&changed), view);
    }
}

#[test]
fn rows_carry_safety_codes_and_unverified_shares_ask_first() {
    let mut alex = friend('a', "Alex", FriendStatus::Active);
    alex.key = lodestone_connect::friend::PersonKey::generate()
        .unwrap()
        .public()
        .to_hex();
    let rows = listed(&book("Me", vec![alex.clone()]));
    assert_eq!(rows[0].safety_code, alex.safety_code());
    assert_eq!(rows[0].safety_code.len(), 19);
    let current = core(vec![('b', vec!["one"], false)]);
    assert!(shares_anything(Some(&current), &key('b')));
    assert!(!shares_anything(Some(&current), &key('a')));
    assert!(!shares_anything(None, &key('b')));
}

/// End to end, as the Host runs it ([`run`]), against the real `lodestone_core`
/// program and an introduction service: two people become friends (as two launchers
/// would), then the Host puts the friend on its core, shares an instance, the friend
/// sees the core in the person's state. A friend file that goes missing stops the Host
/// without touching the core, removing the friend drops them from the core, and a
/// retired identity empties it. Needs the core's program (`cargo build -p lodestone_core
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
        .and_then(|s| lodestone_connect::Service::parse(&s))
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
        // the service under test (LODESTONE_CONNECT_SERVICE), as the Host passes it
        connect_service: CoreConfig::connect_service_now(),
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
        block_on(Friends::create(&DataDir::at(dir), store, service.clone())).unwrap()
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
    let ana_file = dir_a.path().join(launcher_core::friend_list::FILE_NAME);
    let ana = Arc::new(Mutex::new(ana));
    let source = core_source(&data_dir);
    let check = || Job::Check {
        last: None,
        force: true,
    };

    // the Host's first sync: Ben is on the core, with nothing shared
    let synced = block_on(run(
        ana.clone(),
        admin.clone(),
        "Ana's core".into(),
        source.clone(),
        check(),
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
        source.clone(),
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
        source.clone(),
        Job::Share {
            key: ben_key.clone(),
            instance: instance.into(),
            share: false,
        },
    ));
    let core = unshared.core.unwrap().expect("unshared");
    assert_eq!(core.friends.len(), 1);
    assert!(!is_shared(Some(&core), &ben_key, instance));

    // retiring this core's certificate from the Host: friends drop the old one, and
    // the core gets a new one they find instead
    let renewed = block_on(run(
        ana.clone(),
        admin.clone(),
        "Ana's core".into(),
        source.clone(),
        Job::Renew {
            retire_current: true,
        },
    ));
    if renewed.no_certificate_route {
        // a core from before POST /connect/friends/certificate: the Settings say how
        // to get one by hand, and the rest of this test needs the core shared
        assert!(renewed.core_retired);
        eprintln!("this core can't make a new friend certificate: stopping here");
        let _ = process.stop(Duration::from_secs(30));
        return;
    }
    let core = renewed.core.unwrap().expect("renewed");
    assert_ne!(core.fingerprint, fingerprint, "a new certificate");
    assert_eq!(core.friends.len(), 1, "the list stays");
    let refreshed = block_on(ben.refresh()).unwrap();
    assert_eq!(refreshed.dropped_cores, vec![fingerprint.clone()]);
    let cores = ben.book.friend(&ana_key).unwrap().joinable_cores(now());
    let hexes: Vec<String> = cores
        .iter()
        .map(|c| c.fingerprint.iter().map(|b| format!("{b:02x}")).collect())
        .collect();
    assert_eq!(hexes, vec![core.fingerprint.clone()]);
    {
        let book = &block_on(ana.lock()).book;
        assert!(book.retired_cores.contains(&fingerprint));
        assert_eq!(book.my_cores.len(), 1, "only the new certificate is listed");
    }

    // hiding Ben keeps him on the core with what's shared with him (the core serves him
    // nothing meanwhile), and showing him again gives it back
    let share = |share: bool| Job::Share {
        key: ben_key.clone(),
        instance: instance.into(),
        share,
    };
    let sync = |job: Job| {
        block_on(run(
            ana.clone(),
            admin.clone(),
            "Ana's core".into(),
            source.clone(),
            job,
        ))
    };
    sync(share(true)).core.unwrap().expect("shared");
    block_on(async { ana.lock().await.set_hidden(&ben_key, true) }).unwrap();
    let hidden = sync(check()).core.unwrap().expect("synced while hidden");
    let entry = hidden.friends.iter().find(|f| f.key == ben_key).unwrap();
    assert!(
        entry.hidden && entry.shared == [instance.to_string()],
        "{entry:?}"
    );
    block_on(async { ana.lock().await.set_hidden(&ben_key, false) }).unwrap();
    let shown = sync(check()).core.unwrap().expect("synced when shown");
    let entry = shown.friends.iter().find(|f| f.key == ben_key).unwrap();
    assert!(
        !entry.hidden && entry.shared == [instance.to_string()],
        "{entry:?}"
    );
    let renewed = sync(share(false));

    // nothing changed: the next check does nothing
    let again = block_on(run(
        ana.clone(),
        admin.clone(),
        "Ana's core".into(),
        source.clone(),
        Job::Check {
            last: renewed.view.clone(),
            force: false,
        },
    ));
    assert!(again.unchanged);

    // the friend file goes (moved with the data folder, say): the Host stops, and the
    // core keeps Ben (never an empty list because a file is missing)
    let moved = dir_a.path().join("moved.json");
    std::fs::rename(&ana_file, &moved).unwrap();
    let lost = block_on(run(
        ana.clone(),
        admin.clone(),
        "Ana's core".into(),
        source.clone(),
        check(),
    ));
    assert!(lost.lost && lost.core.is_none());
    assert!(!ana_file.exists(), "nothing wrote it again");
    let core = block_on(core_sync::read(&admin)).unwrap();
    assert_eq!(core.friends.len(), 1, "Ben is still on the core");
    std::fs::rename(&moved, &ana_file).unwrap();

    // removing Ben (in the launcher) drops him from the core at the next sync, though
    // there's no one left to share with
    block_on(async { ana.lock().await.remove(&ben_key).await }).unwrap();
    let removed = block_on(run(
        ana.clone(),
        admin.clone(),
        "Ana's core".into(),
        source.clone(),
        check(),
    ));
    assert!(removed.needs_launcher);
    let core = removed.core.unwrap().expect("synced after the removal");
    assert!(core.friends.is_empty(), "{core:?}");

    // a retired identity: the Host sees it, and empties the core on purpose
    block_on(async { ana.lock().await.retire_identity().await }).unwrap();
    let retired = block_on(run(
        ana.clone(),
        admin.clone(),
        "Ana's core".into(),
        source.clone(),
        check(),
    ));
    assert!(retired.retired);
    assert!(retired.core.unwrap().unwrap().friends.is_empty());

    let status = process.stop(Duration::from_secs(30));
    assert!(status.is_some_and(|s| s.success()), "{status:?}");
}
