//! The main window, laid out like a game launcher in the dashboard's style: the
//! top bar, a sidebar with the account box, the instances and Settings, and the
//! selected instance with its tabs and a large Play button.

mod account_box;
pub(crate) mod developer;
mod launcher_settings;
mod play;
mod settings_modal;
mod shared_settings;
mod sidebar;
mod storage_settings;
mod top_bar;
mod trace_state;
mod unsaved;

use crate::theme::px;
pub use launcher_settings::LauncherSettings;
use settings_modal::SettingsModal;
pub use settings_modal::SettingsPage;
pub use shared_settings::SharedSettings;
pub use storage_settings::{StorageEvent, StorageSettings};
pub use top_bar::TOP_BAR_HEIGHT;

use gpui::{
    AnyElement, App, ClipboardItem, Context, Entity, EntityId, FocusHandle, Focusable as _,
    IntoElement, Render, SharedString, Subscription, Task, Window, div, prelude::*,
};
use launcher_core::{
    game::crash::Cause,
    instances::{Instance, Loader},
};

use crate::{
    accounts::{
        Accounts,
        offline_dialog::{OfflineDialog, OfflineDialogDismissed},
        sign_in::{SignInDialog, SignInEvent, Stage},
    },
    components::{Button, ButtonVariant, Dialog, DialogSlot, Icon, focus, settings::Toast},
    i18n::{CommonMsg, ShellMsg as M, ShellPlural as P, format, t, tr, trn},
    instances::{
        CrashInfo, Instances, InstancesEvent,
        console::Console,
        content::{ContentEvent, ContentView},
        import::{ImportDialog, ImportDismissed},
        modpacks::UpdatePrompt,
        new_instance::{NewInstanceDialog, NewInstanceDismissed},
        settings::{DeleteRequested, HostRequested, InstanceSettings, LoaderChangePrompt},
        worlds::{WorldsEvent, WorldsView},
    },
    servers::{Servers, ServersEvent, view::ServersOverlay},
    theme::{self, colors, fonts, leading, radius, text, weights},
    updater::Updater,
};

/// What the main area shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Instance,
    /// Lodestone cores and their servers.
    Servers,
}

/// The tabs of an instance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InstanceTab {
    #[default]
    Play,
    Content,
    Worlds,
    Settings,
    Logs,
}

impl InstanceTab {
    pub const ALL: [InstanceTab; 5] = [
        InstanceTab::Play,
        InstanceTab::Content,
        InstanceTab::Worlds,
        InstanceTab::Settings,
        InstanceTab::Logs,
    ];

    pub fn label(self) -> &'static str {
        match self {
            InstanceTab::Play => t(M::Play),
            InstanceTab::Content => t(M::TabContent),
            InstanceTab::Worlds => t(M::TabWorlds),
            InstanceTab::Settings => t(M::Settings),
            InstanceTab::Logs => t(M::TabLogs),
        }
    }
}

/// A loader's name, or "Vanilla" in the current language for none.
pub(crate) fn loader_name(loader: Loader) -> &'static str {
    match loader.kind() {
        Some(kind) => kind.label(),
        None => t(M::Vanilla),
    }
}

/// The instance's loader with its version, "Fabric 0.16.14", or "Vanilla" in the
/// current language.
pub(crate) fn loader_label(instance: &Instance) -> String {
    match instance.loader.kind() {
        Some(_) => instance.loader_label(),
        None => t(M::Vanilla).to_string(),
    }
}

/// The dialog over the window, if any.
enum Modal {
    NewInstance {
        dialog: Entity<NewInstanceDialog>,
        _events: Subscription,
    },
    /// "Import from another launcher".
    Import {
        dialog: Entity<ImportDialog>,
        _events: Subscription,
    },
    Crash {
        crash: CrashInfo,
        focus: FocusHandle,
        copied: bool,
    },
    LaunchFailed {
        title: SharedString,
        message: SharedString,
        focus: FocusHandle,
    },
    DeleteInstance {
        id: String,
        name: String,
        focus: FocusHandle,
        error: Option<SharedString>,
    },
    SignIn {
        dialog: Entity<SignInDialog>,
        _events: Subscription,
    },
    Offline {
        dialog: Entity<OfflineDialog>,
        _events: Subscription,
    },
    RemoveAccount {
        id: String,
        username: String,
        focus: FocusHandle,
    },
    /// Saving a loader change that leaves mods behind or puts worlds at risk.
    LoaderChange {
        prompt: Box<LoaderChangePrompt>,
        focus: FocusHandle,
    },
    /// Moving a world to the Trash.
    DeleteWorld {
        folder: String,
        name: String,
        focus: FocusHandle,
    },
    /// Replacing a world with one of its backups.
    ReplaceWorld {
        folder: String,
        name: String,
        backup: launcher_core::worlds::Backup,
        focus: FocusHandle,
    },
    /// What a modpack update will do, before it does it.
    ModpackUpdate {
        prompt: Box<UpdatePrompt>,
        backup_worlds: bool,
        focus: FocusHandle,
    },
    /// The disclosure before "Sign in as the Minecraft Launcher" is turned on.
    EnableLauncherIdentity { focus: FocusHandle },
    /// Turning it off, with the accounts that used it.
    DisableLauncherIdentity {
        focus: FocusHandle,
        usernames: Vec<String>,
    },
    /// Save or discard unsaved settings.
    Unsaved {
        focus: FocusHandle,
        target: unsaved::UnsavedTarget,
    },
    /// Removing files from Settings > Storage.
    StorageClean {
        what: launcher_core::disk::Clean,
        bytes: u64,
        focus: FocusHandle,
    },
}

