//! "Download this world", from a server's page: one of the server's backups comes to
//! this computer as a file and becomes a new instance through the snapshot import
//! (`launcher_core::cores::world_download`, then `instances::snapshots`), with the
//! server's world and the mods its players get, never its settings or plugins.
//!
//! The dialog signs in when this computer isn't the core's owner (a Launcher's own key
//! can't read a server's files), lists the backups newest first (or offers to back up now
//! when there are none), then downloads the one picked with a bar and Cancel. The core's
//! file is let go of however the download ends; the downloaded file is deleted once the
//! import dialog it is handed to goes.

use std::sync::Arc;

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    Render, SharedString, Task, Window, div, prelude::*,
};
use launcher_core::{
    Secret,
    cores::{
        CoreClient,
        admin::{AdminClient, backups::BackupInfo},
        hosting::HostAuth,
        world_download::{
            self, DownloadProgress, DownloadStage, Downloaded, WhichBackup, WorldDownloadError,
        },
    },
    game::download::CancelToken,
};

use super::{
    Servers,
    extras::{error_line, label, note, progress_bar},
};
use crate::{
    components::{Button, ButtonVariant, Dialog, ListRow, LoadingDots, TextInput},
    i18n::{CommonMsg, ServersMsg as M, format, t, tr},
    theme::{layout, px},
};

pub enum DownloadWorldEvent {
    Dismissed,
    /// The world came: make an instance from it.
    Downloaded(Arc<Downloaded>),
}

/// Where the dialog is.
enum Step {
    /// The username and password, for a core this computer isn't the owner of.
    SignIn,
    Loading,
    Pick {
        admin: AdminClient,
        backups: Vec<BackupInfo>,
        /// The backup picked; `None` backs up now.
        picked: Option<String>,
    },
    Downloading {
        progress: Arc<DownloadProgress>,
        cancel: CancelToken,
        /// The list it was picked from, to go back to when the download stops.
        list: Picked,
    },
}

/// The signed-in list a download was picked from: Cancel and a failed download go back to
/// it, so a password isn't asked again to try another backup.
#[derive(Clone)]
struct Picked {
    admin: AdminClient,
    backups: Vec<BackupInfo>,
    picked: Option<String>,
}

pub struct DownloadWorldDialog {
    client: CoreClient,
    owner_key: Option<Secret>,
    pub server: String,
    server_name: String,
    core_name: String,
    username: Entity<TextInput>,
    password: Entity<TextInput>,
    /// Where the world is saved: the cache's, emptied at start.
    downloads_dir: std::path::PathBuf,
    step: Step,
    error: Option<SharedString>,
    /// "Back up now" was refused and the server has no backup to pick: Download is off,
    /// so the same refused request isn't sent again.
    cannot_back_up: bool,
    focus: FocusHandle,
    _work: Option<Task<()>>,
    _ticker: Option<Task<()>>,
}

impl EventEmitter<DownloadWorldEvent> for DownloadWorldDialog {}

impl Focusable for DownloadWorldDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl DownloadWorldDialog {
    /// Stops a download at once (the dialog closing: Escape, the close button), so bytes
    /// stop arriving while it fades out; the core's file is let go of.
    pub fn stop_downloading(&self) {
        if let Step::Downloading { cancel, .. } = &self.step {
            cancel.cancel();
        }
    }
}

impl Drop for DownloadWorldDialog {
    fn drop(&mut self) {
        // a download left is stopped, and the core's file let go of
        self.stop_downloading();
    }
}

