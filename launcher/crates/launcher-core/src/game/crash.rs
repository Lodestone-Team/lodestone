//! Telling a crash apart from a normal exit, and explaining it.
//!
//! A game crashed if it exited with a non-zero code (and the user did not stop it),
//! or if it wrote a new file to `crash-reports/`. The summary is the exception line
//! from the crash report or the log, and the likely causes are matched from the
//! report and the last lines of the log.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use super::log4j::{Level, LogLine};

pub mod suspects;

/// A likely cause of a crash, in plain language.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    /// The game or a mod needs a newer (or older) Java.
    WrongJava,
    OutOfMemory,
    /// A native library is missing or for the wrong CPU, such as an arm64 Java
    /// loading x86_64 natives.
    Natives,
    /// The JVM refused a setting, such as an unknown JVM argument.
    JvmOptions,
    /// OpenGL or the window could not be set up.
    Graphics,
    /// A mod couldn't patch the game (a Mixin failure).
    ModPatch,
}

impl Cause {
    pub fn explanation(self) -> &'static str {
        match self {
            Cause::WrongJava => {
                "Wrong Java version. This Minecraft version (or a mod) needs a different Java. Clear the Java override in Settings to use the Java Mojang provides."
            }
            Cause::OutOfMemory => {
                "Out of memory. Raise the maximum memory in Settings, or close other apps."
            }
            Cause::Natives => {
                "Native libraries are missing or are for the wrong processor. On Apple Silicon, versions before 1.19 run in an Intel Java under Rosetta 2; check that Rosetta is installed and that the Java override isn't an arm64 Java."
            }
            Cause::JvmOptions => {
                "Java refused one of the settings. Check the JVM arguments and memory in Settings."
            }
            Cause::Graphics => {
                if cfg!(target_os = "macos") {
                    "The game couldn't set up its window or OpenGL. Update macOS, or try without a custom window size or fullscreen."
                } else {
                    "The game couldn't set up its window or OpenGL. Update your graphics drivers, or try without a custom window size or fullscreen."
                }
            }
            Cause::ModPatch => {
                "A mod couldn't load its changes into the game. It's probably made for a different Minecraft version, or it needs a newer version of another mod. Update the mod or turn it off."
            }
        }
    }
}

/// What the crash dialog shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CrashSummary {
    /// The exception line, such as `java.lang.NullPointerException: ...`.
    pub exception: Option<String>,
    /// The crash report's description, such as `Unexpected error`.
    pub description: Option<String>,
    pub causes: Vec<Cause>,
}

/// Summarizes a crash from the crash report (if one was written) and the end of the
/// log.
pub fn summarize(crash_report: Option<&str>, log: &[LogLine]) -> CrashSummary {
    let mut summary = CrashSummary::default();
    if let Some(report) = crash_report {
        let mut lines = report.lines();
        while let Some(line) = lines.next() {
            if let Some(description) = line.strip_prefix("Description:") {
                summary.description = Some(description.trim().to_string());
                summary.exception = lines
                    .by_ref()
                    .map(str::trim)
                    .find(|line| !line.is_empty())
                    .map(str::to_string);
                break;
            }
        }
    }
    if summary.exception.is_none() {
        summary.exception = exception_from_log(log);
    }

    let mut text = String::new();
    if let Some(report) = crash_report {
        text.push_str(report);
    }
    for line in log.iter().rev().take(200) {
        text.push('\n');
        text.push_str(&line.message);
        if let Some(throwable) = &line.throwable {
            text.push('\n');
            text.push_str(throwable);
        }
    }
    summary.causes = causes(&text);
    summary
}

/// The last exception in the log: a throwable's first line, or a line such as
/// `Exception in thread "main" java.lang.UnsatisfiedLinkError: ...`.
fn exception_from_log(log: &[LogLine]) -> Option<String> {
    for line in log.iter().rev() {
        if let Some(throwable) = &line.throwable
            && let Some(first) = throwable.lines().map(str::trim).find(|l| !l.is_empty())
        {
            return Some(first.to_string());
        }
        let message = line.message.trim();
        if line.level >= Level::Error || looks_like_exception(message) {
            if let Some(index) = message.find("Exception in thread") {
                // Keep the exception, not the thread name.
                let rest = &message[index..];
                if let Some(space) = rest.find(" java.").or_else(|| rest.rfind("\" ")) {
                    return Some(rest[space + 1..].trim_start_matches('"').trim().to_string());
                }
            }
            if looks_like_exception(message) {
                return message.lines().next().map(str::to_string);
            }
        }
    }
    None
}

