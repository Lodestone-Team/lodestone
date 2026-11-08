//! Settings: an instance's game settings (its Settings tab), the launcher's instance
//! defaults, General and Developer (the Settings dialog), and shared game settings.

use std::collections::BTreeMap;

use gpui::App;
use launcher_control::{ErrorObject, codes};
use launcher_core::{
    instances::{Instance, Memory, Resolution, WindowSettings, format_env, parse_env},
    settings::{DEFAULT_MIN_RAM_MB, InstanceDefaults, default_max_ram_mb},
    shared_options::{self, Category},
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    Call, Control, Reply, api_error,
    instances::{failed, params, save_changed, settings_effective, tri},
    ready,
};
use crate::instances::fields::check_memory;

/// The settings an instance can override, by the names the API uses.
pub const INSTANCE_KEYS: [&str; 6] = ["memory", "java", "jvm_args", "game_args", "window", "env"];

/// What `settings.instance.set` and `settings.defaults.set` take. Text as typed in the
/// window: memory as `4096` or `4G`, environment as `NAME=value NAME2="a b"`, the
/// window as `1280x720` (or `default`).
#[derive(Deserialize, Default)]
struct GameSettingsParams {
    #[serde(default)]
    min_memory: Option<String>,
    #[serde(default)]
    max_memory: Option<String>,
    #[serde(default)]
    java: Option<String>,
    #[serde(default)]
    jvm_args: Option<String>,
    #[serde(default)]
    game_args: Option<String>,
    #[serde(default)]
    resolution: Option<String>,
    #[serde(default)]
    fullscreen: Option<bool>,
    #[serde(default)]
    env: Option<String>,
}

impl GameSettingsParams {
    fn is_empty(&self) -> bool {
        self.min_memory.is_none()
            && self.max_memory.is_none()
            && self.java.is_none()
            && self.jvm_args.is_none()
            && self.game_args.is_none()
            && self.resolution.is_none()
            && self.fullscreen.is_none()
            && self.env.is_none()
    }

    /// The memory, checked as the Settings tab checks it.
    fn memory(
        &self,
        current: (u32, u32),
        total: Option<u64>,
    ) -> Result<Option<(u32, u32)>, ErrorObject> {
        if self.min_memory.is_none() && self.max_memory.is_none() {
            return Ok(None);
        }
        let min = self
            .min_memory
            .clone()
            .unwrap_or_else(|| current.0.to_string());
        let max = self
            .max_memory
            .clone()
            .unwrap_or_else(|| current.1.to_string());
        let check = check_memory(&min, &max, total);
        match (check.values, check.error) {
            (Some(values), _) => Ok(Some(values)),
            (None, error) => Err(api_error(
                codes::INVALID_PARAMS,
                format!("Memory: {}", error.unwrap_or_default()),
            )),
        }
    }

    /// The window, changed from `current`.
    fn window(&self, current: WindowSettings) -> Result<Option<WindowSettings>, ErrorObject> {
        if self.resolution.is_none() && self.fullscreen.is_none() {
            return Ok(None);
        }
        let mut window = current;
        if let Some(text) = &self.resolution {
            window.resolution = parse_resolution(text)?;
        }
        if let Some(fullscreen) = self.fullscreen {
            window.fullscreen = fullscreen;
        }
        Ok(Some(window))
    }
}

/// `1280x720` (or `1280×720`, `1280 720`), or `default` for the game's own size. Each
/// side is 100 to 16384, as the Settings tab allows.
pub fn parse_resolution(text: &str) -> Result<Option<Resolution>, ErrorObject> {
    let text = text.trim().to_ascii_lowercase();
    if text.is_empty() || text == "default" || text == "none" {
        return Ok(None);
    }
    let bad = || {
        api_error(
            codes::INVALID_PARAMS,
            format!(
                "\"{text}\" isn't a window size: give WIDTHxHEIGHT, each 100 to 16384, or default."
            ),
        )
    };
    let joined = text.replace(['×', '*'], "x");
    let (width, height) = if joined.contains('x') {
        joined.split_once('x').ok_or_else(bad)?
    } else {
        joined
            .trim()
            .split_once(char::is_whitespace)
            .ok_or_else(bad)?
    };
    let side = |s: &str| {
        s.trim()
            .parse::<u32>()
            .ok()
            .filter(|v| (100..=16384).contains(v))
    };
    match (side(width), side(height)) {
        (Some(width), Some(height)) => Ok(Some(Resolution { width, height })),
        _ => Err(bad()),
    }
}

