//! An instance's Backups tab (not in the dashboard): backing up now, the schedule
//! with how many automatic backups are kept, the storage used, and the list, with
//! restoring (in place or as a new server), downloading, pinning and deleting.
//!
//! The core does the work (`launcher_core::cores::admin::backups`): taking and
//! restoring run in the background, and their progression's events drive the
//! progress bar; the end reloads the list and says how it went. Progress updates
//! only reach owners and admins, so the page also asks the core again every few
//! seconds while a job runs.
//!
//! Reading needs the instance's file read permission (the tab is hidden without it).
//! Backing up and pinning need its Settings permission, restoring, deleting and
//! unpinning its file write permission, the schedule both, and restoring as a new
//! server the core's create permission. A control that is off says why.

use std::{collections::HashSet, path::PathBuf, time::Duration};

use gpui::{
    AnyElement, AppContext as _, Context, Entity, FocusHandle, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, Task, Window, div, prelude::*, relative,
};
use launcher_core::cores::admin::{
    EventInner,
    backups::{
        BackupInfo, BackupKind, BackupList, BackupPolicy, JobKind, RestoreRequest, RestoreScope,
    },
    events::{ProgressionEndValue, ProgressionEvent, ProgressionEventInner, ProgressionStartValue},
    models::{CorePermission, InstancePermission, InstanceState},
};
use lodestone_ui::{
    components::{
        Button, ButtonSize, ButtonVariant, Checkbox, Dialog, DialogSlot, Dropdown, Icon,
        IconButton, ListRow, LoadingDots, TextInput, Toast, Toggle,
        a11y::Accessible,
        segmented,
        settings::{self, SettingRow, content_width, popover_panel, select_button},
        text_tooltip, window_layer,
    },
    file_dialog::{self, ShowsError},
    theme::{colors, ink, leading, line_height, px, radius, text},
};

use super::parts::{self, Confirm};
use crate::{
    i18n::{BackupsMsg as M, BackupsPlural as P, CommonMsg, FormatMsg, format, t, tr, trn},
    state::{HostEvent, HostState},
};

/// The intervals offered, in minutes.
pub const INTERVALS: [u32; 7] = [15, 30, 60, 180, 360, 720, 1440];

/// How often the list is fetched again while a job runs.
const POLL: Duration = Duration::from_secs(4);

/// How many automatic backups are kept: the core's four numbers, offered as three
/// choices (`keep_last`, `keep_hourly`, `keep_daily`, `keep_weekly`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeepPreset {
    Fewer,
    Normal,
    More,
}

impl KeepPreset {
    pub const ALL: [KeepPreset; 3] = [KeepPreset::Fewer, KeepPreset::Normal, KeepPreset::More];

    pub fn values(self) -> (u32, u32, u32, u32) {
        match self {
            KeepPreset::Fewer => (3, 12, 3, 2),
            KeepPreset::Normal => (6, 24, 7, 4),
            KeepPreset::More => (12, 48, 14, 8),
        }
    }

    /// The choice `policy` keeps by, or `None` for numbers set some other way.
    pub fn of(policy: &BackupPolicy) -> Option<KeepPreset> {
        let kept = (
            policy.keep_last,
            policy.keep_hourly,
            policy.keep_daily,
            policy.keep_weekly,
        );
        KeepPreset::ALL.into_iter().find(|p| p.values() == kept)
    }

    pub fn apply(self, policy: &mut BackupPolicy) {
        (
            policy.keep_last,
            policy.keep_hourly,
            policy.keep_daily,
            policy.keep_weekly,
        ) = self.values();
    }

    fn label(self) -> &'static str {
        t(match self {
            KeepPreset::Fewer => M::KeepFewer,
            KeepPreset::Normal => M::KeepNormal,
            KeepPreset::More => M::KeepMore,
        })
    }
}

/// "Every 15 minutes", "Every hour", "Every 3 hours", "Every day".
pub fn interval_label(minutes: u32) -> String {
    match minutes {
        60 => t(M::EveryHour).to_string(),
        1440 => t(M::EveryDay).to_string(),
        m if m > 0 && m.is_multiple_of(1440) => trn!(P::EveryDays, m / 1440),
        m if m > 0 && m.is_multiple_of(60) => trn!(P::EveryHours, m / 60),
        m => trn!(P::EveryMinutes, m),
    }
}

/// When a backup was taken, from unix seconds: "Today, 14:30", "Yesterday, 09:12",
/// "Nov 3, 14:30" ("Nov 3, 2025, 14:30" in another year).
pub fn taken_at(seconds: i64, now: chrono::DateTime<chrono::Local>) -> String {
    use chrono::{Datelike as _, Local, TimeZone as _};
    let Some(time) = Local.timestamp_opt(seconds, 0).single() else {
        return String::new();
    };
    let today = now.date_naive();
    let day = time.date_naive();
    if day == today || today.pred_opt() == Some(day) {
        return format::day_and_time(time, today);
    }
    tr!(
        M::DateAt,
        date = format::short_date(day, day.year() != today.year()),
        time = time.format("%H:%M")
    )
}

/// A backup's name, or when it was taken.
pub fn backup_label(backup: &BackupInfo, now: chrono::DateTime<chrono::Local>) -> String {
    match backup.name.as_deref().map(str::trim) {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => taken_at(backup.created, now),
    }
}

/// How long until unix second `at`: "Next backup in 2 h 5 min", or "soon".
pub fn next_backup(at: i64, now: i64) -> String {
    let left = at - now;
    if left < 60 {
        return t(M::NextBackupSoon).to_string();
    }
    let minutes = left / 60;
    let time = if minutes < 60 {
        tr!(FormatMsg::Minutes, minutes = minutes)
    } else {
        tr!(
            FormatMsg::HoursMinutes,
            hours = minutes / 60,
            minutes = minutes % 60
        )
    };
    tr!(M::NextBackupIn, time = time)
}

/// The schedule's hint: when the next backup is, or what the schedule does when off.
///
/// With "only when players were on", the core doesn't back up a due backup until a
/// player joins, so past that time this says so instead of "soon" for as long as
/// nobody does. `players_on`: the server runs with players on now.
pub fn schedule_hint(
    policy: &BackupPolicy,
    next_scheduled: Option<i64>,
    players_on: bool,
    now: i64,
) -> String {
    match (policy.enabled, next_scheduled) {
        (true, Some(at)) if at <= now && policy.only_when_active && !players_on => {
            t(M::NextBackupWhenPlayerJoins).to_string()
        }
        (true, Some(at)) => next_backup(at, now),
        _ => t(M::AutomaticHint).to_string(),
    }
}

/// The name a backup's zip is saved under: the server's name and the backup's id,
/// without characters file systems refuse.
pub fn download_name(server: &str, backup_id: &str) -> String {
    let clean: String = format!("{server} {backup_id}")
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

/// The backup or restore of this server that is running, as far as the page knows.
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    pub task_id: i64,
    pub kind: JobKind,
    /// 0 to 100, once known: updates only reach owners and admins.
    pub progress: Option<f64>,
    pub message: String,
}

