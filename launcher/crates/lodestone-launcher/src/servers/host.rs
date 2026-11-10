//! "Host on a Lodestone core": a launcher instance becomes a new server, from the
//! instance's Settings tab. See [`launcher_core::cores::hosting`] for the steps. The
//! server is created stopped and never started here. On a core this computer is the
//! owner of ("Connect as owner"), the owner key creates it, so no password is asked;
//! on any other core the user signs in once with their password.

use crate::theme::px;
use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    Render, SharedString, Window, div, prelude::*,
};
use launcher_core::{
    Secret,
    cores::{
        CoreClient, CoreError,
        client::display_address,
        hosting::{self, HostSettings},
        link,
    },
    export,
};

use super::{
    Servers,
    extras::{error_line, label, note, pills},
};
use crate::{
    components::{Button, ButtonVariant, Dialog, LoadingDots, TextInput, Toggle},
    i18n::{CommonMsg, ServersMsg as M, ServersPlural as P, Text, format, t, tr, trn},
    theme::layout,
};

/// The server's memory choices, in MB.
pub const MEMORY_CHOICES: [u32; 4] = [2048, 4096, 6144, 8192];
pub const EULA_URL: &str = "https://aka.ms/MinecraftEULA";

#[derive(Clone)]
pub struct HostRequest {
    pub core_uuid: String,
    /// The username and password, for a core this computer isn't its owner of.
    pub sign_in: Option<(String, Secret)>,
    pub name: String,
    pub max_ram: u32,
    /// The user ticked "I agree to the Minecraft EULA".
    pub accept_eula: bool,
}

/// How hosting may create a server on a core: with the owner key claimed on this
/// computer ("Connect as owner"), or with a session from the user's password, since
/// the key a password sign-in makes may only view, start and stop servers.
pub(super) enum HostAuth {
    OwnerKey(Secret),
    Password(String, Secret),
}

impl HostAuth {
    /// The token the hosting requests send: the owner key, or a new session, which
    /// the caller drops when done. The password is dropped here.
    pub(super) async fn token(self, client: &CoreClient) -> Result<Secret, CoreError> {
        match self {
            HostAuth::OwnerKey(key) => Ok(key),
            HostAuth::Password(username, password) => client.session(&username, &password).await,
        }
    }
}

/// What hosting is doing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostStage {
    Exporting,
    SigningIn,
    /// Uploading a modpack of this many bytes.
    Uploading(u64),
    Creating,
}

impl HostStage {
    fn label(&self) -> String {
        match self {
            HostStage::Exporting => t(M::StageExporting).into(),
            HostStage::SigningIn => t(M::StageSigningIn).into(),
            HostStage::Uploading(bytes) => {
                tr!(M::StageUploading, size = format::bytes_binary(*bytes))
            }
            HostStage::Creating => t(M::StageCreating).into(),
        }
    }
}

/// The server that was created.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostDone {
    pub name: String,
    pub core_name: String,
    pub port: u16,
    /// The mod files the core downloads from Modrinth.
    pub downloads: u64,
    /// The server the instance was linked to before.
    pub relinked: Option<String>,
}

impl HostDone {
    fn label(&self) -> String {
        // a port isn't grouped like a number
        let port = self.port.to_string();
        match &self.relinked {
            Some(server) => trn!(
                P::HostDoneRelinked,
                self.downloads,
                name = self.name,
                core = self.core_name,
                port = port,
                server = server
            ),
            None => trn!(
                P::HostDone,
                self.downloads,
                name = self.name,
                core = self.core_name,
                port = port
            ),
        }
    }
}

