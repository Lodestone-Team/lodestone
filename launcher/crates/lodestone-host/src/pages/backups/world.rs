//! The Snapshots tab's World backups section: plain `.zip` files of the server's world,
//! made only when asked (`launcher_core::cores::admin::world_backups`). Backing up the
//! world, the list with each one's size and how it was made, restoring one into the
//! stopped server (after a safety world backup the core takes), downloading the stored
//! zip with its progress and Cancel, and deleting.
//!
//! A world job shares the server's one job with snapshots: it shows in the page's job
//! card, which has a Cancel for world jobs. Backing up needs the instance's Settings
//! permission, restoring and deleting its file write permission, as for snapshots.

use std::{path::PathBuf, sync::Arc, time::Duration};

use gpui::{
    AnyElement, AppContext as _, Context, Entity, FocusHandle, IntoElement, ParentElement,
    SharedString, Styled, div, prelude::*, relative,
};
use launcher_core::cores::admin::{
    backups::JobKind,
    models::InstancePermission,
    world_backups::{WorldBackupInfo, WorldBackupKind, WorldBackupList},
};
use launcher_core::game::download::{CancelToken, Progress};
use lodestone_ui::{
    components::{
        Button, ButtonSize, ButtonVariant, Dialog, Icon, IconButton, TextInput, Toast,
        settings::{self, SettingRow},
    },
    file_dialog,
    theme::{colors, ink, leading, line_height, px, text},
};

use super::parts::Confirm;
use super::{BackupsPage, ExportRun, Job, parts, tag_with_tip, taken_at, why_not};
use crate::i18n::{BackupsMsg as M, BackupsPlural as P, CommonMsg, format, t, tr, trn};

pub(super) struct CreatingWorld {
    pub focus: FocusHandle,
    pub name: Entity<TextInput>,
    pub busy: bool,
    pub error: Option<String>,
}

pub(super) struct RestoringWorld {
    pub backup: WorldBackupInfo,
    pub label: String,
    pub focus: FocusHandle,
    pub busy: bool,
    pub error: Option<String>,
}

pub(super) struct DeletingWorld {
    pub backup: WorldBackupInfo,
    pub label: String,
    pub focus: FocusHandle,
    pub busy: bool,
}

/// Text from the core shown as it is, with control and direction characters written
/// visibly: the Host can be signed in to another person's core.
fn shown(text: &str) -> String {
    launcher_core::images::display_text(text).into_owned()
}

/// A world backup's name, or when it was made.
pub fn world_label(backup: &WorldBackupInfo, now: chrono::DateTime<chrono::Local>) -> String {
    match backup.name.as_deref().map(str::trim) {
        Some(name) if !name.is_empty() => shown(name),
        _ => taken_at(backup.created, now),
    }
}

/// The name a world backup's zip is saved under: the server's name, the world's and the
/// backup's id, without characters file systems refuse.
pub fn world_download_name(server: &str, level: &str, backup_id: &str) -> String {
    let clean: String = format!("{server}-{level}-{backup_id}")
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    format!("{}.zip", clean.trim().trim_end_matches('.'))
}

/// Whether a world backup may be restored now: never into a running server (the core
/// refuses it), and the dialog says so.
pub fn may_restore_into(state: launcher_core::cores::admin::models::InstanceState) -> bool {
    !state.is_live()
}

impl BackupsPage {
    // ---- Backing up the world ----

    pub(super) fn open_world_create(&mut self, cx: &mut Context<Self>) {
        let name = cx.new(|cx| {
            TextInput::new(cx)
                .with_label(t(M::WorldName))
                .with_automation_id("world-backup-name")
        });
        cx.observe(&name, |_, _, cx| cx.notify()).detach();
        let focus = cx.focus_handle();
        self.world_creating.open(
            CreatingWorld {
                focus,
                name,
                busy: false,
                error: None,
            },
            cx,
        );
    }