/// What a progression event meant for this server's job.
#[derive(Debug, PartialEq)]
pub enum Tracked {
    Started,
    Progress,
    Ended {
        kind: JobKind,
        success: bool,
        message: String,
    },
    /// Not about this server's backups.
    Other,
}

/// Follows this server's backup or restore through its progression events.
pub fn track(job: &mut Option<Job>, uuid: &str, event: &ProgressionEvent) -> Tracked {
    let id = event.event_id;
    match &event.progression_event_inner {
        ProgressionEventInner::ProgressionStart {
            inner: Some(start), ..
        } => {
            let kind = match start {
                ProgressionStartValue::Backup { instance_uuid } if instance_uuid == uuid => {
                    JobKind::Backup
                }
                ProgressionStartValue::Restore { instance_uuid } if instance_uuid == uuid => {
                    JobKind::Restore
                }
                _ => return Tracked::Other,
            };
            *job = Some(Job {
                task_id: id,
                kind,
                progress: None,
                message: String::new(),
            });
            Tracked::Started
        }
        ProgressionEventInner::ProgressionUpdate {
            progress_message,
            progress,
        } => match job {
            Some(job) if job.task_id == id => {
                job.progress = Some((job.progress.unwrap_or(0.0) + progress).clamp(0.0, 100.0));
                job.message = progress_message.clone();
                Tracked::Progress
            }
            _ => Tracked::Other,
        },
        ProgressionEventInner::ProgressionEnd {
            inner: Some(end), ..
        } => {
            let (kind, success, message) = match end {
                ProgressionEndValue::BackupCompleted {
                    instance_uuid,
                    success,
                    message,
                    ..
                } if instance_uuid == uuid => (JobKind::Backup, *success, message.clone()),
                ProgressionEndValue::RestoreCompleted {
                    instance_uuid,
                    success,
                    message,
                    ..
                } if instance_uuid == uuid => (JobKind::Restore, *success, message.clone()),
                _ => return Tracked::Other,
            };
            *job = None;
            Tracked::Ended {
                kind,
                success,
                message,
            }
        }
        _ => Tracked::Other,
    }
}

/// The list said what runs: the page's job, with the newest progress it knows.
fn job_from_list(job: &mut Option<Job>, list: &BackupList) {
    let Some(running) = &list.running else {
        *job = None;
        return;
    };
    let known = job
        .as_ref()
        .filter(|j| j.task_id == running.task_id)
        .and_then(|j| j.progress);
    *job = Some(Job {
        task_id: running.task_id,
        kind: running.kind,
        progress: Some(known.map_or(running.progress, |p| p.max(running.progress))),
        message: running.message.clone(),
    });
}

struct Restoring {
    backup: BackupInfo,
    label: String,
    focus: FocusHandle,
    scope: RestoreScope,
    stop: bool,
    start_after: bool,
    busy: bool,
    error: Option<String>,
}

struct Deleting {
    backup: BackupInfo,
    label: String,
    focus: FocusHandle,
    busy: bool,
}

struct RestoringAsNew {
    backup: BackupInfo,
    focus: FocusHandle,
    name: Entity<TextInput>,
    busy: bool,
    error: Option<String>,
}

pub struct BackupsPage {
    pub state: Entity<HostState>,
    /// The instance.
    pub uuid: String,
    list: Option<BackupList>,
    error: Option<String>,
    /// Counts the list's fetches, so only the newest one is kept.
    serial: u64,
    job: Option<Job>,
    /// "Back up now" was pressed and the core hasn't answered yet.
    starting: bool,
    interval_open: bool,
    /// Backups whose pin is changing or that are downloading, by id.
    busy: HashSet<String>,
    restoring: DialogSlot<Self, Restoring>,
    deleting: DialogSlot<Self, Deleting>,
    restoring_new: DialogSlot<Self, RestoringAsNew>,
    /// Fetches the list again while a job runs.
    poll: Option<Task<()>>,
    _tasks: Vec<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl ShowsError for BackupsPage {
    fn show_error(&mut self, message: SharedString, cx: &mut Context<Self>) {
        self.toast(Toast::error(message), cx);
    }
}

impl BackupsPage {
    pub fn new(state: Entity<HostState>, uuid: String, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![
            cx.observe(&state, |_, _, cx| cx.notify()),
            cx.subscribe(&state, |this, _, event, cx| this.on_host_event(event, cx)),
        ];
        let mut this = Self {
            state,
            uuid,
            list: None,
            error: None,
            serial: 0,
            job: None,
            starting: false,
            interval_open: false,
            busy: HashSet::new(),
            restoring: DialogSlot::new(|this: &mut Self| &mut this.restoring),
            deleting: DialogSlot::new(|this: &mut Self| &mut this.deleting),
            restoring_new: DialogSlot::new(|this: &mut Self| &mut this.restoring_new),
            poll: None,
            _tasks: Vec::new(),
            _subscriptions: subscriptions,
        };
        this.reload(cx);
        this
    }

    fn push(&mut self, task: Task<()>) {
        self._tasks.retain(|task| !task.is_ready());
        self._tasks.push(task);
    }

    fn toast(&self, toast: Toast, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| state.toast(toast, cx));
    }

    fn may(&self, permission: InstancePermission, cx: &Context<Self>) -> bool {
        self.state.read(cx).me.may(permission, &self.uuid)
    }

    fn may_create(&self, cx: &Context<Self>) -> bool {
        self.state
            .read(cx)
            .me
            .may_core(CorePermission::CreateInstance)
    }

    fn instance_state(&self, cx: &Context<Self>) -> InstanceState {
        self.state
            .read(cx)
            .instance(&self.uuid)
            .map_or(InstanceState::Unknown, |i| i.state)
    }

    fn instance_name(&self, cx: &Context<Self>) -> String {
        self.state
            .read(cx)
            .instance(&self.uuid)
            .map(|i| i.name.clone())
            .unwrap_or_default()
    }

    /// Fetches the list again.
    fn reload(&mut self, cx: &mut Context<Self>) {
        self.serial += 1;
        let serial = self.serial;
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let list = cx
                .background_spawn(async move { admin.list_backups(&uuid).await })
                .await;
            this.update(cx, |this, cx| {
                if serial != this.serial {
                    return;
                }
                match list {
                    Ok(list) => {
                        job_from_list(&mut this.job, &list);
                        this.list = Some(list);
                        this.error = None;
                    }
                    Err(err) => this.error = Some(err.to_string()),
                }
                this.keep_polling(cx);
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    /// While a job runs, fetches the list again in a moment (its progress, for users
    /// the core sends no updates to, and its end if the event was missed).
    fn keep_polling(&mut self, cx: &mut Context<Self>) {
        if self.job.is_none() {
            self.poll = None;
            return;
        }
        if self.poll.as_ref().is_some_and(|task| !task.is_ready()) {
            return;
        }
        self.poll = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(POLL).await;
            this.update(cx, |this, cx| {
                this.poll = None;
                this.reload(cx);
            })
            .ok();
        }));
    }

