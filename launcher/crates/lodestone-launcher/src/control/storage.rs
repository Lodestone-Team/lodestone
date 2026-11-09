//! Storage: what the data folder takes (Settings > Storage), and removing what can go.

use gpui::{App, AppContext as _};
use launcher_control::codes;
use launcher_core::disk::{self, Cancel, Clean, DiskError};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    Call, Reply, api_error,
    instances::{params, tri},
    ready,
};

/// `storage.show`: the count, as Settings > Storage shows it.
pub fn show(call: Call, cx: &mut App) -> Reply {
    let instances = call.control.instances.read(cx);
    let data = instances.data_dir().clone();
    let list = instances.list().to_vec();
    cx.background_spawn(async move {
        let report = disk::scan(&data, &list, &Cancel::new())
            .map_err(|err| api_error(codes::FAILED, err.to_string()))?;
        let mut value = serde_json::to_value(&report).unwrap_or(Value::Null);
        if let Value::Object(object) = &mut value {
            object.insert(
                "totals".into(),
                json!({
                    "all": report.total(),
                    "instances": report.instances_total(),
                    "game_files": report.game_files.total(),
                    "unused_game_files": report.game_files.unused.total(),
                    "runtimes": report.runtimes_total(),
                    "unused_runtimes": report.unused_runtimes(),
                    "cache": report.cache,
                    "leftovers": report.leftovers.bytes,
                    "logs": report.logs.bytes,
                    "other": report.other,
                }),
            );
            object.insert("data_dir".into(), json!(data.root()));
        }
        Ok(value)
    })
}

#[derive(Deserialize)]
struct CleanParams {
    /// `game-files`, `runtimes`, `cache`, `leftovers`, `old-logs` or `logs`.
    what: String,
}

/// `storage.clean`: removes unused game files or runtimes, the cache, or logs.
pub fn clean(call: Call, cx: &mut App) -> Reply {
    let params: CleanParams = params!(call);
    let Some(what) = Clean::from_name(&params.what) else {
        return ready(Err(api_error(
            codes::INVALID_PARAMS,
            format!(
                "Unknown \"{}\": use game-files, runtimes, cache, leftovers, old-logs or logs.",
                params.what
            ),
        )));
    };
    tri!(call.confirm(match what {
        Clean::UnusedGameFiles => {
            "deletes the versions, libraries and assets no instance uses (they download again when needed)"
        }
        Clean::UnusedRuntimes => {
            "deletes the Java runtimes no instance uses (they download again when needed)"
        }
        Clean::Cache => "deletes the launcher's cache",
        Clean::Leftovers => {
            "deletes the files of modpack downloads and imports that didn't finish"
        }
        Clean::OldLogs => "deletes the logs and crash reports older than 30 days, for good",
        Clean::AllLogs => "deletes every log and crash report, for good",
    }));
    let instances = call.control.instances.read(cx);
    let running: Vec<String> = instances
        .list()
        .iter()
        .filter(|i| instances.is_active(&i.id))
        .map(|i| i.id.clone())
        .collect();
    if !running.is_empty() && !matches!(what, Clean::OldLogs | Clean::AllLogs | Clean::Leftovers) {
        return ready(Err(api_error(
            codes::BUSY,
            "A game is starting or running. Stop it first (`play stop`).",
        )));
    }
    let data = instances.data_dir().clone();
    let list = instances.list().to_vec();
    cx.background_spawn(async move {
        let result = disk::clean(&data, &list, &running, what, &Cancel::new());
        launcher_core::trace::emit("storage.clean", || match &result {
            Ok(cleaned) => json!({
                "what": what.name(),
                "freed_bytes": cleaned.bytes,
                "files": cleaned.files,
            }),
            Err(err) => json!({ "what": what.name(), "error": err.to_string() }),
        });
        match result {
            Ok(cleaned) => Ok(json!({
                "what": what.name(),
                "freed_bytes": cleaned.bytes,
                "files": cleaned.files,
            })),
            Err(DiskError::Cancelled) => Err(api_error(codes::FAILED, "cancelled")),
            Err(err) => Err(api_error(codes::FAILED, err.to_string())),
        }
    })
}
