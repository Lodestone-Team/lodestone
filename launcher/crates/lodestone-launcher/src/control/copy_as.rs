//! An instance's Minecraft version and loader: fixed once it is made. The loader's
//! version can change (the Settings tab's Loader version), and "Copy with another
//! version or loader" makes a new instance for another version or loader, as the
//! window's copy dialog does, as a job like a modpack import.

use std::time::Duration;

use gpui::{App, AppContext as _};
use launcher_control::codes;
use launcher_core::{
    content::{
        Target,
        copy_as::{self, CopyAsOptions, CopyAsRequest, CopyPlan, WorldsRule},
    },
    game::loaders::{LoaderChoice, LoaderVersion},
    instances::{Instance, Loader},
};
use lodestone_minecraft::versions::mojang::MojangVersionType;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    Call, Reply, api_error,
    instances::{failed, instance_json, params, parse_loader, tri},
    jobs::{Cancel, JobSpec, Progress},
    poll_until, ready,
};

/// The loader version `wanted` (by id or as shown), else the one the picker starts at.
fn pick_loader_version(list: &[LoaderVersion], wanted: Option<&str>) -> Option<String> {
    match wanted {
        Some(wanted) => list
            .iter()
            .find(|v| v.id == wanted || v.display == wanted)
            .map(|v| v.id.clone()),
        None => list
            .iter()
            .find(|v| v.recommended)
            .or_else(|| list.first())
            .map(|v| v.id.clone()),
    }
}

/// Why an instance's loader isn't changed in place, for errors.
fn fixed_loader(instance: &Instance) -> String {
    format!(
        "{}'s Minecraft version and loader stay as it was made ({} {}). `instances copy-as` makes a copy with another version or loader.",
        instance.name,
        instance.game_version,
        instance.loader_label()
    )
}

#[derive(Deserialize)]
struct SetLoaderVersionParams {
    /// A version of the instance's own loader (its id, or as shown); the recommended
    /// one without it.
    #[serde(default)]
    loader_version: Option<String>,
    /// Refused unless it is the instance's own loader: loaders don't change.
    #[serde(default)]
    loader: Option<String>,
}

/// Changes the version of an instance's mod loader, as the Settings tab's Loader version
/// does. The loader itself never changes.
pub fn set_loader_version(call: Call, cx: &mut App) -> Reply {
    let params: SetLoaderVersionParams = params!(call);
    let control = call.control.clone();
    let instance = tri!(control.instance_param(&call, cx));
    tri!(control.idle(&instance, cx));
    // vanilla has no loader: refused as installing a mod into it is
    let Some(kind) = instance.loader.kind() else {
        return ready(Err(api_error(
            codes::CONFLICT,
            format!(
                "{} is vanilla: it has no mod loader. {}",
                instance.name,
                fixed_loader(&instance)
            ),
        )));
    };
    if let Some(loader) = &params.loader
        && tri!(parse_loader(loader)) != Some(kind)
    {
        return ready(Err(api_error(
            codes::INVALID_PARAMS,
            fixed_loader(&instance),
        )));
    }
    let installer = control.instances.read(cx).installer();
    cx.spawn(async move |cx| {
        let game_version = instance.game_version.clone();
        let list = cx
            .background_spawn(async move { installer.loader_versions(kind, &game_version).await })
            .await
            .map_err(|err| api_error(codes::UNAVAILABLE, err.to_string()))?;
        let version = pick_loader_version(&list, params.loader_version.as_deref()).ok_or_else(|| {
            api_error(
                codes::NOT_FOUND,
                format!("{} has no such build for Minecraft {}.", kind.label(), instance.game_version),
            )
        })?;
        // the record as it is now: a session may have ended meanwhile
        let mut changed = cx.update(|cx| control.instance(&instance.id, cx))?;
        if (changed.loader, changed.game_version.as_str()) != (instance.loader, instance.game_version.as_str()) {
            return Err(api_error(
                codes::CONFLICT,
                "The instance's loader or Minecraft version changed meanwhile (a modpack update). Try again.",
            ));
        }
        changed.loader_version = Some(version);
        let saved = control.instances.update(cx, |instances, cx| {
            instances.undoable(&changed.id, cx, |instances, cx| instances.save(&changed, cx))
        });
        failed(saved)?;
        Ok(json!({
            "instance": changed.id,
            "loader": changed.loader,
            "loader_version": changed.loader_version,
            "message": "The loader installs the next time the game starts.",
        }))
    })
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
struct CopyAsParams {
    /// The copy's name; "<name> (<version> <loader>)" without it.
    #[serde(default)]
    name: Option<String>,
    /// A Minecraft version, or "latest" (the newest release); the instance's own
    /// without it.
    #[serde(default)]
    version: Option<String>,
    /// vanilla, fabric, quilt, forge or neoforge; the instance's own without it.
    #[serde(default)]
    loader: Option<String>,
    /// The loader's version; the recommended one without it (or the instance's own,
    /// for the same loader and Minecraft version).
    #[serde(default)]
    loader_version: Option<String>,
    /// Copy the worlds: only to the same or a newer Minecraft version.
    #[serde(default)]
    worlds: bool,
    /// Copy the resource packs, shader packs and options.txt.
    #[serde(default = "yes")]
    packs: bool,
    /// Install each Modrinth mod's build for the new version and loader.
    #[serde(default = "yes")]
    mods: bool,
}

impl CopyAsParams {
    fn options(&self) -> CopyAsOptions {
        CopyAsOptions {
            worlds: self.worlds,
            packs: self.packs,
            mods: self.mods,
        }
    }
}

fn rule_name(rule: WorldsRule) -> &'static str {
    match rule {
        WorldsRule::Same => "same",
        WorldsRule::Newer => "newer",
        WorldsRule::Older => "older",
        WorldsRule::Unknown => "unknown",
    }
}

