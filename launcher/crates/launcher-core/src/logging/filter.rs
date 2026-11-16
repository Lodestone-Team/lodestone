//! Which records reach the log file: a level for the apps' own crates, another for
//! everything else, or a `RUST_LOG`-style filter from [`super::ENV`].

use log::{Level, LevelFilter};

use super::LogLevel;

/// The crates whose records are "ours": Lodestone's (`lodestone_launcher`,
/// `lodestone_host`, `lodestone_ui`, …) and `launcher_core`/`launcher_control`.
const OWN: [&str; 2] = ["lodestone", "launcher"];

/// Levels by target prefix. The longest prefix that matches a record's target decides;
/// without one, `default` does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    default: LevelFilter,
    directives: Vec<(String, LevelFilter)>,
}

impl Filter {
    /// The apps' own crates at `level`, everything else (GPUI, HTTP, TLS) at warnings.
    pub fn defaults(level: LogLevel) -> Self {
        let own = match level {
            LogLevel::Info => LevelFilter::Info,
            LogLevel::Debug => LevelFilter::Debug,
        };
        Self {
            default: LevelFilter::Warn,
            directives: OWN.iter().map(|name| (name.to_string(), own)).collect(),
        }
    }

    /// Reads a filter written like `RUST_LOG`: comma-separated `level`, `target=level`
    /// or `target` (every level) directives, such as `info,lodestone_host=debug,gpui=off`.
    /// A target is a prefix of the records' targets (`launcher_core::cores`).
    pub fn parse(spec: &str) -> Result<Self, String> {
        let mut filter = Self {
            default: LevelFilter::Warn,
            directives: Vec::new(),
        };
        let mut any = false;
        for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            any = true;
            match part.split_once('=') {
                Some((target, level)) => {
                    let target = target.trim();
                    if target.is_empty() {
                        return Err(format!("\"{part}\" names no target"));
                    }
                    filter
                        .directives
                        .push((target.to_string(), parse_level(level.trim())?));
                }
                None => match parse_level(part) {
                    Ok(level) => filter.default = level,
                    // a bare target: everything it records
                    Err(_) if is_target(part) => filter
                        .directives
                        .push((part.to_string(), LevelFilter::Trace)),
                    Err(err) => return Err(err),
                },
            }
        }
        if !any {
            return Err("the filter is empty".into());
        }
        Ok(filter)
    }

    /// Whether a record at `level` from `target` passes.
    pub fn enabled(&self, level: Level, target: &str) -> bool {
        let limit = self
            .directives
            .iter()
            .filter(|(prefix, _)| target.starts_with(prefix.as_str()))
            .max_by_key(|(prefix, _)| prefix.len())
            .map_or(self.default, |(_, level)| *level);
        level <= limit
    }
}

fn parse_level(text: &str) -> Result<LevelFilter, String> {
    text.parse::<LevelFilter>()
        .map_err(|_| format!("\"{text}\" isn't a level (off, error, warn, info, debug, trace)"))
}

fn is_target(text: &str) -> bool {
    text.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_keep_own_crates_at_the_chosen_level_and_others_at_warnings() {
        let info = Filter::defaults(LogLevel::Info);
        assert!(info.enabled(Level::Info, "lodestone_launcher::shell"));
        assert!(info.enabled(Level::Info, "launcher_core::cores::live"));
        assert!(!info.enabled(Level::Debug, "lodestone_host::supervisor"));
        assert!(info.enabled(Level::Warn, "gpui::window"));
        assert!(!info.enabled(Level::Info, "gpui::window"));
        assert!(!info.enabled(Level::Info, "rustls::client"));
        assert!(info.enabled(Level::Error, "zed_reqwest"));

        let debug = Filter::defaults(LogLevel::Debug);
        assert!(debug.enabled(Level::Debug, "lodestone_host::supervisor"));
        assert!(!debug.enabled(Level::Trace, "lodestone_host::supervisor"));
        assert!(!debug.enabled(Level::Debug, "rustls::client"));
    }

    #[test]
    fn env_filters_read_like_rust_log() {
        let filter =
            Filter::parse("info, lodestone_host=debug ,gpui=off,launcher_core::cores").unwrap();
        assert!(filter.enabled(Level::Info, "hyper"));
        assert!(!filter.enabled(Level::Debug, "hyper"));
        assert!(filter.enabled(Level::Debug, "lodestone_host::app"));
        assert!(!filter.enabled(Level::Error, "gpui::window"));
        assert!(filter.enabled(Level::Trace, "launcher_core::cores::live"));
        assert!(!filter.enabled(Level::Debug, "launcher_core::update"));

        // the longest prefix decides
        let filter = Filter::parse("launcher_core=error,launcher_core::update=debug").unwrap();
        assert!(filter.enabled(Level::Debug, "launcher_core::update"));
        assert!(!filter.enabled(Level::Warn, "launcher_core::cores"));
        assert_eq!(Filter::parse("debug").unwrap().default, LevelFilter::Debug);
    }

    #[test]
    fn bad_filters_are_refused() {
        for bad in ["", " , ", "lodestone=loud", "=debug", "what is this"] {
            assert!(Filter::parse(bad).is_err(), "{bad}");
        }
    }
}
