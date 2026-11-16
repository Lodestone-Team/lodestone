//! "Host on Lodestone": a Launcher instance becomes a new server on a host, from the
//! instance's Settings tab. See [`launcher_core::cores::hosting`] for the steps.
//!
//! The dialog goes through them in order: the host, the sign-in (none on a core this
//! computer is the owner of, "Connect as owner") and the worlds to send; then the
//! instance is written as a file and sent, with a bar and Cancel; then what the host
//! read from it (lodestone-ui's `sharing`, as the GPUI Host's "From a file" shows it),
//! with the server's world, name and port and the EULA box, never ticked for the person;
//! then the host makes the server, which the dialog follows to its end and what it didn't
//! get. The server is created stopped and never started here. Backing out, Cancel or
//! closing the dialog lets go of the file the host holds; the instance is linked to the
//! new server only once it was made.

use std::sync::Arc;

use crate::theme::px;
use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    Render, SharedString, Task, Window, div, prelude::*,
};
use launcher_core::{
    Secret,
    cores::{
        CoreError,
        admin::AdminClient,
        admin::instances::{NameProblem, name_problem},
        client::display_address,
        hosting::{self, HostAuth, HostChoices, HostError, HostProgress, HostStage, HostWorld},
    },
    game::download::CancelToken,
};
use lodestone_ui::sharing::{self, Level, Picked};
use lodestone_ui::values::truncate::sanitize;

use super::{
    Servers,
    extras::{error_line, label, note, pills, progress_bar},
};
use crate::{
    components::{
        Button, ButtonVariant, Checkbox, Dialog, Dropdown, Fact, FactLine, FactValue, Kind,
        ListRow, LoadingDots, Num, ObjectChip, StatusBadge, TextInput, Toggle,
        a11y::Accessible as _,
        settings::{popover_panel, select_button},
    },
    i18n::{CommonMsg, FactMsg, ServersMsg as M, SharingMsg as S, Text, format, t, tr},
    theme::{Tone, layout},
};

/// The server's memory choices, in MB.
pub const MEMORY_CHOICES: [u32; 4] = [2048, 4096, 6144, 8192];
pub const EULA_URL: &str = "https://aka.ms/MinecraftEULA";

/// What the first step asks.
#[derive(Clone)]
pub struct HostRequest {
    pub core_uuid: String,
    /// The username and password, for a core this computer isn't the owner of.
    pub sign_in: Option<(String, Secret)>,
    /// The worlds sent, by folder.
    pub worlds: Vec<String>,
    pub max_ram: u32,
}

/// The server that was made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostDone {
    pub name: String,
    pub core_name: String,
    pub port: u16,
    /// The server the instance was linked to before.
    pub relinked: Option<String>,
    /// Whether the instance is linked to the new server: not when it was deleted while
    /// the server was made.
    pub linked: bool,
    /// What the server didn't get, a line each, translated.
    pub left_out: Vec<String>,
}

pub enum HostEvent {
    Dismissed,
    /// The first step's answers: write the instance and send it.
    Submit(HostRequest),
    /// Make the server from the file the host read.
    Create(HostChoices),
    ShowServers,
}

/// A core the dialog offers.
#[derive(Clone, Debug)]
pub struct HostTarget {
    pub uuid: String,
    pub name: String,
    /// The username this computer signed in with, to fill in.
    pub username: Option<String>,
    /// This computer has the core's owner key, so no password is asked.
    pub owner: bool,
}

/// The file the host read, and what the person picks for the server.
pub struct Summary {
    /// Let go of (with the temporary file) when the dialog backs out or closes; taken by
    /// the creation.
    pub uploaded: Option<hosting::Uploaded>,
    pub admin: AdminClient,
    /// The core it is on.
    core_uuid: String,
    name: Entity<TextInput>,
    port: Entity<TextInput>,
    world: Option<String>,
    /// The world list is open.
    world_menu: bool,
    server: Option<String>,
    eula: bool,
    /// The core is asked to make the server.
    submitting: bool,
}

impl Summary {
    /// Whether Create can be pressed: only once the person ticked the EULA, for a file
    /// the host can make a server from, and not while it is asked already.
    fn can_create(&self) -> bool {
        self.eula
            && !self.submitting
            && self
                .uploaded
                .as_ref()
                .is_some_and(|u| sharing::can_create(&u.upload.summary, u.may_write_global))
    }
}

/// Where the dialog is.
pub enum HostStep {
    Setup,
    /// Writing the instance, sending it, the host reading it.
    Working {
        progress: Arc<HostProgress>,
        cancel: CancelToken,
    },
    Summary(Box<Summary>),
    /// The host makes the server; `fraction` is how far along it is.
    Creating {
        fraction: f64,
    },
    Done(HostDone),
    /// The creation was asked for and didn't end well (it failed, or the host is still
    /// setting the server up): the dialog ends here, so the same instance isn't sent and
    /// made again by pressing Continue.
    Ended(SharedString),
}

pub struct HostDialog {
    pub instance_id: String,
    instance_name: String,
    cores: Vec<HostTarget>,
    core: String,
    username: Entity<TextInput>,
    password: Entity<TextInput>,
    max_ram: u32,
    /// `None` while they are read: a big world's folder takes a while to walk.
    worlds: Option<Vec<HostWorld>>,
    /// The worlds to send, by folder.
    send: Vec<String>,
    /// Why this instance can't be hosted.
    blocked: Option<Text>,
    pub step: HostStep,
    pub error: Option<SharedString>,
    focus_handle: FocusHandle,
    _ticker: Option<Task<()>>,
}

impl EventEmitter<HostEvent> for HostDialog {}

impl Focusable for HostDialog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// Whether the person can ask for the server: a name the core takes, a port, and the
/// EULA ticked by them. The errors by field when not.
pub fn check_choices(name: &str, port: &str, eula: bool) -> Result<(String, u16), String> {
    let name = name.trim();
    if let Some(problem) = name_problem(name) {
        return Err(t(match problem {
            NameProblem::Empty => M::NameTheServer,
            NameProblem::TooLong => M::HostNameTooLong,
            NameProblem::ControlCharacters => M::HostNameControl,
            NameProblem::Forbidden => M::HostNameForbidden,
        })
        .to_string());
    }
    let port = port
        .trim()
        .parse::<u16>()
        .ok()
        .filter(|p| *p > 0)
        .ok_or_else(|| t(M::HostPortInvalid).to_string())?;
    if !eula {
        return Err(t(M::AgreeEulaFirst).to_string());
    }
    Ok((name.to_string(), port))
}