/// What a copy takes, as the API shows it before it starts.
pub fn plan_json(
    source: &Instance,
    name: &str,
    loader: Option<&LoaderChoice>,
    plan: &CopyPlan,
) -> Value {
    json!({
        "from": { "id": source.id, "name": source.name, "game_version": source.game_version, "loader": source.loader_label() },
        "to": {
            "name": name,
            "game_version": plan.game_version,
            "loader": plan.loader,
            "loader_version": loader.map(|l| &l.version),
        },
        "worlds_rule": rule_name(plan.worlds_rule),
        "worlds": plan.worlds,
        "resource_packs": plan.resource_packs,
        "shader_packs": plan.shader_packs,
        "options": plan.options,
        "mods": plan.mods.iter().map(|(source, version)| json!({
            "title": source.title,
            "project": source.project_id,
            "version": version.version_number,
            "enabled": source.enabled,
        })).collect::<Vec<_>>(),
        "no_build": plan.no_build.iter().map(|m| json!({ "title": m.title, "project": m.project_id, "file": m.file_name })).collect::<Vec<_>>(),
        "not_on_modrinth": plan.not_on_modrinth,
        "unchecked": plan.unchecked,
    })
}

/// Why worlds can't go to `to`, if they can't.
fn worlds_refusal(rule: WorldsRule, from: &str, to: &str) -> Option<String> {
    match rule {
        WorldsRule::Older => Some(format!(
            "Worlds can't go from Minecraft {from} to {to}: an older version can't open them. Leave --worlds out."
        )),
        WorldsRule::Unknown => Some(format!(
            "Worlds can't go from Minecraft {from} to {to}: it can't be told whether {to} is newer. Leave --worlds out."
        )),
        WorldsRule::Same | WorldsRule::Newer => None,
    }
}

/// Why a copy to the instance's own Minecraft version and loader is refused, if it is:
/// that is a lossy duplicate (no `config/`, mods re-resolved to their newest builds).
fn same_target(source: &Instance, version: &str, loader: Loader) -> Option<String> {
    (source.game_version == version && source.loader == loader).then(|| {
        format!(
            "{} already is Minecraft {version} {}. Choose another --version or --loader; `instances duplicate` copies an instance as it is, and `instances set-loader-version` changes the loader's version.",
            source.name,
            loader.label()
        )
    })
}

/// What the user should hear before a copy that leaves something behind.
fn confirmation_message(plan: &CopyPlan) -> String {
    let mut parts = Vec::new();
    let behind = plan.left_behind().len();
    if behind > 0 {
        parts.push(format!(
            "leaves {behind} mod(s) behind unless another mod requires them (no build for Minecraft {} {}, not on Modrinth, or couldn't be checked)",
            plan.game_version,
            plan.loader.label()
        ));
    }
    if !plan.worlds.is_empty() && plan.worlds_rule == WorldsRule::Newer {
        parts.push(format!(
            "copies {} world(s) that Minecraft {} upgrades when it opens them (they can't go back; the originals stay)",
            plan.worlds.len(),
            plan.game_version
        ));
    }
    format!(
        "The copy {}. Show the user the plan, then pass --yes.",
        parts.join(" and ")
    )
}