/// The root view of the launcher window.
pub struct LauncherShell {
    accounts: Entity<Accounts>,
    instances: Entity<Instances>,
    console: Entity<Console>,
    /// The Content tab: mods, resource packs and shader packs.
    content: Entity<ContentView>,
    /// The Worlds tab: worlds, backups and screenshots.
    worlds: Entity<WorldsView>,
    /// Settings > Instance defaults.
    launcher_settings: Entity<LauncherSettings>,
    /// The Servers page, and its dialogs over the window.
    servers: Entity<Servers>,
    servers_overlay: Entity<ServersOverlay>,
    /// The Play tab's slideshow, for an instance that shows one.
    slideshow: Entity<crate::instances::slideshow::Slideshow>,
    /// Settings > Shared game settings.
    shared_settings: Entity<SharedSettings>,
    /// Settings > Storage.
    storage: Entity<StorageSettings>,
    /// Updating the launcher: Settings > General, and the top bar's notice.
    updater: Entity<Updater>,
    /// The Settings tab of the selected instance, rebuilt when the selection changes.
    settings: Option<(Entity<InstanceSettings>, [Subscription; 4])>,
    page: Page,
    tab: InstanceTab,
    /// The launcher's Settings, a large dialog under the other dialogs.
    settings_modal: DialogSlot<LauncherShell, SettingsModal>,
    settings_page: SettingsPage,
    /// What the debug traces last recorded of the window.
    trace: trace_state::TraceState,
    /// The Settings pages are open over the page, in a narrow window.
    settings_nav_open: bool,
    /// The dialog over the window, and those still fading out.
    modal: DialogSlot<LauncherShell, Modal>,
    account_menu_open: bool,
    /// Settings > General: the Language list is open.
    language_menu_open: bool,
    /// Set by a mouse down on the top bar; the next mouse move starts a window drag.
    top_bar_drag_pending: bool,
    /// Tracked by the outermost element; focus goes here when it is lost, so keys
    /// still reach the window's handlers.
    root_focus: FocusHandle,
    _focus_lost: Option<Subscription>,
    /// Says what Undo or Redo changed, with the way back.
    undo_toast: Toast,
    _observe: Vec<Subscription>,
    /// Draws the window again every minute, so "5 minutes ago" stays true.
    _clock: Task<()>,
}