impl HostDialog {
    pub fn new(
        instance_id: String,
        instance_name: String,
        cores: Vec<HostTarget>,
        worlds: Option<Vec<HostWorld>>,
        blocked: Option<Text>,
        cx: &mut Context<Self>,
    ) -> Self {
        let core = cores.first().map(|c| c.uuid.clone()).unwrap_or_default();
        let username = cores.first().and_then(|c| c.username.clone());
        // a window closed mid-way stops what runs: the summary's file goes with the dialog
        cx.on_release(|this: &mut Self, _| {
            if let HostStep::Working { cancel, .. } = &this.step {
                cancel.cancel();
            }
        })
        .detach();
        Self {
            instance_id,
            instance_name,
            send: worlds
                .as_deref()
                .map(hosting::default_worlds)
                .unwrap_or_default(),
            worlds,
            cores,
            core,
            username: cx.new(|cx| {
                let input = TextInput::new(cx)
                    .with_placeholder(M::Username)
                    .with_automation_id("host-username");
                match username {
                    Some(username) => input.with_text(username),
                    None => input,
                }
            }),
            password: cx.new(|cx| {
                TextInput::new(cx)
                    .with_placeholder(M::Password)
                    .with_automation_id("host-password")
                    .with_masked()
            }),
            max_ram: hosting::DEFAULT_MAX_RAM,
            blocked,
            step: HostStep::Setup,
            error: None,
            focus_handle: cx.focus_handle(),
            _ticker: None,
        }
    }

    /// The instance's worlds, once read; the last played is sent unless others are picked.
    pub fn set_worlds(&mut self, worlds: Vec<HostWorld>, cx: &mut Context<Self>) {
        self.send = hosting::default_worlds(&worlds);
        self.worlds = Some(worlds);
        cx.notify();
    }

    /// Stops what runs while the instance is written and sent: closing the dialog does,
    /// so nothing goes on being exported and uploaded for a window that is gone.
    pub fn stop_working(&mut self) {
        if let HostStep::Working { cancel, .. } = &self.step {
            cancel.cancel();
        }
    }

    /// Back to the first step, saying why.
    pub fn fail(&mut self, error: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.step = HostStep::Setup;
        self._ticker = None;
        self.error = Some(error.into());
        cx.notify();
    }

    /// Writing and sending: redrawn as the bytes move.
    pub fn start_working(
        &mut self,
        progress: Arc<HostProgress>,
        cancel: CancelToken,
        cx: &mut Context<Self>,
    ) {
        self.error = None;
        self.step = HostStep::Working { progress, cancel };
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
        cx.notify();
    }

    /// The file the host read, with the port it offers.
    pub fn show_summary(
        &mut self,
        uploaded: hosting::Uploaded,
        admin: AdminClient,
        core_uuid: String,
        port: u16,
        cx: &mut Context<Self>,
    ) {
        self._ticker = None;
        let summary = &uploaded.upload.summary;
        let world = sharing::default_world(summary);
        let server = summary.server.clone();
        let name = cx.new(|cx| {
            TextInput::new(cx)
                .with_text(sharing::server_name(&self.instance_name))
                .with_label(M::ServerName)
                .with_automation_id("host-server-name")
        });
        let port = cx.new(|cx| {
            TextInput::new(cx)
                .with_text(port.to_string())
                .with_label(M::HostPort)
                .with_automation_id("host-port")
        });
        self.step = HostStep::Summary(Box::new(Summary {
            uploaded: Some(uploaded),
            admin,
            core_uuid,
            name,
            port,
            world,
            server,
            world_menu: false,
            eula: false,
            submitting: false,
        }));
        cx.notify();
    }

    /// The creation was asked for; it runs on the host.
    pub fn creating(&mut self, fraction: f64, cx: &mut Context<Self>) {
        self.error = None;
        self.step = HostStep::Creating { fraction };
        cx.notify();
    }

    /// The core refused the creation: the summary stays, with why.
    pub fn refused(
        &mut self,
        uploaded: hosting::Uploaded,
        error: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        if let HostStep::Summary(summary) = &mut self.step {
            summary.uploaded = Some(uploaded);
            summary.submitting = false;
        }
        self.error = Some(error.into());
        cx.notify();
    }

    pub fn done(&mut self, done: HostDone, cx: &mut Context<Self>) {
        self.error = None;
        self.step = HostStep::Done(done);
        cx.notify();
    }

    /// A creation that was asked for didn't end well: the dialog ends, saying why.
    pub fn ended(&mut self, why: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.error = None;
        self._ticker = None;
        self.step = HostStep::Ended(why.into());
        cx.notify();
    }

    fn pick_core(&mut self, uuid: String, cx: &mut Context<Self>) {
        if !matches!(self.step, HostStep::Setup) {
            return;
        }
        if let Some(username) = self
            .cores
            .iter()
            .find(|c| c.uuid == uuid)
            .and_then(|c| c.username.clone())
        {
            self.username
                .update(cx, |input, cx| input.set_text(username, cx));
        }
        self.core = uuid;
        cx.notify();
    }

    /// Whether this computer has the chosen core's owner key, so no password is asked.
    fn owner_of_core(&self) -> bool {
        self.cores.iter().any(|c| c.uuid == self.core && c.owner)
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.step, HostStep::Setup) || self.blocked.is_some() || self.worlds.is_none()
        {
            return;
        }
        let username = self.username.read(cx).text().trim().to_string();
        let password = self.password.read(cx).text().to_string();
        let owner = self.owner_of_core();
        if !owner && (username.is_empty() || password.is_empty()) {
            return self.fail(
                tr!(M::HostNeedsPassword, computer = t(CommonMsg::ThisComputer)),
                cx,
            );
        }
        self.password
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        cx.emit(HostEvent::Submit(HostRequest {
            core_uuid: self.core.clone(),
            sign_in: (!owner).then(|| (username, Secret::new(password))),
            worlds: self.send.clone(),
            max_ram: self.max_ram,
        }));
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if let HostStep::Working { cancel, .. } = &self.step {
            cancel.cancel();
        }
        cx.notify();
    }

    /// From the summary back to the first step: the host lets go of the file.
    fn back(&mut self, cx: &mut Context<Self>) {
        if let HostStep::Summary(summary) = &self.step
            && !summary.submitting
        {
            self.step = HostStep::Setup;
            self.error = None;
            cx.notify();
        }
    }

    fn create(&mut self, cx: &mut Context<Self>) {
        let HostStep::Summary(summary) = &mut self.step else {
            return;
        };
        if summary.submitting || summary.uploaded.is_none() {
            return;
        }
        let checked = check_choices(
            summary.name.read(cx).text(),
            summary.port.read(cx).text(),
            summary.eula,
        );
        let (name, port) = match checked {
            Ok(checked) => checked,
            Err(error) => {
                self.error = Some(error.into());
                cx.notify();
                return;
            }
        };
        let flavour = summary.uploaded.as_ref().and_then(|uploaded| {
            sharing::chosen_server(&uploaded.upload.summary, summary.server.as_deref())
        });
        summary.submitting = true;
        let choices = HostChoices {
            name,
            port,
            world: summary.world.clone(),
            flavour,
            min_ram: Some(hosting::DEFAULT_MIN_RAM.min(self.max_ram)),
            max_ram: Some(self.max_ram),
            // only ever what the person ticked
            accept_eula: summary.eula,
        };
        self.error = None;
        cx.emit(HostEvent::Create(choices));
        cx.notify();
    }

    /// Takes the file for the creation, with the core it is on.
    pub fn take_upload(&mut self) -> Option<(hosting::Uploaded, AdminClient, String)> {
        let HostStep::Summary(summary) = &mut self.step else {
            return None;
        };
        let uploaded = summary.uploaded.take()?;
        Some((uploaded, summary.admin.clone(), summary.core_uuid.clone()))
    }

    /// Fills in a summary with the EULA ticked, for the offscreen screenshots.
    pub fn preview_tick(&mut self, cx: &mut Context<Self>) {
        if let HostStep::Summary(summary) = &mut self.step {
            summary.eula = true;
        }
        cx.notify();
    }
}