/// Makes a new instance from an instance, for another Minecraft version or loader (or
/// both), as "Copy with another version or loader" does: a job. The instance itself
/// never changes. A copy that leaves mods behind or upgrades worlds needs `confirm`,
/// and the refusal carries the plan.
pub fn copy_as(call: Call, cx: &mut App) -> Reply {
    let params: CopyAsParams = params!(call);
    let control = call.control.clone();
    let source = tri!(control.instance_param(&call, cx));
    tri!(control.idle(&source, cx));
    let kind = match &params.loader {
        Some(loader) => tri!(parse_loader(loader)),
        None => source.loader.kind(),
    };
    if kind.is_none() && params.loader_version.is_some() {
        return ready(Err(api_error(
            codes::INVALID_PARAMS,
            "Vanilla has no mod loader, so it takes no --loader-version.",
        )));
    }
    let installer = control.instances.read(cx).installer();
    cx.spawn(async move |cx| {
        let manifest_installer = installer.clone();
        let manifest = cx
            .background_spawn(async move { manifest_installer.version_manifest().await })
            .await
            .map_err(|err| api_error(codes::UNAVAILABLE, err.to_string()))?;
        let version = match params.version.as_deref().map(str::trim) {
            None | Some("") => source.game_version.clone(),
            Some("latest") => manifest
                .versions
                .iter()
                .find(|v| v.kind == MojangVersionType::Release)
                .map(|v| v.id.clone())
                .ok_or_else(|| api_error(codes::UNAVAILABLE, "Mojang's list has no release."))?,
            Some(wanted) => {
                if !manifest.versions.iter().any(|v| v.id == wanted) {
                    return Err(api_error(
                        codes::NOT_FOUND,
                        format!("Minecraft has no version \"{wanted}\". List them with `instances versions`."),
                    ));
                }
                wanted.to_string()
            }
        };
        let loader = match kind {
            None => None,
            Some(kind) => {
                let game_version = version.clone();
                let list = cx
                    .background_spawn(async move { installer.loader_versions(kind, &game_version).await })
                    .await
                    .map_err(|err| api_error(codes::UNAVAILABLE, err.to_string()))?;
                // the instance's own loader version, where it still fits
                let own = (source.loader.kind() == Some(kind) && source.game_version == version)
                    .then(|| source.loader_version.clone())
                    .flatten();
                let wanted = params.loader_version.clone().or(own);
                let id = pick_loader_version(&list, wanted.as_deref()).ok_or_else(|| match &wanted {
                    Some(wanted) => api_error(
                        codes::NOT_FOUND,
                        format!("{} has no version \"{wanted}\" for Minecraft {version}.", kind.label()),
                    ),
                    None => api_error(
                        codes::UNAVAILABLE,
                        format!("{} has no build for Minecraft {version}.", kind.label()),
                    ),
                })?;
                Some(LoaderChoice { kind, version: id })
            }
        };
        let to_loader = Loader::from_kind(kind);
        if let Some(refusal) = same_target(&source, &version, to_loader) {
            return Err(api_error(codes::INVALID_PARAMS, refusal));
        }
        let ids: Vec<&str> = manifest.versions.iter().map(|v| v.id.as_str()).collect();
        let rule = copy_as::worlds_rule(&source.game_version, &version, &ids);
        if params.worlds
            && let Some(refusal) = worlds_refusal(rule, &source.game_version, &version)
        {
            return Err(api_error(codes::INVALID_PARAMS, refusal));
        }
        let read = control
            .instances
            .update(cx, |instances, cx| instances.copy_contents(&source.id, cx));
        let contents = read.await.map_err(|err| api_error(codes::FAILED, err))?;
        let resolved = if params.mods && to_loader != Loader::None && !contents.mods.is_empty() {
            let lookup = control.instances.update(cx, |instances, cx| {
                instances.resolve_copy_mods(contents.mods.clone(), Target::new(version.clone(), to_loader), cx)
            });
            let Some(lookup) = lookup else {
                return Err(api_error(
                    codes::UNAVAILABLE,
                    "Modrinth can't be reached, so the mods can't be copied. Pass --no-mods to copy without them.",
                ));
            };
            Some(lookup.await.map_err(|err| {
                api_error(
                    codes::UNAVAILABLE,
                    format!("Couldn't look the mods up on Modrinth: {err}. Try again, or pass --no-mods to copy without them."),
                )
            })?)
        } else {
            None
        };
        let plan = copy_as::plan(&contents, &version, to_loader, params.options(), rule, resolved.as_ref());
        let name = params
            .name
            .clone()
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty());
        let shown_name = name
            .clone()
            .unwrap_or_else(|| copy_as::default_name(&source.name, &version, to_loader));
        let plan_value = plan_json(&source, &shown_name, loader.as_ref(), &plan);
        if plan.needs_confirmation() && !super::confirmed(&call.params) {
            return Err(api_error(codes::CONFIRMATION_REQUIRED, confirmation_message(&plan))
                .with_data(json!({ "plan": plan_value })));
        }
        let request = CopyAsRequest { name, loader, plan };
        let reply = cx.update(|cx| -> Reply {
            let seq = control.notices.borrow().last_seq();
            let started = control
                .instances
                .update(cx, |instances, cx| instances.copy_as(&source.id, request, cx));
            let import = tri!(failed(started));
            let Some(progress) = control
                .instances
                .read(cx)
                .imports()
                .iter()
                .find(|j| j.id == import)
                .map(|j| j.progress.clone())
            else {
                return ready(Err(api_error(codes::FAILED, "The copy didn't start.")));
            };
            let watch = control.clone();
            let plan = plan_value.clone();
            let work = cx.spawn(async move |cx| {
                poll_until(cx, None, Duration::from_millis(300), Duration::from_secs(6 * 3600), |cx| {
                    (!watch.instances.read(cx).imports().iter().any(|j| j.id == import)).then_some(())
                })
                .await?;
                // its own notice: another job's can come first
                let notice = watch.notices.borrow().find_since(seq, |n| {
                    n.import == Some(import) && matches!(n.kind, "notice" | "launch_failed")
                });
                match notice {
                    Some(n) if n.is_failure() => Err(api_error(codes::FAILED, format!("{}: {}", n.title, n.message))),
                    Some(n) => Ok(cx.update(|cx| {
                        let instances = watch.instances.read(cx);
                        // this job's copy, not whichever instance is selected now
                        let copy = instances
                            .copy_made_by(import)
                            .and_then(|id| instances.list().iter().find(|i| i.id == id))
                            .cloned();
                        json!({
                            "instance": copy.map(|i| instance_json(&i, instances, watch.servers.read(cx))),
                            "title": n.title,
                            "message": n.message,
                            "plan": plan,
                        })
                    })),
                    None => Err(api_error(codes::CANCELLED, "Cancelled.")),
                }
            });
            let job = control.start_job(
                JobSpec::new("copy_as", format!("Copying {} as {shown_name}", source.name))
                    .instance(&source)
                    .progress(Progress::Install(progress))
                    .cancel(Cancel::Import(import)),
                work,
                cx,
            );
            control.reply_job(&call, job, cx)
        });
        reply.await
    })
}

