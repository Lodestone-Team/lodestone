//! Skins: the launcher's look (Settings > Skin). Listing, choosing, importing and
//! exporting packs.

use std::path::PathBuf;

use gpui::App;
use launcher_control::codes;
use launcher_core::ui_skin::SkinEntry;
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
