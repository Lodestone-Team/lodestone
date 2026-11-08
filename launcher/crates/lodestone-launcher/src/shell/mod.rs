//! The main window, laid out like a game launcher in the dashboard's style: the
//! top bar, a sidebar with the account box, the instances and Settings, and the
//! selected instance with its tabs and a large Play button.

mod account_box;
mod developer;
mod launcher_settings;
mod play;
mod shared_settings;
mod sidebar;
mod top_bar;

pub use launcher_settings::LauncherSettings;
pub use shared_settings::SharedSettings;
pub use top_bar::TOP_BAR_HEIGHT;

use gpui::{
    AnyElement, ClipboardItem, Context, Entity, FocusHandle, Focusable as _, IntoElement, Render,
    SharedString, Subscription, Window, div, prelude::*, px,
};

use crate::{
    accounts::{
        Accounts,
        offline_dialog::{OfflineDialog, OfflineDialogDismissed},
        sign_in::{SignInDialog, SignInEvent, Stage},
    },
    components::{Button, ButtonVariant, Dialog, Icon},
    instances::{
        CrashInfo, Instances, InstancesEvent,
        console::Console,
        content::{ContentEvent, ContentView},
        import::{ImportDialog, ImportDismissed},
        new_instance::{NewInstanceDialog, NewInstanceDismissed},
        settings::{DeleteRequested, InstanceSettings, LoaderChangePrompt},
    },
    theme::{self, colors, fonts, leading, radius, text, weights},
};

/// What the main area shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Instance,
    Settings,
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
            InstanceTab::Play => "Play",
            InstanceTab::Content => "Content",
            InstanceTab::Worlds => "Worlds",
            InstanceTab::Settings => "Settings",
            InstanceTab::Logs => "Logs",
        }
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
    /// The disclosure before "Sign in as the Minecraft Launcher" is turned on.
    EnableLauncherIdentity { focus: FocusHandle },
    /// Turning it off, with the accounts that used it.
    DisableLauncherIdentity {
        focus: FocusHandle,
        usernames: Vec<String>,
    },
}

