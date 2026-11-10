//! `schema`: every command with its arguments, flags, method and examples, as JSON,
//! read from the command line's own definition so it is never out of date.

use clap::{Arg, ArgAction, Command, CommandFactory};
use launcher_control::{PROTOCOL_VERSION, codes};
use serde_json::{Value, json};

use crate::cli::Cli;

/// Each command's method, whether it waits for a job (`--wait`/`--no-wait`), and
/// whether it needs `--yes`.
pub const COMMANDS: &[(&str, &str, bool, bool)] = &[
    ("status", "meta.status", false, false),
    ("ping", "meta.ping", false, false),
    ("schema", "", false, false),
    ("methods", "meta.methods", false, false),
    ("instances list", "instances.list", false, false),
    ("instances show", "instances.show", false, false),
    ("instances select", "instances.select", false, false),
    ("instances versions", "instances.versions", false, false),
    (
        "instances loader-versions",
        "instances.loader_versions",
        false,
        false,
    ),
    ("instances create", "instances.create", false, false),
    ("instances rename", "instances.rename", false, false),
    ("instances group", "instances.set_group", false, false),
    (
        "instances prerelease-updates",
        "instances.set_prerelease_updates",
        false,
        false,
    ),
    ("instances icon", "instances.set_icon", false, false),
    (
        "instances background",
        "instances.set_background",
        false,
        false,
    ),
    ("instances duplicate", "instances.duplicate", false, false),
    ("instances delete", "instances.delete", false, true),
    ("instances export", "instances.export", true, false),
    ("instances verify", "instances.verify", true, false),
    ("instances share-code", "instances.share_code", false, false),
    (
        "instances import-code",
        "instances.import_code",
        true,
        false,
    ),
    ("play launch", "play.launch", true, false),
    ("play stop", "play.stop", true, false),
    ("play status", "play.status", false, false),
    ("play logs", "play.logs", false, false),
    ("play clear-logs", "play.clear_logs", false, true),
    (
        "settings instance get",
        "settings.instance.get",
        false,
        false,
    ),
    (
        "settings instance set",
        "settings.instance.set",
        false,
        false,
    ),
    (
        "settings instance reset",
        "settings.instance.reset",
        false,
        false,
    ),
    (
        "settings defaults get",
        "settings.defaults.get",
        false,
        false,
    ),
    (
        "settings defaults set",
        "settings.defaults.set",
        false,
        false,
    ),
    (
        "settings defaults reset",
        "settings.defaults.reset",
        false,
        false,
    ),
    ("settings general get", "settings.general.get", false, false),
    ("settings general set", "settings.general.set", false, false),
    (
        "settings developer get",
        "settings.developer.get",
        false,
        false,
    ),
    (
        "settings developer set",
        "settings.developer.set",
        false,
        true,
    ),
    ("shared get", "shared.get", false, false),
    ("shared set", "shared.set_value", false, false),
    ("shared category", "shared.set_category", false, false),
    ("shared sync-from", "shared.sync_from", false, false),
    ("shared copy-from", "shared.copy_from", false, false),
    ("shared clear", "shared.clear", false, true),
    ("shared instance get", "shared.instance.get", false, false),
    ("shared instance set", "shared.instance.set", false, false),
    ("content list", "content.list", false, false),
    ("content search", "content.search", false, false),
    ("content versions", "content.versions", false, false),
    ("content install", "content.install", true, false),
    ("content add", "content.add_file", false, false),
    ("content remove", "content.remove", false, true),
    ("content enable", "content.enable", false, false),
    ("content disable", "content.disable", false, false),
    ("content updates", "content.updates", false, false),
    ("content update", "content.update", true, false),
    (
        "content switch-loader",
        "content.switch_loader",
        false,
        true,
    ),
    ("modpacks search", "modpacks.search", false, false),
    ("modpacks install", "modpacks.install", true, false),
    ("modpacks check", "modpacks.check", false, false),
    ("modpacks update", "modpacks.update", true, true),
    ("worlds list", "worlds.list", false, false),
    ("worlds backups", "worlds.backups", false, false),
    ("worlds backup", "worlds.backup", true, false),
    ("worlds restore", "worlds.restore", true, true),
    ("worlds delete", "worlds.delete", false, true),
    ("worlds delete-backup", "worlds.delete_backup", false, true),
    ("accounts list", "accounts.list", false, false),
    ("accounts switch", "accounts.switch", false, false),
    ("accounts add-offline", "accounts.add_offline", false, false),
    ("accounts remove", "accounts.remove", false, true),
    ("accounts sign-in", "accounts.sign_in", true, false),
    ("import detect", "import.detect", false, false),
    ("import run", "import.run", true, false),
    ("cores list", "cores.list", false, false),
    ("cores add", "cores.add", false, false),
    ("cores remove", "cores.remove", false, true),
    ("cores trust", "cores.trust", false, true),
    ("cores untrust", "cores.untrust", false, false),
    (
        "cores trust-certificate",
        "cores.trust_certificate",
        false,
        false,
    ),
    ("cores sign-out", "cores.sign_out", false, false),
    ("cores claim-local", "cores.claim_local", false, false),
    ("servers list", "servers.list", false, false),
    ("servers start", "servers.start", false, false),
    ("servers stop", "servers.stop", false, false),
    ("servers join", "servers.join", true, false),
    (
        "servers create-instance",
        "servers.create_instance",
        true,
        false,
    ),
    ("servers link", "servers.link", false, false),
    ("servers unlink", "servers.unlink", false, false),
    ("servers host", "servers.host", true, false),
    ("prompts list", "prompts.list", false, false),
    ("friends list", "friends.list", false, false),
    ("friends join", "friends.join", true, false),
    ("friends remove", "friends.remove", false, true),
    ("friend-list me", "friend_list.me", false, false),
    ("friend-list set-name", "friend_list.set_name", false, false),
    (
        "friend-list make-link",
        "friend_list.create_link",
        false,
        false,
    ),
    ("friend-list links", "friend_list.links", false, false),
    (
        "friend-list cancel-link",
        "friend_list.cancel_link",
        false,
        false,
    ),
    (
        "friend-list preview",
        "friend_list.preview_link",
        false,
        false,
    ),
    ("friend-list answer", "friend_list.answer_link", false, true),
    ("friend-list replies", "friend_list.replies", false, false),
    ("friend-list accept", "friend_list.accept", false, true),
    ("friend-list list", "friend_list.list", false, false),
    ("friend-list refresh", "friend_list.refresh", false, false),
    ("friend-list verify", "friend_list.verify", false, false),
    ("friend-list nickname", "friend_list.nickname", false, false),
    ("friend-list hide", "friend_list.hide", false, false),
    ("friend-list remove", "friend_list.remove", false, true),
    ("friend-list servers", "friend_list.servers", false, false),
    ("friend-list retire", "friend_list.retire", false, true),
    ("prompts answer", "prompts.answer", false, false),
    ("skins list", "skins.list", false, false),
    ("skins use", "skins.use", false, false),
    ("skins import", "skins.import", false, false),
    ("skins export", "skins.export", false, false),
    ("skins check", "skins.check", false, false),
    ("skins fix", "skins.fix", false, false),
    ("snapshots list", "snapshots.list", false, false),
    ("snapshots create", "snapshots.create", true, false),
    ("snapshots revert", "snapshots.revert", true, true),
    ("snapshots export", "snapshots.export", true, false),
    ("snapshots import", "snapshots.import", true, false),
    ("snapshots delete", "snapshots.delete", false, true),
    ("storage show", "storage.show", false, false),
    ("storage clean", "storage.clean", false, true),
    ("jobs list", "jobs.list", false, false),
    ("jobs get", "jobs.get", false, false),
    ("jobs wait", "jobs.wait", true, false),
    ("jobs cancel", "jobs.cancel", false, false),
    ("events", "events.subscribe", false, false),
    ("trace topics", "trace.topics", false, false),
    ("trace watch", "trace.subscribe", false, false),
    ("trace snapshot", "trace.snapshot", false, false),
    ("perf show", "perf.show", false, false),
    ("perf watch", "perf.watch", false, false),
];

