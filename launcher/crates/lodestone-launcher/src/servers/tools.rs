//! A server's world and resource pack, from the "More" button on its row.
//!
//! - **Resource pack**: what the server's `server.properties` names, read with this
//!   device's key, and "Download to instance", which saves it in the instance's
//!   `resourcepacks/` ([`resource_pack::download`]).
//! - **Download world**: the core zips the world (the server must be stopped) and the
//!   zip is imported into a chosen or new matching instance as a singleplayer copy
//!   ([`server_worlds::import`]). The device key can't read files, so this signs in
//!   once with the password; the session token is dropped after.

use std::{sync::Arc, time::Duration};

use gpui::{
    App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    Render, SharedString, Window, div, prelude::*,
};
use launcher_core::{
    Secret,
    cores::{
        ServerInstance, ServerState,
        resource_pack::{self, ServerResourcePack},
        worlds as server_worlds,
    },
    game::download::{CancelToken, Progress},
};

use super::{
    Servers,
    extras::{error_line, label, megabytes, note, pills},
};
use crate::{
    components::{Button, ButtonVariant, Dialog, Icon, LoadingDots, TextInput},
    theme::{colors, layout},
};

/// Where a download goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Instance(String),
    /// "Create matching instance" first.
    NewMatching,
}

#[derive(Clone, Debug)]
pub enum PackState {
    Loading,
    None,
    Pack(ServerResourcePack),
    Unavailable(SharedString),
}

#[derive(Clone, Debug)]
pub enum ToolBusy {
    World(SharedString, Arc<Progress>),
    Pack,
}

pub enum ToolsEvent {
    Dismissed,
    DownloadWorld {
        target: Target,
        username: String,
        password: Secret,
    },
    DownloadPack(Target),
}

/// The dialog of one server.
pub struct ServerTools {
    pub core_uuid: String,
    pub server: ServerInstance,
    /// The instances a download can go to: `(id, name)`.
    targets: Vec<(String, String)>,
    pub target: Target,
    username: Entity<TextInput>,
    password: Entity<TextInput>,
    pub pack: PackState,
    pub busy: Option<ToolBusy>,
    /// The last result: the message, and whether it is an error.
    pub result: Option<(SharedString, bool)>,
    focus_handle: FocusHandle,
}

impl EventEmitter<ToolsEvent> for ServerTools {}