fn env(text: &str) -> Result<BTreeMap<String, String>, ErrorObject> {
    parse_env(text).map_err(|err| api_error(codes::INVALID_PARAMS, format!("Environment: {err}")))
}

/// An instance's own settings (`null` follows the default) and the defaults.
fn instance_settings_json(instance: &Instance, control: &Control, cx: &App) -> Value {
    let instances = control.instances.read(cx);
    json!({
        "instance": instance.id,
        "name": instance.name,
        "game_version": instance.game_version,
        "loader": instance.loader,
        "loader_version": instance.loader_version,
        "own": {
            "memory": instance.memory.map(|m| json!({ "min": m.min, "max": m.max })),
            "java": instance.java_override,
            "jvm_args": instance.jvm_args,
            "game_args": instance.game_args,
            "window": instance.window.map(window_json),
            "env": instance.env.as_ref().map(format_env),
        },
        "effective": settings_effective(instance, instances),
        "follows_defaults": INSTANCE_KEYS
            .iter()
            .filter(|key| match **key {
                "memory" => instance.memory.is_none(),
                "java" => instance.java_override.is_none(),
                "jvm_args" => instance.jvm_args.is_none(),
                "game_args" => instance.game_args.is_none(),
                "window" => instance.window.is_none(),
                _ => instance.env.is_none(),
            })
            .collect::<Vec<_>>(),
    })
}

fn window_json(window: WindowSettings) -> Value {
    json!({
        "resolution": window.resolution.map(|r| format!("{}x{}", r.width, r.height)),
        "fullscreen": window.fullscreen,
    })
}

pub fn instance_get(call: Call, cx: &mut App) -> Reply {
    let instance = tri!(call.control.instance_param(&call, cx));
    ready(Ok(instance_settings_json(&instance, &call.control, cx)))
}

pub fn instance_set(call: Call, cx: &mut App) -> Reply {
    let params: GameSettingsParams = params!(call);
    let control = &call.control;
    let mut instance = tri!(control.instance_param(&call, cx));
    if params.is_empty() {
        return ready(Err(api_error(
            codes::INVALID_PARAMS,
            "Give a setting to change: min_memory, max_memory, java, jvm_args, game_args, resolution, fullscreen or env.",
        )));
    }
    let (defaults, total) = {
        let instances = control.instances.read(cx);
        (instances.defaults().clone(), instances.total_memory())
    };
    let current = instance.memory(&defaults, total);
    if let Some((min, max)) = tri!(params.memory(current, total)) {
        instance.memory = Some(Memory { min, max });
    }
    if let Some(java) = &params.java {
        let java = java.trim();
        instance.java_override = (!java.is_empty()).then(|| java.into());
        if let Some(path) = &instance.java_override
            && !path.is_absolute()
        {
            return ready(Err(api_error(
                codes::INVALID_PARAMS,
                "The Java override is the absolute path of a java executable.",
            )));
        }
    }
    if let Some(args) = &params.jvm_args {
        instance.jvm_args = Some(args.trim().to_string());
    }
    if let Some(args) = &params.game_args {
        instance.game_args = Some(args.trim().to_string());
    }
    let window = instance.window.unwrap_or_else(|| defaults.window());
    if let Some(window) = tri!(params.window(window)) {
        instance.window = Some(window);
    }
    if let Some(text) = &params.env {
        instance.env = Some(tri!(env(text)));
    }
    let warning = check_memory(
        &instance.memory(&defaults, total).0.to_string(),
        &instance.memory(&defaults, total).1.to_string(),
        total,
    )
    .warning;
    let mut result = tri!(save_changed(control, instance.clone(), cx));
    let current = tri!(control.instance(&instance.id, cx));
    result["settings"] = instance_settings_json(&current, control, cx);
    result["warning"] = json!(warning);
    ready(Ok(result))
}

#[derive(Deserialize)]
struct ResetParams {
    /// Which settings follow the defaults again; all without it.
    #[serde(default)]
    keys: Vec<String>,
}

fn reset_keys(keys: Vec<String>) -> Result<Vec<String>, ErrorObject> {
    if keys.is_empty() || keys.iter().any(|k| k == "all") {
        return Ok(INSTANCE_KEYS.iter().map(|k| k.to_string()).collect());
    }
    if let Some(bad) = keys.iter().find(|k| !INSTANCE_KEYS.contains(&k.as_str())) {
        return Err(api_error(
            codes::INVALID_PARAMS,
            format!(
                "No setting \"{bad}\". Settings: {}.",
                INSTANCE_KEYS.join(", ")
            ),
        ));
    }
    Ok(keys)
}