fn arg_json(arg: &Arg) -> Value {
    let takes_value = matches!(arg.get_action(), ArgAction::Set | ArgAction::Append);
    json!({
        "name": arg.get_id().as_str(),
        "long": arg.get_long().map(|l| format!("--{l}")),
        "short": arg.get_short().map(|s| format!("-{s}")),
        "positional": arg.is_positional(),
        "required": arg.is_required_set(),
        "takes_value": takes_value,
        "repeatable": matches!(arg.get_action(), ArgAction::Append) || arg.get_num_args().is_some_and(|n| n.max_values() > 1),
        "value_name": arg.get_value_names().and_then(|v| v.first()).map(|v| v.as_str()),
        "default": arg.get_default_values().first().map(|v| v.to_string_lossy().into_owned()),
        "help": arg.get_help().map(|h| h.to_string()),
    })
}

/// The leaf commands, with their path from the top.
pub fn leaves(command: &Command, prefix: &str, out: &mut Vec<(String, Command)>) {
    for sub in command.get_subcommands() {
        if sub.get_name() == "help" {
            continue;
        }
        let path = if prefix.is_empty() {
            sub.get_name().to_string()
        } else {
            format!("{prefix} {}", sub.get_name())
        };
        if sub.has_subcommands() {
            leaves(sub, &path, out);
        } else {
            out.push((path, sub.clone()));
        }
    }
}

