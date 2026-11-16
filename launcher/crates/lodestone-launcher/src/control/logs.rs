//! `logs.*`: the launcher's log file ([`launcher_core::logging`]) for
//! `lodestone-launcher-cli logs`. Read-only, and refused like `trace.*` while Debug
//! traces are off. The lines were scrubbed of secrets when they were written.

use gpui::{App, AppContext as _};
use launcher_core::logging::{self, AppKind};
use serde::Deserialize;
use serde_json::{Value, json};

use launcher_control::codes;

use super::{Call, Reply, api_error, ready};

/// The most lines `logs.tail` returns.
pub const MAX_LINES: usize = 2000;

/// `logs.path`: where the logs are, the level, and the files with their sizes.
pub fn path(_: Call, cx: &mut App) -> Reply {
    let Some(dir) = crate::logs::dir() else {
        return ready(Err(api_error(
            codes::NOT_FOUND,
            "The launcher has no logs folder.",
        )));
    };
    let env_override = logging::env_override();
    cx.background_spawn(async move { Ok(describe(&dir, env_override)) })
}

fn describe(dir: &std::path::Path, env_override: bool) -> Value {
    let files: Vec<Value> = logging::files(dir, AppKind::Launcher)
        .into_iter()
        .chain(Some(dir.join(logging::CRASH_FILE)).filter(|p| p.is_file()))
        .filter_map(|path| {
            let meta = std::fs::metadata(&path).ok()?;
            let modified = meta
                .modified()
                .ok()
                .map(|time| chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339());
            Some(json!({
                "name": path.file_name()?.to_string_lossy(),
                "bytes": meta.len(),
                "modified": modified,
            }))
        })
        .collect();
    json!({
        "dir": dir,
        "file": dir.join(AppKind::Launcher.file_name()),
        "crash_file": dir.join(logging::CRASH_FILE),
        "level": logging::level(),
        "env_override": env_override,
        "rotate_at_bytes": logging::MAX_BYTES,
        "keep": logging::KEEP,
        "files": files,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TailParams {
    #[serde(default = "default_lines")]
    lines: usize,
    /// Only records this severe or more: `error`, `warn`, `info` or `debug`.
    #[serde(default)]
    level: Option<String>,
}

fn default_lines() -> usize {
    200
}

/// `logs.tail`: the last lines of the log file (and of `.1` when it is short).
pub fn tail(call: Call, cx: &mut App) -> Reply {
    let params: TailParams = match call.params() {
        Ok(params) => params,
        Err(err) => return ready(Err(err)),
    };
    let level = match parse_level(params.level.as_deref()) {
        Ok(level) => level,
        Err(err) => return ready(Err(err)),
    };
    let lines = params.lines.clamp(1, MAX_LINES);
    let Some(file) = crate::logs::dir().map(|dir| dir.join(AppKind::Launcher.file_name())) else {
        return ready(Err(api_error(
            codes::NOT_FOUND,
            "The launcher has no logs folder.",
        )));
    };
    logging::flush();
    cx.background_spawn(async move {
        let lines = logging::tail(&file, lines, level).map_err(|err| {
            api_error(
                codes::NOT_FOUND,
                format!("Couldn't read {}: {err}", file.display()),
            )
        })?;
        Ok(json!({ "file": file, "lines": lines }))
    })
}

fn parse_level(level: Option<&str>) -> Result<Option<log::Level>, super::ErrorObject> {
    match level {
        None => Ok(None),
        Some(text @ ("error" | "warn" | "info" | "debug")) => Ok(text.parse().ok()),
        Some(other) => Err(api_error(
            codes::INVALID_PARAMS,
            format!("level is error, warn, info or debug, not {other:?}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_are_checked() {
        assert_eq!(parse_level(None).unwrap(), None);
        assert_eq!(parse_level(Some("warn")).unwrap(), Some(log::Level::Warn));
        assert_eq!(
            parse_level(Some("trace")).unwrap_err().code,
            codes::INVALID_PARAMS
        );
    }

    #[test]
    fn the_folder_is_described() {
        let dir = std::env::temp_dir().join(format!("lodestone-logs-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("launcher.log"), "a\n").unwrap();
        std::fs::write(dir.join("launcher.log.1"), "bb\n").unwrap();
        std::fs::write(dir.join(logging::CRASH_FILE), "run x\n").unwrap();
        let described = describe(&dir, false);
        let _ = std::fs::remove_dir_all(&dir);
        let names: Vec<&str> = described["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["launcher.log", "launcher.log.1", "last-crash.txt"]);
        assert_eq!(described["files"][1]["bytes"], 3);
    }
}