pub fn instance_reset(call: Call, cx: &mut App) -> Reply {
    let params: ResetParams = params!(call);
    let control = &call.control;
    let mut instance = tri!(control.instance_param(&call, cx));
    for key in tri!(reset_keys(params.keys)) {
        match key.as_str() {
            "memory" => instance.memory = None,
            "java" => instance.java_override = None,
            "jvm_args" => instance.jvm_args = None,
            "game_args" => instance.game_args = None,
            "window" => instance.window = None,
            _ => instance.env = None,
        }
    }
    let mut result = tri!(save_changed(control, instance.clone(), cx));
    let current = tri!(control.instance(&instance.id, cx));
    result["settings"] = instance_settings_json(&current, control, cx);
    ready(Ok(result))
}

fn defaults_json(control: &Control, cx: &App) -> Value {
    let instances = control.instances.read(cx);
    let defaults = instances.defaults();
    let total = instances.total_memory();
    let (min, max) = defaults.memory(total);
    let window = defaults.window();
    json!({
        "memory": { "min": min, "max": max, "follows_this_mac": defaults.min_ram.is_none() && defaults.max_ram.is_none() },
        "built_in_memory": { "min": DEFAULT_MIN_RAM_MB, "max": default_max_ram_mb(total) },
        "total_memory_bytes": total,
        "jvm_args": defaults.jvm_args,
        "game_args": defaults.game_args,
        "window": window_json(window),
        "env": format_env(&defaults.env),
    })
}

pub fn defaults_get(call: Call, cx: &mut App) -> Reply {
    ready(Ok(json!({ "defaults": defaults_json(&call.control, cx) })))
}

fn save_defaults(control: &Control, defaults: InstanceDefaults, cx: &mut App) -> Reply {
    let saved = control
        .instances
        .update(cx, |instances, cx| instances.save_defaults(defaults, cx));
    tri!(failed(saved));
    ready(Ok(json!({ "defaults": defaults_json(control, cx) })))
}

pub fn defaults_set(call: Call, cx: &mut App) -> Reply {
    let params: GameSettingsParams = params!(call);
    if params.is_empty() {
        return ready(Err(api_error(
            codes::INVALID_PARAMS,
            "Give a setting to change: min_memory, max_memory, jvm_args, game_args, resolution, fullscreen or env.",
        )));
    }
    if params.java.is_some() {
        return ready(Err(api_error(
            codes::INVALID_PARAMS,
            "Java is set per instance; the default is the Java Mojang provides.",
        )));
    }
    let control = &call.control;
    let (mut defaults, total) = {
        let instances = control.instances.read(cx);
        (instances.defaults().clone(), instances.total_memory())
    };
    if let Some((min, max)) = tri!(params.memory(defaults.memory(total), total)) {
        // the built-in values stay unset, so they follow this Mac
        defaults.min_ram = (min != DEFAULT_MIN_RAM_MB).then_some(min);
        defaults.max_ram = (max != default_max_ram_mb(total)).then_some(max);
    }
    if let Some(args) = &params.jvm_args {
        defaults.jvm_args = args.trim().to_string();
    }
    if let Some(args) = &params.game_args {
        defaults.game_args = args.trim().to_string();
    }
    if let Some(window) = tri!(params.window(defaults.window())) {
        defaults.resolution = window.resolution;
        defaults.fullscreen = window.fullscreen;
    }
    if let Some(text) = &params.env {
        defaults.env = tri!(env(text));
    }
    save_defaults(control, defaults, cx)
}

pub fn defaults_reset(call: Call, cx: &mut App) -> Reply {
    let params: ResetParams = params!(call);
    let control = &call.control;
    let mut defaults = control.instances.read(cx).defaults().clone();
    for key in tri!(reset_keys(params.keys)) {
        match key.as_str() {
            "memory" => {
                defaults.min_ram = None;
                defaults.max_ram = None;
            }
            "jvm_args" => defaults.jvm_args.clear(),
            "game_args" => defaults.game_args.clear(),
            "window" => {
                defaults.resolution = None;
                defaults.fullscreen = false;
            }
            "env" => defaults.env.clear(),
            _ => {}
        }
    }
    save_defaults(control, defaults, cx)
}

