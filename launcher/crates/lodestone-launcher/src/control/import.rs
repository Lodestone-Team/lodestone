//! "Import from another launcher": finds other launchers' instances, and imports them
//! one after another as the dialog's queue does.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use gpui::{App, AppContext as _};
use launcher_control::{ErrorObject, codes};
use launcher_core::{
    game::download::CancelToken,
    import::{
        self, CopyOptions, Found, ImportProgress, ImportRequest, Launcher, Locations, detect,
    },
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    Call, Reply, api_error,
    instances::{absolute, params, tri},
    jobs::{Cancel, JobSpec, Progress},
};

#[derive(Deserialize)]
struct DetectParams {
    /// A launcher's data folder, an app bundle, or one instance's folder; this Mac's
    /// usual places without it.
    #[serde(default)]
    folder: Option<PathBuf>,
}

async fn find(folder: Option<PathBuf>) -> Result<Vec<Launcher>, ErrorObject> {
    match folder {
        Some(folder) => {
            detect::read_folder(&folder).map_err(|err| api_error(codes::NOT_FOUND, err))
        }
        None => {
            let locations = Locations::current()
                .ok_or_else(|| api_error(codes::UNAVAILABLE, "No home folder to look in."))?;
            Ok(detect::detect(&locations))
        }
    }
}

fn found_json(found: &Found) -> Value {
    json!({
        "key": found.key,
        "name": found.name,
        "launcher": found.source.label(),
        "version": found.version_label(),
        "game_dir": found.game_dir,
        "notes": found.notes,
    })
}

pub fn detect(call: Call, cx: &mut App) -> Reply {
    let params: DetectParams = params!(call);
    if let Some(folder) = &params.folder {
        tri!(absolute(folder));
    }
    cx.background_spawn(async move {
        let launchers = find(params.folder).await?;
        Ok(json!({
            "launchers": launchers.iter().map(|l| json!({
                "launcher": l.source.label(),
                "root": l.root,
                "instances": l.instances.iter().map(found_json).collect::<Vec<_>>(),
                "problems": l.problems,
            })).collect::<Vec<_>>(),
        }))
    })
}

#[derive(Deserialize)]
struct RunParams {
    /// The `key`s from `import.detect`.
    instances: Vec<String>,
    #[serde(default)]
    folder: Option<PathBuf>,
    /// Leave the worlds behind.
    #[serde(default)]
    without_worlds: bool,
}

pub fn run(call: Call, cx: &mut App) -> Reply {
    let params: RunParams = params!(call);
    if params.instances.is_empty() {
        return super::ready(Err(api_error(
            codes::INVALID_PARAMS,
            "Name the instances to import by their key from `import detect`.",
        )));
    }
    if let Some(folder) = &params.folder {
        tri!(absolute(folder));
    }
    let control = call.control.clone();
    let (installer, store, api) = {
        let instances = control.instances.read(cx);
        (
            instances.installer(),
            instances.store(),
            instances.modrinth(),
        )
    };
    let slot: Arc<Mutex<Option<Arc<ImportProgress>>>> = Arc::new(Mutex::new(None));
    let stage = Arc::new(Mutex::new(String::new()));
    let cancel = CancelToken::new();
    let (job_slot, job_stage, job_cancel) = (slot.clone(), stage.clone(), cancel.clone());
    let instances = control.instances.clone();
    let count = params.instances.len();
    let work = cx.spawn(async move |cx| {
        let launchers = find(params.folder.clone()).await?;
        let all: Vec<Found> = launchers.into_iter().flat_map(|l| l.instances).collect();
        let mut chosen = Vec::new();
        for key in &params.instances {
            let found = all
                .iter()
                .find(|f| &f.key == key)
                .cloned()
                .ok_or_else(|| {
                    api_error(
                        codes::NOT_FOUND,
                        format!("No instance to import has the key \"{key}\". List them with `import detect`."),
                    )
                })?;
            chosen.push(found);
        }
        let mut imported = Vec::new();
        for (index, found) in chosen.into_iter().enumerate() {
            if cancel.is_cancelled() {
                break;
            }
            if let Ok(mut stage) = stage.lock() {
                *stage = format!("Importing {} ({}/{count})", found.name, index + 1);
            }
            let progress = Arc::new(ImportProgress::default());
            if let Ok(mut slot) = slot.lock() {
                *slot = Some(progress.clone());
            }
            let (installer, store, api, cancel) =
                (installer.clone(), store.clone(), api.clone(), cancel.clone());
            let name = found.name.clone();
            let options = CopyOptions {
                without_worlds: params.without_worlds,
            };
            let result = cx
                .background_spawn(async move {
                    let resolved = import::resolve(&installer, &found).await?;
                    if cancel.is_cancelled() {
                        return Err(import::ImportError::Cancelled);
                    }
                    let request = ImportRequest {
                        found,
                        resolved,
                        options,
                    };
                    import::import(&store, &request, api.as_deref(), &progress, &cancel).await
                })
                .await;
            match result {
                Ok(done) => {
                    let id = done.instance.id.clone();
                    instances.update(cx, |instances, cx| {
                        instances.select(&id, cx);
                        instances.reload(cx);
                    });
                    let mut notes = done.notes;
                    if done.skipped.iter().any(|(_, why)| why.contains("link")) {
                        notes.push("Links to outside the game folder were left out.".into());
                    }
                    imported.push(json!({ "id": id, "name": done.instance.name, "notes": notes, "ok": true }));
                }
                Err(import::ImportError::Cancelled) => {
                    return Err(api_error(codes::CANCELLED, "Cancelled.")
                        .with_data(json!({ "imported": imported })));
                }
                Err(err) => imported.push(json!({ "name": name, "ok": false, "error": err.to_string() })),
            }
        }
        Ok(json!({ "imported": imported }))
    });
    let job = control.start_job(
        JobSpec::new("import", format!("Importing {count} instance(s)"))
            .progress(Progress::Import(job_slot))
            .stage(job_stage)
            .cancel(Cancel::Token(job_cancel)),
        work,
        cx,
    );
    control.reply_job(&call, job, cx)
}