impl Focusable for ServerTools {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ServerTools {
    pub fn new(
        core_uuid: String,
        server: ServerInstance,
        targets: Vec<(String, String)>,
        username: Option<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        let target = targets
            .first()
            .map(|(id, _)| Target::Instance(id.clone()))
            .unwrap_or(Target::NewMatching);
        Self {
            core_uuid,
            server,
            targets,
            target,
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
            pack: PackState::Loading,
            busy: None,
            result: None,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn set_pack(&mut self, pack: PackState, cx: &mut Context<Self>) {
        self.pack = pack;
        cx.notify();
    }

    pub fn finish(
        &mut self,
        message: impl Into<SharedString>,
        error: bool,
        cx: &mut Context<Self>,
    ) {
        self.busy = None;
        self.result = Some((message.into(), error));
        cx.notify();
    }

    /// Fills in the password, for the offscreen screenshots.
    pub fn preview_password(&mut self, password: &str, cx: &mut Context<Self>) {
        self.password
            .update(cx, |input, cx| input.set_text(password.to_string(), cx));
    }

    fn download_world(&mut self, cx: &mut Context<Self>) {
        if self.busy.is_some() {
            return;
        }
        let username = self.username.read(cx).text().trim().to_string();
        let password = self.password.read(cx).text().to_string();
        if username.is_empty() || password.is_empty() {
            self.result = Some((
                "Enter your username and password for this core. Downloading a world needs permission to read the server's files, which this Mac's key doesn't have.".into(),
                true,
            ));
            cx.notify();
            return;
        }
        // the password leaves the field at once
        self.password
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        self.result = None;
        cx.emit(ToolsEvent::DownloadWorld {
            target: self.target.clone(),
            username,
            password: Secret::new(password),
        });
    }

    fn target_name(&self) -> String {
        match &self.target {
            Target::Instance(id) => self
                .targets
                .iter()
                .find(|(i, _)| i == id)
                .map(|(_, name)| name.clone())
                .unwrap_or_default(),
            Target::NewMatching => "a new instance".into(),
        }
    }
}

impl Render for ServerTools {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let busy = self.busy.is_some();
        let stopped = matches!(self.server.state, ServerState::Stopped | ServerState::Error);

        let mut choices: Vec<(Target, SharedString)> = self
            .targets
            .iter()
            .map(|(id, name)| (Target::Instance(id.clone()), name.clone().into()))
            .collect();
        choices.push((Target::NewMatching, "New matching instance".into()));
        let pick = {
            let entity = entity.clone();
            move |target: Target, cx: &mut App| {
                entity.update(cx, |this, cx| {
                    if this.busy.is_none() {
                        this.target = target;
                        cx.notify();
                    }
                })
            }
        };

        let pack_section = match &self.pack {
            PackState::Loading => div().child(LoadingDots).into_any_element(),
            PackState::None => note("This server doesn't set a resource pack.").into_any_element(),
            PackState::Unavailable(why) => note(why.clone()).into_any_element(),
            PackState::Pack(pack) => div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(Icon::Cube.render(gpui::px(14.), colors::GRAY_400))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_color(colors::GRAY_300)
                                .child(pack.label()),
                        )
                        .child(
                            Button::new("download-server-pack", "Download to instance")
                                .icon(Icon::Download)
                                .loading(matches!(self.busy, Some(ToolBusy::Pack)))
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if this.busy.is_none() {
                                        this.result = None;
                                        cx.emit(ToolsEvent::DownloadPack(this.target.clone()));
                                    }
                                })),
                        ),
                )
                .child(note(format!(
                    "{}{}Minecraft downloads it when you join; saving it in {} makes it ready offline too.",
                    if pack.required {
                        "Players must accept it to join. "
                    } else {
                        ""
                    },
                    if pack.sha1.is_empty() {
                        "The server gives no SHA-1 for it. "
                    } else {
                        ""
                    },
                    self.target_name()
                )))
                .into_any_element(),
        };

        let world_progress = match &self.busy {
            Some(ToolBusy::World(stage, progress)) => {
                let snapshot = progress.snapshot();
                let text = if snapshot.bytes_done > 0 {
                    if snapshot.bytes_total > 0 {
                        format!(
                            "{stage} {} of {}",
                            megabytes(snapshot.bytes_done),
                            megabytes(snapshot.bytes_total)
                        )
                    } else {
                        format!("{stage} {}", megabytes(snapshot.bytes_done))
                    }
                } else {
                    stage.to_string()
                };
                Some(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .text_color(colors::GRAY_400)
                        .child(LoadingDots)
                        .child(note(text)),
                )
            }
            _ => None,
        };

        Dialog::new(
            "server-tools",
            self.server.name.clone(),
            self.focus_handle.clone(),
        )
        .width(layout::DIALOG_WIDTH + gpui::px(80.))
        .on_close({
            let entity = entity.clone();
            move |_, cx| entity.update(cx, |_, cx| cx.emit(ToolsEvent::Dismissed))
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(label("Instance"))
                .child(pills("tools-target", choices, &self.target, pick))
                .child(label("Resource pack"))
                .child(pack_section)
                .child(label("World"))
                .child(note(if stopped {
                    "A singleplayer copy of the server's world, in a new world folder. Changes you make in it stay on this Mac."
                } else {
                    "Stop the server to download its world: a running server can be writing to it."
                }))
                .child(self.username.clone())
                .child(self.password.clone())
                .child(note(
                    "Your password is sent once to sign in for this download, and isn't kept.",
                ))
                .children(world_progress)
                .children(self.result.clone().map(|(message, error)| {
                    if error {
                        error_line(message)
                    } else {
                        note(message).text_color(colors::GREEN_200)
                    }
                })),
        )
        .footer(
            Button::new("download-world", "Download world")
                .variant(ButtonVariant::Primary)
                .icon(Icon::Download)
                .loading(matches!(self.busy, Some(ToolBusy::World(..))))
                .disabled(busy || !stopped)
                .on_click(cx.listener(|this, _, _, cx| this.download_world(cx))),
        )
        .footer(
            Button::new("server-tools-close", "Close")
                .variant(ButtonVariant::Secondary)
                .on_click(cx.listener(|_, _, _, cx| cx.emit(ToolsEvent::Dismissed))),
        )
    }
}