fn general_json(control: &Control, cx: &App) -> Value {
    json!({ "auto_groups": control.instances.read(cx).auto_groups() })
}

pub fn general_get(call: Call, cx: &mut App) -> Reply {
    ready(Ok(json!({ "general": general_json(&call.control, cx) })))
}

#[derive(Deserialize)]
struct GeneralParams {
    #[serde(default)]
    auto_groups: Option<bool>,
}

pub fn general_set(call: Call, cx: &mut App) -> Reply {
    let params: GeneralParams = params!(call);
    let Some(on) = params.auto_groups else {
        return ready(Err(api_error(
            codes::INVALID_PARAMS,
            "Give a setting to change: auto_groups.",
        )));
    };
    call.control
        .instances
        .update(cx, |instances, cx| instances.set_auto_groups(on, cx));
    let general = general_json(&call.control, cx);
    if general["auto_groups"] != json!(on) {
        let notice = call
            .control
            .instances
            .read(cx)
            .notice()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "Couldn't save the setting.".into());
        return ready(Err(api_error(codes::FAILED, notice)));
    }
    ready(Ok(json!({ "general": general })))
}

fn developer_json(control: &Control, cx: &App) -> Value {
    let accounts = control.accounts.read(cx);
    json!({
        "sign_in_as_minecraft_launcher": accounts.launcher_identity_enabled(),
        "accounts_signed_in_with_it": accounts
            .launcher_identity_accounts()
            .iter()
            .map(|a| json!({ "id": a.id, "username": a.username }))
            .collect::<Vec<_>>(),
    })
}

pub fn developer_get(call: Call, cx: &mut App) -> Reply {
    ready(Ok(
        json!({ "developer": developer_json(&call.control, cx) }),
    ))
}

#[derive(Deserialize)]
struct DeveloperParams {
    sign_in_as_minecraft_launcher: bool,
    /// Turning it off: also sign out the accounts that used it.
    #[serde(default)]
    sign_out_accounts: bool,
}

pub fn developer_set(call: Call, cx: &mut App) -> Reply {
    let params: DeveloperParams = params!(call);
    let control = &call.control;
    let enabled = control.accounts.read(cx).launcher_identity_enabled();
    if params.sign_in_as_minecraft_launcher && !enabled {
        // the window shows its disclosure first
        if let Err(err) = call.confirm(
            "makes new Microsoft sign-ins tell Microsoft that Lodestone Launcher is Mojang's Minecraft Launcher",
        ) {
            let disclosure: Vec<Value> = crate::shell::developer::disclosure()
                .iter()
                .map(|(heading, text)| json!({ "heading": heading, "text": text.to_string() }))
                .collect();
            return ready(Err(err.with_data(json!({ "disclosure": disclosure }))));
        }
    }
    if params.sign_out_accounts {
        if params.sign_in_as_minecraft_launcher {
            return ready(Err(api_error(
                codes::INVALID_PARAMS,
                "sign_out_accounts goes with turning the setting off.",
            )));
        }
        tri!(call.confirm("signs out every account signed in as the Minecraft Launcher"));
        control.accounts.update(cx, |accounts, cx| {
            accounts.sign_out_launcher_identity_accounts(cx)
        });
    }
    let on = params.sign_in_as_minecraft_launcher;
    control
        .accounts
        .update(cx, |accounts, cx| accounts.set_launcher_identity(on, cx));
    if control.accounts.read(cx).launcher_identity_enabled() != on {
        let notice = control
            .accounts
            .read(cx)
            .notice()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "Couldn't save the setting.".into());
        return ready(Err(api_error(codes::FAILED, notice)));
    }
    ready(Ok(json!({ "developer": developer_json(control, cx) })))
}

