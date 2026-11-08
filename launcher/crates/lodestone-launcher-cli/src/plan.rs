//! What each command asks the launcher: a method and its parameters.

use std::path::{Path, PathBuf};

use launcher_control::{ErrorObject, codes};
use serde_json::{Map, Value, json};

use crate::cli::*;

/// What a command does.
#[derive(Debug, PartialEq)]
pub enum Plan {
    /// Printed without the launcher.
    Schema,
    /// One request. `long` ones wait for their job unless --no-wait.
    Call {
        method: &'static str,
        params: Value,
        long: bool,
    },
    /// The console's last lines, then (with `follow`) the new ones.
    Logs {
        instance: String,
        lines: usize,
        follow: Option<Follow>,
    },
    /// Events as JSON lines.
    Events { params: Value, count: Option<usize> },
}

#[derive(Debug, PartialEq, Default)]
pub struct Follow {
    pub until: Option<String>,
    pub until_exit: bool,
}

fn usage(message: impl Into<String>) -> ErrorObject {
    ErrorObject::new(codes::USAGE, message)
}

/// A path from this process's working directory, made absolute for the launcher.
pub fn absolute(path: &Path) -> Result<PathBuf, ErrorObject> {
    let expanded = match path.to_str().and_then(|p| p.strip_prefix("~/")) {
        Some(rest) => std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(rest))
            .unwrap_or_else(|| path.to_path_buf()),
        None => path.to_path_buf(),
    };
    let joined = if expanded.is_absolute() {
        expanded
    } else {
        std::env::current_dir()
            .map(|dir| dir.join(expanded))
            .map_err(|err| usage(format!("No working directory: {err}")))?
    };
    // without `.` parts; `..` is left to the file system, which knows about links
    Ok(joined
        .components()
        .filter(|c| !matches!(c, std::path::Component::CurDir))
        .collect())
}

/// The password, from stdin's first line or LODESTONE_CORE_PASSWORD. Never an argument.
pub fn password(
    args: &PasswordArgs,
    read_stdin: &mut dyn FnMut() -> std::io::Result<String>,
) -> Result<String, ErrorObject> {
    if args.password_stdin {
        let line = read_stdin()
            .map_err(|err| usage(format!("Couldn't read the password from stdin: {err}")))?;
        let password = line.trim_end_matches(['\n', '\r']).to_string();
        if password.is_empty() {
            return Err(usage("The password on stdin is empty."));
        }
        return Ok(password);
    }
    match std::env::var("LODESTONE_CORE_PASSWORD") {
        Ok(password) if !password.is_empty() => Ok(password),
        _ => Err(usage(
            "Give the password with --password-stdin (its first line on stdin) or LODESTONE_CORE_PASSWORD. Never on the command line.",
        )),
    }
}

/// Builds params from pairs, leaving out nulls.
fn params(pairs: Vec<(&str, Value)>) -> Value {
    let mut map = Map::new();
    for (key, value) in pairs {
        if !value.is_null() {
            map.insert(key.to_string(), value);
        }
    }
    Value::Object(map)
}

fn call(method: &'static str, pairs: Vec<(&str, Value)>) -> Plan {
    Plan::Call {
        method,
        params: params(pairs),
        long: false,
    }
}

fn long(method: &'static str, pairs: Vec<(&str, Value)>) -> Plan {
    Plan::Call {
        method,
        params: params(pairs),
        long: true,
    }
}

fn path_value(path: &Option<PathBuf>) -> Result<Value, ErrorObject> {
    Ok(match path {
        Some(path) => json!(absolute(path)?),
        None => Value::Null,
    })
}

fn game_settings(s: &GameSettingsArgs) -> Vec<(&'static str, Value)> {
    vec![
        ("min_memory", json!(s.min_memory)),
        ("max_memory", json!(s.max_memory)),
        ("jvm_args", json!(s.jvm_args)),
        ("game_args", json!(s.game_args)),
        ("resolution", json!(s.resolution)),
        ("fullscreen", json!(s.fullscreen)),
        ("env", json!(s.env)),
    ]
}