impl LauncherShell {
    pub fn new(
        accounts: Entity<Accounts>,
        instances: Entity<Instances>,
        cx: &mut Context<Self>,
    ) -> Self {
        let observe = vec![
            cx.observe(&accounts, |_, _, cx| cx.notify()),
            cx.observe(&instances, |this, _, cx| {
                this.sync_settings(cx);
                cx.notify();
            }),
            cx.subscribe(&instances, |this, _, event, cx| match event {
                InstancesEvent::Crashed(crash) => this.show_crash(crash.clone(), cx),
                InstancesEvent::LaunchFailed { title, message }
                | InstancesEvent::Notice { title, message } => {
                    let focus = cx.focus_handle();
                    this.modal.open(
                        Modal::LaunchFailed {
                            title: title.clone(),
                            message: message.clone(),
                            focus,
                        },
                        cx,
                    );
                }
                InstancesEvent::LoaderChanged(_) => {
                    // The Settings tab shows the loader it was built with; build it again.
                    this.settings = None;
                    this.sync_settings(cx);
                    cx.notify();
                }
            }),
        ];
        let console = cx.new(|cx| Console::new(instances.clone(), cx));
        let content = cx.new(|cx| ContentView::new(instances.clone(), cx));
        let mut observe = observe;
        observe.push(cx.subscribe(&content, |this, _, event, cx| match event {
            ContentEvent::NewFabricInstance => this.open_new_instance_with_loader(cx),
        }));
        let worlds = cx.new(|cx| WorldsView::new(instances.clone(), cx));
        observe.push(cx.subscribe(&worlds, |this, _, event, cx| {
            let modal = match event {
                WorldsEvent::Delete { folder, name } => Modal::DeleteWorld {
                    folder: folder.clone(),
                    name: name.clone(),
                    focus: cx.focus_handle(),
                },
                WorldsEvent::Replace {
                    folder,
                    name,
                    backup,
                } => Modal::ReplaceWorld {
                    folder: folder.clone(),
                    name: name.clone(),
                    backup: backup.clone(),
                    focus: cx.focus_handle(),
                },
            };
            this.modal.open(modal, cx);
        }));
        let launcher_settings = {
            let instances = instances.clone();
            cx.new(|cx| LauncherSettings::new(instances, cx))
        };
        let servers = crate::servers::create(&accounts, &instances, cx);
        let servers_overlay = cx.new(|cx| ServersOverlay::new(servers.clone(), cx));
        observe.push(cx.subscribe(&servers, |this, _, event, cx| match event {
            ServersEvent::OpenInstance(id) => this.select_instance(id, cx),
            ServersEvent::ShowServers => this.show(Page::Servers, cx),
            ServersEvent::Failed { title, message } => {
                let focus = cx.focus_handle();
                this.modal.open(
                    Modal::LaunchFailed {
                        title: title.clone(),
                        message: message.clone(),
                        focus,
                    },
                    cx,
                );
            }
        }));
        // the instance page shows the server an instance is linked to, and its progress
        observe.push(cx.observe(&servers, |_, _, cx| cx.notify()));
        let shared_settings = {
            let instances = instances.clone();
            cx.new(|cx| SharedSettings::new(instances, cx))
        };
        // The Settings dialog draws their "Saved" notices.
        observe.push(cx.observe(&launcher_settings, |_, _, cx| cx.notify()));
        observe.push(cx.observe(&shared_settings, |_, _, cx| cx.notify()));
        let storage = {
            let instances = instances.clone();
            cx.new(|_| StorageSettings::new(instances))
        };
        observe.push(cx.observe(&storage, |_, _, cx| cx.notify()));
        observe.push(cx.subscribe(&storage, |this, _, event, cx| match event {
            StorageEvent::DeleteInstance(id) => this.confirm_delete_instance(id, cx),
            StorageEvent::Confirm { what, bytes } => {
                let focus = cx.focus_handle();
                this.modal.open(
                    Modal::StorageClean {
                        what: *what,
                        bytes: *bytes,
                        focus,
                    },
                    cx,
                );
            }
        }));
        let updater = {
            let instances = instances.clone();
            cx.new(|cx| Updater::new(instances, cx))
        };
        observe.push(cx.observe(&updater, |_, _, cx| cx.notify()));
        let slideshow = {
            let instances = instances.clone();
            cx.new(|_| crate::instances::slideshow::Slideshow::new(instances))
        };
        let mut this = Self {
            slideshow,
            accounts,
            instances,
            console,
            content,
            worlds,
            launcher_settings,
            servers,
            servers_overlay,
            shared_settings,
            storage,
            updater,
            settings: None,
            page: Page::default(),
            tab: InstanceTab::default(),
            settings_modal: DialogSlot::new(|this: &mut LauncherShell| &mut this.settings_modal),
            settings_page: SettingsPage::default(),
            trace: Default::default(),
            settings_nav_open: false,
            modal: DialogSlot::new(|this: &mut LauncherShell| &mut this.modal),
            account_menu_open: false,
            language_menu_open: false,
            top_bar_drag_pending: false,
            root_focus: cx.focus_handle(),
            _focus_lost: None,
            undo_toast: Toast::default(),
            _observe: observe,
            _clock: cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(60))
                        .await;
                    if this.update(cx, |_, cx| cx.notify()).is_err() {
                        return;
                    }
                }
            }),
        };
        this.sync_settings(cx);
        this
    }

    pub fn instances(&self) -> &Entity<Instances> {
        &self.instances
    }

    pub fn updater(&self) -> &Entity<Updater> {
        &self.updater
    }

    pub fn console(&self) -> &Entity<Console> {
        &self.console
    }

    /// For the control API (`crate::control`).
    pub fn accounts(&self) -> &Entity<Accounts> {
        &self.accounts
    }

    pub fn content(&self) -> &Entity<ContentView> {
        &self.content
    }

    pub fn worlds(&self) -> &Entity<WorldsView> {
        &self.worlds
    }

    /// The open "New instance" dialog, for the offscreen screenshots.
    pub fn new_instance_dialog(&self) -> Option<&Entity<NewInstanceDialog>> {
        match self.modal.current() {
            Some(Modal::NewInstance { dialog, .. }) => Some(dialog),
            _ => None,
        }
    }

    /// Asks before applying a modpack update.
    pub fn show_modpack_update(&mut self, prompt: UpdatePrompt, cx: &mut Context<Self>) {
        self.discard_pending_update(cx);
        let focus = cx.focus_handle();
        self.modal.open(
            Modal::ModpackUpdate {
                prompt: Box::new(prompt),
                backup_worlds: true,
                focus,
            },
            cx,
        );
    }

    /// Removes the downloaded pack of an update prompt that is being closed unanswered.
    fn discard_pending_update(&mut self, cx: &mut Context<Self>) {
        if let Some(Modal::ModpackUpdate { prompt, .. }) = self.modal.current() {
            let prompt = prompt.clone();
            self.instances
                .update(cx, |instances, _| instances.discard_update(&prompt));
        }
    }

    fn render_modpack_update(
        &self,
        prompt: &UpdatePrompt,
        backup_worlds: bool,
        focus: FocusHandle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        const SHOWN: usize = 6;
        let plan = &prompt.plan;
        let entity = cx.entity();
        let list = |title: String, items: Vec<String>, color: theme::Color| {
            let more = items.len().saturating_sub(SHOWN);
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .text_size(theme::text_size(text::SMALL))
                        .font_weight(weights::BOLD.into())
                        .text_color(color)
                        .child(title),
                )
                .children(items.into_iter().take(SHOWN).map(|item| {
                    div()
                        .pl_3()
                        .truncate()
                        .text_size(theme::text_size(text::SMALL))
                        .text_color(colors::GRAY_400)
                        .child(item)
                }))
                .when(more > 0, |this| {
                    this.child(
                        div()
                            .pl_3()
                            .text_size(theme::text_size(text::SMALL))
                            .text_color(colors::GRAY_500)
                            .child(trn!(P::AndMore, more)),
                    )
                })
        };
        let count = |n: usize| format::number(n as u64);
        let name = |path: &String| path.rsplit('/').next().unwrap_or(path).to_string();
        let mut groups = Vec::new();
        if !plan.updated.is_empty() {
            groups.push(list(
                tr!(M::UpdatedFiles, count = count(plan.updated.len())),
                plan.updated
                    .iter()
                    .map(|(old, new)| {
                        if old == new {
                            name(new)
                        } else {
                            format!("{} → {}", name(old), name(new))
                        }
                    })
                    .collect(),
                colors::BLUE_200,
            ));
        }
        if !plan.added.is_empty() {
            groups.push(list(
                tr!(M::AddedFiles, count = count(plan.added.len())),
                plan.added.iter().map(name).collect(),
                colors::GREEN_300,
            ));
        }
        if !plan.removed.is_empty() {
            groups.push(list(
                tr!(M::RemovedFiles, count = count(plan.removed.len())),
                plan.removed.iter().map(name).collect(),
                colors::RED_200,
            ));
        }
        if !plan.replaced_own.is_empty() {
            groups.push(list(
                tr!(M::ReplacedOwnFiles, count = count(plan.replaced_own.len())),
                plan.replaced_own.clone(),
                colors::YELLOW_300,
            ));
        }
        let kept: Vec<String> = plan
            .kept_changed
            .iter()
            .chain(&plan.overrides_kept)
            .cloned()
            .collect();
        if !kept.is_empty() {
            groups.push(list(
                tr!(M::KeptFiles, count = count(kept.len())),
                kept,
                colors::GRAY_400,
            ));
        }
        if !plan.overrides_written.is_empty() {
            groups.push(list(
                tr!(M::WrittenFiles, count = count(plan.overrides_written.len())),
                plan.overrides_written.clone(),
                colors::GRAY_400,
            ));
        }
        let loader = match &plan.loader {
            Some(loader) => format!("{} {}", loader.kind.label(), loader.version),
            None => t(M::Vanilla).to_string(),
        };
        let summary = tr!(
            M::ModpackSummary,
            pack = plan.record.title,
            from = if prompt.from_version.is_empty() {
                "?"
            } else {
                &prompt.from_version
            },
            to = plan.record.version_number,
            game = plan.game_version,
            loader = loader,
            size = format::bytes_binary(plan.download_size()),
        );
        Dialog::new(
            "modpack-update",
            tr!(M::UpdateTitle, name = prompt.instance_name),
            focus,
        )
        .width(px(560.))
        .on_close({
            let entity = entity.clone();
            move |_, cx| {
                entity.update(cx, |this, cx| {
                    this.discard_pending_update(cx);
                    this.close_modal(cx)
                })
            }
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .text_color(colors::GRAY_400)
                .child(div().whitespace_normal().child(summary))
                .when(!plan.changes_files(), |this| {
                    this.child(div().child(t(M::NoFilesChange)))
                })
                .child(
                    div()
                        .id("modpack-update-plan")
                        .flex()
                        .flex_col()
                        .gap_2()
                        .max_h(px(260.))
                        .overflow_y_scroll()
                        .px_3()
                        .py_2()
                        .rounded(radius::LG)
                        .bg(colors::GRAY_900)
                        .children(groups),
                )
                .child(
                    div()
                        .whitespace_normal()
                        .text_size(theme::text_size(text::SMALL))
                        .child(t(M::ModpackUpdateSafety)),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            crate::components::Toggle::new("modpack-update-backup", backup_worlds)
                                .on_change(cx.listener(|this, value: &bool, _, cx| {
                                    if let Some(Modal::ModpackUpdate { backup_worlds, .. }) =
                                        this.modal.current_mut()
                                    {
                                        *backup_worlds = *value;
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .text_size(theme::text_size(text::SMALL))
                                .font_weight(weights::BOLD.into())
                                .min_w_0()
                                .child(t(M::BackUpEveryWorld)),
                        ),
                ),
        )
        .footer(
            Button::new("confirm-modpack-update", t(CommonMsg::Update))
                .variant(ButtonVariant::Primary)
                .on_click(cx.listener(|this, _, _, cx| {
                    let update = match this.modal.close(cx) {
                        Some(Modal::ModpackUpdate {
                            prompt,
                            backup_worlds,
                            ..
                        }) => Some(((**prompt).clone(), *backup_worlds)),
                        _ => None,
                    };
                    if let Some((prompt, backup_worlds)) = update {
                        this.instances.update(cx, |instances, cx| {
                            instances.apply_modpack_update(prompt, backup_worlds, cx)
                        });
                    }
                })),
        )
        .footer(
            Button::new("cancel-modpack-update", t(CommonMsg::Cancel))
                .variant(ButtonVariant::Secondary)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.discard_pending_update(cx);
                    this.close_modal(cx)
                })),
        )
        .into_any_element()
    }

    fn render_world_dialog(&self, modal: &Modal, cx: &mut Context<Self>) -> Option<AnyElement> {
        let entity = cx.entity();
        let close = move |_: &mut Window, cx: &mut gpui::App| {
            entity.update(cx, |this, cx| this.close_modal(cx))
        };
        Some(match modal {
            Modal::DeleteWorld {
                folder,
                name,
                focus,
            } => {
                let (folder, name) = (folder.clone(), name.clone());
                Dialog::new(
                    "delete-world",
                    tr!(M::DeleteNamed, name = name),
                    focus.clone(),
                )
                .on_close(close)
                .child(
                    div()
                        .text_color(colors::GRAY_400)
                        .whitespace_normal()
                        .child(tr!(M::DeleteWorldBody, trash = t(CommonMsg::TheTrash))),
                )
                .footer(
                    Button::new("confirm-delete-world", t(CommonMsg::MoveToTrash))
                        .variant(ButtonVariant::Danger)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let (folder, name) = (folder.clone(), name.clone());
                            this.worlds
                                .update(cx, |worlds, cx| worlds.delete_world(folder, name, cx));
                            this.close_modal(cx);
                        })),
                )
                .footer(
                    Button::new("cancel-delete-world", t(CommonMsg::Cancel))
                        .variant(ButtonVariant::Secondary)
                        .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx))),
                )
                .into_any_element()
            }
            Modal::ReplaceWorld {
                folder,
                name,
                backup,
                focus,
            } => {
                let (folder, backup) = (folder.clone(), backup.clone());
                let when = backup
                    .created
                    .map(crate::instances::worlds::when)
                    .unwrap_or_else(|| backup.file_name.clone());
                Dialog::new(
                    "replace-world",
                    tr!(M::ReplaceWorldTitle, name = name),
                    focus.clone(),
                )
                .on_close(close)
                .child(
                    div()
                        .text_color(colors::GRAY_400)
                        .whitespace_normal()
                        .child(tr!(
                            M::ReplaceWorldBody,
                            name = name,
                            when = when.to_lowercase(),
                            trash = t(CommonMsg::TheTrash),
                        )),
                )
                .footer(
                    Button::new("confirm-replace-world", t(M::BackUpAndReplace))
                        .variant(ButtonVariant::Danger)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let (folder, backup) = (folder.clone(), backup.clone());
                            this.worlds.update(cx, |worlds, cx| {
                                worlds.restore_replace(folder, backup, cx)
                            });
                            this.close_modal(cx);
                        })),
                )
                .footer(
                    Button::new("cancel-replace-world", t(CommonMsg::Cancel))
                        .variant(ButtonVariant::Secondary)
                        .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx))),
                )
                .into_any_element()
            }
            _ => return None,
        })
    }

    /// Rebuilds the Settings tab when another instance is selected.
    fn sync_settings(&mut self, cx: &mut Context<Self>) {
        let selected = self.instances.read(cx).selected().cloned();
        let current = self
            .settings
            .as_ref()
            .map(|(settings, _)| settings.read(cx).instance_id().to_string());
        // Another instance was selected while this one's Settings tab has unsaved
        // changes: ask first, and keep the tab until then.
        if let Some((view, _)) = &self.settings
            && current.as_deref() != selected.as_ref().map(|i| i.id.as_str())
            && view.read(cx).has_changes(cx)
        {
            let target = unsaved::UnsavedTarget::Instance(view.clone());
            self.ask_unsaved(target, None, cx);
            return;
        }
        match selected {
            None => self.settings = None,
            Some(instance) if current.as_deref() != Some(instance.id.as_str()) => {
                let instances = self.instances.clone();
                let servers = self.servers.clone();
                let settings =
                    cx.new(|cx| InstanceSettings::new(instances, servers, &instance, cx));
                let subscriptions = [
                    cx.subscribe(&settings, |this, _, DeleteRequested(id), cx| {
                        this.confirm_delete_instance(id, cx)
                    }),
                    cx.subscribe(&settings, |this, _, prompt: &LoaderChangePrompt, cx| {
                        this.show_loader_change(prompt.clone(), cx)
                    }),
                    cx.subscribe(&settings, |this, _, prompt: &UpdatePrompt, cx| {
                        this.show_modpack_update(prompt.clone(), cx)
                    }),
                    cx.subscribe(&settings, |this, _, HostRequested(id), cx| {
                        this.servers
                            .update(cx, |servers, cx| servers.open_host(id, cx))
                    }),
                ];
                self.settings = Some((settings, subscriptions));
            }
            Some(_) => {}
        }
    }

    pub fn shared_settings(&self) -> &Entity<SharedSettings> {
        &self.shared_settings
    }

    pub fn settings_view(&self) -> Option<&Entity<InstanceSettings>> {
        self.settings.as_ref().map(|(settings, _)| settings)
    }

    /// Shows the launch error dialog, for the offscreen screenshots.
    pub fn preview_launch_failed(&mut self, title: &str, message: &str, cx: &mut Context<Self>) {
        let focus = cx.focus_handle();
        self.modal.open(
            Modal::LaunchFailed {
                title: title.to_string().into(),
                message: message.to_string().into(),
                focus,
            },
            cx,
        );
    }

    /// Shows the crash dialog.
    pub fn show_crash(&mut self, crash: CrashInfo, cx: &mut Context<Self>) {
        let focus = cx.focus_handle();
        self.modal.open(
            Modal::Crash {
                crash,
                focus,
                copied: false,
            },
            cx,
        );
    }

    /// Asks before saving a loader change (from the Settings tab).
    pub fn show_loader_change(&mut self, prompt: LoaderChangePrompt, cx: &mut Context<Self>) {
        let focus = cx.focus_handle();
        self.modal.open(
            Modal::LoaderChange {
                prompt: Box::new(prompt),
                focus,
            },
            cx,
        );
    }

    /// Saves the change the dialog asked about, turning the mismatched mods off first if
    /// `turn_off`.
    fn finish_loader_change(&mut self, turn_off: bool, cx: &mut Context<Self>) {
        let Some(Modal::LoaderChange { prompt, .. }) = self.modal.current() else {
            return;
        };
        let prompt = (**prompt).clone();
        self.modal.close(cx);
        let disable: Vec<String> = if turn_off {
            prompt
                .check
                .mismatched
                .iter()
                .map(|m| m.file_name.clone())
                .collect()
        } else {
            Vec::new()
        };
        if let Some(settings) = self.settings_view().cloned() {
            settings.update(cx, |settings, cx| {
                settings.commit(prompt.instance, &disable, cx)
            });
        }
        cx.notify();
    }

    fn render_loader_change(
        &self,
        prompt: &LoaderChangePrompt,
        focus: FocusHandle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        const SHOWN: usize = 8;
        let to = loader_name(prompt.instance.loader);
        let check = &prompt.check;
        let title = match prompt.instance.loader.kind() {
            Some(_) => tr!(M::SwitchToLoader, loader = to),
            None => t(M::SwitchToVanilla).to_string(),
        };
        let mods_text = match prompt.instance.loader.kind() {
            Some(_) => tr!(M::ModsWontLoadOn, loader = to),
            None => t(M::ModsWontLoadVanilla).to_string(),
        };
        let has_mods = !check.mismatched.is_empty();
        let entity = cx.entity();
        Dialog::new("loader-change", title, focus)
            .on_close(move |_, cx| entity.update(cx, |this, cx| this.close_modal(cx)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .text_color(colors::GRAY_400)
                    .when(has_mods, |this| {
                        let more = check.mismatched.len().saturating_sub(SHOWN);
                        this.child(div().whitespace_normal().child(mods_text))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .px_3()
                                    .py_2()
                                    .rounded(radius::LG)
                                    .bg(colors::GRAY_900)
                                    .children(check.mismatched.iter().take(SHOWN).map(|m| {
                                        div()
                                            .flex()
                                            .flex_row()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .truncate()
                                                    .text_color(colors::GRAY_300)
                                                    .child(m.name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .flex_none()
                                                    .text_color(colors::GRAY_500)
                                                    .child(tr!(M::ModMadeFor, loader = m.made_for)),
                                            )
                                    }))
                                    .when(more > 0, |this| {
                                        this.child(
                                            div()
                                                .text_color(colors::GRAY_500)
                                                .child(trn!(P::AndMore, more)),
                                        )
                                    }),
                            )
                            .child(div().whitespace_normal().child(t(M::TurningOffRenames)))
                    })
                    .when(check.worlds > 0 && check.worlds_at_risk, |this| {
                        this.child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_2()
                                .px_3()
                                .py_2()
                                .rounded(radius::LG)
                                .border_1()
                                .border_color(colors::YELLOW_300.opacity(50))
                                .bg(colors::YELLOW_FADED.opacity(15))
                                .text_color(colors::GRAY_300)
                                .child(
                                    Icon::TriangleExclamation.render(px(14.), colors::YELLOW_300),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .whitespace_normal()
                                        .child(t(M::WorldsAtRisk)),
                                ),
                        )
                    }),
            )
            .when(has_mods, |this| {
                this.footer(
                    Button::new("loader-change-disable", t(M::TurnOffAndSave))
                        .variant(ButtonVariant::Primary)
                        .on_click(
                            cx.listener(|this, _, _, cx| this.finish_loader_change(true, cx)),
                        ),
                )
            })
            .footer(
                Button::new(
                    "loader-change-keep",
                    if has_mods {
                        t(M::SaveAndKeep)
                    } else {
                        t(CommonMsg::Save)
                    },
                )
                .variant(if has_mods {
                    ButtonVariant::Secondary
                } else {
                    ButtonVariant::Primary
                })
                .on_click(cx.listener(|this, _, _, cx| this.finish_loader_change(false, cx))),
            )
            .footer(
                Button::new("loader-change-cancel", t(CommonMsg::Cancel))
                    .variant(ButtonVariant::Ghost)
                    .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx))),
            )
            .into_any_element()
    }

    fn confirm_delete_instance(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(instance) = self
            .instances
            .read(cx)
            .list()
            .iter()
            .find(|i| i.id == id)
            .cloned()
        else {
            return;
        };
        let focus = cx.focus_handle();
        self.modal.open(
            Modal::DeleteInstance {
                id: instance.id,
                name: instance.name,
                focus,
                error: None,
            },
            cx,
        );
    }

    pub fn select_instance(&mut self, id: &str, cx: &mut Context<Self>) {
        self.show(Page::Instance, cx);
        self.instances
            .update(cx, |instances, cx| instances.select(id, cx));
    }

    pub fn show(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        // the Servers page polls its cores only while it is shown
        self.servers.update(cx, |servers, cx| {
            servers.set_visible(page == Page::Servers, cx)
        });
        cx.notify();
    }

    pub fn servers(&self) -> &Entity<Servers> {
        &self.servers
    }

    pub fn select_tab(&mut self, tab: InstanceTab, cx: &mut Context<Self>) {
        self.tab = tab;
        cx.notify();
    }

    fn set_account_menu(&mut self, open: bool, cx: &mut Context<Self>) {
        self.account_menu_open = open;
        cx.notify();
    }

    /// Closes the dialog; it fades out.
    fn close_modal(&mut self, cx: &mut Context<Self>) {
        // A loader change that wasn't confirmed: the Settings tab shows the saved loader.
        if matches!(self.modal.current(), Some(Modal::LoaderChange { .. }))
            && let Some(settings) = self.settings_view().cloned()
        {
            settings.update(cx, |settings, cx| settings.revert_loader(cx));
        }
        self.modal.close(cx);
    }

    /// Whether `dialog` is the open dialog's view. Events from a dialog that is fading
    /// out are ignored, so it can't close the dialog that replaced it.
    fn is_open_dialog(&self, dialog: EntityId) -> bool {
        match self.modal.current() {
            Some(Modal::NewInstance { dialog: open, .. }) => open.entity_id() == dialog,
            Some(Modal::Import { dialog: open, .. }) => open.entity_id() == dialog,
            Some(Modal::SignIn { dialog: open, .. }) => open.entity_id() == dialog,
            Some(Modal::Offline { dialog: open, .. }) => open.entity_id() == dialog,
            _ => false,
        }
    }

    fn open_new_instance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_new_instance_with(None, Some(window), cx);
    }

    /// Opens "New instance" with Fabric chosen, from the Content tab of a vanilla
    /// instance.
    fn open_new_instance_with_loader(&mut self, cx: &mut Context<Self>) {
        self.open_new_instance_with(None, None, cx);
        if let Some(Modal::NewInstance { dialog, .. }) = self.modal.current() {
            dialog.update(cx, |dialog, cx| {
                dialog.choose_loader(launcher_core::game::loaders::LoaderKind::Fabric, cx)
            });
        }
    }

    /// Opens "New instance", with the fields filled in for screenshots.
    pub fn open_new_instance_with(
        &mut self,
        fields: Option<(&str, &str)>,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        self.account_menu_open = false;
        let instances = self.instances.clone();
        let dialog = cx.new(|cx| {
            let dialog = NewInstanceDialog::new(instances, cx);
            match fields {
                Some((name, search)) => dialog.with_fields(name, search, cx),
                None => dialog,
            }
        });
        let subscription = cx.subscribe(&dialog, |this, dialog, _: &NewInstanceDismissed, cx| {
            if !this.is_open_dialog(dialog.entity_id()) {
                return;
            }
            this.show(Page::Instance, cx);
            this.tab = InstanceTab::Play;
            this.close_modal(cx)
        });
        if let Some(window) = window {
            window.focus(&dialog.focus_handle(cx), cx);
        }
        self.modal.open(
            Modal::NewInstance {
                dialog,
                _events: subscription,
            },
            cx,
        );
    }

    fn open_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let instances = self.instances.clone();
        let dialog = cx.new(|cx| ImportDialog::new(instances, cx));
        window.focus(&dialog.focus_handle(cx), cx);
        self.show_import(dialog, cx);
    }

    /// Shows the import dialog (a prepared one for the offscreen screenshots).
    pub fn show_import(&mut self, dialog: Entity<ImportDialog>, cx: &mut Context<Self>) {
        self.account_menu_open = false;
        let subscription = cx.subscribe(&dialog, |this, dialog, _: &ImportDismissed, cx| {
            if !this.is_open_dialog(dialog.entity_id()) {
                return;
            }
            this.page = Page::Instance;
            this.close_modal(cx)
        });
        self.modal.open(
            Modal::Import {
                dialog,
                _events: subscription,
            },
            cx,
        );
    }

    fn open_sign_in(&mut self, window: Option<&mut Window>, cx: &mut Context<Self>) {
        self.open_sign_in_at(Stage::Choose, window, cx);
    }

    /// Opens the sign-in dialog at `stage`. Only the screenshots start past
    /// [`Stage::Choose`].
    pub fn open_sign_in_at(
        &mut self,
        stage: Stage,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        self.account_menu_open = false;
        let accounts = self.accounts.clone();
        let dialog = cx.new(|cx| SignInDialog::new(accounts, cx).with_stage(stage));
        let subscription = cx.subscribe(&dialog, |this, dialog, event, cx| {
            if !this.is_open_dialog(dialog.entity_id()) {
                return;
            }
            match event {
                SignInEvent::Dismissed => this.close_modal(cx),
                SignInEvent::UseOffline => this.open_offline(None, None, cx),
            }
        });
        if let Some(window) = window {
            window.focus(&dialog.focus_handle(cx), cx);
        }
        self.modal.open(
            Modal::SignIn {
                dialog,
                _events: subscription,
            },
            cx,
        );
    }

    /// Opens "Add offline account", with `username` filled in for screenshots.
    pub fn open_offline(
        &mut self,
        username: Option<&str>,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        self.account_menu_open = false;
        let accounts = self.accounts.clone();
        let dialog = cx.new(|cx| {
            let dialog = OfflineDialog::new(accounts, cx);
            match username {
                Some(name) => dialog.with_username(name, cx),
                None => dialog,
            }
        });
        let subscription = cx.subscribe(&dialog, |this, dialog, _: &OfflineDialogDismissed, cx| {
            if this.is_open_dialog(dialog.entity_id()) {
                this.close_modal(cx)
            }
        });
        if let Some(window) = window {
            window.focus(&dialog.focus_handle(cx), cx);
        }
        self.modal.open(
            Modal::Offline {
                dialog,
                _events: subscription,
            },
            cx,
        );
    }

    /// Asks before removing the account.
    pub fn confirm_remove(
        &mut self,
        id: &str,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        self.account_menu_open = false;
        let Some(account) = self
            .accounts
            .read(cx)
            .accounts()
            .iter()
            .find(|a| a.id == id)
            .cloned()
        else {
            return;
        };
        let focus = cx.focus_handle();
        if let Some(window) = window {
            window.focus(&focus, cx);
        }
        self.modal.open(
            Modal::RemoveAccount {
                id: account.id,
                username: account.username,
                focus,
            },
            cx,
        );
    }

    /// Opens the account menu, for the screenshots.
    pub fn open_account_menu(&mut self, cx: &mut Context<Self>) {
        self.set_account_menu(true, cx);
    }

    /// Renders a dialog from its state: the open one, or one fading out.
    fn render_modal(&self, modal: &Modal, cx: &mut Context<Self>) -> Option<AnyElement> {
        let close = |cx: &mut Context<Self>| {
            let entity = cx.entity();
            move |_: &mut Window, cx: &mut gpui::App| {
                entity.update(cx, |this, cx| this.close_modal(cx))
            }
        };
        Some(match modal {
            Modal::NewInstance { dialog, .. } => dialog.clone().into_any_element(),
            Modal::Import { dialog, .. } => dialog.clone().into_any_element(),
            Modal::Crash {
                crash,
                focus,
                copied,
            } => render_crash(crash, focus.clone(), *copied, close(cx), cx),
            Modal::LaunchFailed {
                title,
                message,
                focus,
            } => Dialog::new("launch-failed", title.clone(), focus.clone())
                .on_close(close(cx))
                .child(
                    div()
                        .text_color(colors::GRAY_400)
                        .whitespace_normal()
                        .children(message.split("\n\n").map(|paragraph| {
                            let command = paragraph.starts_with("softwareupdate");
                            div()
                                .pb_2()
                                .when(command, |this| {
                                    this.font_family(fonts::MONO)
                                        .text_color(colors::GRAY_300)
                                        .px_3()
                                        .py_2()
                                        .rounded(radius::LG)
                                        .bg(colors::GRAY_900)
                                })
                                .child(paragraph.to_string())
                        })),
                )
                .footer(
                    Button::new("close-launch-failed", t(CommonMsg::Close))
                        .variant(ButtonVariant::Secondary)
                        .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx))),
                )
                .into_any_element(),
            Modal::DeleteInstance {
                id,
                name,
                focus,
                error,
            } => {
                let id = id.clone();
                Dialog::new(
                    "delete-instance",
                    tr!(M::DeleteNamed, name = name),
                    focus.clone(),
                )
                .on_close(close(cx))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .text_color(colors::GRAY_400)
                        .child(tr!(M::DeleteInstanceBody, trash = t(CommonMsg::TheTrash)))
                        .when_some(error.clone(), |this, error| {
                            this.child(div().text_color(colors::RED_200).child(error))
                        }),
                )
                .footer(
                    Button::new("confirm-delete-instance", t(CommonMsg::MoveToTrash))
                        .variant(ButtonVariant::Danger)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let result = this
                                .instances
                                .update(cx, |instances, cx| instances.delete(&id, cx));
                            match result {
                                Ok(()) => {
                                    this.tab = InstanceTab::Play;
                                    this.close_modal(cx);
                                    if this.settings_open()
                                        && this.settings_page == SettingsPage::Storage
                                    {
                                        this.storage.update(cx, |storage, cx| storage.rescan(cx));
                                    }
                                }
                                Err(err) => {
                                    if let Some(Modal::DeleteInstance { error, .. }) =
                                        this.modal.current_mut()
                                    {
                                        *error = Some(err.into());
                                    }
                                    cx.notify();
                                }
                            }
                        })),
                )
                .footer(
                    Button::new("cancel-delete-instance", t(CommonMsg::Cancel))
                        .variant(ButtonVariant::Secondary)
                        .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx))),
                )
                .into_any_element()
            }
            Modal::LoaderChange { prompt, focus } => {
                self.render_loader_change(prompt, focus.clone(), cx)
            }
            Modal::StorageClean { what, bytes, focus } => {
                self.render_storage_clean(*what, *bytes, focus.clone(), cx)
            }
            Modal::DeleteWorld { .. } | Modal::ReplaceWorld { .. } => {
                self.render_world_dialog(modal, cx)?
            }
            Modal::ModpackUpdate {
                prompt,
                backup_worlds,
                focus,
            } => self.render_modpack_update(prompt, *backup_worlds, focus.clone(), cx),
            Modal::EnableLauncherIdentity { focus } => {
                self.render_enable_launcher_identity(focus.clone(), cx)
            }
            Modal::DisableLauncherIdentity { focus, usernames } => {
                self.render_disable_launcher_identity(focus.clone(), usernames, cx)
            }
            Modal::Unsaved { focus, target } => self.render_unsaved(focus.clone(), target, cx),
            Modal::SignIn { dialog, .. } => dialog.clone().into_any_element(),
            Modal::Offline { dialog, .. } => dialog.clone().into_any_element(),
            Modal::RemoveAccount {
                id,
                username,
                focus,
            } => {
                let id = id.clone();
                Dialog::new(
                    "remove-account",
                    tr!(M::RemoveAccountTitle, name = username),
                    focus.clone(),
                )
                .on_close(close(cx))
                .child(
                    div()
                        .text_color(colors::GRAY_400)
                        .child(tr!(M::RemoveAccountBody, keychain = t(CommonMsg::Keychain))),
                )
                .footer(
                    Button::new("confirm-remove", t(M::RemoveAccount))
                        .variant(ButtonVariant::Danger)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.accounts
                                .update(cx, |accounts, cx| accounts.remove(&id, cx));
                            this.close_modal(cx);
                        })),
                )
                .footer(
                    Button::new("cancel-remove", t(CommonMsg::Cancel))
                        .variant(ButtonVariant::Secondary)
                        .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx))),
                )
                .into_any_element()
            }
        })
    }
}