    fn on_host_event(&mut self, event: &HostEvent, cx: &mut Context<Self>) {
        let HostEvent::Core(event) = event else {
            return;
        };
        let EventInner::ProgressionEvent(progression) = &event.event_inner else {
            return;
        };
        match track(&mut self.job, &self.uuid, progression) {
            Tracked::Started => {
                self.reload(cx);
                cx.notify();
            }
            Tracked::Progress => cx.notify(),
            Tracked::Ended {
                kind,
                success,
                message,
            } => {
                let message = if !message.is_empty() {
                    message
                } else {
                    t(match (kind, success) {
                        (JobKind::Restore, true) => M::RestoreDone,
                        (JobKind::Restore, false) => M::RestoreFailed,
                        (_, true) => M::BackupDone,
                        (_, false) => M::BackupFailed,
                    })
                    .to_string()
                };
                self.toast(
                    if success {
                        Toast::success(message)
                    } else {
                        Toast::error(message)
                    },
                    cx,
                );
                // the server's version and port may have changed with its files
                if kind == JobKind::Restore && success {
                    self.state.update(cx, |state, cx| state.reload(cx));
                }
                self.reload(cx);
            }
            Tracked::Other => {}
        }
    }

    // ---- Backing up ----

    fn back_up_now(&mut self, cx: &mut Context<Self>) {
        if self.starting || self.job.is_some() {
            return;
        }
        self.starting = true;
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        parts::spawn(
            cx,
            async move { admin.create_backup(&uuid, None).await },
            |this, result, cx| {
                this.starting = false;
                match result {
                    Ok(task_id) => {
                        if this.job.is_none() {
                            this.job = Some(Job {
                                task_id,
                                kind: JobKind::Backup,
                                progress: None,
                                message: String::new(),
                            });
                        }
                        this.reload(cx);
                    }
                    Err(err) => this.toast(Toast::error(err.to_string()), cx),
                }
            },
        );
    }

    // ---- The schedule ----

    /// Changes the schedule here at once, and saves it.
    fn change_policy(&mut self, change: impl FnOnce(&mut BackupPolicy), cx: &mut Context<Self>) {
        let Some(list) = &mut self.list else {
            return;
        };
        change(&mut list.policy);
        let policy = list.policy.clone();
        self.interval_open = false;
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        parts::spawn(
            cx,
            async move { admin.set_backup_policy(&uuid, &policy).await },
            |this, result, cx| {
                match result {
                    Ok(saved) => {
                        if let Some(list) = &mut this.list {
                            list.policy = saved;
                        }
                        parts::saved(&this.state, cx);
                    }
                    Err(err) => {
                        this.toast(Toast::error(tr!(M::ScheduleSaveFailed, error = err)), cx)
                    }
                }
                // when the next one is, or the schedule as the core has it
                this.reload(cx);
            },
        );
    }

    // ---- A backup's actions ----

    fn set_pinned(&mut self, backup: &BackupInfo, pinned: bool, cx: &mut Context<Self>) {
        if !self.busy.insert(backup.id.clone()) {
            return;
        }
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let (uuid, id) = (self.uuid.clone(), backup.id.clone());
        parts::spawn(
            cx,
            {
                let id = id.clone();
                async move { admin.update_backup(&uuid, &id, None, Some(pinned)).await }
            },
            move |this, result, cx| {
                this.busy.remove(&id);
                match result {
                    Ok(info) => {
                        if let Some(list) = &mut this.list
                            && let Some(b) = list.backups.iter_mut().find(|b| b.id == info.id)
                        {
                            *b = info;
                        }
                    }
                    Err(err) => this.toast(Toast::error(err.to_string()), cx),
                }
            },
        );
    }

    fn open_restore(&mut self, backup: &BackupInfo, cx: &mut Context<Self>) {
        let label = backup_label(backup, chrono::Local::now());
        let start_after = self.may(InstancePermission::Start, cx);
        let focus = cx.focus_handle();
        self.restoring.open(
            Restoring {
                backup: backup.clone(),
                label,
                focus,
                scope: RestoreScope::Everything,
                stop: true,
                start_after,
                busy: false,
                error: None,
            },
            cx,
        );
    }

    fn restore(&mut self, cx: &mut Context<Self>) {
        let running = self.instance_state(cx).is_live();
        let Some(shown) = self.restoring.current_mut() else {
            return;
        };
        if shown.busy || (running && !shown.stop) {
            return;
        }
        shown.busy = true;
        shown.error = None;
        let request = RestoreRequest {
            scope: shown.scope,
            stop: shown.stop,
            start_after: running && shown.start_after,
            safety_backup: true,
        };
        let (id, label) = (shown.backup.id.clone(), shown.label.clone());
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        parts::spawn(
            cx,
            async move { admin.restore_backup(&uuid, &id, &request).await },
            move |this, result, cx| match result {
                Ok(task_id) => {
                    this.restoring.close(cx);
                    this.toast(Toast::info(tr!(M::RestoreStarted, name = label)), cx);
                    this.job = Some(Job {
                        task_id,
                        kind: JobKind::Restore,
                        progress: None,
                        message: String::new(),
                    });
                    this.reload(cx);
                }
                Err(err) => {
                    // a 403 says what the restore would change that the user may not
                    if let Some(shown) = this.restoring.current_mut() {
                        shown.busy = false;
                        shown.error = Some(err.to_string());
                    }
                }
            },
        );
    }

    fn open_delete(&mut self, backup: &BackupInfo, cx: &mut Context<Self>) {
        let focus = cx.focus_handle();
        self.deleting.open(
            Deleting {
                backup: backup.clone(),
                label: backup_label(backup, chrono::Local::now()),
                focus,
                busy: false,
            },
            cx,
        );
    }