/// The dialog's text for an error.
fn download_error(err: &WorldDownloadError, core: &str) -> String {
    match err {
        WorldDownloadError::NeedsUpdate => tr!(M::DownloadWorldTooOld, core = core),
        WorldDownloadError::Forbidden => t(M::DownloadWorldForbidden).to_string(),
        WorldDownloadError::BackupForbidden => t(M::DownloadWorldBackupForbidden).to_string(),
        WorldDownloadError::NoRoom { needed, free } => tr!(
            crate::i18n::InstanceSettingsMsg::ImportNoRoom,
            size = format::bytes(*needed),
            free = format::bytes(*free)
        ),
        WorldDownloadError::NoBackups => t(M::DownloadWorldNoBackups).to_string(),
        WorldDownloadError::BackupFailed(why) => tr!(
            M::DownloadWorldBackupFailed,
            error = launcher_core::images::display_text(why)
        ),
        WorldDownloadError::Cancelled => t(CommonMsg::Cancel).to_string(),
        // the core's own text (a 404 it explains, say) is shown on one line, escaped
        WorldDownloadError::Core(err) => {
            launcher_core::images::display_text(&super::core_error(err)).into_owned()
        }
    }
}

impl DownloadWorldDialog {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        client: CoreClient,
        owner_key: Option<Secret>,
        server: String,
        server_name: String,
        core_name: String,
        username: Option<String>,
        downloads_dir: std::path::PathBuf,
        cx: &mut Context<Self>,
    ) -> Self {
        let owner = owner_key.is_some();
        let mut this = Self {
            client,
            owner_key,
            server,
            server_name,
            core_name,
            username: cx.new(|cx| {
                let input = TextInput::new(cx)
                    .with_placeholder(M::Username)
                    .with_automation_id("download-world-username");
                match username {
                    Some(username) => input.with_text(username),
                    None => input,
                }
            }),
            password: cx.new(|cx| {
                TextInput::new(cx)
                    .with_placeholder(M::Password)
                    .with_automation_id("download-world-password")
                    .with_masked()
            }),
            downloads_dir,
            step: Step::SignIn,
            error: None,
            cannot_back_up: false,
            focus: cx.focus_handle(),
            _work: None,
            _ticker: None,
        };
        if owner {
            this.sign_in(cx);
        }
        this
    }

    /// Signs in (the owner key, or a session from the password) and lists the backups.
    fn sign_in(&mut self, cx: &mut Context<Self>) {
        let auth = match &self.owner_key {
            Some(key) => HostAuth::OwnerKey(key.clone()),
            None => {
                let username = self.username.read(cx).text().trim().to_string();
                let password = self.password.read(cx).text().to_string();
                if username.is_empty() || password.is_empty() {
                    self.error = Some(t(M::DownloadWorldNeedsPassword).into());
                    cx.notify();
                    return;
                }
                self.password
                    .update(cx, |input, cx| input.set_text(String::new(), cx));
                HostAuth::Password {
                    username,
                    password: Secret::new(password),
                }
            }
        };
        self.error = None;
        self.step = Step::Loading;
        let (client, server, core) = (
            self.client.clone(),
            self.server.clone(),
            self.core_name.clone(),
        );
        let work = cx.background_spawn(async move {
            let admin = auth.admin(client).await.map_err(WorldDownloadError::from)?;
            let backups = world_download::backups(&admin, &server).await?;
            Ok::<_, WorldDownloadError>((admin, backups))
        });
        self._work = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok((admin, backups)) => {
                        let picked = backups.first().map(|b| b.id.clone());
                        this.step = Step::Pick {
                            admin,
                            backups,
                            picked,
                        };
                    }
                    Err(err) => this.fail(download_error(&err, &core), cx),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn fail(&mut self, error: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.step = Step::SignIn;
        self._ticker = None;
        self.error = Some(error.into());
        cx.notify();
    }

    /// Back to the list a download was picked from, saying why it stopped. When "Back up
    /// now" made a backup first (`made`), the list is read again with it, and it is picked:
    /// trying again downloads it instead of making another.
    fn back_to_list(
        &mut self,
        error: impl Into<SharedString>,
        made: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self._ticker = None;
        let step = std::mem::replace(&mut self.step, Step::SignIn);
        self.step = match step {
            Step::Downloading { list, .. } => Step::Pick {
                admin: list.admin,
                backups: list.backups,
                picked: made.clone().or(list.picked),
            },
            other => other,
        };
        self.error = Some(error.into());
        if let (Some(made), Step::Pick { admin, .. }) = (made, &self.step) {
            let (admin, server) = (admin.clone(), self.server.clone());
            let read =
                cx.background_spawn(async move { world_download::backups(&admin, &server).await });
            // detached: this runs inside the stopped download's task, which `_work` holds
            cx.spawn(async move |this, cx| {
                let Ok(list) = read.await else {
                    return;
                };
                this.update(cx, |this, cx| {
                    if let Step::Pick {
                        backups, picked, ..
                    } = &mut this.step
                        && picked.as_deref() == Some(made.as_str())
                    {
                        *backups = list;
                        cx.notify();
                    }
                })
                .ok();
            })
            .detach();
        }
        cx.notify();
    }

    /// "Back up now" was refused (the user may read the server's files, not back it up):
    /// the newest backup is picked, so Download doesn't send the same refused request; on
    /// a server with no backup, Download is turned off and the message says to ask its
    /// owner.
    fn after_backup_refused(&mut self) {
        if let Step::Pick {
            backups, picked, ..
        } = &mut self.step
            && picked.is_none()
        {
            *picked = backups.first().map(|b| b.id.clone());
            if picked.is_none() {
                self.cannot_back_up = true;
                self.error = Some(t(M::DownloadWorldNothingToDownload).into());
            }
        }
    }

    fn download(&mut self, cx: &mut Context<Self>) {
        let Step::Pick {
            admin,
            backups,
            picked,
        } = &self.step
        else {
            return;
        };
        if picked.is_none() && self.cannot_back_up {
            return;
        }
        let which = match picked {
            Some(id) => WhichBackup::Id(id.clone()),
            None => WhichBackup::New,
        };
        let list = Picked {
            admin: admin.clone(),
            backups: backups.clone(),
            picked: picked.clone(),
        };
        let admin = admin.clone();
        let progress = Arc::new(DownloadProgress::default());
        let cancel = CancelToken::new();
        let dir = self.downloads_dir.clone();
        self.step = Step::Downloading {
            progress: progress.clone(),
            cancel: cancel.clone(),
            list,
        };
        self.error = None;
        let (server, core) = (self.server.clone(), self.core_name.clone());
        let made = progress.clone();
        let work = cx.background_spawn({
            let cancel = cancel.clone();
            async move {
                world_download::download(
                    &admin,
                    &server,
                    which,
                    &dir,
                    &progress,
                    &cancel,
                    &launcher_core::cores::player_image::tokio_sleep(),
                )
                .await
            }
        });
        self._ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(250))
                    .await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        }));
        self._work = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| match result {
                Ok(downloaded) => {
                    this._ticker = None;
                    cx.emit(DownloadWorldEvent::Downloaded(Arc::new(downloaded)));
                }
                Err(WorldDownloadError::Cancelled) => {
                    this.back_to_list(t(M::DownloadWorldCancelled), made.backup_made(), cx)
                }
                Err(err) => {
                    let refused = matches!(err, WorldDownloadError::BackupForbidden);
                    this.back_to_list(download_error(&err, &core), made.backup_made(), cx);
                    if refused {
                        this.after_backup_refused();
                    }
                }
            })
            .ok();
        }));
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        match &self.step {
            Step::Downloading { cancel, .. } => cancel.cancel(),
            _ => cx.emit(DownloadWorldEvent::Dismissed),
        }
    }

    fn pick(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        if let Step::Pick { picked, .. } = &mut self.step {
            *picked = id;
        }
        cx.notify();
    }

    fn render_pick(
        &self,
        backups: &[BackupInfo],
        picked: Option<&str>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let entity = cx.entity();
        let mut rows: Vec<gpui::AnyElement> = backups
            .iter()
            .enumerate()
            .map(|(i, backup)| {
                let id = backup.id.clone();
                let when = chrono::DateTime::from_timestamp(backup.created, 0)
                    .map(|time| {
                        let time = time.with_timezone(&chrono::Local);
                        format::day_and_time(time, chrono::Local::now().date_naive())
                    })
                    .unwrap_or_else(|| backup.id.clone());
                let name = match &backup.name {
                    Some(name) => format!("{name} · {when}"),
                    None => when,
                };
                let mut detail = format::bytes(backup.total_bytes);
                if backup.may_be_inconsistent() {
                    detail = format!("{detail} · {}", t(M::DownloadWorldMidWrite));
                }
                let entity = entity.clone();
                ListRow::new(
                    SharedString::from(format!("download-world-backup-{i}")),
                    name,
                )
                .detail(detail)
                .selected(picked == Some(id.as_str()))
                .on_click(move |_, _, cx| {
                    let id = id.clone();
                    entity.update(cx, |this, cx| this.pick(Some(id), cx))
                })
                .into_any_element()
            })
            .collect();
        rows.push(
            ListRow::new("download-world-backup-new", t(M::DownloadWorldBackUpNow))
                .detail(t(M::DownloadWorldBackUpNowHint))
                .selected(picked.is_none())
                .on_click(move |_, _, cx| entity.update(cx, |this, cx| this.pick(None, cx)))
                .into_any_element(),
        );
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(note(tr!(M::DownloadWorldIntro, server = self.server_name)))
            .child(label(t(M::DownloadWorldBackups)))
            .child(div().flex().flex_col().children(rows))
            .children(self.error.clone().map(error_line))
            .into_any_element()
    }
}