/// The dialog's text for an error hosting ran into.
pub fn host_error(err: &HostError) -> String {
    match err {
        HostError::NeedsUpdate => t(M::HostTooOld).to_string(),
        HostError::Forbidden(_) => t(M::HostNoPermission).to_string(),
        // the core's own reason, in English: shown on one line
        HostError::Refused(why) => tr!(
            M::HostCreationFailed,
            error = launcher_core::images::display_text(why)
        ),
        HostError::Running => t(M::HostGameRunning).to_string(),
        HostError::Quilt => tr!(S::ImageUnsupported, loader = "Quilt"),
        HostError::Image(launcher_core::images::ImageError::NoRoom { needed, free }) => tr!(
            crate::i18n::InstanceSettingsMsg::ImportNoRoom,
            size = format::bytes(*needed),
            free = format::bytes(*free)
        ),
        HostError::Cancelled => t(CommonMsg::Cancel).to_string(),
        HostError::StillCreating => t(M::HostStillCreating).to_string(),
        HostError::Core(CoreError::SignedOut) => t(M::SignedOutRightAway).to_string(),
        HostError::Core(err) => super::core_error(err),
        HostError::Image(err) => err.to_string(),
    }
}

impl HostDialog {
    fn render_setup(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let entity = cx.entity();
        let core_choices: Vec<(String, SharedString)> = self
            .cores
            .iter()
            .map(|c| (c.uuid.clone(), c.name.clone().into()))
            .collect();
        let owner = self.owner_of_core();
        let pick_core = {
            let entity = entity.clone();
            move |uuid: String, cx: &mut App| entity.update(cx, |this, cx| this.pick_core(uuid, cx))
        };
        let memory: Vec<(u32, SharedString)> = MEMORY_CHOICES
            .iter()
            .map(|mb| (*mb, format::memory_mb((*mb).into()).into()))
            .collect();
        let pick_memory = {
            let entity = entity.clone();
            move |mb: u32, cx: &mut App| {
                entity.update(cx, |this, cx| {
                    this.max_ram = mb;
                    cx.notify();
                })
            }
        };
        let loading = self.worlds.is_none();
        let worlds: Vec<gpui::AnyElement> = self
            .worlds
            .iter()
            .flatten()
            .enumerate()
            .map(|(i, world)| {
                let folder = world.folder.clone();
                let checked = self.send.contains(&folder);
                let entity = entity.clone();
                // the name is the checkbox's label; the size stands apart, right-aligned
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(
                        div().flex_1().min_w_0().child(
                            Checkbox::new(
                                SharedString::from(format!("host-world-{i}")),
                                checked,
                                sanitize(&world.name, 120),
                            )
                            .on_change(move |value, _, cx| {
                                let (value, folder) = (*value, folder.clone());
                                entity.update(cx, |this, cx| {
                                    this.send.retain(|f| *f != folder);
                                    if value {
                                        this.send.push(folder);
                                    }
                                    cx.notify();
                                })
                            }),
                        ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_size(crate::theme::text_size(crate::theme::text::SMALL))
                            .child(Num::bytes(world.size).min_w(px(64.)).align_right()),
                    )
                    .into_any_element()
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(crate::instances::facts::named_card(&self.instance_name).id("host-instance"))
            .child(note(t(M::HostIntro)))
            .children(self.blocked.as_ref().map(|why| error_line(why.get())))
            .when(self.blocked.is_none(), |this| {
                this.child(label(t(M::CoreLabel)))
                    .child(pills("host-core", core_choices, &self.core, pick_core))
                    .when(!owner, |this| {
                        this.child(self.username.clone())
                            .child(self.password.clone())
                            .child(note(t(M::PasswordNotKept)))
                    })
                    .child(label(t(M::HostWorlds)))
                    .when(loading, |this| {
                        this.child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .text_color(crate::theme::ink::secondary())
                                .child(LoadingDots)
                                .child(note(t(M::HostWorldsLoading))),
                        )
                    })
                    .when(!loading && worlds.is_empty(), |this| {
                        this.child(note(t(M::HostNoWorlds)))
                    })
                    .when(!worlds.is_empty(), |this| {
                        this.child(div().flex().flex_col().gap_1().children(worlds))
                            .child(note(t(M::HostWorldsHint)))
                    })
                    .child(label(t(M::Memory)))
                    .child(pills("host-memory", memory, &self.max_ram, pick_memory))
            })
            .children(self.error.clone().map(error_line))
            .into_any_element()
    }

    fn render_working(&self, progress: &HostProgress) -> gpui::AnyElement {
        let snapshot = progress.bytes.snapshot();
        let fraction = if snapshot.bytes_total == 0 {
            0.0
        } else {
            snapshot.bytes_done as f32 / snapshot.bytes_total as f32
        };
        let text = match progress.stage() {
            HostStage::Exporting => t(M::StageExporting),
            HostStage::Uploading => t(M::HostStageSending),
            HostStage::Reading => t(M::HostStageReading),
        };
        // what was sent, in a box that doesn't resize as the numbers grow, and the
        // share of it
        let sent =
            (progress.stage() == HostStage::Uploading && snapshot.bytes_total > 0).then(|| {
                FactLine::row("host-sent")
                    .fact(Fact::bare(format::percent(
                        (fraction * 100.0).round().clamp(0.0, 100.0) as u64,
                    )))
                    .fact(Fact::new(
                        t(M::Sent),
                        Num::fraction_bytes(snapshot.bytes_done, snapshot.bytes_total)
                            .min_w(px(96.)),
                    ))
            });
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(crate::instances::facts::named_card(&self.instance_name).id("host-instance"))
            .child(note(t(M::HostIntro)))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .text_color(crate::theme::ink::secondary())
                    .child(LoadingDots)
                    .child(note(text)),
            )
            .when(progress.stage() != HostStage::Reading, |this| {
                this.child(progress_bar(fraction))
            })
            .children(sent)
            .into_any_element()
    }

    fn render_summary(&self, summary: &Summary, cx: &mut Context<Self>) -> gpui::AnyElement {
        let entity = cx.entity();
        let Some(uploaded) = &summary.uploaded else {
            return div().into_any_element();
        };
        let file = &uploaded.upload.summary;
        let lines = sharing::summary_lines(
            file,
            Picked {
                world: summary.world.as_deref(),
                server: summary.server.as_deref(),
                keep_operators: false,
                may_write_global: uploaded.may_write_global,
                // the person's own game: no maker line, no warning to trust its sender
                own_file: true,
            },
        );
        let busy = summary.submitting;
        // the world: a dropdown button showing the one picked, its list each world
        // with its size, and "No world"
        let mut worlds: Vec<(Option<String>, SharedString, Option<String>)> = file
            .worlds
            .iter()
            .map(|world| {
                (
                    Some(world.folder.clone()),
                    world.shown_name().to_string().into(),
                    Some(format::bytes(world.size)),
                )
            })
            .collect();
        worlds.push((None, t(S::ImageNoWorld).into(), None));
        let shown = worlds
            .iter()
            .find(|(folder, _, _)| *folder == summary.world)
            .map(|(_, name, _)| name.clone())
            .unwrap_or_default();
        let open = summary.world_menu && !busy;
        let set_menu = |entity: Entity<Self>, open: Option<bool>| {
            move |cx: &mut App| {
                entity.update(cx, |this, cx| {
                    if let HostStep::Summary(summary) = &mut this.step {
                        summary.world_menu = open.unwrap_or(!summary.world_menu);
                    }
                    cx.notify();
                })
            }
        };
        let mut button = select_button("host-pick-world", div().truncate().child(shown.clone()))
            .aria_label(t(S::ImageWorld))
            .aria_value(shown)
            .w_full()
            .justify_between();
        if !busy {
            let toggle = set_menu(entity.clone(), None);
            let press = set_menu(entity.clone(), None);
            button = button
                .expandable(open, move |_, cx| toggle(cx))
                .on_press(move |_, _, cx| press(cx));
        }
        let world_picker = Dropdown::new(button).when(open, |this| {
            let mut list = popover_panel("host-pick-world-list").min_w(px(280.));
            for (index, (folder, name, size)) in worlds.into_iter().enumerate() {
                let selected = folder == summary.world;
                let entity = entity.clone();
                let mut row = ListRow::new(("host-pick-world", index), name).selected(selected);
                if let Some(size) = size {
                    row = row.detail(size);
                }
                list = list.child(row.on_click(move |_, _, cx| {
                    let folder = folder.clone();
                    entity.update(cx, |this, cx| {
                        if let HostStep::Summary(summary) = &mut this.step {
                            summary.world = folder;
                            summary.world_menu = false;
                        }
                        cx.notify();
                    })
                }));
            }
            let close = set_menu(entity.clone(), Some(false));
            this.popover("host-pick-world-menu", list, move |_, cx| close(cx))
        });
        let software: Option<gpui::AnyElement> = (file.server_choices.len() > 1).then(|| {
            let choices: Vec<(String, SharedString)> = file
                .server_choices
                .iter()
                .map(|c| (c.clone(), c.clone().into()))
                .collect();
            let selected = summary.server.clone().unwrap_or_default();
            let entity = entity.clone();
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(label(t(S::ImageServerSoftware)))
                .child(pills(
                    "host-server-software",
                    choices,
                    &selected,
                    move |choice, cx| {
                        entity.update(cx, |this, cx| {
                            if let HostStep::Summary(summary) = &mut this.step {
                                summary.server = Some(choice);
                            }
                            cx.notify();
                        })
                    },
                ))
                .child(note(t(S::ImageServerSoftwareHint)))
                .into_any_element()
        });
        let eula = summary.eula;
        let toggle = entity.clone();
        div()
            .id("host-summary")
            .flex()
            .flex_col()
            .gap_3()
            .max_h(px(520.))
            .overflow_y_scroll()
            .children(lines.notes.into_iter().map(note))
            .children(lines.alerts.into_iter().map(|(level, text)| match level {
                Level::Error => error_line(text),
                Level::Warning => note(text).text_color(crate::theme::colors::YELLOW),
            }))
            .children(lines.details.into_iter().map(note))
            .children(software)
            .child(label(t(S::ImageWorld)))
            .child(world_picker)
            .child(note(t(S::ImageWorldHint)))
            .child(label(t(M::ServerName)))
            .child(summary.name.clone())
            .child(label(t(M::HostPort)))
            .child(summary.port.clone())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(
                        Toggle::new("host-eula", eula)
                            .disabled(busy)
                            .label(tr!(M::EulaAgree, url = EULA_URL))
                            .on_change(move |value, _, cx| {
                                let value = *value;
                                toggle.update(cx, |this, cx| {
                                    if let HostStep::Summary(summary) = &mut this.step {
                                        summary.eula = value;
                                    }
                                    cx.notify();
                                })
                            }),
                    )
                    .child(note(tr!(M::EulaAgree, url = EULA_URL)).flex_1().min_w_0())
                    .child(
                        Button::new("host-read-eula", t(M::ReadEula))
                            .variant(ButtonVariant::Ghost)
                            .on_click(|_, _, cx| cx.open_url(EULA_URL)),
                    ),
            )
            .children(self.error.clone().map(error_line))
            .into_any_element()
    }