    fn create_world_backup(&mut self, cx: &mut Context<Self>) {
        let Some(shown) = self.world_creating.current_mut() else {
            return;
        };
        if shown.busy {
            return;
        }
        let typed = shown.name.read(cx).text().trim().to_string();
        let name = (!typed.is_empty()).then_some(typed);
        shown.busy = true;
        shown.error = None;
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        parts::spawn(
            cx,
            async move { admin.create_world_backup(&uuid, name.as_deref()).await },
            |this, result, cx| match result {
                Ok(task_id) => {
                    this.world_creating.close(cx);
                    if this.job.is_none() {
                        this.job = Some(Job {
                            task_id,
                            kind: JobKind::WorldBackup,
                            progress: None,
                            message: String::new(),
                        });
                    }
                    this.reload(cx);
                }
                Err(err) => {
                    if let Some(shown) = this.world_creating.current_mut() {
                        shown.busy = false;
                        shown.error = Some(err.to_string());
                    }
                }
            },
        );
    }

    /// Cancel on the job card, for a world job. A restore already putting the world in
    /// place can't stop: the core says so (409), and a toast says it.
    pub(super) fn cancel_world_job(&mut self, cx: &mut Context<Self>) {
        let Some(job) = &self.job else {
            return;
        };
        if self.cancelling || !matches!(job.kind, JobKind::WorldBackup | JobKind::WorldRestore) {
            return;
        }
        self.cancelling = true;
        cx.notify();
        let task_id = job.task_id;
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        parts::spawn(
            cx,
            async move { admin.cancel_world_backup_task(&uuid, task_id).await },
            |this, result, cx| {
                this.cancelling = false;
                if let Err(err) = result {
                    this.toast(Toast::error(tr!(M::WorldCancelFailed, error = err)), cx);
                }
                // the job's end says it stopped
                this.reload(cx);
            },
        );
    }

    // ---- A world backup's actions ----

    fn open_world_restore(&mut self, backup: &WorldBackupInfo, cx: &mut Context<Self>) {
        let focus = cx.focus_handle();
        self.world_restoring.open(
            RestoringWorld {
                backup: backup.clone(),
                label: world_label(backup, chrono::Local::now()),
                focus,
                busy: false,
                error: None,
            },
            cx,
        );
    }

    fn restore_world(&mut self, cx: &mut Context<Self>) {
        let state = self.instance_state(cx);
        let Some(shown) = self.world_restoring.current_mut() else {
            return;
        };
        if shown.busy || !may_restore_into(state) {
            return;
        }
        shown.busy = true;
        shown.error = None;
        let (id, label) = (shown.backup.id.clone(), shown.label.clone());
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        parts::spawn(
            cx,
            async move { admin.restore_world_backup(&uuid, &id).await },
            move |this, result, cx| match result {
                Ok(task_id) => {
                    this.world_restoring.close(cx);
                    this.toast(Toast::info(tr!(M::WorldRestoreStarted, name = label)), cx);
                    this.job = Some(Job {
                        task_id,
                        kind: JobKind::WorldRestore,
                        progress: None,
                        message: String::new(),
                    });
                    this.reload(cx);
                }
                // a 403 says what the restore would change that the user may not, a 409
                // that the server runs or another job does
                Err(err) => {
                    if let Some(shown) = this.world_restoring.current_mut() {
                        shown.busy = false;
                        shown.error = Some(err.to_string());
                    }
                }
            },
        );
    }

    fn open_world_delete(&mut self, backup: &WorldBackupInfo, cx: &mut Context<Self>) {
        let focus = cx.focus_handle();
        self.world_deleting.open(
            DeletingWorld {
                backup: backup.clone(),
                label: world_label(backup, chrono::Local::now()),
                focus,
                busy: false,
            },
            cx,
        );
    }