impl Render for DownloadWorldDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let dialog = Dialog::new(
            "download-world",
            tr!(M::DownloadWorldTitle, server = self.server_name),
            self.focus.clone(),
        )
        .width(layout::dialog_width() + px(80.))
        .on_close(move |_, cx| entity.update(cx, |_, cx| cx.emit(DownloadWorldEvent::Dismissed)));
        let cancel = Button::new("download-world-cancel", t(CommonMsg::Cancel))
            .on_click(cx.listener(|this, _, _, cx| this.cancel(cx)));
        match &self.step {
            Step::SignIn => dialog
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(note(tr!(M::DownloadWorldIntro, server = self.server_name)))
                        .when(self.owner_key.is_none(), |this| {
                            this.child(note(t(M::DownloadWorldSignIn)))
                                .child(self.username.clone())
                                .child(self.password.clone())
                                .child(note(t(M::PasswordNotKept)))
                        })
                        .children(self.error.clone().map(error_line)),
                )
                .footer(
                    Button::new("download-world-sign-in", t(CommonMsg::Continue))
                        .variant(ButtonVariant::Primary)
                        .on_click(cx.listener(|this, _, _, cx| this.sign_in(cx))),
                )
                .footer(cancel)
                .into_any_element(),
            Step::Loading => dialog
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(LoadingDots)
                        .child(note(t(CommonMsg::Loading))),
                )
                .footer(cancel)
                .into_any_element(),
            Step::Pick {
                backups, picked, ..
            } => {
                let (backups, picked) = (backups.clone(), picked.clone());
                let nothing = picked.is_none() && self.cannot_back_up;
                dialog
                    .child(self.render_pick(&backups, picked.as_deref(), cx))
                    .footer(
                        Button::new("download-world-download", t(M::DownloadWorldButton))
                            .variant(ButtonVariant::Primary)
                            .disabled(nothing)
                            .on_click(cx.listener(|this, _, _, cx| this.download(cx))),
                    )
                    .footer(cancel)
                    .into_any_element()
            }
            Step::Downloading { progress, .. } => {
                let snapshot = progress.bytes.snapshot();
                let text = match progress.stage() {
                    DownloadStage::BackingUp => t(M::DownloadWorldBackingUp).to_string(),
                    DownloadStage::Preparing => t(M::DownloadWorldPreparing).to_string(),
                    DownloadStage::Downloading => tr!(
                        M::DownloadWorldDownloading,
                        done = format::bytes(snapshot.bytes_done),
                        total = format::bytes(snapshot.bytes_total)
                    ),
                };
                let fraction = (snapshot.bytes_total > 0)
                    .then(|| snapshot.bytes_done as f32 / snapshot.bytes_total as f32);
                dialog
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(LoadingDots)
                                    .child(note(text)),
                            )
                            .children(fraction.map(progress_bar)),
                    )
                    .footer(cancel)
                    .into_any_element()
            }
        }
    }
}