#[cfg(test)]
mod tests {
    use launcher_core::content::copy_as::{Contents, ResolvedMods, SourceMod};

    use super::*;

    fn version(id: &str, recommended: bool) -> LoaderVersion {
        LoaderVersion {
            id: id.into(),
            display: format!("v{id}"),
            stable: true,
            recommended,
        }
    }

    #[test]
    fn copy_params_take_the_dialogs_defaults() {
        let params: CopyAsParams =
            serde_json::from_value(json!({ "instance": "Survival", "version": "1.21.1" })).unwrap();
        assert_eq!(params.options(), CopyAsOptions::default());
        assert_eq!(params.loader, None);
        let params: CopyAsParams = serde_json::from_value(json!({
            "instance": "Survival", "loader": "quilt", "worlds": true, "packs": false, "mods": false,
            "confirm": true, "wait": true,
        }))
        .unwrap();
        assert_eq!(
            params.options(),
            CopyAsOptions {
                worlds: true,
                packs: false,
                mods: false,
            }
        );
    }

    #[test]
    fn loader_versions_are_picked_by_id_or_as_shown() {
        let list = [
            version("0.16.14", false),
            version("0.16.10", true),
            version("0.15.0", false),
        ];
        assert_eq!(pick_loader_version(&list, None).as_deref(), Some("0.16.10"));
        assert_eq!(
            pick_loader_version(&list, Some("0.15.0")).as_deref(),
            Some("0.15.0")
        );
        assert_eq!(
            pick_loader_version(&list, Some("v0.16.14")).as_deref(),
            Some("0.16.14")
        );
        assert_eq!(pick_loader_version(&list, Some("9.9")), None);
        assert_eq!(pick_loader_version(&[], None), None);
    }