    fn delete_world(&mut self, cx: &mut Context<Self>) {
        let Some(shown) = self.world_deleting.current_mut() else {
            return;
        };
        if shown.busy {
            return;
        }
        shown.busy = true;
        let id = shown.backup.id.clone();
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        parts::spawn(
            cx,
            {
                let id = id.clone();
                async move { admin.delete_world_backup(&uuid, &id).await }
            },
            move |this, result, cx| match result {
                Ok(()) => {
                    this.world_deleting.close(cx);
                    if let Some(list) = &mut this.world_list {
                        list.backups.retain(|b| b.id != id);
                    }
                    this.toast(Toast::success(t(M::WorldDeleted)), cx);
                    this.reload(cx);
                }
                Err(err) => {
                    if let Some(shown) = this.world_deleting.current_mut() {
                        shown.busy = false;
                    }
                    this.toast(Toast::error(err.to_string()), cx);
                }
            },
        );
    }

    /// Asks where to save the world backup's zip, then downloads the stored zip. The row
    /// shows its progress and a Cancel while the page shows it. Like a snapshot's
    /// download, the task outlives the page and says how it went through the Host's
    /// toasts.
    fn download_world(&mut self, backup: &WorldBackupInfo, cx: &mut Context<Self>) {
        if self.world_downloads.contains_key(&backup.id) {
            return;
        }
        let downloads = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        let name = world_download_name(&self.instance_name(cx), &backup.level, &backup.id);
        let id = backup.id.clone();
        let state = self.state.downgrade();
        cx.spawn(async move |this, cx| {
            let dialog = cx.update(|cx| cx.prompt_for_new_path(&downloads, Some(&name)));
            let Some(out) = file_dialog::chosen(dialog, &this, cx).await else {
                return;
            };
            let run = ExportRun {
                progress: Arc::new(Progress::default()),
                cancel: CancelToken::new(),
            };
            let Ok((admin, uuid)) = this.update(cx, |this, cx| {
                this.world_downloads.insert(id.clone(), run.clone());
                cx.notify();
                (this.state.read(cx).admin.clone(), this.uuid.clone())
            }) else {
                return;
            };
            // the row's bar moves while the page shows this download
            let ticker = cx.spawn({
                let (this, id, progress) = (this.clone(), id.clone(), run.progress.clone());
                async move |cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(250))
                            .await;
                        let shown = this.update(cx, |this, cx| {
                            let shown = this
                                .world_downloads
                                .get(&id)
                                .is_some_and(|run| Arc::ptr_eq(&run.progress, &progress));
                            if shown {
                                cx.notify();
                            }
                            shown
                        });
                        if !matches!(shown, Ok(true)) {
                            break;
                        }
                    }
                }
            });
            let result = cx
                .background_spawn({
                    let (out, id, run) = (out.clone(), id.clone(), run.clone());
                    async move {
                        admin
                            .download_world_backup_to(&uuid, &id, &out, &run.progress, &run.cancel)
                            .await
                    }
                })
                .await;
            drop(ticker);
            let toast = match result {
                Ok(_) => {
                    let file = out
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    Toast::success(tr!(M::Downloaded, name = file))
                        .action(t(CommonMsg::ShowInFileManager), move |_, cx| {
                            cx.reveal_path(&out)
                        })
                }
                Err(_) if run.cancel.is_cancelled() => Toast::info(t(M::WorldDownloadCancelled)),
                Err(err) => Toast::error(tr!(M::WorldDownloadFailed, error = err)),
            };
            state.update(cx, |state, cx| state.toast(toast, cx)).ok();
            this.update(cx, |this, cx| {
                this.world_downloads.remove(&id);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn cancel_world_download(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(run) = self.world_downloads.get(id) {
            run.cancel.cancel();
        }
        cx.notify();
    }

    // ---- Rendering ----

    /// The section: its heading and lead with "Back up world…" beside them, the space
    /// the world backups use, and the list.
    pub(super) fn render_world_section(&self, cx: &mut Context<Self>) -> AnyElement {
        let may_settings = self.may(InstancePermission::Settings, cx);
        let why = if !may_settings {
            Some(t(M::NoSettingsPermission))
        } else if self.job.is_some() {
            Some(t(M::JobRunning))
        } else {
            None
        };
        let button = Button::new("world-backup-now", t(M::WorldBackUp))
            .variant(ButtonVariant::Primary)
            .icon(Icon::FloppyDisk)
            .disabled(why.is_some() || self.world_list.is_none())
            .on_click(cx.listener(|this, _, _, cx| this.open_world_create(cx)));
        let header = super::section_header(
            t(M::WorldSection),
            t(M::WorldLead),
            why_not("world-backup-now-why", why, button),
        );
        let mut section = div().flex().flex_col().gap(px(8.)).child(header);
        if let Some(error) = &self.world_error {
            section = section.child(parts::error(tr!(M::WorldLoadFailed, error = error)));
        }
        if let Some(list) = &self.world_list {
            section = section
                .child(world_usage(list))
                .child(self.render_world_list(list, cx));
        }
        section.into_any_element()
    }

    fn render_world_list(&self, list: &WorldBackupList, cx: &mut Context<Self>) -> AnyElement {
        let rows: Vec<AnyElement> = if list.backups.is_empty() {
            vec![parts::empty_row(t(M::WorldEmpty))]
        } else {
            list.backups
                .iter()
                .map(|backup| self.world_row(backup, cx))
                .collect()
        };
        settings::card(rows).into_any_element()
    }

    fn world_row(&self, backup: &WorldBackupInfo, cx: &mut Context<Self>) -> AnyElement {
        let now = chrono::Local::now();
        let id = |part: &str| SharedString::from(format!("world-backup-{}-{part}", backup.id));
        let may_write = self.may(InstancePermission::WriteFiles, cx);
        let restore_why = if !may_write {
            Some(t(M::NoWritePermission))
        } else if self.job.is_some() {
            Some(t(M::JobRunning))
        } else {
            None
        };
        // the core refuses a delete while a world job runs (it may be reading the zip)
        let world_job = self
            .job
            .as_ref()
            .is_some_and(|job| matches!(job.kind, JobKind::WorldBackup | JobKind::WorldRestore));
        let delete_why = if !may_write {
            Some(t(M::NoWritePermission))
        } else if world_job {
            Some(t(M::JobRunning))
        } else {
            None
        };

        let before_restoring = backup
            .before_restoring
            .as_deref()
            .map(|name| tr!(M::WorldBeforeRestoreHint, name = shown(name)));
        let mut tags = Vec::new();
        match backup.kind {
            WorldBackupKind::BeforeRestore => tags.push(tag_with_text_tip(
                id("kind"),
                t(M::TagBeforeRestore),
                before_restoring,
                colors::VIOLET,
            )),
            WorldBackupKind::Manual => tags.push(tag_with_tip(
                id("kind"),
                t(M::TagManual),
                None,
                colors::GRAY_400,
            )),
            // a kind a newer core added: no tag rather than a wrong one
            WorldBackupKind::Unknown => {}
        }
        if backup.live {
            tags.push(tag_with_tip(
                id("live"),
                t(M::TagLive),
                Some(t(M::TagLiveHint)),
                colors::GREEN,
            ));
        }
        if backup.may_be_inconsistent() {
            tags.push(tag_with_tip(
                id("inconsistent"),
                t(M::TagInconsistent),
                Some(t(M::WorldTagInconsistentHint)),
                colors::YELLOW,
            ));
        }
        if backup.left_out_count > 0 {
            tags.push(tag_with_tip(
                id("left-out"),
                &trn!(P::WorldLeftOut, backup.left_out_count),
                Some(t(M::WorldLeftOutHint)),
                colors::YELLOW,
            ));
        }

        let hint = if backup.game_version.is_empty() {
            format::bytes(backup.size)
        } else {
            tr!(
                M::WorldSizeVersion,
                size = format::bytes(backup.size),
                version = shown(&backup.game_version)
            )
        };
        let restore = {
            let b = backup.clone();
            Button::new(id("restore"), t(M::RestoreAction))
                .size(ButtonSize::Slim)
                .disabled(restore_why.is_some())
                .on_click(cx.listener(move |this, _, _, cx| this.open_world_restore(&b, cx)))
        };
        let download: AnyElement = match self.world_downloads.get(&backup.id) {
            Some(run) => {
                let snapshot = run.progress.snapshot();
                let share = if snapshot.bytes_total == 0 {
                    0.0
                } else {
                    (snapshot.bytes_done as f64 / snapshot.bytes_total as f64).clamp(0.0, 1.0)
                };
                let backup_id = backup.id.clone();
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .w(px(72.))
                            .h(px(4.))
                            .overflow_hidden()
                            .rounded_full()
                            .bg(colors::GRAY_700)
                            .child(
                                div()
                                    .h_full()
                                    .rounded_full()
                                    .bg(colors::BLUE)
                                    .w(relative((share as f32).max(0.02))),
                            ),
                    )
                    .child(
                        div()
                            .text_size(text::SMALL)
                            .text_color(ink::muted())
                            .child(format::percent((share * 100.0).round() as u64)),
                    )
                    .child(
                        IconButton::new(id("download-cancel"), Icon::Xmark)
                            .label(t(CommonMsg::Cancel))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.cancel_world_download(&backup_id, cx)
                            })),
                    )
                    .into_any_element()
            }
            None => {
                let b = backup.clone();
                IconButton::new(id("download"), Icon::Download)
                    .label(t(M::WorldDownloadLabel))
                    .on_click(cx.listener(move |this, _, _, cx| this.download_world(&b, cx)))
                    .into_any_element()
            }
        };
        let delete = {
            let b = backup.clone();
            IconButton::new(id("delete"), Icon::TrashCan)
                .label(t(M::WorldDeleteLabel))
                .color(colors::RED_200)
                .disabled(delete_why.is_some())
                .on_click(cx.listener(move |this, _, _, cx| this.open_world_delete(&b, cx)))
        };
        SettingRow::new(world_label(backup, now))
            .key(SharedString::from(format!("world-backup-{}", backup.id)))
            .hint(hint)
            .child(why_not(id("restore-why"), restore_why, restore))
            .child(download)
            .child(why_not(id("delete-why"), delete_why, delete))
            .below(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(6.))
                    .children(tags),
            )
            .into_any_element()
    }

    pub(super) fn render_world_create(
        &self,
        shown: &CreatingWorld,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let close = cx.entity().downgrade();
        let cancel = close.clone();
        Dialog::new(
            "world-backup-create",
            t(M::WorldCreateTitle),
            shown.focus.clone(),
        )
        .on_close(move |_, cx| {
            close
                .update(cx, |this, cx| {
                    this.world_creating.close(cx);
                })
                .ok();
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(parts::paragraph(t(M::WorldCreateBody)))
                .child(parts::field(t(M::WorldName), &shown.name))
                .children(shown.error.clone().map(parts::error)),
        )
        .footer(
            Button::new(("world-backup-create", 1usize), t(M::WorldCreate))
                .variant(ButtonVariant::Primary)
                .loading(shown.busy)
                .disabled(shown.busy)
                .on_click(cx.listener(|this, _, _, cx| this.create_world_backup(cx))),
        )
        .footer(
            Button::new(("world-backup-create", 2usize), t(CommonMsg::Cancel))
                .disabled(shown.busy)
                .on_click(move |_, _, cx| {
                    cancel
                        .update(cx, |this, cx| {
                            this.world_creating.close(cx);
                        })
                        .ok();
                }),
        )
        .into_any_element()
    }

    pub(super) fn render_world_restore(
        &self,
        shown: &RestoringWorld,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let may = may_restore_into(self.instance_state(cx));
        // a job that started after the dialog opened (a scheduled snapshot) holds the lock
        let job_running = self.job.is_some();
        let close = cx.entity().downgrade();
        let cancel = close.clone();
        Dialog::new(
            "world-backup-restore",
            tr!(M::WorldRestoreTitle, name = shown.label),
            shown.focus.clone(),
        )
        .on_close(move |_, cx| {
            close
                .update(cx, |this, cx| {
                    this.world_restoring.close(cx);
                })
                .ok();
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    div()
                        .text_size(text::SMALL)
                        .line_height(line_height(text::SMALL, leading::NORMAL))
                        .text_color(ink::muted())
                        .child(t(M::WorldRestoreBody)),
                )
                .when(!may, |this| this.child(parts::warning(t(M::WorldMustStop))))
                .when(may && job_running, |this| {
                    this.child(parts::warning(t(M::JobRunning)))
                })
                .child(parts::paragraph(t(M::WorldSafetyNote)))
                .children(shown.error.clone().map(parts::error)),
        )
        .footer(
            Button::new(("world-backup-restore", 1usize), t(M::Restore))
                .variant(ButtonVariant::Danger)
                .loading(shown.busy)
                .disabled(shown.busy || !may || job_running)
                .on_click(cx.listener(|this, _, _, cx| this.restore_world(cx))),
        )
        .footer(
            Button::new(("world-backup-restore", 2usize), t(CommonMsg::Cancel))
                .disabled(shown.busy)
                .on_click(move |_, _, cx| {
                    cancel
                        .update(cx, |this, cx| {
                            this.world_restoring.close(cx);
                        })
                        .ok();
                }),
        )
        .into_any_element()
    }

    pub(super) fn render_world_delete(
        &self,
        shown: &DeletingWorld,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let this = cx.entity().downgrade();
        let close = this.clone();
        Confirm {
            id: "delete-world-backup",
            title: t(M::WorldDeleteTitle).into(),
            body: tr!(M::WorldDeleteBody, name = shown.label).into(),
            action: t(CommonMsg::Delete).into(),
            danger: true,
            busy: shown.busy,
        }
        .render(
            shown.focus.clone(),
            move |_, cx| {
                this.update(cx, |this, cx| this.delete_world(cx)).ok();
            },
            move |_, cx| {
                close
                    .update(cx, |this, cx| {
                        this.world_deleting.close(cx);
                    })
                    .ok();
            },
        )
        .into_any_element()
    }

    /// Sample world backups, for the development screenshots' `--demo`.
    #[cfg(feature = "screenshot")]
    pub(super) fn demo_world_list(now: i64) -> WorldBackupList {
        serde_json::from_value(serde_json::json!({
            "backups": [
                {"id": "20261112-143000-1a2b", "name": "Before the raid", "kind": "manual",
                 "created": now - 30 * 60, "level": "world", "size": 182_000_000u64,
                 "file_count": 1840, "game_version": "1.21.4", "live": true,
                 "quiesced": true},
                {"id": "20261111-090000-3c4d", "name": "Before restoring “Spawn built”",
                 "kind": "before_restore", "created": now - 30 * 3600, "level": "world",
                 "size": 176_000_000u64, "file_count": 1822, "game_version": "1.21.4",
                 "before_restoring": "Spawn built"},
                {"id": "20261108-200000-5e6f", "kind": "manual", "created": now - 4 * 86_400,
                 "level": "world", "size": 171_000_000u64, "file_count": 1790,
                 "left_out_count": 2, "game_version": "1.21.3", "live": true,
                 "quiesced": false},
            ],
            "usage": {"total_bytes": 529_000_000u64, "disk_free": 182_000_000_000u64},
        }))
        .expect("the demo world backups")
    }

    /// A World backups scene for the development screenshots' `--world-backup` (with
    /// `--tab backups --demo`): `create`, `restore`, `restore-running` (the server runs),
    /// `restore-busy` (a job runs), `delete`, `running` (a world backup at 37 % with its
    /// Cancel) or `download` (a row downloading, at 55 %).
    #[cfg(feature = "screenshot")]
    pub fn world_backup_for_screenshot(&mut self, scene: &str, cx: &mut Context<Self>) {
        let Some(first) = self
            .world_list
            .as_ref()
            .and_then(|l| l.backups.first().cloned())
        else {
            return;
        };
        match scene {
            "create" => self.open_world_create(cx),
            "restore" | "restore-running" | "restore-busy" => {
                use launcher_core::cores::admin::models::InstanceState;
                let to = if scene == "restore-running" {
                    InstanceState::Running
                } else {
                    InstanceState::Stopped
                };
                if scene == "restore-busy" {
                    // a scheduled snapshot started after the dialog opened
                    self.job = Some(Job {
                        task_id: 7_380_000_000_000_000_003,
                        kind: JobKind::Backup,
                        progress: Some(42.0),
                        message: "Copying".to_string(),
                    });
                }
                let uuid = self.uuid.clone();
                self.state.update(cx, |state, cx| {
                    let fakes = Some(&mut state.fake_instances);
                    for list in [state.instances.as_mut(), fakes].into_iter().flatten() {
                        for instance in list.iter_mut().filter(|i| i.uuid == uuid) {
                            instance.state = to;
                        }
                    }
                    cx.notify();
                });
                self.open_world_restore(&first, cx)
            }
            "delete" => self.open_world_delete(&first, cx),
            "running" => {
                self.job = Some(Job {
                    task_id: 7_380_000_000_000_000_002,
                    kind: JobKind::WorldBackup,
                    progress: Some(37.0),
                    message: "Copying the world".to_string(),
                });
            }
            "download" => {
                let progress = Arc::new(Progress::default());
                progress.set(launcher_core::game::download::ProgressSnapshot {
                    files_total: 1,
                    files_done: 0,
                    bytes_total: 182_000_000,
                    bytes_done: 100_100_000,
                });
                self.world_downloads.insert(
                    first.id.clone(),
                    ExportRun {
                        progress,
                        cancel: CancelToken::new(),
                    },
                );
            }
            _ => {}
        }
        cx.notify();
    }
}