pub enum HostEvent {
    Dismissed,
    Submit(HostRequest),
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

pub struct HostDialog {
    pub instance_id: String,
    instance_name: String,
    cores: Vec<HostTarget>,
    core: String,
    username: Entity<TextInput>,
    password: Entity<TextInput>,
    name: Entity<TextInput>,
    max_ram: u32,
    eula: bool,
    /// Why this instance can't be hosted.
    blocked: Option<Text>,
    pub stage: Option<HostStage>,
    pub error: Option<SharedString>,
    /// The server was created.
    pub done: Option<HostDone>,
    focus_handle: FocusHandle,
}

impl EventEmitter<HostEvent> for HostDialog {}

impl Focusable for HostDialog {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl HostDialog {
    pub fn new(
        instance_id: String,
        instance_name: String,
        cores: Vec<HostTarget>,
        blocked: Option<Text>,
        cx: &mut Context<Self>,
    ) -> Self {
        let core = cores.first().map(|c| c.uuid.clone()).unwrap_or_default();
        let username = cores.first().and_then(|c| c.username.clone());
        Self {
            instance_id,
            name: cx.new(|cx| {
                TextInput::new(cx)
                    .with_text(instance_name.clone())
                    .with_label(M::ServerName)
                    .with_automation_id("host-server-name")
            }),
            instance_name,
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
            eula: false,
            blocked,
            stage: None,
            error: None,
            done: None,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn fail(&mut self, error: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.stage = None;
        self.error = Some(error.into());
        cx.notify();
    }

    pub fn set_stage(&mut self, stage: HostStage, cx: &mut Context<Self>) {
        self.stage = Some(stage);
        cx.notify();
    }

    /// Fills in the dialog, for the offscreen screenshots.
    pub fn preview(&mut self, password: &str, eula: bool, cx: &mut Context<Self>) {
        self.password
            .update(cx, |input, cx| input.set_text(password.to_string(), cx));
        self.eula = eula;
        cx.notify();
    }

    fn pick_core(&mut self, uuid: String, cx: &mut Context<Self>) {
        if self.stage.is_some() {
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
        if self.stage.is_some() || self.blocked.is_some() || self.done.is_some() {
            return;
        }
        let username = self.username.read(cx).text().trim().to_string();
        let password = self.password.read(cx).text().to_string();
        let name = self.name.read(cx).text().trim().to_string();
        if name.is_empty() {
            return self.fail(t(M::NameTheServer), cx);
        }
        let owner = self.owner_of_core();
        if !owner && (username.is_empty() || password.is_empty()) {
            return self.fail(
                tr!(M::HostNeedsPassword, computer = t(CommonMsg::ThisComputer)),
                cx,
            );
        }
        if !self.eula {
            return self.fail(t(M::AgreeEulaFirst), cx);
        }
        self.password
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        self.error = None;
        self.stage = Some(HostStage::Exporting);
        cx.emit(HostEvent::Submit(HostRequest {
            core_uuid: self.core.clone(),
            sign_in: (!owner).then(|| (username, Secret::new(password))),
            name,
            max_ram: self.max_ram,
            accept_eula: self.eula,
        }));
        cx.notify();
    }
}

impl Render for HostDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let busy = self.stage.is_some();
        let close = {
            let entity = entity.clone();
            move |_: &mut Window, cx: &mut App| {
                entity.update(cx, |_, cx| cx.emit(HostEvent::Dismissed))
            }
        };
        let dialog = Dialog::new("host-modpack", t(M::HostTitle), self.focus_handle.clone())
            .width(layout::dialog_width() + px(80.))
            .on_close(close);

        if let Some(done) = &self.done {
            return dialog
                .child(note(done.label()).text_color(crate::theme::ink::text()))
                .footer(
                    Button::new("host-show-servers", t(M::ShowOnServersPage))
                        .variant(ButtonVariant::Primary)
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(HostEvent::ShowServers))),
                )
                .into_any_element();
        }

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
                    if this.stage.is_none() {
                        this.max_ram = mb;
                        cx.notify();
                    }
                })
            }
        };
        let eula = self.eula;
        let toggle_eula = entity.clone();

        dialog
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(note(tr!(M::HostIntro, name = self.instance_name)))
                    .children(self.blocked.as_ref().map(|why| error_line(why.get())))
                    .when(self.blocked.is_none(), |this| {
                        this.child(label(t(M::CoreLabel)))
                            .child(pills("host-core", core_choices, &self.core, pick_core))
                            .when(!owner, |this| {
                                this.child(self.username.clone())
                                    .child(self.password.clone())
                                    .child(note(t(M::PasswordNotKept)))
                            })
                            .child(label(t(M::ServerName)))
                            .child(self.name.clone())
                            .child(label(t(M::Memory)))
                            .child(pills("host-memory", memory, &self.max_ram, pick_memory))
                            .child(note(tr!(M::GamePortHint, port = hosting::FIRST_PORT)))
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
                                                toggle_eula.update(cx, |this, cx| {
                                                    this.eula = value;
                                                    cx.notify();
                                                })
                                            }),
                                    )
                                    .child(
                                        note(tr!(M::EulaAgree, url = EULA_URL)).flex_1().min_w_0(),
                                    )
                                    .child(
                                        Button::new("host-read-eula", t(M::ReadEula))
                                            .variant(ButtonVariant::Ghost)
                                            .on_click(|_, _, cx| cx.open_url(EULA_URL)),
                                    ),
                            )
                    })
                    .children(self.stage.as_ref().map(|stage| {
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .text_color(crate::theme::ink::secondary())
                            .child(LoadingDots)
                            .child(note(stage.label()))
                    }))
                    .children(self.error.clone().map(error_line)),
            )
            .footer(
                Button::new("host-submit", t(M::CreateServer))
                    .variant(ButtonVariant::Primary)
                    .loading(busy)
                    .disabled(busy || self.blocked.is_some() || !eula)
                    .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
            )
            .footer(
                Button::new("host-cancel", t(CommonMsg::Cancel))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(HostEvent::Dismissed))),
            )
            .into_any_element()
    }
}