impl Servers {
    /// Opens the server's dialog: the instances it can download into, and its pack.
    pub fn open_tools(&mut self, core_uuid: &str, server: ServerInstance, cx: &mut Context<Self>) {
        let Some(index) = self.core_index(core_uuid) else {
            return;
        };
        let linked = self.linked_instance(core_uuid, &server.uuid).cloned();
        let mut targets: Vec<(String, String)> = Vec::new();
        for instance in self.instances.read(cx).list() {
            if Some(&instance.id) == linked.as_ref() {
                targets.insert(0, (instance.id.clone(), instance.name.clone()));
            } else if instance.game_version == server.game_version {
                targets.push((instance.id.clone(), instance.name.clone()));
            }
        }
        let core = &self.cores[index];
        let username = core.saved.username.clone();
        let key = core.key.clone();
        let client = self.client(core);
        let uuid = server.uuid.clone();
        let tools =
            cx.new(|cx| ServerTools::new(core_uuid.to_string(), server, targets, username, cx));
        let subscription = cx.subscribe(&tools, |this, tools, event, cx| match event {
            ToolsEvent::Dismissed => {
                this.extras.tools = None;
                cx.notify();
            }
            ToolsEvent::DownloadWorld {
                target,
                username,
                password,
            } => this.download_world(
                &tools,
                target.clone(),
                username.clone(),
                password.clone(),
                cx,
            ),
            ToolsEvent::DownloadPack(target) => this.download_pack(&tools, target.clone(), cx),
        });
        self.extras.tools = Some((tools.clone(), subscription));
        cx.notify();
        if self.preview {
            return;
        }
        let Some(key) = key else {
            tools.update(cx, |t, cx| {
                t.set_pack(
                    PackState::Unavailable(
                        "Sign in to this core to see the server's resource pack.".into(),
                    ),
                    cx,
                )
            });
            return;
        };
        let fetch = cx.background_spawn(async move { client.resource_pack(&key, &uuid).await });
        self.tasks.push(cx.spawn(async move |_, cx| {
            let pack = match fetch.await {
                Ok(Some(pack)) => PackState::Pack(pack),
                Ok(None) => PackState::None,
                Err(err) => PackState::Unavailable(err.to_string().into()),
            };
            tools.update(cx, |t, cx| t.set_pack(pack, cx));
        }));
    }

    /// The instance a download goes to; "New matching instance" creates it first.
    fn tool_target(
        &mut self,
        core_uuid: &str,
        server: &ServerInstance,
        target: &Target,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        let id = match target {
            Target::Instance(id) => id.clone(),
            Target::NewMatching => {
                let before = self.linked_instance(core_uuid, &server.uuid).cloned();
                self.create_instance(core_uuid, server.clone(), cx);
                match self.linked_instance(core_uuid, &server.uuid).cloned() {
                    Some(id) if Some(&id) != before.as_ref() => id,
                    // a server with client mods: the instance is made once its pack is
                    // downloaded (and, for a core that isn't trusted, the advisory is
                    // answered), so the download waits for it
                    _ if server.has_pack() && self.notice.is_none() => {
                        // the dialog closes so any prompt about the mods can show
                        self.extras.tools = None;
                        self.notice = Some((
                            "Creating the matching instance".into(),
                            "It is made once the server's mods are downloaded. Then open More… on the server again, pick it, and download again."
                                .into(),
                        ));
                        cx.notify();
                        return Err(String::new());
                    }
                    _ => {
                        return Err(self
                            .notice
                            .take()
                            .map(|(_, message)| message.to_string())
                            .unwrap_or_else(|| "Couldn't create the instance.".into()));
                    }
                }
            }
        };
        if self.instances.read(cx).is_active(&id) {
            return Err("Quit the game in that instance first.".into());
        }
        Ok(id)
    }