    fn render_creating(&self, fraction: f64) -> gpui::AnyElement {
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
                    .text_color(crate::theme::ink::secondary())
                    .child(LoadingDots)
                    .child(note(t(M::StageCreating))),
            )
            .child(progress_bar(fraction as f32))
            .child(note(t(M::HostCreatingHint)))
            .into_any_element()
    }

    fn render_done(&self, done: &HostDone) -> gpui::AnyElement {
        // a port isn't grouped like a number
        let port = done.port.to_string();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                super::facts::server_card(&done.name)
                    .id("host-done-server")
                    .fact(Fact::bare_named(
                        t(FactMsg::Status),
                        StatusBadge::new(t(M::StateStopped), Tone::Neutral),
                    ))
                    .fact(super::facts::host_fact(&done.core_name))
                    .fact(Fact::new(t(FactMsg::Port), FactValue::mono(port))),
            )
            .child(
                note(t(if done.linked {
                    M::HostCreated
                } else {
                    M::HostCreatedUnlinked
                }))
                .text_color(crate::theme::ink::text()),
            )
            .children(done.relinked.as_ref().map(|server| {
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(note(t(M::HostCreatedRelinked)))
                    .child(
                        FactLine::row("host-done-relinked").fact(Fact::new(
                            t(M::LinkedBefore),
                            ObjectChip::new(sanitize(server, 120))
                                .kind(Kind::Server)
                                .muted(),
                        )),
                    )
            }))
            .when(!done.left_out.is_empty(), |this| {
                this.child(label(t(M::HostLeftOut)))
                    .children(done.left_out.iter().cloned().map(note))
            })
            .into_any_element()
    }
}