impl Servers {
    /// Opens "Download this world" for the server `server` on core `core_uuid`.
    pub fn open_download_world(
        &mut self,
        core_uuid: &str,
        server: &str,
        server_name: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.core_index(core_uuid) else {
            return;
        };
        let core = &self.cores[index];
        let client = self.client(core);
        let owner_key = self.owner_key(core_uuid);
        let (core_name, username) = (core.saved.name.clone(), core.saved.username.clone());
        let dir = self.instances.read(cx).world_downloads_dir();
        let (server, server_name) = (server.to_string(), server_name.to_string());
        let dialog = cx.new(|cx| {
            DownloadWorldDialog::new(
                client,
                owner_key,
                server,
                server_name.clone(),
                core_name,
                username,
                dir,
                cx,
            )
        });
        let subscription = cx.subscribe(&dialog, move |this, dialog, event, cx| {
            if this.extras.download_world.current().map(|(open, _)| open) != Some(&dialog) {
                return;
            }
            match event {
                DownloadWorldEvent::Dismissed => {
                    dialog.read(cx).stop_downloading();
                    this.extras.download_world.close(cx);
                }
                DownloadWorldEvent::Downloaded(file) => {
                    this.extras.download_world.close(cx);
                    cx.emit(super::ServersEvent::ImportWorld {
                        file: file.clone(),
                        server: server_name.clone(),
                    });
                }
            }
        });
        self.extras.download_world.open((dialog, subscription), cx);
    }