impl Servers {
    /// Opens "Host on a Lodestone core" for a launcher instance.
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
        let blocked = hosting::check_hostable(&instance)
            .err()
            .map(Text::from)
            .or_else(|| cores.is_empty().then(|| M::AddCoreFirst.into()));
        let dialog = cx.new(|cx| {
            HostDialog::new(
                instance.id.clone(),
                instance.name.clone(),
                cores,
                blocked,
                cx,
            )
        });
        let subscription = cx.subscribe(&dialog, |this, dialog, event, cx| {
            // a dialog that is fading out has nothing more to say
            if this.extras.host.current().map(|(open, _)| open) != Some(&dialog) {
                return;
            }
            match event {
                HostEvent::Dismissed => {
                    this.extras.host.close(cx);
                }
                HostEvent::ShowServers => {
                    this.extras.host.close(cx);
                    cx.emit(super::ServersEvent::ShowServers);
                }
                HostEvent::Submit(request) => this.host(&dialog, request.clone(), cx),
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

    /// How hosting on core `uuid` signs in: its owner key, else `sign_in`.
    pub(super) fn host_auth(
        &self,
        uuid: &str,
        sign_in: Option<(String, Secret)>,
    ) -> Option<HostAuth> {
        match self.owner_key(uuid) {
            Some(key) => Some(HostAuth::OwnerKey(key)),
            None => sign_in.map(|(username, password)| HostAuth::Password(username, password)),
        }
    }

    fn host(&mut self, dialog: &Entity<HostDialog>, request: HostRequest, cx: &mut Context<Self>) {
        let Some(index) = self.core_index(&request.core_uuid) else {
            return dialog.update(cx, |d, cx| d.fail(t(M::CoreGone), cx));
        };
        let Some(auth) = self.host_auth(&request.core_uuid, request.sign_in.clone()) else {
            return dialog.update(cx, |d, cx| {
                d.fail(
                    tr!(M::HostNeedsPassword, computer = t(CommonMsg::ThisComputer)),
                    cx,
                )
            });
        };
        let signs_in = matches!(auth, HostAuth::Password(..));
        let instance_id = dialog.read(cx).instance_id.clone();
        let store = self.instances.read(cx).store();
        let Ok(instance) = store.get(&instance_id) else {
            return dialog.update(cx, |d, cx| d.fail(t(M::InstanceGone), cx));
        };
        let game_dir = store.game_dir(&instance_id);
        let instance_dir = store.dir(&instance_id);
        let core = &self.cores[index];
        let client = self.client(core);
        let core_name = core.saved.name.clone();
        let core_url = display_address(&core.url);
        let core_uuid = core.saved.uuid.clone();
        let previous = link::load_link(&instance_dir);
        let refresh_core = core_uuid.clone();
        let (stage_tx, mut stage_rx) = futures::channel::mpsc::unbounded::<HostStage>();
        let work = cx.background_spawn(async move {
            let on_modrinth = export::modrinth_files(&instance_dir);
            let plan = export::plan(&instance, &game_dir, &on_modrinth)
                .map_err(|e| HostError::Other(e.to_string()))?;
            let file = instance_dir.join(format!(".lodestone-host-{}.mrpack", std::process::id()));
            let written = export::write(&plan, &file)
                .map_err(|e| e.to_string())
                .and_then(|()| hosting::read_pack_file(&file));
            let _ = std::fs::remove_file(&file);
            let bytes = written.map_err(HostError::Other)?;
            if signs_in {
                let _ = stage_tx.unbounded_send(HostStage::SigningIn);
            }
            let session = auth.token(&client).await?;
            let port = client.free_port(&session, hosting::FIRST_PORT).await?;
            let _ = stage_tx.unbounded_send(HostStage::Uploading(bytes.len() as u64));
            let file_name = format!("{}.mrpack", request.name.replace(['/', '\\', ':'], "-"));
            let uploaded = client.upload_pack(&session, &file_name, &bytes).await?;
            if let Some(reason) = uploaded.unsupported_reason {
                return Err(HostError::Other(reason));
            }
            let upload_id = uploaded
                .upload_id
                .ok_or_else(|| HostError::Other(t(M::PackNotKept).into()))?;
            let _ = stage_tx.unbounded_send(HostStage::Creating);
            let settings = HostSettings {
                name: request.name.clone(),
                port,
                min_ram: hosting::DEFAULT_MIN_RAM.min(request.max_ram),
                max_ram: request.max_ram,
                accept_eula: request.accept_eula,
            };
            let uuid = client
                .create_server(&session, &upload_id, &settings)
                .await?;
            drop(session);
            let server_link = hosting::host_link(&core_uuid, &core_url, &uuid, &request.name);
            link::save_link(&instance_dir, &server_link)
                .map_err(|e| HostError::Other(e.to_string()))?;
            Ok::<_, HostError>((request.name, port, uploaded.server_file_count))
        });
        let stages = {
            let dialog = dialog.downgrade();
            cx.spawn(async move |_, cx| {
                use futures::StreamExt as _;
                while let Some(stage) = stage_rx.next().await {
                    let _ = dialog.update(cx, |d, cx| {
                        if d.stage.is_some() {
                            d.set_stage(stage, cx)
                        }
                    });
                }
            })
        };
        self.tasks.push(stages);
        let dialog = dialog.clone();
        self.tasks.push(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok((name, port, downloads)) => {
                    dialog.update(cx, |d, cx| {
                        d.stage = None;
                        d.done = Some(HostDone {
                            name,
                            core_name,
                            port,
                            downloads: downloads.into(),
                            relinked: previous.map(|link| link.server_name),
                        });
                        cx.notify();
                    });
                    this.reload_links(cx);
                    if let Some(index) = this.core_index(&refresh_core) {
                        this.refresh(index, cx);
                    }
                    cx.notify();
                }
                Err(HostError::Core(CoreError::SignedOut)) => {
                    dialog.update(cx, |d, cx| d.fail(t(M::SignedOutRightAway), cx))
                }
                Err(HostError::Core(err)) => {
                    dialog.update(cx, |d, cx| d.fail(super::core_error(&err), cx))
                }
                Err(HostError::Other(err)) => dialog.update(cx, |d, cx| d.fail(err, cx)),
            });
        }));
    }

    /// Shows "Host on a Lodestone core" in a given state, for the offscreen screenshots.
    /// `stage` and `done` are the English the dialog shows then; it shows the same
    /// messages in the current language: uploading a 3.2 MB modpack, and the instance
    /// hosted on the first core, on port 25566, with 24 mod files.
    pub fn preview_host(
        &mut self,
        instance_id: &str,
        stage: Option<&str>,
        done: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        self.open_host(instance_id, cx);
        let core_name = self
            .cores
            .first()
            .map(|c| c.saved.name.clone())
            .unwrap_or_default();
        if let Some((dialog, _)) = self.extras.host.current() {
            dialog.update(cx, |d, cx| {
                d.preview("correct horse", true, cx);
                d.stage = stage.map(|_| HostStage::Uploading(3_355_443));
                d.done = done.map(|_| HostDone {
                    name: d.instance_name.clone(),
                    core_name,
                    port: 25566,
                    downloads: 24,
                    relinked: None,
                });
            });
        }
    }
}

/// Why hosting failed.
#[derive(Debug)]
enum HostError {
    Core(CoreError),
    Other(String),
}

impl From<CoreError> for HostError {
    fn from(err: CoreError) -> Self {
        HostError::Core(err)
    }
}