    fn delete(&mut self, cx: &mut Context<Self>) {
        let Some(shown) = self.deleting.current_mut() else {
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
                async move { admin.delete_backup(&uuid, &id).await }
            },
            move |this, result, cx| match result {
                Ok(()) => {
                    this.deleting.close(cx);
                    if let Some(list) = &mut this.list {
                        list.backups.retain(|b| b.id != id);
                    }
                    this.toast(Toast::success(t(M::Deleted)), cx);
                    this.reload(cx);
                }
                Err(err) => {
                    if let Some(shown) = this.deleting.current_mut() {
                        shown.busy = false;
                    }
                    this.toast(Toast::error(err.to_string()), cx);
                }
            },
        );
    }

    fn open_restore_as_new(&mut self, backup: &BackupInfo, cx: &mut Context<Self>) {
        let default = tr!(M::NewDefaultName, name = self.instance_name(cx));
        let name = cx.new(|cx| {
            TextInput::new(cx)
                .with_label(t(M::NewName))
                .with_automation_id("backup-new-name")
                .with_text(default)
        });
        cx.observe(&name, |_, _, cx| cx.notify()).detach();
        let focus = cx.focus_handle();
        self.restoring_new.open(
            RestoringAsNew {
                backup: backup.clone(),
                focus,
                name,
                busy: false,
                error: None,
            },
            cx,
        );
    }

    fn restore_as_new(&mut self, cx: &mut Context<Self>) {
        let default_name = tr!(M::NewDefaultName, name = self.instance_name(cx));
        let Some(shown) = self.restoring_new.current_mut() else {
            return;
        };
        if shown.busy {
            return;
        }
        let typed = shown.name.read(cx).text().trim().to_string();
        let name = (!typed.is_empty()).then_some(typed);
        shown.busy = true;
        shown.error = None;
        let id = shown.backup.id.clone();
        let shown_name = name.clone().unwrap_or(default_name);
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        parts::spawn(
            cx,
            async move {
                admin
                    .restore_backup_as_new(&uuid, &id, name.as_deref())
                    .await
            },
            move |this, result, cx| match result {
                // the new server comes as an InstanceCreation, which the state adds
                Ok(_) => {
                    this.restoring_new.close(cx);
                    this.toast(Toast::info(tr!(M::NewStarted, name = shown_name)), cx);
                }
                Err(err) => {
                    if let Some(shown) = this.restoring_new.current_mut() {
                        shown.busy = false;
                        shown.error = Some(err.to_string());
                    }
                }
            },
        );
    }

    /// Asks where to save the backup's zip, then has the core zip it and saves it.
    ///
    /// The download outlives the page: switching tabs rebuilds the page, and must not
    /// cancel a download that takes minutes. So the task is detached and says how it
    /// went through the Host's toasts. The core client writes the zip next to the
    /// chosen file and only replaces it once complete.
    fn download(&mut self, backup: &BackupInfo, cx: &mut Context<Self>) {
        if self.busy.contains(&backup.id) {
            return;
        }
        let downloads = dirs::download_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        let name = download_name(&self.instance_name(cx), &backup.id);
        let id = backup.id.clone();
        let state = self.state.downgrade();
        cx.spawn(async move |this, cx| {
            let dialog = cx.update(|cx| cx.prompt_for_new_path(&downloads, Some(&name)));
            let Some(out) = file_dialog::chosen(dialog, &this, cx).await else {
                return;
            };
            let Ok((admin, uuid)) = this.update(cx, |this, cx| {
                this.busy.insert(id.clone());
                this.toast(Toast::info(t(M::Zipping)), cx);
                cx.notify();
                (this.state.read(cx).admin.clone(), this.uuid.clone())
            }) else {
                return;
            };
            let result = cx
                .background_spawn({
                    let (out, id) = (out.clone(), id.clone());
                    async move { admin.download_backup_to(&uuid, &id, &out).await }
                })
                .await;
            // the page may be gone by now: the toast goes through the Host
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
                Err(err) => Toast::error(tr!(M::DownloadFailed, error = err)),
            };
            state.update(cx, |state, cx| state.toast(toast, cx)).ok();
            this.update(cx, |this, cx| {
                this.busy.remove(&id);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // ---- Rendering ----

    fn render_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let may_settings = self.may(InstancePermission::Settings, cx);
        let running_job = self.job.is_some();
        let why = if !may_settings {
            Some(t(M::NoSettingsPermission))
        } else if running_job {
            Some(t(M::JobRunning))
        } else {
            None
        };
        let button = Button::new("backup-now", t(M::BackUpNow))
            .variant(ButtonVariant::Primary)
            .icon(Icon::ClockRotateLeft)
            .loading(self.starting)
            .disabled(why.is_some() || self.starting || self.list.is_none())
            .on_click(cx.listener(|this, _, _, cx| this.back_up_now(cx)));
        // the page's heading already says "Backups": the lead, and the button beside it
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(px(16.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(text::SMALL)
                    .line_height(line_height(text::SMALL, leading::NORMAL))
                    .text_color(ink::muted())
                    .child(t(M::Lead)),
            )
            .child(
                div()
                    .flex_none()
                    .child(why_not("backup-now-why", why, button)),
            )
            .into_any_element()
    }

    fn render_job(&self, job: &Job) -> AnyElement {
        let title = t(match job.kind {
            JobKind::Restore => M::Restoring,
            _ => M::BackingUp,
        });
        let track = div()
            .relative()
            .h(px(4.))
            .w_full()
            .overflow_hidden()
            .rounded_full()
            .bg(colors::GRAY_700);
        let bar = match job.progress {
            Some(progress) => track.child(
                div()
                    .h_full()
                    .rounded_full()
                    .bg(colors::BLUE)
                    .w(relative((progress / 100.0).clamp(0.02, 1.0) as f32)),
            ),
            // no updates reach this user: a bar that moves until the end
            None => track.child(indeterminate()),
        };
        div()
            .id("backup-job")
            .flex()
            .flex_col()
            .gap(px(6.))
            .px(px(16.))
            .py(px(12.))
            .rounded(radius::XL)
            .border_1()
            .border_color(colors::BLUE_FADED.opacity(50))
            .bg(colors::GRAY_850)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .child(div().text_color(ink::text()).child(title))
                    .when_some(job.progress, |this, p| {
                        this.child(
                            div()
                                .text_size(text::SMALL)
                                .text_color(ink::muted())
                                .child(format::percent(p.round() as u64)),
                        )
                    }),
            )
            .when(!job.message.is_empty(), |this| {
                this.child(
                    div()
                        .text_size(text::SMALL)
                        .text_color(ink::muted())
                        .truncate()
                        .child(job.message.clone()),
                )
            })
            .child(bar)
            .into_any_element()
    }

    fn render_schedule(&self, list: &BackupList, cx: &mut Context<Self>) -> AnyElement {
        let policy = &list.policy;
        let may = self.may(InstancePermission::Settings, cx)
            && self.may(InstancePermission::WriteFiles, cx);
        let why = (!may).then(|| t(M::NoSchedulePermission));
        let now = chrono::Local::now().timestamp();
        let players_on = self.state.read(cx).instance(&self.uuid).is_some_and(|i| {
            i.state == InstanceState::Running && i.player_count.is_some_and(|n| n > 0)
        });
        let hint = schedule_hint(policy, list.next_scheduled, players_on, now);
        let mut rows = vec![
            SettingRow::new(t(M::Automatic))
                .hint(hint)
                .child(why_not(
                    "backup-schedule-why",
                    why,
                    Toggle::new("backup-schedule", policy.enabled)
                        .label(t(M::Automatic))
                        .disabled(!may)
                        .on_change(cx.listener(|this, on: &bool, _, cx| {
                            let on = *on;
                            this.change_policy(|p| p.enabled = on, cx)
                        })),
                ))
                .into_any_element(),
        ];
        if policy.enabled {
            rows.push(
                SettingRow::new(t(M::Interval))
                    .child(why_not(
                        "backup-interval-why",
                        why,
                        self.interval_select(policy.interval_minutes, may, cx),
                    ))
                    .into_any_element(),
            );
            rows.push(
                SettingRow::new(t(M::OnlyWhenActive))
                    .hint(t(M::OnlyWhenActiveHint))
                    .child(why_not(
                        "backup-active-why",
                        why,
                        Toggle::new("backup-active", policy.only_when_active)
                            .label(t(M::OnlyWhenActive))
                            .disabled(!may)
                            .on_change(cx.listener(|this, on: &bool, _, cx| {
                                let on = *on;
                                this.change_policy(|p| p.only_when_active = on, cx)
                            })),
                    ))
                    .into_any_element(),
            );
            let preset = KeepPreset::of(policy);
            // numbers set another way show as a fourth choice, which changes nothing
            let mut choices: Vec<(Option<KeepPreset>, SharedString)> = KeepPreset::ALL
                .into_iter()
                .map(|p| (Some(p), p.label().into()))
                .collect();
            if preset.is_none() {
                choices.push((None, t(M::KeepCustom).into()));
            }
            let page = cx.entity().downgrade();
            rows.push(
                SettingRow::new(t(M::Keep))
                    .hint(t(M::KeepHint))
                    .child(why_not(
                        "backup-keep-why",
                        why,
                        segmented(
                            "backup-keep",
                            choices,
                            Some(&preset),
                            !may,
                            move |picked: Option<KeepPreset>, _, cx| {
                                let Some(picked) = picked else {
                                    return;
                                };
                                page.update(cx, |this, cx| {
                                    this.change_policy(|p| picked.apply(p), cx)
                                })
                                .ok();
                            },
                        ),
                    ))
                    .into_any_element(),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(settings::section(t(M::Schedule), None))
            .child(settings::card(rows))
            .into_any_element()
    }

    fn interval_select(&self, current: u32, enabled: bool, cx: &mut Context<Self>) -> AnyElement {
        let open = self.interval_open && enabled;
        let label = SharedString::from(interval_label(current));
        let button = select_button("backup-interval", div().child(label.clone()))
            .aria_label(SharedString::from(t(M::Interval)))
            .aria_value(label)
            .min_w(px(180.))
            .map(|this| {
                if enabled {
                    let page = cx.entity().downgrade();
                    this.expandable(open, move |_, cx| {
                        page.update(cx, |this, cx| {
                            this.interval_open = !open;
                            cx.notify();
                        })
                        .ok();
                    })
                    .on_press(cx.listener(|this, _, _, cx| {
                        this.interval_open = !this.interval_open;
                        cx.notify();
                    }))
                } else {
                    this.opacity(0.5).cursor_default()
                }
            });
        let page = cx.entity().downgrade();
        Dropdown::new(button)
            .align_end()
            .when(open, |this| {
                let mut panel = popover_panel("backup-interval-list").min_w(px(200.));
                // an interval set another way stays in the list
                let mut options = INTERVALS.to_vec();
                if !options.contains(&current) {
                    options.push(current);
                    options.sort_unstable();
                }
                for minutes in options {
                    panel = panel.child(
                        ListRow::new(
                            SharedString::from(format!("backup-interval-{minutes}")),
                            interval_label(minutes),
                        )
                        .selected(minutes == current)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.change_policy(|p| p.interval_minutes = minutes, cx)
                        })),
                    );
                }
                this.popover("backup-interval-menu", panel, move |_, cx| {
                    page.update(cx, |this, cx| {
                        this.interval_open = false;
                        cx.notify();
                    })
                    .ok();
                })
            })
            .into_any_element()
    }

    fn render_storage(&self, list: &BackupList) -> AnyElement {
        let mut parts = vec![
            trn!(P::Count, list.backups.len()),
            tr!(
                M::StorageFiles,
                size = format::bytes(list.usage.total_bytes)
            ),
            tr!(
                M::StorageStored,
                size = format::bytes(list.usage.stored_bytes)
            ),
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

    fn backup_row(&self, backup: &BackupInfo, cx: &mut Context<Self>) -> AnyElement {
        let now = chrono::Local::now();
        let id = |part: &str| SharedString::from(format!("backup-{}-{part}", backup.id));
        let busy = self.busy.contains(&backup.id);
        let job = self.job.is_some();
        let may_settings = self.may(InstancePermission::Settings, cx);
        let may_write = self.may(InstancePermission::WriteFiles, cx);
        let restore_why = if !may_write {
            Some(t(M::NoWritePermission))
        } else if job {
            Some(t(M::JobRunning))
        } else {
            None
        };
        // unpinning lets the schedule delete it: that needs the write permission too
        let pin_why = if !may_settings {
            Some(t(M::NoSettingsPermission))
        } else if backup.pinned && !may_write {
            Some(t(M::NoWritePermission))
        } else {
            None
        };
        let delete_why = (!may_write).then(|| t(M::NoWritePermission));

        let mut tags = vec![tag_with_tip(
            id("kind"),
            t(match backup.kind {
                BackupKind::Scheduled => M::TagAutomatic,
                BackupKind::BeforeRestore => M::TagBeforeRestore,
                _ => M::TagManual,
            }),
            None,
            match backup.kind {
                BackupKind::BeforeRestore => colors::VIOLET,
                _ => colors::GRAY_400,
            },
        )];
        if backup.pinned {
            tags.push(tag_with_tip(
                id("pinned"),
                t(M::TagPinned),
                Some(t(M::TagPinnedHint)),
                colors::BLUE_300,
            ));
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
                Some(t(M::TagInconsistentHint)),
                colors::YELLOW,
            ));
        }
        if backup.skipped_count > 0 {
            tags.push(tag_with_tip(
                id("skipped"),
                &trn!(P::Skipped, backup.skipped_count),
                Some(t(M::TagSkippedHint)),
                colors::YELLOW,
            ));
        }

        let size = tr!(
            M::SizeNew,
            size = format::bytes(backup.total_bytes),
            new = format::bytes(backup.new_bytes)
        );
        let hint = if backup.game_version.is_empty() {
            size
        } else {
            format!("{size} · Minecraft {}", backup.game_version)
        };

        let restore = {
            let b = backup.clone();
            Button::new(id("restore"), t(M::RestoreAction))
                .size(ButtonSize::Slim)
                .disabled(restore_why.is_some())
                .on_click(cx.listener(move |this, _, _, cx| this.open_restore(&b, cx)))
        };
        let download = {
            let b = backup.clone();
            IconButton::new(id("download"), Icon::Download)
                .label(t(M::DownloadLabel))
                .disabled(busy)
                .on_click(cx.listener(move |this, _, _, cx| this.download(&b, cx)))
        };
        let pin = {
            let b = backup.clone();
            let pinned = backup.pinned;
            IconButton::new(id("pin"), Icon::Thumbtack)
                .label(t(if pinned { M::UnpinLabel } else { M::PinLabel }))
                .color(if pinned {
                    colors::BLUE_300
                } else {
                    colors::GRAY_500
                })
                .disabled(busy || pin_why.is_some())
                .on_click(cx.listener(move |this, _, _, cx| this.set_pinned(&b, !pinned, cx)))
        };
        let new_server = self.may_create(cx).then(|| {
            let b = backup.clone();
            IconButton::new(id("new"), Icon::SquarePlus)
                .label(t(M::RestoreAsNewLabel))
                .on_click(cx.listener(move |this, _, _, cx| this.open_restore_as_new(&b, cx)))
        });
        let delete = {
            let b = backup.clone();
            IconButton::new(id("delete"), Icon::TrashCan)
                .label(t(M::DeleteLabel))
                .color(colors::RED_200)
                .disabled(delete_why.is_some())
                .on_click(cx.listener(move |this, _, _, cx| this.open_delete(&b, cx)))
        };

        let mut row = SettingRow::new(backup_label(backup, now))
            .key(SharedString::from(format!("backup-{}", backup.id)))
            .hint(hint)
            .child(why_not(id("restore-why"), restore_why, restore))
            .child(download)
            .child(why_not(id("pin-why"), pin_why, pin));
        if let Some(new_server) = new_server {
            row = row.child(new_server);
        }
        row.child(why_not(id("delete-why"), delete_why, delete))
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

    fn render_list(&self, list: &BackupList, cx: &mut Context<Self>) -> AnyElement {
        let rows: Vec<AnyElement> = if list.backups.is_empty() {
            vec![parts::empty_row(t(M::Empty))]
        } else {
            list.backups
                .iter()
                .map(|backup| self.backup_row(backup, cx))
                .collect()
        };
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(self.render_storage(list))
            .child(settings::card(rows))
            .into_any_element()
    }

    fn render_restore(&self, shown: &Restoring, cx: &mut Context<Self>) -> AnyElement {
        let running = self.instance_state(cx).is_live();
        let page = cx.entity().downgrade();
        let scope = if shown.backup.has_world {
            segmented(
                "backup-restore-scope",
                vec![
                    (RestoreScope::Everything, t(M::ScopeEverything).into()),
                    (RestoreScope::World, t(M::ScopeWorld).into()),
                ],
                Some(&shown.scope),
                shown.busy,
                move |scope: RestoreScope, _, cx| {
                    page.update(cx, |this, cx| {
                        if let Some(shown) = this.restoring.current_mut() {
                            shown.scope = scope;
                        }
                        cx.notify();
                    })
                    .ok();
                },
            )
            .into_any_element()
        } else {
            parts::paragraph(t(M::NoWorld))
        };
        let scope_hint = t(match shown.scope {
            RestoreScope::Everything => M::ScopeEverythingHint,
            RestoreScope::World => M::ScopeWorldHint,
        });
        let checkbox =
            |id: &'static str, checked: bool, label: M, set: fn(&mut Restoring, bool)| {
                let page = cx.entity().downgrade();
                Checkbox::new(id, checked, t(label))
                    .disabled(shown.busy)
                    .on_change(move |value, _, cx| {
                        let value = *value;
                        page.update(cx, |this, cx| {
                            if let Some(shown) = this.restoring.current_mut() {
                                set(shown, value);
                            }
                            cx.notify();
                        })
                        .ok();
                    })
            };
        let options = running.then(|| {
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(checkbox(
                    "backup-restore-stop",
                    shown.stop,
                    M::StopFirst,
                    |s, v| s.stop = v,
                ))
                .when(shown.stop, |this| {
                    this.child(checkbox(
                        "backup-restore-start",
                        shown.start_after,
                        M::StartAfter,
                        |s, v| s.start_after = v,
                    ))
                })
                .when(!shown.stop, |this| {
                    this.child(parts::warning(t(M::MustStop)))
                })
        });
        let close = cx.entity().downgrade();
        let cancel = close.clone();
        Dialog::new(
            "backup-restore",
            tr!(M::RestoreTitle, name = shown.label),
            shown.focus.clone(),
        )
        .on_close(move |_, cx| {
            close
                .update(cx, |this, cx| {
                    this.restoring.close(cx);
                })
                .ok();
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(scope)
                .child(
                    div()
                        .text_size(text::SMALL)
                        .line_height(line_height(text::SMALL, leading::NORMAL))
                        .text_color(ink::muted())
                        .child(scope_hint),
                )
                .children(options)
                .child(parts::paragraph(t(M::SafetyNote)))
                .children(shown.error.clone().map(parts::error)),
        )
        .footer(
            Button::new(("backup-restore", 1usize), t(M::Restore))
                .variant(ButtonVariant::Danger)
                .loading(shown.busy)
                .disabled(shown.busy || (running && !shown.stop))
                .on_click(cx.listener(|this, _, _, cx| this.restore(cx))),
        )
        .footer(
            Button::new(("backup-restore", 2usize), t(CommonMsg::Cancel))
                .disabled(shown.busy)
                .on_click(move |_, _, cx| {
                    cancel
                        .update(cx, |this, cx| {
                            this.restoring.close(cx);
                        })
                        .ok();
                }),
        )
        .into_any_element()
    }

    fn render_delete(&self, shown: &Deleting, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity().downgrade();
        let close = this.clone();
        Confirm {
            id: "delete-backup",
            title: t(M::DeleteTitle).into(),
            body: tr!(M::DeleteBody, name = shown.label).into(),
            action: t(CommonMsg::Delete).into(),
            danger: true,
            busy: shown.busy,
        }
        .render(
            shown.focus.clone(),
            move |_, cx| {
                this.update(cx, |this, cx| this.delete(cx)).ok();
            },
            move |_, cx| {
                close
                    .update(cx, |this, cx| {
                        this.deleting.close(cx);
                    })
                    .ok();
            },
        )
        .into_any_element()
    }

    fn render_restore_as_new(&self, shown: &RestoringAsNew, cx: &mut Context<Self>) -> AnyElement {
        let close = cx.entity().downgrade();
        let cancel = close.clone();
        Dialog::new("backup-new", t(M::NewTitle), shown.focus.clone())
            .on_close(move |_, cx| {
                close
                    .update(cx, |this, cx| {
                        this.restoring_new.close(cx);
                    })
                    .ok();
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(parts::paragraph(t(M::NewBody)))
                    .child(parts::field(t(M::NewName), &shown.name))
                    .children(shown.error.clone().map(parts::error)),
            )
            .footer(
                Button::new(("backup-new", 1usize), t(M::NewCreate))
                    .variant(ButtonVariant::Primary)
                    .loading(shown.busy)
                    .disabled(shown.busy)
                    .on_click(cx.listener(|this, _, _, cx| this.restore_as_new(cx))),
            )
            .footer(
                Button::new(("backup-new", 2usize), t(CommonMsg::Cancel))
                    .disabled(shown.busy)
                    .on_click(move |_, _, cx| {
                        cancel
                            .update(cx, |this, cx| {
                                this.restoring_new.close(cx);
                            })
                            .ok();
                    }),
            )
            .into_any_element()
    }

    /// Sample backups, a running backup and the schedule on, for the development
    /// screenshots' `--demo`.
    #[cfg(feature = "screenshot")]
    pub fn demo(&mut self, cx: &mut Context<Self>) {
        let now = chrono::Local::now().timestamp();
        let list: BackupList = serde_json::from_value(serde_json::json!({
            "backups": [
                {"id": "20261112-141000-a1b2", "name": "Before updating to 1.21.4",
                 "kind": "manual", "created": now - 20 * 60, "pinned": true, "live": true,
                 "quiesced": true, "file_count": 2880, "skipped_count": 0,
                 "total_bytes": 1_240_000_000u64, "new_bytes": 34_000_000u64,
                 "stored_bytes": 12_000_000u64, "duration_ms": 8200,
                 "game_version": "1.21.3", "has_world": true},
                {"id": "20261112-120000-c3d4", "kind": "scheduled", "created": now - 2 * 3600,
                 "live": true, "quiesced": false, "file_count": 2875, "skipped_count": 3,
                 "total_bytes": 1_230_000_000u64, "new_bytes": 18_500_000u64,
                 "stored_bytes": 6_100_000u64, "duration_ms": 5400,
                 "game_version": "1.21.3", "has_world": true},
                {"id": "20261111-180500-e5f6", "name": "Before restoring “Nov 9, 18:00”",
                 "kind": "before_restore", "created": now - 26 * 3600, "live": false,
                 "quiesced": true, "file_count": 2860, "total_bytes": 1_210_000_000u64,
                 "new_bytes": 2_400_000u64, "stored_bytes": 900_000u64,
                 "game_version": "1.21.3", "has_world": true},
                {"id": "20261109-060000-0a1b", "kind": "scheduled", "created": now - 3 * 86_400,
                 "live": false, "quiesced": true, "file_count": 2840,
                 "total_bytes": 1_190_000_000u64, "new_bytes": 41_000_000u64,
                 "stored_bytes": 15_000_000u64, "game_version": "1.21.3", "has_world": true},
                {"id": "20261031-210000-2c3d", "kind": "manual", "created": now - 12 * 86_400,
                 "live": false, "quiesced": true, "file_count": 310,
                 "total_bytes": 96_000_000u64, "new_bytes": 96_000_000u64,
                 "stored_bytes": 61_000_000u64, "game_version": "1.21.3", "has_world": false},
            ],
            "policy": {"enabled": true, "interval_minutes": 360, "only_when_active": true,
                "keep_last": 6, "keep_hourly": 24, "keep_daily": 7, "keep_weekly": 4},
            "running": {"kind": "backup", "task_id": "7380000000000000001",
                "started": now - 40, "progress": 42.0,
                "message": "Storing files (1,204 of 2,880)"},
            "usage": {"total_bytes": 4_966_000_000u64, "stored_bytes": 1_310_000_000u64,
                "disk_free": 182_000_000_000u64},
            "next_scheduled": now + 2 * 3600 + 5 * 60,
        }))
        .expect("the demo list");
        // a fetch on its way (which fails for the made-up server) is dropped
        self.serial += 1;
        self.error = None;
        job_from_list(&mut self.job, &list);
        self.list = Some(list);
        self.poll = None;
        cx.notify();
    }
}

/// `control` as it is, or with a tooltip saying why it is off.
fn why_not(
    id: impl Into<SharedString>,
    why: Option<&'static str>,
    control: impl IntoElement,
) -> AnyElement {
    match why {
        None => control.into_any_element(),
        Some(why) => div()
            .id(id.into())
            .child(control)
            .tooltip(text_tooltip(why))
            .into_any_element(),
    }
}

/// A tag, with a tooltip saying what it means.
fn tag_with_tip(
    id: SharedString,
    label: &str,
    tip: Option<&'static str>,
    color: lodestone_ui::theme::Color,
) -> AnyElement {
    let tag = settings::tag(label.to_string(), color);
    match tip {
        None => tag.into_any_element(),
        Some(tip) => tag.id(id).tooltip(text_tooltip(tip)).into_any_element(),
    }
}

/// A short bar sliding along its track, for progress that isn't known. Still with
/// reduced motion.
fn indeterminate() -> AnyElement {
    use gpui::{Animation, AnimationExt as _};
    let bar = div()
        .absolute()
        .top_0()
        .h_full()
        .w(relative(0.3))
        .rounded_full()
        .bg(colors::BLUE);
    if lodestone_ui::components::prefers_reduced_motion() {
        return bar.left(relative(0.35)).into_any_element();
    }
    bar.with_animation(
        "backup-job-indeterminate",
        Animation::new(Duration::from_millis(1400)).repeat(),
        |bar, delta| bar.left(relative(delta * 1.3 - 0.3)),
    )
    .into_any_element()
}

impl Render for BackupsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut sections: Vec<AnyElement> = vec![self.render_header(cx)];
        if let Some(job) = &self.job {
            sections.push(self.render_job(job));
        }
        if let Some(error) = &self.error {
            sections.push(parts::error(tr!(M::LoadFailed, error = error)));
        }
        match self.list.clone() {
            None if self.error.is_none() => sections.push(
                div()
                    .py(px(24.))
                    .flex()
                    .justify_center()
                    .child(LoadingDots)
                    .into_any_element(),
            ),
            None => {}
            Some(list) => {
                sections.push(self.render_schedule(&list, cx));
                sections.push(self.render_list(&list, cx));
            }
        }
        let mut dialogs = self
            .restoring
            .render(window, |shown| Some(self.render_restore(shown, cx)));
        dialogs.extend(
            self.deleting
                .render(window, |shown| Some(self.render_delete(shown, cx))),
        );
        dialogs.extend(
            self.restoring_new
                .render(window, |shown| Some(self.render_restore_as_new(shown, cx))),
        );
        div()
            .id("backups")
            .size_full()
            .overflow_y_scroll()
            .text_size(text::MEDIUM)
            .line_height(line_height(text::MEDIUM, leading::NORMAL))
            .child(
                div()
                    .w_full()
                    .max_w(content_width())
                    .pb(px(64.))
                    .flex()
                    .flex_col()
                    .gap(px(24.))
                    .children(sections),
            )
            .children(window_layer(window, dialogs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::cores::admin::events::parse;

    fn progression(json: &str) -> ProgressionEvent {
        let event = parse(&format!(
            r#"{{"event_inner":{{"type":"ProgressionEvent","event_id":"9","progression_event_inner":{json}}},
                "details":"","snowflake":"10","level":"Info","caused_by":{{"type":"System"}}}}"#
        ))
        .expect("an event");
        match event.event_inner {
            EventInner::ProgressionEvent(p) => p,
            _ => panic!("a progression"),
        }
    }

    #[test]
    fn presets_match_the_policy() {
        let mut policy = BackupPolicy::default();
        assert_eq!(KeepPreset::of(&policy), Some(KeepPreset::Normal));
        KeepPreset::More.apply(&mut policy);
        assert_eq!(KeepPreset::of(&policy), Some(KeepPreset::More));
        assert_eq!(
            (policy.keep_last, policy.keep_weekly),
            (12, 8),
            "every number changes"
        );
        policy.keep_daily = 30;
        assert_eq!(KeepPreset::of(&policy), None, "custom");
        KeepPreset::Fewer.apply(&mut policy);
        assert_eq!(KeepPreset::of(&policy), Some(KeepPreset::Fewer));
    }

    #[test]
    fn intervals_have_names() {
        crate::i18n::force(crate::i18n::Locale::En);
        assert_eq!(interval_label(15), "Every 15 minutes");
        assert_eq!(interval_label(60), "Every hour");
        assert_eq!(interval_label(180), "Every 3 hours");
        assert_eq!(interval_label(1440), "Every day");
        assert_eq!(interval_label(10080), "Every 7 days");
        assert_eq!(interval_label(90), "Every 90 minutes");
        assert!(INTERVALS.contains(&BackupPolicy::default().interval_minutes));
    }

    #[test]
    fn labels_and_times() {
        use chrono::TimeZone as _;
        crate::i18n::force(crate::i18n::Locale::En);
        let now = chrono::Local
            .with_ymd_and_hms(2026, 11, 12, 15, 0, 0)
            .single()
            .unwrap();
        let at = |d, h, m| {
            chrono::Local
                .with_ymd_and_hms(2026, 11, d, h, m, 0)
                .single()
                .unwrap()
                .timestamp()
        };
        assert_eq!(taken_at(at(12, 14, 30), now), "Today, 14:30");
        assert_eq!(taken_at(at(11, 9, 5), now), "Yesterday, 09:05");
        assert_eq!(taken_at(at(3, 18, 0), now), "Nov 3, 18:00");
        let mut backup: BackupInfo = serde_json::from_value(serde_json::json!({
            "id": "20261103-180000-abcd", "created": at(3, 18, 0), "name": "  "
        }))
        .unwrap();
        assert_eq!(backup_label(&backup, now), "Nov 3, 18:00", "a blank name");
        backup.name = Some("Before 1.21".into());
        assert_eq!(backup_label(&backup, now), "Before 1.21");
        assert_eq!(next_backup(1000, 990), "Next backup soon");
        assert_eq!(next_backup(1000 + 25 * 60, 1000), "Next backup in 25 min");
        assert_eq!(
            next_backup(1000 + 2 * 3600 + 5 * 60, 1000),
            "Next backup in 2 h 5 min"
        );
    }

    #[test]
    fn a_due_backup_waiting_for_players_says_so() {
        let mut policy = BackupPolicy {
            enabled: true,
            only_when_active: true,
            ..BackupPolicy::default()
        };
        let now = 10_000;
        // nobody on, and the time has passed: the core waits for a player
        assert_eq!(
            schedule_hint(&policy, Some(now - 3600), false, now),
            "Next backup once a player joins"
        );
        // with players on it runs at the scheduler's next tick
        assert_eq!(
            schedule_hint(&policy, Some(now - 3600), true, now),
            "Next backup soon"
        );
        // still ahead: the time it's due
        assert_eq!(
            schedule_hint(&policy, Some(now + 25 * 60), false, now),
            "Next backup in 25 min"
        );
        // backing up whether or not anyone played: it's due, so soon
        policy.only_when_active = false;
        assert_eq!(
            schedule_hint(&policy, Some(now - 3600), false, now),
            "Next backup soon"
        );
        policy.enabled = false;
        assert_eq!(
            schedule_hint(&policy, None, false, now),
            "Nothing is saved when nothing changed since the last backup."
        );
    }

    #[test]
    fn download_names_are_safe() {
        assert_eq!(
            download_name("Survival SMP", "20261112-101500-ab12"),
            "Survival SMP 20261112-101500-ab12.zip"
        );
        assert_eq!(download_name("a/b:c?", "x"), "a_b_c_ x.zip");
    }

    #[test]
    fn a_job_follows_its_events() {
        let mut job = None;
        let other = progression(
            r#"{"type":"ProgressionStart","progression_name":"x","inner":{"type":"Backup","instance_uuid":"other"}}"#,
        );
        assert_eq!(track(&mut job, "i1", &other), Tracked::Other);
        assert!(job.is_none());
        let start = progression(
            r#"{"type":"ProgressionStart","progression_name":"Backing up SMP","total":100,"inner":{"type":"Backup","instance_uuid":"i1"}}"#,
        );
        assert_eq!(track(&mut job, "i1", &start), Tracked::Started);
        assert_eq!(job.as_ref().unwrap().progress, None);
        let update = progression(
            r#"{"type":"ProgressionUpdate","progress_message":"Storing files","progress":5}"#,
        );
        track(&mut job, "i1", &update);
        assert_eq!(track(&mut job, "i1", &update), Tracked::Progress);
        let running = job.clone().unwrap();
        assert_eq!(running.progress, Some(10.0), "updates add up");
        assert_eq!(running.message, "Storing files");
        let end = progression(
            r#"{"type":"ProgressionEnd","success":true,"inner":{"type":"BackupCompleted","instance_uuid":"i1","backup_id":null,"success":true,"message":"Nothing changed since the last backup"}}"#,
        );
        assert_eq!(
            track(&mut job, "i1", &end),
            Tracked::Ended {
                kind: JobKind::Backup,
                success: true,
                message: "Nothing changed since the last backup".into()
            }
        );
        assert!(job.is_none());
        // another kind of end isn't this job's
        let unknown = progression(
            r#"{"type":"ProgressionEnd","success":true,"inner":{"type":"SomethingNew","instance_uuid":"i1"}}"#,
        );
        assert_eq!(track(&mut job, "i1", &unknown), Tracked::Other);
    }

    #[test]
    fn the_list_keeps_the_newest_progress() {
        let mut list: BackupList = serde_json::from_value(serde_json::json!({
            "running": {"kind": "restore", "task_id": "9", "progress": 20.0, "message": "Staging"}
        }))
        .unwrap();
        let mut job = Some(Job {
            task_id: 9,
            kind: JobKind::Restore,
            progress: Some(35.0),
            message: String::new(),
        });
        job_from_list(&mut job, &list);
        assert_eq!(job.as_ref().unwrap().progress, Some(35.0));
        assert_eq!(job.as_ref().unwrap().message, "Staging");
        list.running = None;
        job_from_list(&mut job, &list);
        assert!(job.is_none(), "it ended");
    }
}