fn looks_like_exception(line: &str) -> bool {
    let first_word = line.split([':', ' ']).next().unwrap_or_default();
    (first_word.ends_with("Exception") || first_word.ends_with("Error")) && first_word.contains('.')
        || line.starts_with("Exception in thread")
        || line.starts_with("Error: ")
}

/// The causes whose signs appear in `text`.
pub fn causes(text: &str) -> Vec<Cause> {
    let checks: [(Cause, &[&str]); 6] = [
        (
            Cause::WrongJava,
            &[
                "UnsupportedClassVersionError",
                "has been compiled by a more recent version of the Java Runtime",
                "requires a newer Java",
                "class file version",
            ],
        ),
        (
            Cause::OutOfMemory,
            &[
                "OutOfMemoryError",
                "Java heap space",
                "Could not reserve enough space",
                "GC overhead limit exceeded",
            ],
        ),
        (
            Cause::Natives,
            &[
                "UnsatisfiedLinkError",
                "no lwjgl in java.library.path",
                "Failed to locate library",
                "incompatible architecture",
                "Can't load library",
            ],
        ),
        (
            Cause::JvmOptions,
            &[
                "Unrecognized VM option",
                "Unrecognized option",
                "Could not create the Java Virtual Machine",
                "Invalid maximum heap size",
                "Invalid initial heap size",
            ],
        ),
        (
            Cause::Graphics,
            &[
                "GLFW error",
                "Pixel format not accelerated",
                "No OpenGL context",
                "Failed to create window",
                "WGL: The driver does not appear to support OpenGL",
            ],
        ),
        (
            Cause::ModPatch,
            &[
                "Mixin apply for mod",
                "Mixin apply failed",
                "MixinApplyError",
                "InvalidInjectionException",
                "Critical injection failure",
                "FAILED during APPLY",
            ],
        ),
    ];
    checks
        .into_iter()
        .filter(|(_, signs)| signs.iter().any(|sign| text.contains(sign)))
        .map(|(cause, _)| cause)
        .collect()
}

/// Whether a log line says the game has finished loading: the sound engine starts at
/// the end of the first resource reload, just before the main menu, in every version
/// and loader since 1.12.
pub fn reached_menu(line: &str) -> bool {
    line.contains("Sound engine started")
}

/// Whether the log says the mod loader refused the mods: one needs a mod that's
/// missing or of the wrong version, or two can't be loaded together. Fabric and Quilt
/// exit; Forge and NeoForge show an error screen and stay open.
pub fn mods_refused(text: &str) -> bool {
    const SIGNS: &[&str] = &[
        "Incompatible mods found!",
        "Incompatible mod set!",
        "Mod resolution failed",
        "Mod resolution encountered an incompatible mod set",
        "Missing or unsupported mandatory dependencies",
        "Mod loading failures have occurred",
        "Failed to resolve mods",
    ];
    SIGNS.iter().any(|sign| text.contains(sign))
}

/// The names of the crash reports in `<game dir>/crash-reports` now.
pub fn crash_reports(game_dir: &Path) -> BTreeSet<String> {
    fs::read_dir(game_dir.join("crash-reports"))
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "txt"))
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

/// The newest crash report that was not in `before`.
pub fn new_crash_report(game_dir: &Path, before: &BTreeSet<String>) -> Option<PathBuf> {
    // Report names start with the date and time, so the last is the newest.
    crash_reports(game_dir)
        .into_iter()
        .rev()
        .find(|name| !before.contains(name))
        .map(|name| game_dir.join("crash-reports").join(name))
}

/// How a game run ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// It exited normally, or the user stopped it.
    Exited,
    Crashed {
        exit_code: Option<i32>,
        report: Option<PathBuf>,
    },
}