fn update_mods(update: bool, skip: bool) -> Value {
    match (update, skip) {
        (true, _) => json!(true),
        (_, true) => json!(false),
        _ => Value::Null,
    }
}

/// The plan for `command`. `read_stdin` gives stdin's first line, for passwords.
pub fn plan(
    command: &Command,
    read_stdin: &mut dyn FnMut() -> std::io::Result<String>,
) -> Result<Plan, ErrorObject> {
    Ok(match command {
        Command::Status => call("meta.status", vec![]),
        Command::Ping => call("meta.ping", vec![]),
        Command::Schema => Plan::Schema,
        Command::Methods => call("meta.methods", vec![]),
        Command::Instances(c) => match c {
            Instances::List => call("instances.list", vec![]),
            Instances::Show(i) => call("instances.show", vec![("instance", json!(i.instance))]),
            Instances::Select(i) => call("instances.select", vec![("instance", json!(i.instance))]),
            Instances::Versions {
                snapshots,
                old,
                limit,
            } => call(
                "instances.versions",
                vec![
                    ("snapshots", json!(snapshots)),
                    ("old", json!(old)),
                    ("limit", json!(limit)),
                ],
            ),
            Instances::LoaderVersions {
                loader,
                game_version,
            } => call(
                "instances.loader_versions",
                vec![
                    ("loader", json!(loader)),
                    ("game_version", json!(game_version)),
                ],
            ),
            Instances::Create {
                name,
                version,
                loader,
                loader_version,
                group,
            } => call(
                "instances.create",
                vec![
                    ("name", json!(name)),
                    ("version", json!(version)),
                    ("loader", json!(loader)),
                    ("loader_version", json!(loader_version)),
                    ("group", json!(group)),
                ],
            ),
            Instances::Rename { instance, name } => call(
                "instances.rename",
                vec![("instance", json!(instance)), ("name", json!(name))],
            ),
            Instances::Group {
                instance,
                group,
                none,
            } => {
                if group.is_none() && !none {
                    return Err(usage("Give a group, or --none."));
                }
                call(
                    "instances.set_group",
                    vec![("instance", json!(instance)), ("group", json!(group))],
                )
            }
            Instances::Icon {
                instance,
                image,
                library,
                emoji,
                tile,
                clear,
            } => call(
                "instances.set_icon",
                vec![
                    ("instance", json!(instance)),
                    ("image", path_value(image)?),
                    ("library", json!(library)),
                    ("emoji", json!(emoji)),
                    ("tile", json!(tile)),
                    ("clear", json!(clear)),
                ],
            ),
            Instances::Background {
                instance,
                image,
                clear,
            } => call(
                "instances.set_background",
                vec![
                    ("instance", json!(instance)),
                    ("image", path_value(image)?),
                    ("clear", json!(clear)),
                ],
            ),
            Instances::Duplicate(i) => {
                call("instances.duplicate", vec![("instance", json!(i.instance))])
            }
            Instances::Delete(i) => call("instances.delete", vec![("instance", json!(i.instance))]),
            Instances::Export { instance, path } => long(
                "instances.export",
                vec![
                    ("instance", json!(instance)),
                    ("path", json!(absolute(path)?)),
                ],
            ),
            Instances::Verify(i) => long("instances.verify", vec![("instance", json!(i.instance))]),
        },
        Command::Play(c) => match c {
            Play::Launch {
                instance,
                join,
                no_join,
                update_mods: update,
                skip_update,
                accept_advisory,
            } => long(
                "play.launch",
                vec![
                    ("instance", json!(instance)),
                    ("join", json!(join)),
                    ("no_join", json!(no_join)),
                    ("update_mods", update_mods(*update, *skip_update)),
                    ("accept_advisory", json!(accept_advisory)),
                ],
            ),
            Play::Stop(i) => long("play.stop", vec![("instance", json!(i.instance))]),
            Play::Status { instance } => call("play.status", vec![("instance", json!(instance))]),
            Play::Logs {
                instance,
                lines,
                follow,
                until,
                until_exit,
            } => Plan::Logs {
                instance: instance.clone(),
                lines: *lines,
                follow: follow.then(|| Follow {
                    until: until.clone(),
                    until_exit: *until_exit,
                }),
            },
            Play::ClearLogs(i) => call("play.clear_logs", vec![("instance", json!(i.instance))]),
        },
        Command::Settings(c) => match c {
            Settings::Instance(c) => match c {
                InstanceSettings::Get(i) => call(
                    "settings.instance.get",
                    vec![("instance", json!(i.instance))],
                ),
                InstanceSettings::Set {
                    instance,
                    settings,
                    java,
                } => {
                    let mut pairs = game_settings(settings);
                    pairs.push(("instance", json!(instance)));
                    pairs.push(("java", json!(java)));
                    call("settings.instance.set", pairs)
                }
                InstanceSettings::Reset { instance, keys } => call(
                    "settings.instance.reset",
                    vec![("instance", json!(instance)), ("keys", json!(keys))],
                ),
            },
            Settings::Defaults(c) => match c {
                DefaultSettings::Get => call("settings.defaults.get", vec![]),
                DefaultSettings::Set { settings } => {
                    call("settings.defaults.set", game_settings(settings))
                }
                DefaultSettings::Reset { keys } => {
                    call("settings.defaults.reset", vec![("keys", json!(keys))])
                }
            },
            Settings::General(c) => match c {
                GeneralSettings::Get => call("settings.general.get", vec![]),
                GeneralSettings::Set { auto_groups } => call(
                    "settings.general.set",
                    vec![("auto_groups", json!(auto_groups))],
                ),
            },
            Settings::Developer(c) => match c {
                DeveloperSettings::Get => call("settings.developer.get", vec![]),
                DeveloperSettings::Set {
                    sign_in_as_minecraft_launcher,
                    sign_out_accounts,
                } => call(
                    "settings.developer.set",
                    vec![
                        (
                            "sign_in_as_minecraft_launcher",
                            json!(sign_in_as_minecraft_launcher),
                        ),
                        ("sign_out_accounts", json!(sign_out_accounts)),
                    ],
                ),
            },
        },
        Command::Shared(c) => match c {
            Shared::Get => call("shared.get", vec![]),
            Shared::Set { key, value } => call(
                "shared.set_value",
                vec![("key", json!(key)), ("value", json!(value))],
            ),
            Shared::Category { category, on, .. } => call(
                "shared.set_category",
                vec![("category", json!(category)), ("shared", json!(on))],
            ),
            Shared::SyncFrom { instance, .. } => {
                call("shared.sync_from", vec![("instance", json!(instance))])
            }
            Shared::CopyFrom(i) => call("shared.copy_from", vec![("instance", json!(i.instance))]),
            Shared::Clear => call("shared.clear", vec![]),
            Shared::Instance(c) => match c {
                SharedInstance::Get(i) => {
                    call("shared.instance.get", vec![("instance", json!(i.instance))])
                }
                SharedInstance::Set {
                    instance,
                    enabled,
                    override_keys,
                    share,
                    keep_changes,
                    dismiss_changes,
                    values,
                } => {
                    let mut set = Map::new();
                    for pair in values {
                        let (key, value) = pair.split_once('=').ok_or_else(|| {
                            usage(format!("--value takes KEY=VALUE, not \"{pair}\"."))
                        })?;
                        set.insert(key.to_string(), json!(value));
                    }
                    call(
                        "shared.instance.set",
                        vec![
                            ("instance", json!(instance)),
                            ("enabled", json!(enabled)),
                            ("override", json!(override_keys)),
                            ("share", json!(share)),
                            ("keep_changes", json!(keep_changes)),
                            ("dismiss_changes", json!(dismiss_changes)),
                            ("set", Value::Object(set)),
                        ],
                    )
                }
            },
        },
        Command::Content(c) => match c {
            Content::List { instance, kind } => call(
                "content.list",
                vec![("instance", json!(instance)), ("kind", json!(kind))],
            ),
            Content::Search {
                query,
                instance,
                kind,
                all,
                sort,
                limit,
                offset,
            } => call(
                "content.search",
                vec![
                    ("query", json!(query)),
                    ("instance", json!(instance)),
                    ("kind", json!(kind)),
                    ("all", json!(all)),
                    ("sort", json!(sort)),
                    ("limit", json!(limit)),
                    ("offset", json!(offset)),
                ],
            ),
            Content::Versions {
                project,
                instance,
                limit,
            } => call(
                "content.versions",
                vec![
                    ("project", json!(project)),
                    ("instance", json!(instance)),
                    ("limit", json!(limit)),
                ],
            ),
            Content::Install {
                instance,
                project,
                version,
                kind,
                no_deps,
                force,
            } => long(
                "content.install",
                vec![
                    ("instance", json!(instance)),
                    ("project", json!(project)),
                    ("version", json!(version)),
                    ("kind", json!(kind)),
                    ("no_deps", json!(no_deps)),
                    ("force", json!(force)),
                ],
            ),
            Content::Add {
                instance,
                path,
                kind,
            } => call(
                "content.add_file",
                vec![
                    ("instance", json!(instance)),
                    ("path", json!(absolute(path)?)),
                    ("kind", json!(kind)),
                ],
            ),
            Content::Remove(a) => call("content.remove", items(a)),
            Content::Enable(a) => call("content.enable", items(a)),
            Content::Disable(a) => call("content.disable", items(a)),
            Content::Updates { instance, kind } => call(
                "content.updates",
                vec![("instance", json!(instance)), ("kind", json!(kind))],
            ),
            Content::Update {
                instance,
                items,
                kind,
                ..
            } => long(
                "content.update",
                vec![
                    ("instance", json!(instance)),
                    ("items", json!(items)),
                    ("kind", json!(kind)),
                ],
            ),
            Content::SwitchLoader {
                instance,
                loader,
                loader_version,
                disable_mismatched,
            } => call(
                "content.switch_loader",
                vec![
                    ("instance", json!(instance)),
                    ("loader", json!(loader)),
                    ("loader_version", json!(loader_version)),
                    ("disable_mismatched", json!(disable_mismatched)),
                ],
            ),
        },
        Command::Modpacks(c) => match c {
            Modpacks::Search {
                query,
                sort,
                limit,
                offset,
            } => call(
                "modpacks.search",
                vec![
                    ("query", json!(query)),
                    ("sort", json!(sort)),
                    ("limit", json!(limit)),
                    ("offset", json!(offset)),
                ],
            ),
            Modpacks::Install {
                project,
                file,
                version,
                name,
            } => long(
                "modpacks.install",
                vec![
                    ("project", json!(project)),
                    ("file", path_value(file)?),
                    ("version", json!(version)),
                    ("name", json!(name)),
                ],
            ),
            Modpacks::Check(i) => call("modpacks.check", vec![("instance", json!(i.instance))]),
            Modpacks::Update {
                instance,
                version,
                no_backup_worlds,
            } => long(
                "modpacks.update",
                vec![
                    ("instance", json!(instance)),
                    ("version", json!(version)),
                    ("backup_worlds", json!(!no_backup_worlds)),
                ],
            ),
        },
        Command::Worlds(c) => match c {
            Worlds::List(i) => call("worlds.list", vec![("instance", json!(i.instance))]),
            Worlds::Backups(w) => call("worlds.backups", world(w)),
            Worlds::Backup(w) => long("worlds.backup", world(w)),
            Worlds::Restore {
                world: w,
                backup,
                replace,
            } => {
                let mut pairs = world(w);
                pairs.push(("backup", json!(backup)));
                pairs.push(("mode", json!(if *replace { "replace" } else { "copy" })));
                long("worlds.restore", pairs)
            }
            Worlds::Delete(w) => call("worlds.delete", world(w)),
            Worlds::DeleteBackup { world: w, backup } => {
                let mut pairs = world(w);
                pairs.push(("backup", json!(backup)));
                call("worlds.delete_backup", pairs)
            }
            Worlds::Download {
                instance,
                server,
                core,
                username,
                password: p,
            } => long(
                "worlds.download",
                vec![
                    ("instance", json!(instance)),
                    ("server", json!(server)),
                    ("core", json!(core)),
                    ("username", json!(username)),
                    ("password", json!(password(p, read_stdin)?)),
                ],
            ),
        },
        Command::Accounts(c) => match c {
            Accounts::List => call("accounts.list", vec![]),
            Accounts::Switch { account } => {
                call("accounts.switch", vec![("account", json!(account))])
            }
            Accounts::AddOffline { username } => {
                call("accounts.add_offline", vec![("username", json!(username))])
            }
            Accounts::Remove { account } => {
                call("accounts.remove", vec![("account", json!(account))])
            }
            Accounts::SignIn => long("accounts.sign_in", vec![]),
        },
        Command::Import(c) => match c {
            Import::Detect { folder } => {
                call("import.detect", vec![("folder", path_value(folder)?)])
            }
            Import::Run {
                keys,
                folder,
                without_worlds,
            } => long(
                "import.run",
                vec![
                    ("instances", json!(keys)),
                    ("folder", path_value(folder)?),
                    ("without_worlds", json!(without_worlds)),
                ],
            ),
        },
        Command::Cores(c) => match c {
            Cores::List { cached } => call("cores.list", vec![("cached", json!(cached))]),
            Cores::Add {
                address,
                username,
                password: p,
                expiry,
                console_read,
                console_write,
                fingerprint,
            } => {
                let password = match username {
                    Some(_) => json!(password(p, read_stdin)?),
                    None if p.password_stdin => {
                        return Err(usage("--password-stdin goes with --username."));
                    }
                    None => Value::Null,
                };
                call(
                    "cores.add",
                    vec![
                        ("address", json!(address)),
                        ("username", json!(username)),
                        ("password", password),
                        ("expiry", json!(expiry)),
                        ("console_read", json!(console_read)),
                        ("console_write", json!(console_write)),
                        ("fingerprint", json!(fingerprint)),
                    ],
                )
            }
            Cores::Remove { core } => call("cores.remove", vec![("core", json!(core))]),
            Cores::Trust { core } => call("cores.trust", vec![("core", json!(core))]),
            Cores::Untrust { core } => call("cores.untrust", vec![("core", json!(core))]),
            Cores::TrustCertificate { core, fingerprint } => call(
                "cores.trust_certificate",
                vec![("core", json!(core)), ("fingerprint", json!(fingerprint))],
            ),
            Cores::SignOut { core } => call("cores.sign_out", vec![("core", json!(core))]),
        },
        Command::Servers(c) => match c {
            Servers::List { core, cached } => call(
                "servers.list",
                vec![("core", json!(core)), ("cached", json!(cached))],
            ),
            Servers::Start(s) => call("servers.start", server(s)),
            Servers::Stop(s) => call("servers.stop", server(s)),
            Servers::Join {
                server: s,
                update_mods: update,
                skip_update,
                accept_advisory,
            } => {
                let mut pairs = server(s);
                pairs.push(("update_mods", update_mods(*update, *skip_update)));
                pairs.push(("accept_advisory", json!(accept_advisory)));
                long("servers.join", pairs)
            }
            Servers::CreateInstance {
                server: s,
                accept_advisory,
            } => {
                let mut pairs = server(s);
                pairs.push(("accept_advisory", json!(accept_advisory)));
                long("servers.create_instance", pairs)
            }
            Servers::Link {
                instance,
                server: s,
            } => {
                let mut pairs = server(s);
                pairs.push(("instance", json!(instance)));
                call("servers.link", pairs)
            }
            Servers::Unlink(i) => call("servers.unlink", vec![("instance", json!(i.instance))]),
            Servers::Host {
                instance,
                core,
                username,
                password: p,
                name,
                max_ram,
                accept_eula,
            } => {
                // the EULA is checked before the password is read
                let password = if *accept_eula {
                    json!(password(p, read_stdin)?)
                } else {
                    json!("")
                };
                long(
                    "servers.host",
                    vec![
                        ("instance", json!(instance)),
                        ("core", json!(core)),
                        ("username", json!(username)),
                        ("password", password),
                        ("name", json!(name)),
                        ("max_ram", json!(max_ram)),
                        ("accept_eula", json!(accept_eula)),
                    ],
                )
            }
            Servers::ResourcePack {
                server: s,
                instance,
            } => {
                let mut pairs = server(s);
                pairs.push(("instance", json!(instance)));
                call("servers.resource_pack", pairs)
            }
        },
        Command::Prompts(c) => match c {
            Prompts::List => call("prompts.list", vec![]),
            Prompts::Answer {
                prompt,
                answer,
                fingerprint,
            } => call(
                "prompts.answer",
                vec![
                    ("prompt", json!(prompt)),
                    ("answer", json!(answer)),
                    ("fingerprint", json!(fingerprint)),
                ],
            ),
        },
        Command::Jobs(c) => match c {
            Jobs::List { running } => call("jobs.list", vec![("running", json!(running))]),
            Jobs::Get { job } => call("jobs.get", vec![("job", json!(job))]),
            Jobs::Wait { job } => Plan::Call {
                method: "jobs.wait",
                params: json!({ "job": job }),
                long: true,
            },
            Jobs::Cancel { job } => call("jobs.cancel", vec![("job", json!(job))]),
        },
        Command::Events(e) => Plan::Events {
            params: params(vec![
                ("topics", json!(e.topics)),
                ("instance", json!(e.instance)),
                ("backlog", json!(e.backlog)),
            ]),
            count: e.count,
        },
    })
}

