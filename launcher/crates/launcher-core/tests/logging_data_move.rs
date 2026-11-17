//! Moving the default data folder while the logger holds its log open, in a process
//! of its own (`logging::install` is once per process). Windows refuses to rename a
//! folder with an open file in it, so the move closes the log for the rename: it is
//! one rename, not a copy, and the log carries on in the default folder.

use std::{fs, sync::atomic::Ordering};

use launcher_core::{
    data_move::{self, MoveProgress, MoveRequest},
    disk::Cancel,
    logging::{self, AppKind, Config, LogLevel},
};

#[test]
fn moving_the_default_folder_renames_it_and_keeps_the_log() {
    let root = tempfile::tempdir().unwrap();
    let default = root.path().join("Lodestone Launcher");
    let logs = default.join(logging::LOGS_DIR);
    fs::create_dir_all(default.join("instances/a")).unwrap();
    fs::write(default.join("instances/a/instance.json"), "{}").unwrap();
    logging::install(Config {
        app: AppKind::Launcher,
        dir: Some(logs.clone()),
        level: LogLevel::Info,
        env_filter: None,
        version: "9.9.9-test".into(),
    });
    log::warn!("before the move");

    let to = root.path().join("Games/Lodestone");
    let progress = MoveProgress::default();
    data_move::run(
        &MoveRequest {
            from: default.clone(),
            to: to.clone(),
            default: default.clone(),
        },
        &progress,
        &Cancel::default(),
    )
    .unwrap();
    // one rename: a copy would count the bytes
    assert_eq!(progress.total.load(Ordering::Relaxed), 1);
    assert!(to.join("instances/a/instance.json").is_file());
    assert!(
        !to.join(logging::LOGS_DIR).exists(),
        "the logs stayed behind"
    );

    log::warn!("after the move");
    logging::flush();
    let text = fs::read_to_string(logs.join("launcher.log")).unwrap();
    assert!(
        text.contains("before the move") && text.contains("after the move"),
        "{text}"
    );
    logging::end_session();
}