    /// Shows "Download this world" at a step, for the offscreen screenshots: `pick` (the
    /// backups listed), `progress` (half of a 180 MB world downloaded) or
    /// `backup-forbidden` (the list again after "Back up now" was refused) or
    /// `backup-forbidden-none` (refused on a server with no backups: nothing to download).
    pub fn preview_download_world(&mut self, step: &str, cx: &mut Context<Self>) {
        let Some(core) = self.cores.first() else {
            return;
        };
        let core_uuid = core.saved.uuid.clone();
        let server = core.servers().first().cloned();
        let (uuid, name) = server
            .map(|s| (s.uuid, s.name))
            .unwrap_or_else(|| ("smp".into(), "Survival SMP".into()));
        let admin = AdminClient::new(self.client(core), Secret::new("preview"));
        self.open_download_world(&core_uuid, &uuid, &name, cx);
        let Some((dialog, _)) = self.extras.download_world.current() else {
            return;
        };
        dialog.update(cx, |d, cx| {
            d._work = None;
            let backup = |id: &str, name: Option<&str>, created: i64, live: bool| -> BackupInfo {
                serde_json::from_value(serde_json::json!({
                    "id": id, "name": name, "created": created, "live": live,
                    "quiesced": !live, "total_bytes": 182_000_000u64
                }))
                .expect("a sample backup")
            };
            match step {
                "progress" => {
                    d.step = Step::Downloading {
                        progress: Arc::new(DownloadProgress::preview_downloading()),
                        cancel: CancelToken::new(),
                        list: Picked {
                            admin,
                            backups: Vec::new(),
                            picked: None,
                        },
                    };
                }
                _ => {
                    // `backup-forbidden`: "Back up now" refused, back at the list
                    let refused = step.starts_with("backup-forbidden");
                    d.step = Step::Pick {
                        admin,
                        backups: if step == "backup-forbidden-none" {
                            Vec::new()
                        } else {
                            vec![
                                backup("b2", None, 1_794_700_000, true),
                                backup("b1", Some("Before the End fight"), 1_794_300_000, false),
                            ]
                        },
                        picked: if refused { None } else { Some("b2".into()) },
                    };
                    if refused {
                        d.error = Some(
                            download_error(&WorldDownloadError::BackupForbidden, &d.core_name)
                                .into(),
                        );
                        d.after_backup_refused();
                    }
                }
            }
            cx.notify();
        });
    }
}