fn category(name: &str) -> Result<Category, ErrorObject> {
    Category::ALL
        .into_iter()
        .find(|c| {
            serde_json::to_value(c)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .as_deref()
                == Some(name)
                || c.label().eq_ignore_ascii_case(name)
        })
        .ok_or_else(|| {
            api_error(
                codes::INVALID_PARAMS,
                format!(
                    "No category \"{name}\". Categories: {}.",
                    Category::ALL
                        .iter()
                        .filter_map(|c| serde_json::to_value(c).ok())
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )
        })
}

fn shared_error(err: impl std::fmt::Display) -> ErrorObject {
    api_error(codes::FAILED, err.to_string())
}

fn row_json(row: &shared_options::Row) -> Value {
    json!({
        "key": row.key,
        "label": row.label,
        "category": row.category,
        "shared": row.shared.as_ref().map(|v| row.show(v)),
        "shared_raw": row.shared,
        "local": row.local.as_ref().map(|v| row.show(v)),
        "local_raw": row.local,
        "overridden": row.overridden,
    })
}

pub fn shared_get(call: Call, cx: &mut App) -> Reply {
    let control = &call.control;
    let instances = control.instances.read(cx);
    let profile = tri!(shared_options::load_profile(instances.data_dir()).map_err(shared_error));
    let rows = shared_options::rows(&profile, None);
    ready(Ok(json!({
        "categories": Category::ALL.iter().map(|c| json!({
            "id": c,
            "label": c.label(),
            "description": c.description(),
            "shared": profile.shares(*c),
        })).collect::<Vec<_>>(),
        "sync_from": profile.preferred.as_ref().map(|id| json!({
            "instance": id,
            "name": instances.instance_name(id),
        })),
        "values": rows.iter().map(row_json).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
struct SetValueParams {
    key: String,
    value: String,
}

pub fn shared_set_value(call: Call, cx: &mut App) -> Reply {
    let params: SetValueParams = params!(call);
    let data = call.control.instances.read(cx).data_dir().clone();
    tri!(shared_options::set_value(&data, &params.key, &params.value).map_err(shared_error));
    shared_get(call, cx)
}

#[derive(Deserialize)]
struct CategoryParams {
    category: String,
    shared: bool,
}

pub fn shared_set_category(call: Call, cx: &mut App) -> Reply {
    let params: CategoryParams = params!(call);
    let category = tri!(category(&params.category));
    let data = call.control.instances.read(cx).data_dir().clone();
    let result = shared_options::update_profile(&data, |profile| {
        if params.shared {
            profile.disabled.remove(&category);
        } else {
            profile.disabled.insert(category);
        }
    });
    tri!(result.map_err(shared_error));
    shared_get(call, cx)
}

pub fn shared_sync_from(call: Call, cx: &mut App) -> Reply {
    let control = call.control.clone();
    let data = control.instances.read(cx).data_dir().clone();
    let result = match call.params.get("instance").and_then(Value::as_str) {
        Some(reference) => {
            let instance = tri!(control.instance(reference, cx));
            let dir = control.instances.read(cx).instance_dir(&instance.id);
            shared_options::set_preferred(&data, Some(&instance.id), Some(&dir))
        }
        None => shared_options::set_preferred(&data, None, None),
    };
    tri!(result.map_err(shared_error));
    shared_get(call, cx)
}

pub fn shared_copy_from(call: Call, cx: &mut App) -> Reply {
    let control = call.control.clone();
    let instance = tri!(control.instance_param(&call, cx));
    let (data, dir, jar) = {
        let instances = control.instances.read(cx);
        (
            instances.data_dir().clone(),
            instances.instance_dir(&instance.id),
            instances.client_jar(&instance.game_version),
        )
    };
    let copied = tri!(
        shared_options::copy_from_instance(&data, &dir, &instance.game_version, Some(&jar))
            .map_err(shared_error)
    );
    let mut result = match shared_get(call, cx).now_or_never_ready() {
        Some(Ok(value)) => value,
        Some(Err(err)) => return ready(Err(err)),
        None => json!({}),
    };
    result["copied"] = json!(copied);
    ready(Ok(result))
}

pub fn shared_clear(call: Call, cx: &mut App) -> Reply {
    tri!(call.confirm("forgets every shared game setting (instances keep their own files)"));
    let data = call.control.instances.read(cx).data_dir().clone();
    tri!(shared_options::reset(&data).map_err(shared_error));
    shared_get(call, cx)
}

fn instance_sharing_json(
    control: &Control,
    instance: &Instance,
    cx: &App,
) -> Result<Value, ErrorObject> {
    let instances = control.instances.read(cx);
    let data = instances.data_dir();
    let dir = instances.instance_dir(&instance.id);
    let jar = instances.client_jar(&instance.game_version);
    let profile = shared_options::load_profile(data).map_err(shared_error)?;
    let sharing = shared_options::load_instance(&dir).map_err(shared_error)?;
    let mut local = BTreeMap::new();
    let mut dv = None;
    if let Ok((file, Some(target))) =
        shared_options::read_instance_options(&dir, &instance.game_version, Some(&jar))
    {
        local = shared_options::instance_values(&file, target.dv);
        dv = Some(target.dv);
    }
    let rows: Vec<Value> = shared_options::rows(&profile, Some((&sharing, &local)))
        .iter()
        .filter(|row| {
            row.local.is_some() || dv.is_none_or(|dv| shared_options::exists_at(&row.key, dv))
        })
        .map(row_json)
        .collect();
    Ok(json!({
        "instance": instance.id,
        "enabled": sharing.enabled,
        "sync_from": profile.preferred.as_deref() == Some(instance.id.as_str()),
        "overrides": sharing.overrides,
        "changed_in_game": sharing.changed,
        "played": dv.is_some(),
        "values": rows,
    }))
}

pub fn shared_instance_get(call: Call, cx: &mut App) -> Reply {
    let instance = tri!(call.control.instance_param(&call, cx));
    ready(instance_sharing_json(&call.control, &instance, cx))
}

#[derive(Deserialize)]
struct InstanceSharingParams {
    #[serde(default)]
    enabled: Option<bool>,
    /// Keys this instance keeps its own value of.
    #[serde(default, rename = "override")]
    override_keys: Vec<String>,
    /// Keys that follow the shared value again.
    #[serde(default)]
    share: Vec<String>,
    /// Keep the settings changed in-game as this instance's overrides.
    #[serde(default)]
    keep_changes: bool,
    /// Forget the settings changed in-game.
    #[serde(default)]
    dismiss_changes: bool,
    /// Values for this instance's overridden keys, written to its `options.txt`.
    #[serde(default)]
    set: BTreeMap<String, String>,
}

pub fn shared_instance_set(call: Call, cx: &mut App) -> Reply {
    let params: InstanceSharingParams = params!(call);
    let control = &call.control;
    let instance = tri!(control.instance_param(&call, cx));
    tri!(control.idle(&instance, cx));
    let (dir, jar) = {
        let instances = control.instances.read(cx);
        (
            instances.instance_dir(&instance.id),
            instances.client_jar(&instance.game_version),
        )
    };
    if let Some(enabled) = params.enabled {
        tri!(shared_options::update_instance(&dir, |s| s.enabled = enabled).map_err(shared_error));
    }
    if !params.override_keys.is_empty() {
        tri!(
            shared_options::set_overrides(
                &dir,
                params.override_keys.iter().map(String::as_str),
                true
            )
            .map_err(shared_error)
        );
    }
    if !params.share.is_empty() {
        tri!(
            shared_options::set_overrides(&dir, params.share.iter().map(String::as_str), false)
                .map_err(shared_error)
        );
    }
    if params.keep_changes {
        tri!(shared_options::keep_changes_as_overrides(&dir).map_err(shared_error));
    }
    if params.dismiss_changes {
        tri!(shared_options::dismiss_changes(&dir).map_err(shared_error));
    }
    for (key, value) in &params.set {
        tri!(
            shared_options::set_instance_value(
                &dir,
                &instance.game_version,
                Some(&jar),
                key,
                value
            )
            .map_err(shared_error)
        );
    }
    ready(instance_sharing_json(control, &instance, cx))
}

/// A reply that is ready already, read without waiting.
trait NowOrNever {
    fn now_or_never_ready(self) -> Option<super::ApiResult>;
}

impl NowOrNever for Reply {
    fn now_or_never_ready(self) -> Option<super::ApiResult> {
        futures::FutureExt::now_or_never(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_window_sizes() {
        let size = |text: &str| parse_resolution(text).map(|r| r.map(|r| (r.width, r.height)));
        assert_eq!(size("1280x720").unwrap(), Some((1280, 720)));
        assert_eq!(size("1920 × 1080").unwrap(), Some((1920, 1080)));
        assert_eq!(size("default").unwrap(), None);
        assert!(size("99x720").is_err());
        assert!(size("1280").is_err());
        assert!(size("20000x100").is_err());
    }

    #[test]
    fn reset_takes_known_keys() {
        assert_eq!(reset_keys(vec![]).unwrap().len(), INSTANCE_KEYS.len());
        assert_eq!(reset_keys(vec!["memory".into()]).unwrap(), ["memory"]);
        assert_eq!(
            reset_keys(vec!["colour".into()]).unwrap_err().code,
            codes::INVALID_PARAMS
        );
    }

    #[test]
    fn categories_by_id_or_label() {
        assert_eq!(category("video").unwrap(), Category::Video);
        assert_eq!(category("Sound").unwrap(), Category::Sound);
        assert!(category("graphics").is_err());
    }
}