fn items(a: &ItemsArgs) -> Vec<(&'static str, Value)> {
    vec![
        ("instance", json!(a.instance)),
        ("items", json!(a.items)),
        ("kind", json!(a.kind)),
    ]
}

fn world(w: &WorldArgs) -> Vec<(&'static str, Value)> {
    vec![("instance", json!(w.instance)), ("world", json!(w.world))]
}

fn server(s: &ServerArg) -> Vec<(&'static str, Value)> {
    vec![("server", json!(s.server)), ("core", json!(s.core))]
}

#[cfg(test)]
mod tests {
    use clap::Parser as _;

    use super::*;
    use crate::cli::Cli;

    fn plan_of(args: &[&str]) -> Result<Plan, ErrorObject> {
        let mut argv = vec!["lodestone-launcher-cli"];
        argv.extend_from_slice(args);
        let cli = Cli::try_parse_from(argv).expect("the command line parses");
        plan(&cli.command, &mut || Ok("hunter2\n".to_string()))
    }

    fn call_of(args: &[&str]) -> (&'static str, Value, bool) {
        match plan_of(args).unwrap() {
            Plan::Call {
                method,
                params,
                long,
            } => (method, params, long),
            other => panic!("not a call: {other:?}"),
        }
    }

    #[test]
    fn commands_become_methods_and_params() {
        assert_eq!(
            call_of(&[
                "instances",
                "create",
                "--version",
                "1.21.1",
                "--loader",
                "fabric",
                "--name",
                "Survival"
            ]),
            (
                "instances.create",
                json!({ "name": "Survival", "version": "1.21.1", "loader": "fabric" }),
                false
            )
        );
        assert_eq!(
            call_of(&["content", "install", "Survival", "sodium", "--no-deps"]),
            (
                "content.install",
                json!({ "instance": "Survival", "project": "sodium", "no_deps": true, "force": false }),
                true
            )
        );
        let (method, params, _) = call_of(&["instances", "group", "Survival", "--none"]);
        assert_eq!(method, "instances.set_group");
        assert_eq!(params, json!({ "instance": "Survival" }));
        let (_, params, _) = call_of(&["play", "launch", "Smp", "--skip-update"]);
        assert_eq!(params["update_mods"], json!(false));
        let (_, params, _) = call_of(&["worlds", "restore", "S", "World", "--replace"]);
        assert_eq!(
            params,
            json!({ "instance": "S", "world": "World", "backup": "latest", "mode": "replace" })
        );
        let (_, params, _) = call_of(&[
            "shared",
            "instance",
            "set",
            "S",
            "--override",
            "guiScale",
            "--value",
            "guiScale=2",
        ]);
        assert_eq!(params["override"], json!(["guiScale"]));
        assert_eq!(params["set"], json!({ "guiScale": "2" }));
        assert!(plan_of(&["instances", "group", "S"]).is_err());
    }