/// "3 world backups · 529 MB · 182 GB free".
fn world_usage(list: &WorldBackupList) -> AnyElement {
    let mut parts = vec![
        trn!(P::WorldCount, list.backups.len()),
        format::bytes(list.usage.total_bytes),
    ];
    if let Some(free) = list.usage.disk_free {
        parts.push(tr!(M::StorageFree, size = format::bytes(free)));
    }
    div()
        .text_size(text::SMALL)
        .text_color(ink::muted())
        .child(parts.join(" · "))
        .into_any_element()
}

/// A tag with a tooltip whose text is made at run time (a backup's name in it).
fn tag_with_text_tip(
    id: SharedString,
    label: &str,
    tip: Option<String>,
    color: lodestone_ui::theme::Color,
) -> AnyElement {
    let tag = settings::tag(label.to_string(), color);
    match tip {
        None => tag.into_any_element(),
        Some(tip) => tag
            .id(id)
            .tooltip(lodestone_ui::components::text_tooltip(tip))
            .into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::cores::admin::models::InstanceState;

    #[test]
    fn world_labels_and_names() {
        use chrono::TimeZone as _;
        crate::i18n::force(crate::i18n::Locale::En);
        let now = chrono::Local
            .with_ymd_and_hms(2026, 11, 12, 15, 0, 0)
            .single()
            .unwrap();
        let created = chrono::Local
            .with_ymd_and_hms(2026, 11, 12, 14, 30, 0)
            .single()
            .unwrap()
            .timestamp();
        let mut backup: WorldBackupInfo = serde_json::from_value(serde_json::json!({
            "id": "20261112-143000-abcd", "created": created, "name": " "
        }))
        .unwrap();
        assert_eq!(world_label(&backup, now), "Today, 14:30");
        backup.name = Some("Spawn\u{202e}txt".into());
        assert_eq!(
            world_label(&backup, now),
            "Spawn\\u{202E}txt",
            "shown visibly"
        );
        assert_eq!(
            world_download_name("Survival SMP", "world", "20261112-143000-abcd"),
            "Survival SMP-world-20261112-143000-abcd.zip"
        );
        assert_eq!(world_download_name("a/b:c?", "w", "x"), "a_b_c_-w-x.zip");
    }

    #[test]
    fn a_running_server_isnt_restored_into() {
        assert!(may_restore_into(InstanceState::Stopped));
        assert!(!may_restore_into(InstanceState::Running));
        assert!(!may_restore_into(InstanceState::Starting));
    }
}