/// A likely cause of a crash, in the current language.
fn cause_text(cause: Cause) -> &'static str {
    t(match cause {
        Cause::WrongJava => M::CauseWrongJava,
        Cause::OutOfMemory => M::CauseOutOfMemory,
        Cause::Natives => M::CauseNatives,
        Cause::JvmOptions => M::CauseJvmOptions,
        Cause::Graphics => M::CauseGraphics,
    })
}

/// The crash dialog: the exception, the likely causes in plain words, and the crash
/// report and log.
fn render_crash(
    crash: &CrashInfo,
    focus: FocusHandle,
    copied: bool,
    on_close: impl Fn(&mut Window, &mut gpui::App) + 'static,
    cx: &mut Context<LauncherShell>,
) -> AnyElement {
    let summary = &crash.summary;
    let headline = match (&summary.description, crash.exit_code) {
        (Some(description), _) => tr!(M::MinecraftCrashed, description = description),
        (None, Some(code)) => tr!(M::MinecraftExitCode, code = code),
        (None, None) => t(M::MinecraftStopped).to_string(),
    };
    let report = crash.report.clone();
    let log_text = crash.log_text.clone();
    Dialog::new(
        "crash",
        tr!(M::CrashTitle, name = crash.instance_name),
        focus,
    )
    .on_close(on_close)
    .child(
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_color(colors::GRAY_400).child(headline))
            .when_some(summary.exception.clone(), |this, exception| {
                this.child(
                    div()
                        .px_3()
                        .py_2()
                        .rounded(radius::LG)
                        .bg(colors::GRAY_900)
                        .border_1()
                        .border_color(colors::RED_200.opacity(30))
                        .font_family(fonts::MONO)
                        .text_size(theme::text_size(text::SMALL))
                        .text_color(colors::RED_200)
                        .child(exception),
                )
            })
            .when(!summary.causes.is_empty(), |this| {
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_size(theme::text_size(text::SMALL))
                                .font_weight(weights::BOLD.into())
                                .text_color(colors::GRAY_500)
                                .child(t(M::LikelyCause).to_uppercase()),
                        )
                        .children(summary.causes.iter().map(|cause| {
                            div()
                                .text_size(theme::text_size(text::MEDIUM))
                                .text_color(colors::GRAY_300)
                                .child(cause_text(*cause))
                        })),
                )
            }),
    )
    .when_some(report, |this, report| {
        this.footer(
            Button::new("open-crash-report", t(M::OpenCrashReport))
                .variant(ButtonVariant::Primary)
                .on_click(move |_, _, cx| cx.open_with_system(&report)),
        )
    })
    .footer(
        Button::new(
            "copy-crash-log",
            if copied {
                t(CommonMsg::Copied)
            } else {
                t(M::CopyLog)
            },
        )
        .variant(ButtonVariant::Secondary)
        .on_click(cx.listener(move |this, _, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(log_text.clone()));
            if let Some(Modal::Crash { copied, .. }) = this.modal.current_mut() {
                *copied = true;
            }
            cx.notify();
        })),
    )
    .footer(
        Button::new("close-crash", t(CommonMsg::Close))
            .variant(ButtonVariant::Ghost)
            .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx))),
    )
    .into_any_element()
}