impl Render for HostDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let close = {
            let entity = entity.clone();
            move |_: &mut Window, cx: &mut App| {
                entity.update(cx, |_, cx| cx.emit(HostEvent::Dismissed))
            }
        };
        let dialog = Dialog::new("host-modpack", t(M::HostTitle), self.focus_handle.clone())
            .width(layout::dialog_width() + px(80.))
            .on_close(close);
        let dismiss = || {
            Button::new("host-close", t(CommonMsg::Close))
                .on_click(cx.listener(|_, _, _, cx| cx.emit(HostEvent::Dismissed)))
        };
        match &self.step {
            HostStep::Setup => {
                let body = self.render_setup(cx);
                dialog
                    .child(body)
                    .footer(
                        Button::new("host-submit", t(CommonMsg::Continue))
                            .variant(ButtonVariant::Primary)
                            .disabled(self.blocked.is_some() || self.worlds.is_none())
                            .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
                    )
                    .footer(
                        Button::new("host-cancel", t(CommonMsg::Cancel))
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(HostEvent::Dismissed))),
                    )
                    .into_any_element()
            }
            HostStep::Working { progress, .. } => {
                let progress = progress.clone();
                dialog
                    .child(self.render_working(&progress))
                    .footer(
                        Button::new("host-cancel", t(CommonMsg::Cancel))
                            .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                    )
                    .into_any_element()
            }
            HostStep::Summary(summary) => {
                let can_create = summary.can_create();
                let submitting = summary.submitting;
                // drawn from the step taken out for a moment: the summary reads the rest of
                // the dialog too
                let step = std::mem::replace(&mut self.step, HostStep::Setup);
                let body = match &step {
                    HostStep::Summary(summary) => self.render_summary(summary, cx),
                    _ => div().into_any_element(),
                };
                self.step = step;
                dialog
                    .child(body)
                    .footer(
                        Button::new("host-create", t(M::CreateServer))
                            .variant(ButtonVariant::Primary)
                            .loading(submitting)
                            .disabled(!can_create)
                            .on_click(cx.listener(|this, _, _, cx| this.create(cx))),
                    )
                    .footer(
                        Button::new("host-back", t(CommonMsg::Back))
                            .disabled(submitting)
                            .on_click(cx.listener(|this, _, _, cx| this.back(cx))),
                    )
                    .into_any_element()
            }
            HostStep::Creating { fraction } => dialog
                .child(self.render_creating(*fraction))
                .footer(dismiss())
                .into_any_element(),
            HostStep::Done(done) => dialog
                .child(self.render_done(done))
                .footer(
                    Button::new("host-show-servers", t(M::ShowOnServersPage))
                        .variant(ButtonVariant::Primary)
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(HostEvent::ShowServers))),
                )
                .footer(dismiss())
                .into_any_element(),
            HostStep::Ended(why) => dialog
                .child(error_line(why.clone()))
                .footer(
                    Button::new("host-show-servers", t(M::ShowOnServersPage))
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(HostEvent::ShowServers))),
                )
                .footer(dismiss().variant(ButtonVariant::Primary))
                .into_any_element(),
        }
    }
}

/// The core a hosting runs on: its client, how it signs in, and how it is named.
pub(crate) struct HostTo {
    pub client: launcher_core::cores::CoreClient,
    pub auth: HostAuth,
    pub core_name: String,
    pub core_url: String,
    pub core_uuid: String,
}

impl Servers {
    /// Opens "Host on Lodestone" for a launcher instance.
    pub fn open_host(&mut self, instance_id: &str, cx: &mut Context<Self>) {
        let instance = self
            .instances
            .read(cx)
            .list()
            .iter()
            .find(|i| i.id == instance_id)
            .cloned();
        let Some(instance) = instance else {
            return;
        };
        let cores: Vec<HostTarget> = self
            .cores
            .iter()
            .map(|c| HostTarget {
                uuid: c.saved.uuid.clone(),
                name: c.saved.name.clone(),
                username: c.saved.username.clone(),
                owner: self.owner_key(&c.saved.uuid).is_some(),
            })
            .collect();
        let quilt = instance.loader.kind() == Some(launcher_core::game::loaders::LoaderKind::Quilt);
        let blocked = if quilt {
            Some(Text::from(tr!(S::ImageUnsupported, loader = "Quilt")))
        } else {
            cores.is_empty().then(|| M::AddCoreFirst.into())
        };
        let store = self.instances.read(cx).store();
        let dirs = launcher_core::content::InstanceDirs::new(store.dir(&instance.id));
        let dialog = cx.new(|cx| {
            HostDialog::new(
                instance.id.clone(),
                instance.name.clone(),
                cores,
                None,
                blocked,
                cx,
            )
        });
        // Read away from the window: each world's folder is walked for its size.
        let read = cx.background_spawn(async move { hosting::worlds(&dirs) });
        let weak = dialog.downgrade();
        cx.spawn(async move |_, cx| {
            let worlds = read.await;
            let _ = weak.update(cx, |d, cx| d.set_worlds(worlds, cx));
        })
        .detach();
        let subscription = cx.subscribe(&dialog, |this, dialog, event, cx| {
            // a dialog that is fading out has nothing more to say
            if this.extras.host.current().map(|(open, _)| open) != Some(&dialog) {
                return;
            }
            match event {
                HostEvent::Dismissed => {
                    dialog.update(cx, |d, _| d.stop_working());
                    this.extras.host.close(cx);
                }
                HostEvent::ShowServers => {
                    this.extras.host.close(cx);
                    cx.emit(super::ServersEvent::ShowServers);
                }
                HostEvent::Submit(request) => this.host(&dialog, request.clone(), cx),
                HostEvent::Create(choices) => this.host_create(&dialog, choices.clone(), cx),
            }
        });
        self.extras.host.open((dialog, subscription), cx);
    }