fn examples(command: &Command) -> Vec<String> {
    command
        .get_after_help()
        .map(|h| h.to_string())
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("lodestone-launcher-cli") || l.starts_with("printf"))
        .map(str::to_string)
        .collect()
}

pub fn schema() -> Value {
    let root = Cli::command();
    let mut list = Vec::new();
    leaves(&root, "", &mut list);
    let global: Vec<Value> = root
        .get_arguments()
        .filter(|a| a.is_global_set())
        .map(arg_json)
        .collect();
    let commands: Vec<Value> = list
        .iter()
        .map(|(path, command)| {
            let entry = COMMANDS.iter().find(|(p, ..)| p == path);
            json!({
                "name": path,
                "about": command.get_about().map(|a| a.to_string()),
                "method": entry.map(|e| e.1).filter(|m| !m.is_empty()),
                "long_running": entry.is_some_and(|e| e.2),
                "needs_yes": entry.is_some_and(|e| e.3),
                "args": command
                    .get_arguments()
                    .filter(|a| !a.is_global_set() && a.get_id() != "help" && a.get_id() != "version")
                    .map(arg_json)
                    .collect::<Vec<_>>(),
                "examples": examples(command),
            })
        })
        .collect();
    json!({
        "name": "lodestone-launcher-cli",
        "version": env!("CARGO_PKG_VERSION"),
        "protocol": PROTOCOL_VERSION,
        "platform_supported": cfg!(unix),
        "global_args": global,
        "commands": commands,
        "exit_codes": exit_codes(),
        "error_codes": codes::ALL.iter().map(|(code, exit, meaning)| json!({ "code": code, "exit": exit, "meaning": meaning })).collect::<Vec<_>>(),
        "output": {
            "json": "One JSON document on stdout (the default when stdout isn't a terminal, or with --json).",
            "progress": "Long commands print progress to stderr, one JSON object per line: {\"progress\": {...}}.",
            "error": "Errors go to stderr as {\"error\": {\"code\", \"message\", \"data\"}}.",
            "streams": "play logs --follow, events and trace watch print one JSON object per line on stdout (trace watch prints readable lines in a terminal unless --json).",
        },
    })
}

fn exit_codes() -> Vec<Value> {
    let mut by_exit: std::collections::BTreeMap<i32, Vec<&str>> = Default::default();
    for (code, exit, _) in codes::ALL {
        by_exit.entry(*exit).or_default().push(code);
    }
    std::iter::once(json!({ "code": 0, "error_codes": [], "meaning": "Success." }))
        .chain(
            by_exit
                .into_iter()
                .map(|(exit, codes)| json!({ "code": exit, "error_codes": codes })),
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_line_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn every_command_has_a_method_and_examples() {
        let mut list = Vec::new();
        leaves(&Cli::command(), "", &mut list);
        for (path, command) in &list {
            assert!(
                COMMANDS.iter().any(|(p, ..)| p == path),
                "`{path}` is missing from COMMANDS"
            );
            assert!(!examples(command).is_empty(), "`{path}` has no examples");
            assert!(command.get_about().is_some(), "`{path}` has no help");
        }
        for (path, ..) in COMMANDS {
            assert!(
                list.iter().any(|(p, _)| p == path),
                "`{path}` isn't a command"
            );
        }
    }

    #[test]
    fn the_schema_is_json_with_every_command() {
        let schema = schema();
        let commands = schema["commands"].as_array().unwrap();
        assert_eq!(commands.len(), COMMANDS.len());
        let delete = commands
            .iter()
            .find(|c| c["name"] == "instances delete")
            .unwrap();
        assert_eq!(delete["method"], "instances.delete");
        assert_eq!(delete["needs_yes"], true);
        assert!(
            schema["global_args"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["long"] == "--yes")
        );
    }
}
