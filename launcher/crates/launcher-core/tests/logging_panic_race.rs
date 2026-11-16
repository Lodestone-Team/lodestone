//! The panic hook under load, in a process of its own (`logging::install` sets the
//! process's logger and panic hook): two threads panicking at once, a panic whose
//! message holds terminal escapes, and a place that panics over and over.

use std::{
    fs,
    sync::{Arc, Barrier},
    thread,
};

use launcher_core::logging::{self, AppKind, CRASH_FILE, Config, LogLevel, PREVIOUS_CRASH_FILE};

#[test]
fn panics_at_once_or_in_floods_are_all_kept_in_order() {
    let data = tempfile::tempdir().unwrap();
    let dir = data.path().join("logs");
    // an older run's crash, already reported
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join(CRASH_FILE),
        "run old 2026-11-15T08:00:00Z\nan older crash\n",
    )
    .unwrap();
    logging::install(Config {
        app: AppKind::Launcher,
        dir: Some(dir.clone()),
        level: LogLevel::Info,
        env_filter: None,
        version: "9.9.9-test".into(),
    });
    let log_file = dir.join("launcher.log");

    // Two threads panicking at the same moment: the first replaces the old run's
    // file, the other is added to the new one, never to the old one.
    let barrier = Arc::new(Barrier::new(2));
    let threads: Vec<_> = ["first racer", "second racer"]
        .into_iter()
        .map(|message| {
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                panic!("{message}");
            })
        })
        .collect();
    for thread in threads {
        assert!(thread.join().is_err());
    }
    let crash = fs::read_to_string(dir.join(CRASH_FILE)).unwrap();
    assert!(crash.starts_with("run "), "{crash}");
    assert!(
        crash.contains("first racer") && crash.contains("second racer"),
        "{crash}"
    );
    assert!(!crash.contains("an older crash"), "{crash}");
    let previous = fs::read_to_string(dir.join(PREVIOUS_CRASH_FILE)).unwrap();
    assert_eq!(previous, "run old 2026-11-15T08:00:00Z\nan older crash\n");
    assert!(
        !fs::read_dir(&dir).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")),
        "no temporary file is left"
    );

    // Terminal escapes in a message (OSC 52 writes the clipboard) are written as
    // escapes in the crash file, as in the log.
    let _ = thread::spawn(|| panic!("from a core: \u{1b}]52;c;aGk=\u{7} done")).join();
    let crash = fs::read_to_string(dir.join(CRASH_FILE)).unwrap();
    assert!(
        crash.contains(r"from a core: \u{1b}]52;c;aGk=\u{7} done"),
        "{crash}"
    );
    assert!(!crash.contains('\u{1b}') && !crash.contains('\u{7}'));

    // A place that panics on every connection: the log takes a few reports a
    // minute, the crash file keeps the newest.
    let flooding = |n: usize| {
        let _ =
            thread::spawn(move || panic!("{}", format!("flood {n} {}", "x".repeat(4000)))).join();
    };
    for n in 0..40 {
        flooding(n);
    }
    logging::flush();
    let text = fs::read_to_string(&log_file).unwrap();
    assert_eq!(
        text.matches("flood ").count(),
        logging::LOGGED_PER_MINUTE as usize,
        "a few reports a minute from one place"
    );
    let crash = fs::read_to_string(dir.join(CRASH_FILE)).unwrap();
    // whichever racer took the file first leads
    assert!(crash.contains(" racer\n"), "the first report leads");
    assert!(crash.contains("flood 39 "), "the newest is kept");
    assert!(
        !crash.contains("flood 38 "),
        "one newest, not all: {}",
        crash.len()
    );
    assert!(crash.len() < 80 * 1024, "bounded: {}", crash.len());
    // The panic after a flood is still the newest.
    let _ = thread::spawn(|| panic!("the fatal panic")).join();
    let crash = fs::read_to_string(dir.join(CRASH_FILE)).unwrap();
    assert!(crash.contains("the fatal panic") && !crash.contains("flood 39 "));
    assert!(crash.starts_with("run "));
    let text = fs::read_to_string(&log_file).unwrap();
    assert!(text.contains("the fatal panic"), "another place is logged");
}