    #[test]
    fn worlds_are_refused_for_an_older_or_unknown_version() {
        assert!(
            worlds_refusal(WorldsRule::Older, "1.21.4", "1.20.1")
                .unwrap()
                .contains("older version")
        );
        assert!(worlds_refusal(WorldsRule::Unknown, "1.21.1", "24w14a").is_some());
        assert_eq!(worlds_refusal(WorldsRule::Newer, "1.20.1", "1.21.1"), None);
        assert_eq!(worlds_refusal(WorldsRule::Same, "1.21.1", "1.21.1"), None);
    }

    #[test]
    fn a_copy_needs_another_version_or_loader() {
        let source = Instance {
            name: "Survival".into(),
            game_version: "1.21.1".into(),
            loader: Loader::Fabric,
            loader_version: Some("0.16.10".into()),
            ..Instance::default()
        };
        let refusal = same_target(&source, "1.21.1", Loader::Fabric).unwrap();
        assert!(refusal.contains("instances duplicate"), "{refusal}");
        assert!(
            refusal.contains("instances set-loader-version"),
            "{refusal}"
        );
        assert_eq!(same_target(&source, "1.21.4", Loader::Fabric), None);
        assert_eq!(same_target(&source, "1.21.1", Loader::Quilt), None);
        assert_eq!(same_target(&source, "1.21.1", Loader::None), None);
    }

    #[test]
    fn the_plan_says_what_goes_and_what_stays() {
        let source = Instance {
            id: "survival".into(),
            name: "Survival".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Forge,
            loader_version: Some("47.4.0".into()),
            ..Instance::default()
        };
        let sodium = SourceMod {
            project_id: "AANobbMI".into(),
            title: "Sodium".into(),
            file_name: "sodium.jar".into(),
            enabled: true,
        };
        let jei = SourceMod {
            project_id: "u6dRKJwZ".into(),
            title: "JEI".into(),
            file_name: "jei.jar".into(),
            enabled: false,
        };
        let contents = Contents {
            worlds: vec!["New World".into()],
            resource_packs: vec!["Faithful.zip".into()],
            options: true,
            mods: vec![sodium.clone(), jei],
            other_mods: vec!["Homemade".into()],
            ..Contents::default()
        };
        let build: lodestone_minecraft::modrinth::RawVersion = serde_json::from_value(json!({
            "id": "s1", "project_id": "AANobbMI", "name": "Sodium", "version_number": "0.6.0",
            "version_type": "release", "loaders": ["fabric"], "game_versions": ["1.21.1"],
        }))
        .unwrap();
        let resolved = ResolvedMods::sort(&contents.mods, |m| {
            (m.project_id == sodium.project_id).then(|| build.clone())
        });
        let options = CopyAsOptions {
            worlds: true,
            ..CopyAsOptions::default()
        };
        let plan = copy_as::plan(
            &contents,
            "1.21.1",
            Loader::Fabric,
            options,
            WorldsRule::Newer,
            Some(&resolved),
        );
        let fabric = LoaderChoice {
            kind: launcher_core::game::loaders::LoaderKind::Fabric,
            version: "0.16.14".into(),
        };
        let value = plan_json(&source, "Survival (1.21.1 Fabric)", Some(&fabric), &plan);
        assert_eq!(value["from"]["loader"], "Forge 47.4.0");
        assert_eq!(
            value["to"],
            json!({ "name": "Survival (1.21.1 Fabric)", "game_version": "1.21.1", "loader": "fabric", "loader_version": "0.16.14" })
        );
        assert_eq!(value["worlds_rule"], "newer");
        assert_eq!(value["worlds"], json!(["New World"]));
        assert_eq!(
            value["mods"],
            json!([{ "title": "Sodium", "project": "AANobbMI", "version": "0.6.0", "enabled": true }])
        );
        assert_eq!(value["no_build"][0]["title"], "JEI");
        assert_eq!(value["not_on_modrinth"], json!(["Homemade"]));
        let message = confirmation_message(&plan);
        assert!(message.contains("leaves 2 mod(s) behind"), "{message}");
        assert!(message.contains("upgrades"), "{message}");
        assert!(message.ends_with("pass --yes."));
    }
}