impl LauncherShell {
    /// Undo (`undo` true) or Redo of a setting, with a toast that says what changed
    /// and offers the way back.
    fn undo_step(&mut self, undo: bool, cx: &mut Context<Self>) {
        let outcome = if undo {
            crate::undo::undo(cx)
        } else {
            crate::undo::redo(cx)
        };
        let (message, error, back): (String, bool, Option<bool>) = match outcome {
            crate::undo::Outcome::Undid(change) => {
                (tr!(M::Undid, change = change), false, Some(false))
            }
            crate::undo::Outcome::Redid(change) => {
                (tr!(M::Redid, change = change), false, Some(true))
            }
            crate::undo::Outcome::Refused(reason) => (reason, true, None),
            crate::undo::Outcome::Nothing => return,
        };
        let action = back.map(|undo_again| {
            let label: SharedString = t(if undo_again { M::Undo } else { M::Redo }).into();
            let action: crate::components::settings::ToastAction =
                std::rc::Rc::new(move |window: &mut Window, cx: &mut App| {
                    if undo_again {
                        window.dispatch_action(Box::new(crate::Undo), cx);
                    } else {
                        window.dispatch_action(Box::new(crate::Redo), cx);
                    }
                });
            (label, action)
        });
        self.undo_toast.show_with(
            message,
            error,
            action,
            |this: &mut Self| &mut this.undo_toast,
            cx,
        );
        cx.refresh_windows();
    }