/// Decides whether a run crashed.
pub fn outcome(exit_code: Option<i32>, stopped_by_user: bool, report: Option<PathBuf>) -> Outcome {
    let failed = !stopped_by_user && exit_code != Some(0);
    if failed || report.is_some() {
        Outcome::Crashed { exit_code, report }
    } else {
        Outcome::Exited
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::log4j::Stream;

    const REPORT: &str = "---- Minecraft Crash Report ----\n// Don't be sad, have a hug! <3\n\nTime: 2026-11-07 12:00:00\nDescription: Initializing game\n\njava.lang.OutOfMemoryError: Java heap space\n\tat net.minecraft.client.Minecraft.<init>(Minecraft.java:1)\n";

    #[test]
    fn summarizes_a_crash_report() {
        let summary = summarize(Some(REPORT), &[]);
        assert_eq!(summary.description.as_deref(), Some("Initializing game"));
        assert_eq!(
            summary.exception.as_deref(),
            Some("java.lang.OutOfMemoryError: Java heap space")
        );
        assert_eq!(summary.causes, [Cause::OutOfMemory]);
    }

    #[test]
    fn summarizes_from_the_log_without_a_report() {
        let log = vec![
            LogLine::plain(Stream::Stdout, "Loading natives"),
            LogLine::plain(
                Stream::Stderr,
                "Exception in thread \"main\" java.lang.UnsatisfiedLinkError: /natives/liblwjgl.dylib: dlopen failed (mach-o file, but is an incompatible architecture (have 'x86_64', need 'arm64'))",
            ),
            LogLine::plain(
                Stream::Stderr,
                "\tat java.lang.ClassLoader$NativeLibrary.load(Native Method)",
            ),
        ];
        let summary = summarize(None, &log);
        assert!(
            summary
                .exception
                .as_deref()
                .unwrap()
                .starts_with("java.lang.UnsatisfiedLinkError: /natives/liblwjgl.dylib")
        );
        assert_eq!(summary.causes, [Cause::Natives]);
    }

    #[test]
    fn uses_the_last_throwable() {
        let mut line = LogLine::plain(Stream::Stdout, "Unreported exception thrown!");
        line.level = Level::Error;
        line.throwable =
            Some("java.lang.IllegalStateException: Lock is no longer valid\n\tat x".into());
        let summary = summarize(None, &[LogLine::plain(Stream::Stdout, "ok"), line]);
        assert_eq!(
            summary.exception.as_deref(),
            Some("java.lang.IllegalStateException: Lock is no longer valid")
        );
        assert!(summary.causes.is_empty());
    }

    #[test]
    fn recognises_wrong_java_and_jvm_options() {
        let log = [LogLine::plain(
            Stream::Stderr,
            "Error: LinkageError occurred while loading main class net.minecraft.client.main.Main\n\tjava.lang.UnsupportedClassVersionError: net/minecraft/client/main/Main has been compiled by a more recent version of the Java Runtime (class file version 65.0)",
        )];
        let summary = summarize(None, &log);
        assert_eq!(summary.causes, [Cause::WrongJava]);
        assert!(
            summary
                .exception
                .unwrap()
                .starts_with("Error: LinkageError")
        );
        assert_eq!(
            causes(
                "Unrecognized VM option 'UseZGC'\nError: Could not create the Java Virtual Machine."
            ),
            [Cause::JvmOptions]
        );
        assert_eq!(
            causes("GLFW error 65543: NSGL: Failed to find a suitable pixel format"),
            [Cause::Graphics]
        );
    }

    #[test]
    fn recognises_loading_milestones() {
        assert!(reached_menu("Sound engine started"));
        assert!(!reached_menu("Starting up SoundSystem..."));
        assert!(mods_refused(
            "Incompatible mods found!\n - Mod 'Zenith' (zenith) requires mod 'Porting Lib'"
        ));
        assert!(mods_refused(
            "Missing or unsupported mandatory dependencies:\n\tMod ID: 'flywheel'"
        ));
        assert!(!mods_refused("Loading 212 mods"));
    }

    #[test]
    fn finds_new_crash_reports() {
        let dir = tempfile::tempdir().unwrap();
        let reports = dir.path().join("crash-reports");
        assert!(crash_reports(dir.path()).is_empty());
        fs::create_dir_all(&reports).unwrap();
        fs::write(reports.join("crash-2026-11-01_10.00.00-client.txt"), "old").unwrap();
        let before = crash_reports(dir.path());
        assert_eq!(new_crash_report(dir.path(), &before), None);
        fs::write(reports.join("crash-2026-11-07_12.00.00-client.txt"), REPORT).unwrap();
        fs::write(reports.join("notes.md"), "not a report").unwrap();
        assert_eq!(
            new_crash_report(dir.path(), &before),
            Some(reports.join("crash-2026-11-07_12.00.00-client.txt"))
        );
    }

    #[test]
    fn outcomes() {
        assert_eq!(outcome(Some(0), false, None), Outcome::Exited);
        assert_eq!(outcome(None, true, None), Outcome::Exited);
        assert_eq!(outcome(Some(137), true, None), Outcome::Exited);
        assert_eq!(
            outcome(Some(1), false, None),
            Outcome::Crashed {
                exit_code: Some(1),
                report: None
            }
        );
        // A crash report means a crash even with exit code 0 (old versions exit
        // normally after writing one).
        let report = PathBuf::from("/game/crash-reports/crash.txt");
        assert_eq!(
            outcome(Some(0), false, Some(report.clone())),
            Outcome::Crashed {
                exit_code: Some(0),
                report: Some(report)
            }
        );
    }
}
