//! Skins: the launcher's look (Settings > Skin). Listing, choosing, importing and
//! exporting packs.

use std::path::PathBuf;

use gpui::App;
use launcher_control::codes;
use launcher_core::ui_skin::{SkinEntry, SkinStore, readable};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    Call, Reply, api_error,
    instances::{absolute, params, tri},
    ready,
};

fn entry_json(entry: &SkinEntry, current: &str) -> Value {
    json!({
        "id": entry.id,
        "name": entry.spec.name,
        "builtin": entry.builtin,
        "in_use": entry.id == current,
        "skin": entry.spec,
    })
}

/// `skins.list`: every skin, built-in first, and which one is in use.
pub fn list(call: Call, cx: &mut App) -> Reply {
    let instances = call.control.instances.read(cx);
    let current = crate::skin::look(cx).id.clone();
    let skins: Vec<Value> = instances
        .skins()
        .list()
        .iter()
        .map(|entry| entry_json(entry, &current))
        .collect();
    ready(Ok(json!({
        "skins": skins,
        "in_use": current,
        "safe_mode": crate::skin::safe_mode(),
        "error": crate::skin::error(cx),
    })))
}

#[derive(Deserialize)]
struct UseParams {
    /// A skin's id; `default` (or nothing) for the default look.
    #[serde(default)]
    id: Option<String>,
}

/// `skins.use`: uses a skin.
pub fn use_skin(call: Call, cx: &mut App) -> Reply {
    let params: UseParams = params!(call);
    let id = params.id.filter(|id| id != "default");
    if let Some(id) = &id
        && let Err(err) = call.control.instances.read(cx).skins().get(id)
    {
        return ready(Err(api_error(codes::NOT_FOUND, err.to_string())));
    }
    call.control
        .instances
        .update(cx, |instances, cx| instances.set_skin(id, cx));
    ready(Ok(json!({ "in_use": crate::skin::look(cx).id })))
}

#[derive(Deserialize)]
struct ImportParams {
    /// A .zip file or a folder, absolute.
    path: PathBuf,
    /// Use it once imported.
    #[serde(default)]
    r#use: bool,
}

/// `skins.import`: imports a pack from a .zip file or a folder.
pub fn import(call: Call, cx: &mut App) -> Reply {
    let params: ImportParams = params!(call);
    tri!(absolute(&params.path));
    let store = call.control.instances.read(cx).skins();
    let id = match store.import(&params.path) {
        Ok(id) => id,
        Err(err) => return ready(Err(api_error(codes::INVALID_PARAMS, err.to_string()))),
    };
    if params.r#use {
        call.control
            .instances
            .update(cx, |instances, cx| instances.set_skin(Some(id.clone()), cx));
    }
    let entry = store.get(&id).ok();
    let current = crate::skin::look(cx).id.clone();
    ready(Ok(
        json!({ "skin": entry.map(|e| entry_json(&e, &current)) }),
    ))
}

#[derive(Deserialize)]
struct ExportParams {
    /// The skin; the one in use without it.
    #[serde(default)]
    id: Option<String>,
    /// The .zip file to write, absolute.
    path: PathBuf,
}

/// `skins.export`: writes a skin to a .zip file.
pub fn export(call: Call, cx: &mut App) -> Reply {
    let params: ExportParams = params!(call);
    tri!(absolute(&params.path));
    let id = params
        .id
        .unwrap_or_else(|| crate::skin::look(cx).id.clone());
    let store = call.control.instances.read(cx).skins();
    match store.export(&id, &params.path) {
        Ok(()) => ready(Ok(json!({ "id": id, "path": params.path }))),
        Err(err) => ready(Err(api_error(codes::FAILED, err.to_string()))),
    }
}

fn checks_json(checks: &[readable::Check]) -> Value {
    let short = checks.iter().filter(|c| !c.passes()).count();
    json!({
        "passes": short == 0,
        "short": short,
        "checks": checks.iter().map(|c| json!({
            "surface": c.surface.key(),
            "role": c.role.key(),
            "color": c.color.hex(),
            "ratio": (c.ratio * 100.0).round() / 100.0,
            "target": c.target,
            "passes": c.passes(),
            "fix": c.fix.as_ref().map(|f| f.describe()),
        })).collect::<Vec<_>>(),
    })
}

#[derive(Deserialize)]
struct CheckParams {
    #[serde(default)]
    id: Option<String>,
    /// A pack to check instead: a .zip file or a folder, absolute.
    #[serde(default)]
    path: Option<PathBuf>,
}

/// `skins.check`: readability checks for a skin or a pack.
pub fn check(call: Call, cx: &mut App) -> Reply {
    let params: CheckParams = params!(call);
    if let Some(path) = params.path {
        tri!(absolute(&path));
        return ready(match SkinStore::check_pack(&path) {
            Ok((spec, checks)) => {
                let mut out = checks_json(&checks);
                out["valid"] = json!(true);
                out["name"] = json!(spec.name);
                Ok(out)
            }
            Err(err) => Ok(json!({ "valid": false, "error": err.to_string() })),
        });
    }
    let look = crate::skin::look(cx);
    let id = params.id.unwrap_or_else(|| look.id.clone());
    let checks = if id == look.id {
        crate::skin::checks(&look)
    } else {
        let store = call.control.instances.read(cx).skins();
        let entry = tri!(
            store
                .get(&id)
                .map_err(|err| api_error(codes::NOT_FOUND, err.to_string()))
        );
        let measured = store.measure(&id, &entry.spec);
        readable::checks(&entry.spec, &|surface| measured.get(&surface).copied())
    };
    let mut out = checks_json(&checks);
    out["id"] = json!(id);
    ready(Ok(out))
}

#[derive(Deserialize)]
struct FixParams {
    #[serde(default)]
    id: Option<String>,
}

/// `skins.fix`: applies the suggested fixes to an imported skin.
pub fn fix(call: Call, cx: &mut App) -> Reply {
    let params: FixParams = params!(call);
    let id = params
        .id
        .unwrap_or_else(|| crate::skin::look(cx).id.clone());
    let store = call.control.instances.read(cx).skins();
    let entry = tri!(
        store
            .get(&id)
            .map_err(|err| api_error(codes::NOT_FOUND, err.to_string()))
    );
    if entry.builtin {
        return ready(Err(api_error(
            codes::INVALID_PARAMS,
            "a built-in skin can't be changed; make a copy with New skin… first",
        )));
    }
    let measured = store.measure(&id, &entry.spec);
    let before = entry.spec.clone();
    let mut spec = entry.spec;
    let applied = readable::fix_all(&mut spec, &|surface| measured.get(&surface).copied());
    if applied > 0 {
        if let Err(err) = store.save(&id, &spec) {
            return ready(Err(api_error(codes::FAILED, err.to_string())));
        }
        launcher_core::trace::settings_changed("skin", &before, &spec, json!({ "skin": id }));
        call.control.instances.update(cx, |instances, cx| {
            if instances.skin() == Some(id.as_str()) {
                instances.reload_skin(cx);
            }
        });
    }
    let checks = readable::checks(&spec, &|surface| measured.get(&surface).copied());
    let mut out = checks_json(&checks);
    out["id"] = json!(id);
    out["applied"] = json!(applied);
    ready(Ok(out))
}
