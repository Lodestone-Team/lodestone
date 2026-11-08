//! "Host on a Lodestone core": a launcher instance becomes a new server, from the
//! instance's Settings tab. See [`launcher_core::cores::hosting`] for the steps. The
//! server is created stopped and never started here.

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    Render, SharedString, Window, div, prelude::*,
};
use launcher_core::{
    Secret,
    cores::{
        CoreError,
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
    theme::{colors, layout},
};

/// The server's memory choices, in MB.
pub const MEMORY_CHOICES: [u32; 4] = [2048, 4096, 6144, 8192];
pub const EULA_URL: &str = "https://aka.ms/MinecraftEULA";

#[derive(Clone)]
pub struct HostRequest {
    pub core_uuid: String,
    pub username: String,
    pub password: Secret,
    pub name: String,
    pub max_ram: u32,
}

pub enum HostEvent {
    Dismissed,
    Submit(HostRequest),
    ShowServers,
}

pub struct HostDialog {
    pub instance_id: String,
    instance_name: String,
    /// `(uuid, name, username)` of each core.
    cores: Vec<(String, String, Option<String>)>,
    core: String,
    username: Entity<TextInput>,
    password: Entity<TextInput>,
    name: Entity<TextInput>,
    max_ram: u32,
    eula: bool,
    /// Why this instance can't be hosted.
    blocked: Option<SharedString>,
    pub stage: Option<SharedString>,
    pub error: Option<SharedString>,
    /// The server was created: the message.
    pub done: Option<SharedString>,
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
        cores: Vec<(String, String, Option<String>)>,
        blocked: Option<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        let core = cores.first().map(|c| c.0.clone()).unwrap_or_default();
        let username = cores.first().and_then(|c| c.2.clone());
        Self {
            instance_id,
            name: cx.new(|cx| TextInput::new(cx).with_text(instance_name.clone())),
            instance_name,
            cores,
            core,
            username: cx.new(|cx| {
                let input = TextInput::new(cx).with_placeholder("Username");
                match username {
                    Some(username) => input.with_text(username),
                    None => input,
                }
            }),
            password: cx.new(|cx| {
                TextInput::new(cx)
                    .with_placeholder("Password")
                    .with_masked()
            }),
            max_ram: hosting::DEFAULT_MAX_RAM,
            eula: false,
            blocked: blocked.map(Into::into),
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

    pub fn set_stage(&mut self, stage: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.stage = Some(stage.into());
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
        if let Some((_, _, Some(username))) = self.cores.iter().find(|c| c.0 == uuid) {
            let username = username.clone();
            self.username
                .update(cx, |input, cx| input.set_text(username, cx));
        }
        self.core = uuid;
        cx.notify();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.stage.is_some() || self.blocked.is_some() || self.done.is_some() {
            return;
        }
        let username = self.username.read(cx).text().trim().to_string();
        let password = self.password.read(cx).text().to_string();
        let name = self.name.read(cx).text().trim().to_string();
        if name.is_empty() {
            return self.fail("Give the server a name.", cx);
        }
        if username.is_empty() || password.is_empty() {
            return self.fail(
                "Enter your username and password for this core. Creating a server needs more than this Mac's key may do.",
                cx,
            );
        }
        if !self.eula {
            return self.fail(
                "Agree to the Minecraft EULA first: the core accepts it for the new server.",
                cx,
            );
        }
        self.password
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        self.error = None;
        self.stage = Some("Exporting the instance…".into());
        cx.emit(HostEvent::Submit(HostRequest {
            core_uuid: self.core.clone(),
            username,
            password: Secret::new(password),
            name,
            max_ram: self.max_ram,
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
        let dialog = Dialog::new(
            "host-modpack",
            "Host on a Lodestone core",
            self.focus_handle.clone(),
        )
        .width(layout::DIALOG_WIDTH + gpui::px(80.))
        .on_close(close);

        if let Some(done) = self.done.clone() {
            return dialog
                .child(note(done).text_color(colors::GRAY_300))
                .footer(
                    Button::new("host-show-servers", "Show on Servers page")
                        .variant(ButtonVariant::Primary)
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(HostEvent::ShowServers))),
                )
                .footer(
                    Button::new("host-close", "Close")
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(HostEvent::Dismissed))),
                )
                .into_any_element();
        }

        let core_choices: Vec<(String, SharedString)> = self
            .cores
            .iter()
            .map(|(uuid, name, _)| (uuid.clone(), name.clone().into()))
            .collect();
        let pick_core = {
            let entity = entity.clone();
            move |uuid: String, cx: &mut App| entity.update(cx, |this, cx| this.pick_core(uuid, cx))
        };
        let memory: Vec<(u32, SharedString)> = MEMORY_CHOICES
            .iter()
            .map(|mb| (*mb, format!("{} GB", mb / 1024).into()))
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
                    .child(note(format!(
                        "Makes a server from {}'s mods and configs, as a Modrinth modpack. Client-only mods from Modrinth are left out. The server is created stopped.",
                        self.instance_name
                    )))
                    .children(self.blocked.clone().map(error_line))
                    .when(self.blocked.is_none(), |this| {
                        this.child(label("Core"))
                            .child(pills("host-core", core_choices, &self.core, pick_core))
                            .child(self.username.clone())
                            .child(self.password.clone())
                            .child(note("Your password is sent once to sign in, and isn't kept."))
                            .child(label("Server name"))
                            .child(self.name.clone())
                            .child(label("Memory"))
                            .child(pills("host-memory", memory, &self.max_ram, pick_memory))
                            .child(note(format!(
                                "The game port is the first free one from {} on the core's machine.",
                                hosting::FIRST_PORT
                            )))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_3()
                                    .child(Toggle::new("host-eula", eula).disabled(busy).on_change(
                                        move |value, _, cx| {
                                            let value = *value;
                                            toggle_eula.update(cx, |this, cx| {
                                                this.eula = value;
                                                cx.notify();
                                            })
                                        },
                                    ))
                                    .child(
                                        note(format!(
                                            "I agree to the Minecraft EULA ({EULA_URL}). Lodestone Core accepts it for the new server."
                                        ))
                                        .flex_1()
                                        .min_w_0(),
                                    ),
                            )
                    })
                    .children(self.stage.clone().map(|stage| {
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .text_color(colors::GRAY_400)
                            .child(LoadingDots)
                            .child(note(stage))
                    }))
                    .children(self.error.clone().map(error_line)),
            )
            .footer(
                Button::new("host-submit", "Create server")
                    .variant(ButtonVariant::Primary)
                    .loading(busy)
                    .disabled(busy || self.blocked.is_some() || !eula)
                    .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
            )
            .footer(
                Button::new("host-cancel", "Cancel")
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
        let cores: Vec<(String, String, Option<String>)> = self
            .cores
            .iter()
            .map(|c| {
                (
                    c.saved.uuid.clone(),
                    c.saved.name.clone(),
                    c.saved.username.clone(),
                )
            })
            .collect();
        let blocked = hosting::check_hostable(&instance).err().or_else(|| {
            cores
                .is_empty()
                .then(|| "Add your Lodestone core on the Servers page first.".to_string())
        });
        let dialog = cx.new(|cx| {
            HostDialog::new(
                instance.id.clone(),
                instance.name.clone(),
                cores,
                blocked,
                cx,
            )
        });
        let subscription = cx.subscribe(&dialog, |this, dialog, event, cx| match event {
            HostEvent::Dismissed => {
                this.extras.host = None;
                cx.notify();
            }
            HostEvent::ShowServers => {
                this.extras.host = None;
                cx.emit(super::ServersEvent::ShowServers);
                cx.notify();
            }
            HostEvent::Submit(request) => this.host(&dialog, request.clone(), cx),
        });
        self.extras.host = Some((dialog, subscription));
        cx.notify();
    }

    fn host(&mut self, dialog: &Entity<HostDialog>, request: HostRequest, cx: &mut Context<Self>) {
        let Some(index) = self.core_index(&request.core_uuid) else {
            return dialog.update(cx, |d, cx| d.fail("That core is gone.", cx));
        };
        let instance_id = dialog.read(cx).instance_id.clone();
        let store = self.instances.read(cx).store();
        let Ok(instance) = store.get(&instance_id) else {
            return dialog.update(cx, |d, cx| d.fail("The instance is gone.", cx));
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
        let (stage_tx, mut stage_rx) = futures::channel::mpsc::unbounded::<String>();
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
            let _ = stage_tx.unbounded_send("Signing in…".into());
            let session = client.session(&request.username, &request.password).await?;
            drop(request.password);
            let port = client.free_port(&session, hosting::FIRST_PORT).await?;
            let _ = stage_tx.unbounded_send(format!(
                "Uploading the modpack ({:.1} MB)…",
                bytes.len() as f64 / (1024.0 * 1024.0)
            ));
            let file_name = format!("{}.mrpack", request.name.replace(['/', '\\', ':'], "-"));
            let uploaded = client.upload_pack(&session, &file_name, &bytes).await?;
            if let Some(reason) = uploaded.unsupported_reason {
                return Err(HostError::Other(reason));
            }
            let upload_id = uploaded
                .upload_id
                .ok_or_else(|| HostError::Other("The core didn't keep the modpack.".into()))?;
            let _ = stage_tx.unbounded_send("Creating the server…".into());
            let settings = HostSettings {
                name: request.name.clone(),
                port,
                min_ram: hosting::DEFAULT_MIN_RAM.min(request.max_ram),
                max_ram: request.max_ram,
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
                    let relinked = previous
                        .map(|link| format!(" This instance was linked to {}; it now belongs to the new server.", link.server_name))
                        .unwrap_or_default();
                    dialog.update(cx, |d, cx| {
                        d.stage = None;
                        d.done = Some(format!(
                            "{name} is on {core_name}, on port {port}. The core is setting it up now: it installs Fabric and downloads {downloads} mod file(s) from Modrinth.\n\nThe server is stopped. Start it from the Servers page or the dashboard when you're ready; this instance is ready to join it.{relinked}"
                        ).into());
                        cx.notify();
                    });
                    this.reload_links(cx);
                    if let Some(index) = this.core_index(&refresh_core) {
                        this.refresh(index, cx);
                    }
                    cx.notify();
                }
                Err(HostError::Core(CoreError::SignedOut)) => dialog.update(cx, |d, cx| {
                    d.fail("The core signed you out right after signing in. Try again.", cx)
                }),
                Err(HostError::Core(err)) => dialog.update(cx, |d, cx| d.fail(err.to_string(), cx)),
                Err(HostError::Other(err)) => dialog.update(cx, |d, cx| d.fail(err, cx)),
            });
        }));
    }

    /// Shows "Host on a Lodestone core" in a given state, for the offscreen screenshots.
    pub fn preview_host(
        &mut self,
        instance_id: &str,
        stage: Option<&str>,
        done: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        self.open_host(instance_id, cx);
        if let Some((dialog, _)) = &self.extras.host {
            dialog.update(cx, |d, cx| {
                d.preview("correct horse", true, cx);
                d.stage = stage.map(|s| s.to_string().into());
                d.done = done.map(|s| s.to_string().into());
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