    #[test]
    fn passwords_come_from_stdin_only() {
        let (_, params, _) = call_of(&[
            "cores",
            "add",
            "10.0.0.5",
            "--username",
            "alice",
            "--password-stdin",
        ]);
        assert_eq!(params["password"], "hunter2");
        // there is no --password flag
        assert!(
            Cli::try_parse_from([
                "lodestone-launcher-cli",
                "cores",
                "add",
                "x",
                "--password",
                "p"
            ])
            .is_err()
        );
        // without the EULA the password isn't even read
        let (_, params, _) = call_of(&[
            "servers",
            "host",
            "S",
            "--core",
            "c",
            "--username",
            "a",
            "--password-stdin",
        ]);
        assert_eq!(params["password"], "");
        assert_eq!(params["accept_eula"], false);
    }

    #[test]
    fn paths_are_made_absolute() {
        let (_, params, _) = call_of(&["instances", "export", "S", "out.mrpack"]);
        let path = PathBuf::from(params["path"].as_str().unwrap());
        assert!(path.is_absolute());
        assert!(path.ends_with("out.mrpack"));
    }

    #[test]
    fn follow_and_events_are_streams() {
        assert_eq!(
            plan_of(&["play", "logs", "S", "--follow", "--until", "Done"]).unwrap(),
            Plan::Logs {
                instance: "S".into(),
                lines: 100,
                follow: Some(Follow {
                    until: Some("Done".into()),
                    until_exit: false
                }),
            }
        );
        assert!(matches!(
            plan_of(&["events", "--topics", "state,notices", "--count", "3"]).unwrap(),
            Plan::Events { count: Some(3), .. }
        ));
        assert!(
            Cli::try_parse_from([
                "lodestone-launcher-cli",
                "play",
                "logs",
                "S",
                "--until",
                "x"
            ])
            .is_err()
        );
    }

    #[test]
    fn global_flags_go_anywhere() {
        let cli = Cli::try_parse_from([
            "lodestone-launcher-cli",
            "instances",
            "list",
            "--json",
            "--yes",
            "--timeout",
            "5",
        ])
        .unwrap();
        assert!(cli.global.json && cli.global.yes);
        assert_eq!(cli.global.timeout, Some(5));
        assert!(
            Cli::try_parse_from(["lodestone-launcher-cli", "--json", "--human", "status"]).is_err()
        );
        assert!(
            Cli::try_parse_from(["lodestone-launcher-cli", "--wait", "--no-wait", "status"])
                .is_err()
        );
        let json = crate::run::mode(&cli.global, true);
        assert_eq!(json, crate::output::Mode::Json);
        let quiet = Cli::try_parse_from(["lodestone-launcher-cli", "status"]).unwrap();
        assert_eq!(
            crate::run::mode(&quiet.global, false),
            crate::output::Mode::Json
        );
        assert_eq!(
            crate::run::mode(&quiet.global, true),
            crate::output::Mode::Human
        );
    }
}
