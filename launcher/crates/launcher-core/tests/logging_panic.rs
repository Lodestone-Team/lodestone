//! The panic hook, in a process of its own: `logging::install` sets the process's
//! logger and panic hook, which would catch other tests' panics.

use std::{fs, thread};

use launcher_core::logging::{self, AppKind, CRASH_FILE, Config, LogLevel};

#[test]
fn a_panic_on_a_background_thread_is_logged_and_kept() {
    // The data folders are temporary: nothing goes to the real ones.
    let data = tempfile::tempdir().unwrap();
    // SAFETY: set before any other thread of this test process reads the environment.
    unsafe { std::env::set_var(launcher_control::paths::DATA_DIR_ENV, data.path()) };
    let location = launcher_control::paths::locate().unwrap();
    let dir = launcher_control::paths::logs_dir(&location.default);
    assert!(dir.starts_with(data.path()));

    let started = logging::install(Config {
        app: AppKind::Launcher,
        dir: Some(dir.clone()),
        level: LogLevel::Info,
        env_filter: None,
        version: "9.9.9-test".into(),
    });
    assert!(started.previous_crash.is_none());
    assert!(!started.env_override);
    let log_file = logging::log_file().unwrap();
    assert_eq!(log_file, dir.join("launcher.log"));
    assert!(log_file.starts_with(data.path()));

    let joined = thread::Builder::new()
        .name("gpui-dispatch-test".into())
        .spawn(|| panic!("a timer made outside a runtime (token=hunter2)"))
        .unwrap()
        .join();
    assert!(joined.is_err());

    // Flushed by the hook: read without flushing.
    let text = fs::read_to_string(&log_file).unwrap();
    assert!(text.contains("ERROR panic: thread 'gpui-dispatch-test'"), "{text}");
    assert!(text.contains("a timer made outside a runtime"), "{text}");
    assert!(text.contains("tests/logging_panic.rs:"), "{text}");
    assert!(text.contains("backtrace:"), "{text}");
    assert!(text.contains("9.9.9-test"), "{text}");
    assert!(!text.contains("hunter2"), "{text}");

    let crash = fs::read_to_string(dir.join(CRASH_FILE)).unwrap();
    let header = crash.lines().next().unwrap();
    assert!(header.starts_with("run ") && header.ends_with("Lodestone Launcher 9.9.9-test"));
    assert!(crash.contains("a timer made outside a runtime"));
    assert!(!crash.contains("hunter2"));

    // A second panic in the same run (GPUI's "panic in a function that cannot
    // unwind" follows the first) is added to it.
    let _ = thread::spawn(|| panic!("the second panic")).join();
    let crash = fs::read_to_string(dir.join(CRASH_FILE)).unwrap();
    assert!(crash.contains("a timer made outside a runtime") && crash.contains("the second panic"));
    assert!(crash.contains("thread '<unnamed>'"));

    // The next start notices it, once; a clean quit after that shows nothing.
    let run_id = header.split(' ').nth(1).unwrap().to_string();
    let session = fs::read_to_string(dir.join(logging::SESSION_FILE)).unwrap();
    assert!(session.contains(&run_id));

    logging::end_session();
    assert!(!dir.join(logging::SESSION_FILE).exists());
    // Everything this test wrote is in the temporary data folder.
    for entry in fs::read_dir(data.path()).unwrap() {
        assert_eq!(entry.unwrap().file_name(), "logs");
    }
}