    /// The owner key claimed on this computer for core `uuid`, which may create servers.
    pub(crate) fn owner_key(&self, uuid: &str) -> Option<Secret> {
        self.cores
            .iter()
            .find(|c| c.saved.uuid == uuid && c.saved.local_claim)?
            .key
            .clone()
    }

    /// The core `uuid` for hosting: its owner key, else `sign_in`. `None` when it is gone
    /// (`Err(false)`) or there is no way to sign in (`Err(true)`).
    pub(crate) fn host_to(
        &self,
        uuid: &str,
        sign_in: Option<(String, Secret)>,
    ) -> Result<HostTo, bool> {
        let index = self.core_index(uuid).ok_or(false)?;
        let auth = match self.owner_key(uuid) {
            Some(key) => HostAuth::OwnerKey(key),
            None => {
                let (username, password) = sign_in.ok_or(true)?;
                HostAuth::Password { username, password }
            }
        };
        let core = &self.cores[index];
        Ok(HostTo {
            client: self.client(core),
            auth,
            core_name: core.saved.name.clone(),
            core_url: display_address(&core.url),
            core_uuid: core.saved.uuid.clone(),
        })
    }

    /// Follows up a creation's end for `instance_id`: links it to the new server
    /// `server` (named `name`) on core `core_uuid` only when it was made
    /// (`hosting::link_when_made`), then shows the links and the core's servers again.
    pub(super) fn link_hosted(
        &mut self,
        instance_id: &str,
        (core_uuid, core_url): (&str, &str),
        (server, name): (&str, &str),
        ended: Result<hosting::CreationEnd, HostError>,
        cx: &mut Context<Self>,
    ) -> Result<hosting::Linked, HostError> {
        let dir = self.instances.read(cx).store().dir(instance_id);
        let link = hosting::host_link(core_uuid, core_url, server, name);
        let linked = hosting::link_when_made(&dir, ended, &link);
        if matches!(linked, Ok(hosting::Linked::Made { .. })) {
            self.reload_links(cx);
        }
        if let Some(index) = self.core_index(core_uuid) {
            self.refresh(index, cx);
        }
        cx.notify();
        linked
    }

    /// The first step's answers: signs in, asks the core whether it makes servers from
    /// files, writes the instance and sends it, and shows what the core read.
    fn host(&mut self, dialog: &Entity<HostDialog>, request: HostRequest, cx: &mut Context<Self>) {
        let to = match self.host_to(&request.core_uuid, request.sign_in.clone()) {
            Ok(to) => to,
            Err(false) => return dialog.update(cx, |d, cx| d.fail(t(M::CoreGone), cx)),
            Err(true) => {
                return dialog.update(cx, |d, cx| {
                    d.fail(
                        tr!(M::HostNeedsPassword, computer = t(CommonMsg::ThisComputer)),
                        cx,
                    )
                });
            }
        };
        let instance_id = dialog.read(cx).instance_id.clone();
        let instances = self.instances.read(cx);
        let store = instances.store();
        let images = instances.images();
        let temp_dir = instances.host_uploads_dir();
        let Ok(instance) = store.get(&instance_id) else {
            return dialog.update(cx, |d, cx| d.fail(t(M::InstanceGone), cx));
        };
        let progress = Arc::new(HostProgress::default());
        let cancel = CancelToken::new();
        dialog.update(cx, |d, cx| {
            d.start_working(progress.clone(), cancel.clone(), cx)
        });
        let core_uuid = to.core_uuid.clone();
        let work = cx.background_spawn(async move {
            let admin = to.auth.admin(to.client).await.map_err(HostError::from)?;
            hosting::probe(&admin).await?;
            let uploaded = hosting::upload(
                &admin,
                &images,
                &instance,
                &request.worlds,
                &temp_dir,
                &progress,
                &cancel,
            )
            .await?;
            // Cancel pressed while the core read the file: dropping the upload lets it go
            let cancelled = || {
                if cancel.is_cancelled() {
                    Err(HostError::Cancelled)
                } else {
                    Ok(())
                }
            };
            cancelled()?;
            let port = admin
                .free_port(hosting::FIRST_PORT, &cancel)
                .await
                .unwrap_or(hosting::FIRST_PORT);
            cancelled()?;
            Ok::<_, HostError>((uploaded, admin, port))
        });
        let dialog = dialog.downgrade();
        self.tasks.push(cx.spawn(async move |_, cx| {
            let result = work.await;
            let _ = dialog.update(cx, |d, cx| match result {
                Ok((uploaded, admin, port)) => d.show_summary(uploaded, admin, core_uuid, port, cx),
                Err(HostError::Cancelled) => {
                    d.step = HostStep::Setup;
                    d._ticker = None;
                    cx.notify();
                }
                Err(err) => d.fail(host_error(&err), cx),
            });
        }));
    }