    /// One zoom level up or down, as Ctrl/⌘ + and − do.
    pub(crate) fn zoom_by(&mut self, up: bool, cx: &mut Context<Self>) {
        let zoom = theme::zoom::step(theme::zoom::get(), up);
        self.instances
            .update(cx, |instances, cx| instances.set_zoom(zoom, cx));
    }
}

impl LauncherShell {
    /// Points the slideshow at the selected instance, and runs it only while the Play
    /// tab is on screen.
    fn sync_slideshow(&mut self, cx: &mut Context<Self>) {
        use launcher_core::instances::BackgroundMode;
        let target = self.instances.read(cx).selected().and_then(|instance| {
            match instance.background_mode() {
                BackgroundMode::Slideshow(source) => Some((instance.id.clone(), source)),
                _ => None,
            }
        });
        let shown = self.page == Page::Instance && self.tab == InstanceTab::Play;
        self.slideshow
            .update(cx, |slideshow, cx| slideshow.sync(target, shown, cx));
    }

    /// The Play tab's slideshow, for the offscreen screenshots.
    pub fn slideshow(&self) -> &Entity<crate::instances::slideshow::Slideshow> {
        &self.slideshow
    }
}

impl Render for LauncherShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.trace_frame(window, cx);
        if self._focus_lost.is_none() {
            self._focus_lost = Some(focus::install(&self.root_focus, window, cx));
        }
        // Text and spacing are in rems: zoom them with everything else.
        window.set_rem_size(theme::zoom::rem_size());
        self.sync_slideshow(cx);
        let main = match self.page {
            Page::Instance => play::render(self, cx).into_any_element(),
            Page::Servers => self.servers.clone().into_any_element(),
        };
        let settings = self.settings_modal.render(window, |modal| {
            Some(self.render_settings(modal, window, cx))
        });
        let modal = self
            .modal
            .render(window, |modal| self.render_modal(modal, cx));

        div()
            .id("launcher-shell")
            .track_focus(&self.root_focus)
            // For the debug traces: where the pointer went down, to place clicks on
            // elements that aren't shared controls.
            .capture_any_mouse_down(|event, _, _| {
                let (x, y) = (f32::from(event.position.x), f32::from(event.position.y));
                launcher_core::trace::emit("ui.pointer", || {
                    serde_json::json!({
                        "x": x.round(),
                        "y": y.round(),
                        "button": format!("{:?}", event.button).to_lowercase(),
                        "clicks": event.click_count,
                    })
                });
            })
            .on_action(|_: &focus::FocusNext, window, cx| focus::step(true, window, cx))
            .on_action(|_: &focus::FocusPrev, window, cx| focus::step(false, window, cx))
            .on_action(cx.listener(|this, _: &crate::ZoomIn, _, cx| this.zoom_by(true, cx)))
            .on_action(cx.listener(|this, _: &crate::ZoomOut, _, cx| this.zoom_by(false, cx)))
            .on_action(cx.listener(|this, _: &crate::Undo, _, cx| this.undo_step(true, cx)))
            .on_action(cx.listener(|this, _: &crate::Redo, _, cx| this.undo_step(false, cx)))
            .on_action(cx.listener(|this, _: &crate::ResetZoom, _, cx| {
                this.instances
                    .update(cx, |instances, cx| instances.set_zoom(1.0, cx))
            }))
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors::GRAY_875)
            .font_family(fonts::SANS)
            .map(crate::i18n::locale_fonts)
            .text_color(colors::GRAY_300)
            .text_size(theme::text_size(text::MEDIUM))
            .line_height(theme::line_height(text::MEDIUM, leading::NORMAL))
            .child(top_bar::render(self, cx))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(sidebar::render(self, cx))
                    .child(div().flex().flex_1().min_w_0().h_full().child(main)),
            )
            .children(settings)
            .children(modal)
            .child(self.servers_overlay.clone())
            .children(self.undo_toast.render())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In English the crash causes read as `launcher-core` writes them.
    #[test]
    fn causes_match_the_core_in_english() {
        for cause in [
            Cause::WrongJava,
            Cause::OutOfMemory,
            Cause::Natives,
            Cause::JvmOptions,
            Cause::Graphics,
        ] {
            assert_eq!(cause_text(cause), cause.explanation());
        }
    }
}