/// The root view of the launcher window.
pub struct LauncherShell {
    accounts: Entity<Accounts>,
    instances: Entity<Instances>,
    console: Entity<Console>,
    /// The Content tab: mods, resource packs and shader packs.
    content: Entity<ContentView>,
    /// The launcher's Settings page.
    launcher_settings: Entity<LauncherSettings>,
    /// Settings > Shared game settings.
    shared_settings: Entity<SharedSettings>,
    /// The Settings tab of the selected instance, rebuilt when the selection changes.
    settings: Option<(Entity<InstanceSettings>, [Subscription; 2])>,
    page: Page,
    tab: InstanceTab,
    modal: Option<Modal>,
    account_menu_open: bool,
    /// Set by a mouse down on the top bar; the next mouse move starts a window drag.
    top_bar_drag_pending: bool,
    _observe: Vec<Subscription>,
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
                    this.modal = Some(Modal::LaunchFailed {
                        title: title.clone(),
                        message: message.clone(),
                        focus: cx.focus_handle(),
                    });
                    cx.notify();
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
        let launcher_settings = {
            let instances = instances.clone();
            cx.new(|cx| LauncherSettings::new(instances, cx))
        };
        let shared_settings = {
            let instances = instances.clone();
            cx.new(|cx| SharedSettings::new(instances, cx))
        };
        let mut this = Self {
            accounts,
            instances,
            console,
            content,
            launcher_settings,
            shared_settings,
            settings: None,
            page: Page::default(),
            tab: InstanceTab::default(),
            modal: None,
            account_menu_open: false,
            top_bar_drag_pending: false,
            _observe: observe,
        };
        this.sync_settings(cx);
        this
    }

    pub fn instances(&self) -> &Entity<Instances> {
        &self.instances
    }

    pub fn console(&self) -> &Entity<Console> {
        &self.console
    }

    pub fn content(&self) -> &Entity<ContentView> {
        &self.content
    }

    /// Rebuilds the Settings tab when another instance is selected.
    fn sync_settings(&mut self, cx: &mut Context<Self>) {
        let selected = self.instances.read(cx).selected().cloned();
        let current = self
            .settings
            .as_ref()
            .map(|(settings, _)| settings.read(cx).instance_id().to_string());
        match selected {
            None => self.settings = None,
            Some(instance) if current.as_deref() != Some(instance.id.as_str()) => {
                let instances = self.instances.clone();
                let settings = cx.new(|cx| InstanceSettings::new(instances, &instance, cx));
                let subscriptions = [
                    cx.subscribe(&settings, |this, _, DeleteRequested(id), cx| {
                        this.confirm_delete_instance(id, cx)
                    }),
                    cx.subscribe(&settings, |this, _, prompt: &LoaderChangePrompt, cx| {
                        this.show_loader_change(prompt.clone(), cx)
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
        self.modal = Some(Modal::LaunchFailed {
            title: title.to_string().into(),
            message: message.to_string().into(),
            focus: cx.focus_handle(),
        });
        cx.notify();
    }

    /// Shows the crash dialog.
    pub fn show_crash(&mut self, crash: CrashInfo, cx: &mut Context<Self>) {
        self.modal = Some(Modal::Crash {
            crash,
            focus: cx.focus_handle(),
            copied: false,
        });
        cx.notify();
    }

    /// Asks before saving a loader change (from the Settings tab).
    pub fn show_loader_change(&mut self, prompt: LoaderChangePrompt, cx: &mut Context<Self>) {
        self.modal = Some(Modal::LoaderChange {
            prompt: Box::new(prompt),
            focus: cx.focus_handle(),
        });
        cx.notify();
    }

    /// Saves the change the dialog asked about, turning the mismatched mods off first if
    /// `turn_off`.
    fn finish_loader_change(&mut self, turn_off: bool, cx: &mut Context<Self>) {
        let Some(Modal::LoaderChange { prompt, .. }) = self.modal.take() else {
            return;
        };
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
        let to = prompt.instance.loader.label();
        let check = &prompt.check;
        let title = match prompt.instance.loader.kind() {
            Some(_) => format!("Switch to {to}?"),
            None => "Switch to vanilla?".to_string(),
        };
        let mods_text = match prompt.instance.loader.kind() {
            Some(_) => {
                format!("These enabled mods are made for another loader and won't load on {to}:")
            }
            None => {
                "Vanilla Minecraft loads no mods, so these enabled mods won't load:".to_string()
            }
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
                                            .child(div().text_color(colors::GRAY_300).child(m.name.clone()))
                                            .child(div().text_color(colors::GRAY_500).child(format!("{} mod", m.made_for)))
                                    }))
                                    .when(more > 0, |this| {
                                        this.child(div().text_color(colors::GRAY_500).child(format!("and {more} more")))
                                    }),
                            )
                            .child(div().whitespace_normal().child(
                                "Turning them off renames them to .disabled, so you can turn them back on in Content.",
                            ))
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
                                .child(Icon::TriangleExclamation.render(px(14.), colors::YELLOW_300))
                                .child(div().flex_1().min_w_0().whitespace_normal().child(
                                    "Worlds that used blocks or items from these mods can lose them. Back up your worlds first.",
                                )),
                        )
                    }),
            )
            .when(has_mods, |this| {
                this.footer(
                    Button::new("loader-change-disable", "Turn them off and save")
                        .variant(ButtonVariant::Primary)
                        .on_click(cx.listener(|this, _, _, cx| this.finish_loader_change(true, cx))),
                )
            })
            .footer(
                Button::new(
                    "loader-change-keep",
                    if has_mods { "Save and keep them" } else { "Save" },
                )
                .variant(if has_mods { ButtonVariant::Secondary } else { ButtonVariant::Primary })
                .on_click(cx.listener(|this, _, _, cx| this.finish_loader_change(false, cx))),
            )
            .footer(
                Button::new("loader-change-cancel", "Cancel")
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
        self.modal = Some(Modal::DeleteInstance {
            id: instance.id,
            name: instance.name,
            focus: cx.focus_handle(),
            error: None,
        });
        cx.notify();
    }

    pub fn select_instance(&mut self, id: &str, cx: &mut Context<Self>) {
        self.page = Page::Instance;
        self.instances
            .update(cx, |instances, cx| instances.select(id, cx));
    }

    pub fn show(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        cx.notify();
    }

    pub fn select_tab(&mut self, tab: InstanceTab, cx: &mut Context<Self>) {
        self.tab = tab;
        cx.notify();
    }

    fn set_account_menu(&mut self, open: bool, cx: &mut Context<Self>) {
        self.account_menu_open = open;
        cx.notify();
    }

    fn close_modal(&mut self, cx: &mut Context<Self>) {
        self.modal = None;
        cx.notify();
    }

    fn open_new_instance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_new_instance_with(None, Some(window), cx);
    }

    /// Opens "New instance" with Fabric chosen, from the Content tab of a vanilla
    /// instance.
    fn open_new_instance_with_loader(&mut self, cx: &mut Context<Self>) {
        self.open_new_instance_with(None, None, cx);
        if let Some(Modal::NewInstance { dialog, .. }) = &self.modal {
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
        let subscription = cx.subscribe(&dialog, |this, _, _: &NewInstanceDismissed, cx| {
            this.page = Page::Instance;
            this.tab = InstanceTab::Play;
            this.close_modal(cx)
        });
        if let Some(window) = window {
            window.focus(&dialog.focus_handle(cx), cx);
        }
        self.modal = Some(Modal::NewInstance {
            dialog,
            _events: subscription,
        });
        cx.notify();
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
        let subscription = cx.subscribe(&dialog, |this, _, _: &ImportDismissed, cx| {
            this.page = Page::Instance;
            this.close_modal(cx)
        });
        self.modal = Some(Modal::Import {
            dialog,
            _events: subscription,
        });
        cx.notify();
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
        let subscription = cx.subscribe(&dialog, |this, _, event, cx| match event {
            SignInEvent::Dismissed => this.close_modal(cx),
            SignInEvent::UseOffline => this.open_offline(None, None, cx),
        });
        if let Some(window) = window {
            window.focus(&dialog.focus_handle(cx), cx);
        }
        self.modal = Some(Modal::SignIn {
            dialog,
            _events: subscription,
        });
        cx.notify();
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
        let subscription = cx.subscribe(&dialog, |this, _, _: &OfflineDialogDismissed, cx| {
            this.close_modal(cx)
        });
        if let Some(window) = window {
            window.focus(&dialog.focus_handle(cx), cx);
        }
        self.modal = Some(Modal::Offline {
            dialog,
            _events: subscription,
        });
        cx.notify();
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
        self.modal = Some(Modal::RemoveAccount {
            id: account.id,
            username: account.username,
            focus,
        });
        cx.notify();
    }

    /// Opens the account menu, for the screenshots.
    pub fn open_account_menu(&mut self, cx: &mut Context<Self>) {
        self.set_account_menu(true, cx);
    }

    fn render_modal(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let close = |cx: &mut Context<Self>| {
            let entity = cx.entity();
            move |_: &mut Window, cx: &mut gpui::App| {
                entity.update(cx, |this, cx| this.close_modal(cx))
            }
        };
        Some(match self.modal.as_ref()? {
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
                    Button::new("close-launch-failed", "Close")
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
                Dialog::new("delete-instance", format!("Delete {name}?"), focus.clone())
                    .on_close(close(cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .text_color(colors::GRAY_400)
                            .child("The instance's folder, with its worlds, mods and settings, moves to the Trash. You can put it back from there.")
                            .when_some(error.clone(), |this, error| {
                                this.child(div().text_color(colors::RED_200).child(error))
                            }),
                    )
                    .footer(
                        Button::new("confirm-delete-instance", "Move to Trash")
                            .variant(ButtonVariant::Danger)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let result = this
                                    .instances
                                    .update(cx, |instances, cx| instances.delete(&id, cx));
                                match result {
                                    Ok(()) => {
                                        this.tab = InstanceTab::Play;
                                        this.close_modal(cx);
                                    }
                                    Err(err) => {
                                        if let Some(Modal::DeleteInstance { error, .. }) = &mut this.modal {
                                            *error = Some(err.into());
                                        }
                                        cx.notify();
                                    }
                                }
                            })),
                    )
                    .footer(
                        Button::new("cancel-delete-instance", "Cancel")
                            .variant(ButtonVariant::Secondary)
                            .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx))),
                    )
                    .into_any_element()
            }
            Modal::LoaderChange { prompt, focus } => {
                self.render_loader_change(prompt, focus.clone(), cx)
            }
            Modal::EnableLauncherIdentity { focus } => {
                self.render_enable_launcher_identity(focus.clone(), cx)
            }
            Modal::DisableLauncherIdentity { focus, usernames } => {
                self.render_disable_launcher_identity(focus.clone(), usernames, cx)
            }
            Modal::SignIn { dialog, .. } => dialog.clone().into_any_element(),
            Modal::Offline { dialog, .. } => dialog.clone().into_any_element(),
            Modal::RemoveAccount {
                id,
                username,
                focus,
            } => {
                let id = id.clone();
                Dialog::new("remove-account", format!("Remove {username}?"), focus.clone())
                    .on_close(close(cx))
                    .child(
                        div()
                            .text_color(colors::GRAY_400)
                            .child("Lodestone Launcher forgets this account and deletes its saved sign-in from your keychain. You can add it again later."),
                    )
                    .footer(
                        Button::new("confirm-remove", "Remove account")
                            .variant(ButtonVariant::Danger)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.accounts
                                    .update(cx, |accounts, cx| accounts.remove(&id, cx));
                                this.close_modal(cx);
                            })),
                    )
                    .footer(
                        Button::new("cancel-remove", "Cancel")
                            .variant(ButtonVariant::Secondary)
                            .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx))),
                    )
                    .into_any_element()
            }
        })
    }
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
        (Some(description), _) => format!("Minecraft crashed: {description}."),
        (None, Some(code)) => format!("Minecraft stopped with exit code {code}."),
        (None, None) => "Minecraft stopped unexpectedly.".to_string(),
    };
    let report = crash.report.clone();
    let log_text = crash.log_text.clone();
    Dialog::new("crash", format!("{} crashed", crash.instance_name), focus)
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
                                    .child("LIKELY CAUSE"),
                            )
                            .children(summary.causes.iter().map(|cause| {
                                div()
                                    .text_size(theme::text_size(text::MEDIUM))
                                    .text_color(colors::GRAY_300)
                                    .child(cause.explanation())
                            })),
                    )
                }),
        )
        .when_some(report, |this, report| {
            this.footer(
                Button::new("open-crash-report", "Open crash report")
                    .variant(ButtonVariant::Primary)
                    .on_click(move |_, _, cx| cx.open_with_system(&report)),
            )
        })
        .footer(
            Button::new("copy-crash-log", if copied { "Copied" } else { "Copy log" })
                .variant(ButtonVariant::Secondary)
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(log_text.clone()));
                    if let Some(Modal::Crash { copied, .. }) = &mut this.modal {
                        *copied = true;
                    }
                    cx.notify();
                })),
        )
        .footer(
            Button::new("close-crash", "Close")
                .variant(ButtonVariant::Ghost)
                .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx))),
        )
        .into_any_element()
}

impl Render for LauncherShell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let main = match self.page {
            Page::Instance => play::render(self, cx).into_any_element(),
            // The global game settings, then Developer.
            Page::Settings => div()
                .id("settings-page")
                .flex()
                .flex_col()
                .size_full()
                .overflow_y_scroll()
                .pb_8()
                .child(self.launcher_settings.clone())
                .child(self.shared_settings.clone())
                .child(developer::render(self, cx))
                .into_any_element(),
        };
        let modal = self.render_modal(cx);

        div()
            .id("launcher-shell")
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors::GRAY_875)
            .font_family(fonts::SANS)
            .text_color(colors::GRAY_300)
            .text_size(theme::text_size(text::MEDIUM))
            .line_height(theme::line_height(text::MEDIUM, leading::NORMAL))
            .child(top_bar::render(cx))
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
            .children(modal)
    }
}