    /// Asks the core to make the server, follows the creation, and links the instance to
    /// the server once it was made. This goes on when the dialog is closed.
    fn host_create(
        &mut self,
        dialog: &Entity<HostDialog>,
        choices: HostChoices,
        cx: &mut Context<Self>,
    ) {
        let Some((mut uploaded, admin, core_uuid)) = dialog.update(cx, |d, _| d.take_upload())
        else {
            return;
        };
        let instance_id = dialog.read(cx).instance_id.clone();
        let (core_name, core_url) = self
            .core_index(&core_uuid)
            .map(|i| {
                (
                    self.cores[i].saved.name.clone(),
                    display_address(&self.cores[i].url),
                )
            })
            .unwrap_or_default();
        let dialog = dialog.downgrade();
        let name = choices.name.clone();
        let port = choices.port;
        let created = cx.background_spawn({
            let admin = admin.clone();
            async move {
                let made = hosting::create(&admin, &mut uploaded, &choices).await;
                (made, uploaded)
            }
        });
        self.tasks.push(cx.spawn(async move |this, cx| {
            let (made, uploaded) = created.await;
            let uuid = match made {
                Ok(uuid) => uuid,
                Err(err) => {
                    let text = host_error(&err);
                    let _ = dialog.update(cx, |d, cx| d.refused(uploaded, text, cx));
                    return;
                }
            };
            drop(uploaded);
            let _ = dialog.update(cx, |d, cx| d.creating(0.0, cx));
            let fraction = Arc::new(std::sync::Mutex::new(0.0f64));
            let ended = cx.background_spawn({
                let (admin, uuid, fraction) = (admin.clone(), uuid.clone(), fraction.clone());
                async move {
                    hosting::wait_created(
                        &admin,
                        &uuid,
                        &|f| {
                            if let Ok(mut at) = fraction.lock() {
                                *at = f;
                            }
                        },
                        &CancelToken::new(),
                        &launcher_core::cores::player_image::tokio_sleep(),
                    )
                    .await
                }
            });
            let ticker = {
                let (dialog, fraction) = (dialog.clone(), fraction.clone());
                cx.spawn(async move |cx| {
                    loop {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(500))
                            .await;
                        let at = fraction.lock().map(|f| *f).unwrap_or_default();
                        if dialog
                            .update(cx, |d, cx| {
                                if matches!(d.step, HostStep::Creating { .. }) {
                                    d.creating(at, cx)
                                }
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                })
            };
            let ended = ended.await;
            drop(ticker);
            let _ = this.update(cx, |this, cx| {
                let linked = this.link_hosted(
                    &instance_id,
                    (&core_uuid, &core_url),
                    (&uuid, &name),
                    ended,
                    cx,
                );
                // Past here the server may exist: the dialog ends either way, never back
                // at the first step where Continue would send and make it again.
                let (outcome, relinked, linked) = match linked {
                    Ok(hosting::Linked::Made { end, previous }) => (end, previous, true),
                    Ok(hosting::Linked::InstanceGone { end }) => (end, None, false),
                    Ok(hosting::Linked::Failed(why)) => {
                        let text = match why.filter(|why| !why.trim().is_empty()) {
                            Some(why) => tr!(
                                M::HostCreationFailed,
                                error = launcher_core::images::display_text(&why)
                            ),
                            None => t(M::HostCreationFailedPlain).to_string(),
                        };
                        let _ = dialog.update(cx, |d, cx| d.ended(text, cx));
                        return;
                    }
                    Err(err) => {
                        let text = host_error(&err);
                        let _ = dialog.update(cx, |d, cx| d.ended(text, cx));
                        return;
                    }
                };
                let left_out = outcome
                    .outcome
                    .as_ref()
                    .map(sharing::outcome_lines)
                    .unwrap_or_else(|| {
                        // an older core says it only in English
                        outcome
                            .message
                            .as_deref()
                            .and_then(|m| m.strip_prefix("Server created successfully. "))
                            .map(|m| vec![launcher_core::images::display_text(m).into_owned()])
                            .unwrap_or_default()
                    });
                let _ = dialog.update(cx, |d, cx| {
                    d.done(
                        HostDone {
                            name: name.clone(),
                            core_name: core_name.clone(),
                            port,
                            relinked,
                            linked,
                            left_out,
                        },
                        cx,
                    )
                });
            });
        }));
    }

    /// Shows "Host on Lodestone" at a step, for the offscreen screenshots: `setup`,
    /// `uploading` (half of a 180 MB file sent), `summary` (a sample of what the host
    /// read, the EULA ticked), `summary-world` (the same, its world list open), `summary-no-global` (a file whose jars need global files,
    /// for a user who may not change them: Create stays off), `creating`, `outcome`
    /// (made, with files left out), `outcome-unlinked` (the same, the instance deleted
    /// meanwhile), `too-old` (a core from before milestone 6) and `still-creating` (the dialog's end
    /// when the host is still setting the server up).
    pub fn preview_host(&mut self, instance_id: &str, step: &str, cx: &mut Context<Self>) {
        self.open_host(instance_id, cx);
        let core_name = self
            .cores
            .first()
            .map(|c| c.saved.name.clone())
            .unwrap_or_default();
        let admin = self
            .cores
            .first()
            .map(|c| AdminClient::new(self.client(c), Secret::new("preview")));
        let Some((dialog, _)) = self.extras.host.current() else {
            return;
        };
        dialog.update(cx, |d, cx| match step {
            "uploading" => {
                let progress = Arc::new(HostProgress::preview_uploading());
                d.start_working(progress, CancelToken::new(), cx);
            }
            // `summary-world`: the same, its world list open
            "summary" | "summary-world" => {
                let core_uuid = d.core.clone();
                if let (Some(admin), Some(uploaded)) =
                    (admin, hosting::Uploaded::preview(sample_summary()))
                {
                    d.show_summary(uploaded, admin, core_uuid, 25566, cx);
                    d.preview_tick(cx);
                    if step == "summary-world"
                        && let HostStep::Summary(summary) = &mut d.step
                    {
                        summary.world_menu = true;
                    }
                }
            }
            "summary-no-global" => {
                let core_uuid = d.core.clone();
                let mut summary = sample_summary();
                summary["needs_global_files"] = serde_json::json!(true);
                if let (Some(admin), Some(mut uploaded)) =
                    (admin, hosting::Uploaded::preview(summary))
                {
                    uploaded.may_write_global = false;
                    d.show_summary(uploaded, admin, core_uuid, 25566, cx);
                    d.preview_tick(cx);
                }
            }
            "creating" => d.creating(0.6, cx),
            // `outcome-unlinked`: the instance was deleted while its server was made
            "outcome" | "outcome-unlinked" => {
                let linked = step == "outcome";
                let outcome: launcher_core::cores::admin::images::CreationOutcome =
                    serde_json::from_value(serde_json::json!({
                        "not_downloaded": {"names": ["mods/jade.jar"], "count": 1},
                        "no_source": {"names": ["mods/create.jar"], "count": 1}
                    }))
                    .unwrap_or_default();
                d.done(
                    HostDone {
                        name: d.instance_name.clone(),
                        core_name: core_name.clone(),
                        port: 25566,
                        relinked: linked.then(|| "Old SMP".into()),
                        linked,
                        left_out: sharing::outcome_lines(&outcome),
                    },
                    cx,
                );
            }
            "too-old" => d.fail(host_error(&HostError::NeedsUpdate), cx),
            "still-creating" => d.ended(host_error(&HostError::StillCreating), cx),
            _ => {}
        });
    }
}

/// What a host reads from a sample instance, for the summary's screenshot.
fn sample_summary() -> serde_json::Value {
    serde_json::json!({
        "image": "d18e86ae", "file_name": "Survival 1.20.1.lodeimg", "label": "Survival 1.20.1",
        "made_by": "launcher", "producer_version": "1.4.0",
        "minecraft": "1.20.1", "loader": "fabric", "loader_version": "0.16.10",
        "server": "Fabric", "server_choices": ["Fabric"],
        "mods_from_modrinth": {"names": ["lithium.jar", "ferritecore.jar"], "count": 22},
        "server_mods": 23,
        "carried_mods": {"names": ["my-own-mod.jar"], "count": 1},
        "client_only": {"names": ["sodium.jar", "iris.jar"], "count": 2},
        "worlds": [
            {"folder": "Island", "name": "Survival island", "size": 182_000_000u64, "last_played": 1_794_000_000}
        ],
        "default_world": "Island", "files": 640, "total_bytes": 260_000_000u64,
        "download_bytes": 14_000_000u64, "needed_bytes": 470_000_000u64,
        "disk_free": 182_000_000_000u64, "not_taken": ["client_files"]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_server_needs_a_name_a_port_and_the_eula_ticked() {
        crate::i18n::force(crate::i18n::Locale::En);
        assert_eq!(
            check_choices(" Survival ", "25566", true),
            Ok(("Survival".to_string(), 25566))
        );
        // the EULA is never taken as ticked
        assert_eq!(
            check_choices("Survival", "25566", false),
            Err(t(M::AgreeEulaFirst).to_string())
        );
        assert!(check_choices("", "25566", true).is_err());
        assert_eq!(
            check_choices("a/b", "25566", true),
            Err(t(M::HostNameForbidden).to_string())
        );
        assert_eq!(
            check_choices(&"a".repeat(101), "25566", true),
            Err(t(M::HostNameTooLong).to_string())
        );
        assert_eq!(
            check_choices("a\u{7}b", "25566", true),
            Err(t(M::HostNameControl).to_string())
        );
        assert!(check_choices("Survival", "0", true).is_err());
        assert!(check_choices("Survival", "70000", true).is_err());
        assert!(check_choices("Survival", "port", true).is_err());
    }
}

/// The dialog in a window: what backing out and closing do with the file the host holds.
#[cfg(all(test, feature = "screenshot"))]
mod dialog_tests {
    use std::io::{BufRead as _, BufReader, Write as _};
    use std::sync::Mutex;

    use gpui::TestAppContext;

    use super::*;

    /// A core that answers every request with `null` and records each request line.
    fn fake_core() -> (AdminClient, Arc<Mutex<Vec<String>>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        {
            let seen = seen.clone();
            std::thread::spawn(move || {
                for stream in listener.incoming().flatten() {
                    let mut reader = BufReader::new(stream);
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_err() {
                        continue;
                    }
                    // the headers, up to the empty line; these requests have no body
                    let mut header = String::new();
                    while reader.read_line(&mut header).is_ok_and(|n| n > 2) {
                        header.clear();
                    }
                    seen.lock().unwrap().push(line.trim().to_string());
                    let _ = reader.get_mut().write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 4\r\nConnection: close\r\n\r\nnull",
                    );
                }
            });
        }
        let base = url::Url::parse(&format!("http://127.0.0.1:{port}")).unwrap();
        let client = launcher_core::cores::CoreClient::new(
            launcher_core::http::default_client().unwrap(),
            base,
        );
        (AdminClient::new(client, Secret::new("owner-key")), seen)
    }

    fn held(admin: &AdminClient, id: &str) -> hosting::Uploaded {
        let mut upload: launcher_core::cores::admin::images::ImageUpload =
            serde_json::from_value(serde_json::json!({ "upload_id": id })).unwrap();
        upload.summary = serde_json::from_value(sample_summary()).unwrap();
        hosting::Uploaded::held(upload, admin.clone())
    }

    fn wait_for(seen: &Mutex<Vec<String>>, line: &str) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !seen.lock().unwrap().iter().any(|l| l == line) {
            assert!(std::time::Instant::now() < deadline, "{line} never came");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// Closing the dialog while the instance is written or sent stops that work: Escape
    /// (Dismissed) at once, and a window released without it too.
    #[test]
    fn closing_while_sending_stops_the_work() {
        crate::i18n::force(crate::i18n::Locale::En);
        let mut cx = TestAppContext::single();
        cx.update(crate::components::init);
        let new_dialog = |cx: &mut TestAppContext| {
            cx.update(|cx| {
                cx.new(|cx| {
                    HostDialog::new(
                        "inst".into(),
                        "Survival".into(),
                        Vec::new(),
                        Some(Vec::new()),
                        None,
                        cx,
                    )
                })
            })
        };
        let dialog = new_dialog(&mut cx);
        let cancel = CancelToken::new();
        dialog.update(&mut cx, |d, cx| {
            d.start_working(
                Arc::new(HostProgress::preview_uploading()),
                cancel.clone(),
                cx,
            );
            d.stop_working();
        });
        assert!(cancel.is_cancelled(), "Dismissed stops it");

        let dialog = new_dialog(&mut cx);
        let cancel = CancelToken::new();
        dialog.update(&mut cx, |d, cx| {
            d.start_working(
                Arc::new(HostProgress::preview_uploading()),
                cancel.clone(),
                cx,
            );
        });
        drop(dialog);
        cx.update(|_| {});
        cx.run_until_parked();
        assert!(cancel.is_cancelled(), "a released dialog stops it");
    }

    /// A summary starts with the EULA unticked and Create off; Back and closing the
    /// dialog both let go of the host's file.
    #[test]
    fn backing_out_or_closing_lets_go_of_the_hosts_file() {
        crate::i18n::force(crate::i18n::Locale::En);
        let mut cx = TestAppContext::single();
        cx.update(crate::components::init);
        let (admin, seen) = fake_core();
        let target = HostTarget {
            uuid: "core-1".into(),
            name: "Home".into(),
            username: None,
            owner: true,
        };
        let dialog = cx.update(|cx| {
            cx.new(|cx| {
                HostDialog::new(
                    "inst".into(),
                    "Survival".into(),
                    vec![target],
                    Some(Vec::new()),
                    None,
                    cx,
                )
            })
        });
        let show = |cx: &mut TestAppContext, id: &str| {
            let uploaded = held(&admin, id);
            dialog.update(cx, |d, cx| {
                d.show_summary(uploaded, admin.clone(), "core-1".into(), 25566, cx);
                let HostStep::Summary(summary) = &d.step else {
                    panic!("no summary");
                };
                assert!(!summary.eula, "the EULA starts unticked");
                assert!(!summary.can_create(), "Create starts off");
            });
        };

        show(&mut cx, "u1");
        dialog.update(&mut cx, |d, cx| d.back(cx));
        assert!(dialog.read_with(&cx, |d, _| matches!(d.step, HostStep::Setup)));
        wait_for(&seen, "DELETE /api/v1/instance/image/upload/u1 HTTP/1.1");

        // ticking the EULA turns Create on; closing the dialog lets go of the file
        show(&mut cx, "u2");
        dialog.update(&mut cx, |d, cx| {
            d.preview_tick(cx);
            let HostStep::Summary(summary) = &d.step else {
                panic!("no summary");
            };
            assert!(summary.can_create());
        });
        drop(dialog);
        // the window's next update releases what nothing holds any more
        cx.update(|_| {});
        cx.run_until_parked();
        wait_for(&seen, "DELETE /api/v1/instance/image/upload/u2 HTTP/1.1");
    }
}