    /// Keeps the dialog's progress moving while it is busy.
    fn tick_tools(&mut self, tools: &Entity<ServerTools>, cx: &mut Context<Self>) {
        let tools = tools.downgrade();
        self.tasks.push(cx.spawn(async move |_, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let busy = tools
                    .update(cx, |t, cx| {
                        cx.notify();
                        t.busy.is_some()
                    })
                    .unwrap_or(false);
                if !busy {
                    break;
                }
            }
        }));
    }

    fn download_world(
        &mut self,
        tools: &Entity<ServerTools>,
        target: Target,
        username: String,
        password: Secret,
        cx: &mut Context<Self>,
    ) {
        let (core_uuid, server) = {
            let t = tools.read(cx);
            (t.core_uuid.clone(), t.server.clone())
        };
        let Some(index) = self.core_index(&core_uuid) else {
            return;
        };
        let client = self.client(&self.cores[index]);
        let instance_id = match self.tool_target(&core_uuid, &server, &target, cx) {
            Ok(id) => id,
            Err(message) => {
                return tools.update(cx, |t, cx| t.finish(message, true, cx));
            }
        };
        let instances = self.instances.read(cx);
        let dirs = instances.content_dirs(&instance_id);
        let instance_name = instances
            .list()
            .iter()
            .find(|i| i.id == instance_id)
            .map(|i| i.name.clone())
            .unwrap_or_default();
        let progress = Arc::new(Progress::default());
        let stage = |text: &str| ToolBusy::World(text.to_string().into(), progress.clone());
        tools.update(cx, |t, cx| {
            t.target = Target::Instance(instance_id.clone());
            t.busy = Some(stage("Signing in…"));
            cx.notify();
        });
        self.tick_tools(tools, cx);
        let (stage_tx, stage_rx) = futures::channel::mpsc::unbounded::<&'static str>();
        let work = cx.background_spawn({
            let progress = progress.clone();
            let server_name = server.name.clone();
            let uuid = server.uuid.clone();
            async move {
                let session = client.session(&username, &password).await?;
                drop(password);
                let _ = stage_tx.unbounded_send("The core is zipping the world…");
                let download = client.world_download(&session, &uuid).await?;
                drop(session);
                let _ = stage_tx.unbounded_send("Downloading");
                std::fs::create_dir_all(&dirs.instance).map_err(|e| e.to_string())?;
                let zip = dirs.instance.join(format!(
                    ".lodestone-server-world-{}.zip",
                    std::process::id()
                ));
                let cancel = CancelToken::new();
                let result = async {
                    client
                        .download_file(&download.key, &zip, 64 << 30, &progress, &cancel)
                        .await?;
                    let _ = stage_tx.unbounded_send("Importing…");
                    server_worlds::import(&dirs, &zip, &download, &server_name, &progress, &cancel)
                        .map_err(|e| ToolError::Other(e.to_string()))
                }
                .await;
                let _ = std::fs::remove_file(&zip);
                result
            }
        });
        let stages = {
            let tools = tools.downgrade();
            let progress = progress.clone();
            cx.spawn(async move |_, cx| {
                use futures::StreamExt as _;
                let mut stage_rx = stage_rx;
                while let Some(stage) = stage_rx.next().await {
                    let progress = progress.clone();
                    let _ = tools.update(cx, |t, cx| {
                        if t.busy.is_some() {
                            t.busy = Some(ToolBusy::World(stage.into(), progress));
                            cx.notify();
                        }
                    });
                }
            })
        };
        self.tasks.push(stages);
        let tools = tools.clone();
        self.tasks.push(cx.spawn(async move |this, cx| {
            let result = work.await;
            let (message, error) = match result {
                Ok(folder) => (
                    format!(
                        "Imported as the world \"{folder}\" in {instance_name}. Open it from Singleplayer; it's a copy, so changes don't go back to the server."
                    ),
                    false,
                ),
                Err(ToolError::Core(err)) => (err.to_string(), true),
                Err(ToolError::Other(err)) => (err, true),
            };
            tools.update(cx, |t, cx| t.finish(message, error, cx));
            let _ = this.update(cx, |_, cx| cx.notify());
        }));
    }

    fn download_pack(
        &mut self,
        tools: &Entity<ServerTools>,
        target: Target,
        cx: &mut Context<Self>,
    ) {
        let (core_uuid, server, pack) = {
            let t = tools.read(cx);
            let PackState::Pack(pack) = &t.pack else {
                return;
            };
            (t.core_uuid.clone(), t.server.clone(), pack.clone())
        };
        let instance_id = match self.tool_target(&core_uuid, &server, &target, cx) {
            Ok(id) => id,
            Err(message) => return tools.update(cx, |t, cx| t.finish(message, true, cx)),
        };
        let packs = self
            .instances
            .read(cx)
            .game_dir(&instance_id)
            .join("resourcepacks");
        let http = self.http.clone();
        tools.update(cx, |t, cx| {
            t.target = Target::Instance(instance_id.clone());
            t.busy = Some(ToolBusy::Pack);
            cx.notify();
        });
        let name = server.name.clone();
        let work = cx.background_spawn(async move {
            resource_pack::download(http, &pack, &name, &packs).await
        });
        let tools = tools.clone();
        self.tasks.push(cx.spawn(async move |_, cx| {
            let (message, error) = match work.await {
                Ok(path) => (
                    format!(
                        "Saved as {} in the instance's resource packs. Turn it on there to use it on other servers too.",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    false,
                ),
                Err(err) => (err, true),
            };
            tools.update(cx, |t, cx| t.finish(message, error, cx));
        }));
    }

    /// Shows a server's dialog in a given state, for the offscreen screenshots.
    pub fn preview_tools(
        &mut self,
        core_uuid: &str,
        server: ServerInstance,
        pack: PackState,
        busy: Option<ToolBusy>,
        result: Option<(&str, bool)>,
        cx: &mut Context<Self>,
    ) {
        self.open_tools(core_uuid, server, cx);
        if let Some((tools, _)) = &self.extras.tools {
            tools.update(cx, |t, cx| {
                t.pack = pack;
                t.busy = busy;
                t.result = result.map(|(m, e)| (m.to_string().into(), e));
                t.preview_password("correct horse", cx);
                cx.notify();
            });
        }
    }
}

/// Why a world download failed.
#[derive(Debug)]
enum ToolError {
    Core(launcher_core::cores::CoreError),
    Other(String),
}

impl From<launcher_core::cores::CoreError> for ToolError {
    fn from(err: launcher_core::cores::CoreError) -> Self {
        ToolError::Core(err)
    }
}

impl From<String> for ToolError {
    fn from(err: String) -> Self {
        ToolError::Other(err)
    }
}
