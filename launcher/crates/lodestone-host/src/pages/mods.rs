//! An instance's Mods tab, or Plugins for Paper and Spigot (`ModsTab.tsx`): what is
//! installed (`InstalledMods.tsx`, with the client modpack of a Fabric server), and
//! Modrinth through the core to find more (`ModrinthSearch.tsx`,
//! `ModrinthProjectView.tsx`).
//!
//! Reading needs the instance's file read permission (the tab is hidden without it);
//! every change needs the write permission, and its button says why when it is off.
//! The core does the work: installs, updates, uploads, enabling and deleting.

mod browse;
mod installed;
mod project;
#[cfg(test)]
mod tests;

use std::collections::HashSet;

use gpui::{
    AnyElement, AppContext as _, Context, Entity, FocusHandle, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, Task, Window, div, prelude::*,
};
use launcher_core::cores::admin::{
    InstanceInfo,
    models::{CorePermission, InstancePermission},
    mods::{InstalledModKind, InstalledModList, InstalledModLoader, ModInstallResult},
};
use lodestone_ui::{
    components::{Icon, Tabs, Toast, window_layer},
    theme::{colors, ink, leading, line_height, px, radius, text},
};

use self::{browse::Browse, project::ProjectView};
use crate::{
    i18n::{InstanceMsg as M, t, tr},
    routes::{ModSupport, mod_support},
    state::HostState,
};

/// Installed or Browse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SubView {
    Installed,
    Browse,
}

/// What the tab installs into (`InstallTarget`): from the core's list once it has
/// come, from the instance until then.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub kind: InstalledModKind,
    pub loader: InstalledModLoader,
    pub game_version: String,
}

impl Target {
    fn of(instance: &InstanceInfo, list: Option<&InstalledModList>) -> Option<Target> {
        let support = mod_support(instance)?;
        if let Some(list) = list {
            return Some(Target {
                kind: list.kind,
                loader: list.loader,
                game_version: list.game_version.clone(),
            });
        }
        use launcher_core::cores::admin::models::{Game, MinecraftVariant};
        let loader = match &instance.game_type {
            Game::MinecraftJava { variant } => match variant {
                MinecraftVariant::Fabric => InstalledModLoader::Fabric,
                MinecraftVariant::Forge => InstalledModLoader::Forge,
                MinecraftVariant::Paper => InstalledModLoader::Paper,
                MinecraftVariant::Spigot => InstalledModLoader::Spigot,
                _ => InstalledModLoader::Unknown,
            },
            _ => InstalledModLoader::Unknown,
        };
        Some(Target {
            kind: match support {
                ModSupport::Mods => InstalledModKind::Mod,
                ModSupport::Plugins => InstalledModKind::Plugin,
            },
            loader,
            game_version: instance.version.clone(),
        })
    }

    pub fn is_plugin(&self) -> bool {
        self.kind == InstalledModKind::Plugin
    }
}

/// A confirmation dialog of the tab.
enum Prompt {
    /// Delete a mod: its file and name.
    Delete { file_name: String, name: String },
    /// Uploaded files that exist already: the core's message.
    Overwrite {
        files: Vec<(String, Vec<u8>)>,
        message: String,
    },
    /// A version for another loader or game version.
    Incompatible { version_id: String, body: String },
}

struct Shown {
    focus: FocusHandle,
    prompt: Prompt,
}

pub struct ModsPage {
    pub state: Entity<HostState>,
    /// The instance.
    pub uuid: String,
    view: SubView,
    /// Browse is made when first opened, then kept, with its search and page.
    browse: Option<Browse>,
    /// The project picked in Browse.
    project: Option<ProjectView>,
    // the installed list
    list: Option<InstalledModList>,
    list_error: Option<String>,
    loading: bool,
    /// Updates are checked from the first "Check for updates" on.
    check_updates: bool,
    /// Mods being turned on or off, or updated, by file name.
    busy_files: HashSet<String>,
    updating_all: bool,
    uploading: bool,
    downloading_pack: bool,
    dialog: lodestone_ui::components::DialogSlot<Self, Shown>,
    /// Counts the list's fetches, so only the newest one is kept.
    list_serial: u64,
    _tasks: Vec<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl ModsPage {
    pub fn new(state: Entity<HostState>, uuid: String, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![cx.observe(&state, |_, _, cx| cx.notify())];
        let mut this = Self {
            state,
            uuid,
            view: SubView::Installed,
            browse: None,
            project: None,
            list: None,
            list_error: None,
            loading: false,
            check_updates: false,
            busy_files: HashSet::new(),
            updating_all: false,
            uploading: false,
            downloading_pack: false,
            dialog: lodestone_ui::components::DialogSlot::new(|this: &mut Self| &mut this.dialog),
            list_serial: 0,
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

    fn can_read(&self, cx: &Context<Self>) -> bool {
        self.state
            .read(cx)
            .me
            .may(InstancePermission::ReadFiles, &self.uuid)
    }

    pub(crate) fn can_write(&self, cx: &Context<Self>) -> bool {
        self.state
            .read(cx)
            .me
            .may(InstancePermission::WriteFiles, &self.uuid)
    }

    fn can_create(&self, cx: &Context<Self>) -> bool {
        self.state
            .read(cx)
            .me
            .may_core(CorePermission::CreateInstance)
    }

    pub(crate) fn target(&self, cx: &Context<Self>) -> Option<Target> {
        let instance = self.state.read(cx).instance(&self.uuid)?;
        Target::of(instance, self.list.as_ref())
    }

    /// Fetches the installed list again (with updates once they were asked for).
    pub(crate) fn reload(&mut self, cx: &mut Context<Self>) {
        if !self.can_read(cx) {
            return;
        }
        self.loading = true;
        self.list_serial += 1;
        let serial = self.list_serial;
        let check = self.check_updates;
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        cx.notify();
        let task = cx.spawn(async move |this, cx| {
            let list = cx
                .background_spawn(async move { admin.installed_mods(&uuid, check).await })
                .await;
            this.update(cx, |this, cx| {
                if serial != this.list_serial {
                    return;
                }
                this.loading = false;
                match list {
                    Ok(list) => {
                        this.list = Some(list);
                        this.list_error = None;
                    }
                    Err(err) => this.list_error = Some(err.to_string()),
                }
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    fn open_prompt(&mut self, prompt: Prompt, cx: &mut Context<Self>) {
        let focus = cx.focus_handle();
        self.dialog.open(Shown { focus, prompt }, cx);
    }

    /// The toast after an install: what was installed, removed and skipped, and which
    /// mods players need too (`InstallResultMessage`).
    pub(crate) fn result_toast(title: String, result: &ModInstallResult) -> Toast {
        let mut lines = vec![title];
        let label = |m: &launcher_core::cores::admin::mods::InstalledMod| match m.version() {
            Some(v) => format!("{} {v}", m.name()),
            None => m.name().to_string(),
        };
        if !result.installed.is_empty() {
            let mods: Vec<String> = result.installed.iter().map(label).collect();
            lines.push(tr!(M::ModsResultInstalled, mods = mods.join(", ")));
        }
        if !result.removed.is_empty() {
            lines.push(tr!(M::ModsResultRemoved, files = result.removed.join(", ")));
        }
        if !result.skipped.is_empty() {
            let skipped: Vec<String> = result
                .skipped
                .iter()
                .map(|s| format!("{} ({})", s.project_id, s.reason))
                .collect();
            lines.push(tr!(M::ModsResultSkipped, projects = skipped.join(", ")));
        }
        let needed: Vec<&str> = result
            .installed
            .iter()
            .filter(|m| m.players_need())
            .map(|m| m.name())
            .collect();
        if !needed.is_empty() {
            lines.push(tr!(M::ModsResultPlayersNeed, mods = needed.join(", ")));
        }
        Toast::success(lines.join("\n"))
    }

    fn render_dialog(&self, shown: &Shown, cx: &mut Context<Self>) -> AnyElement {
        use lodestone_ui::components::{Button, ButtonVariant, Dialog};
        let target = self.target(cx);
        let plugin = target.as_ref().is_some_and(Target::is_plugin);
        let (title, body, confirm) = match &shown.prompt {
            Prompt::Delete { file_name, name } => (
                t(if plugin {
                    M::ModsDeleteTitlePlugin
                } else {
                    M::ModsDeleteTitleMod
                })
                .to_string(),
                tr!(M::ModsDeleteBody, name = name, fileName = file_name),
                t(crate::i18n::CommonMsg::Delete).to_string(),
            ),
            Prompt::Overwrite { message, .. } => (
                t(M::ModsOverwriteTitle).to_string(),
                tr!(
                    M::ModsOverwriteBody,
                    message = if message.is_empty() {
                        t(M::ModsOverwriteDefault).to_string()
                    } else {
                        message.clone()
                    }
                ),
                t(M::ModsOverwrite).to_string(),
            ),
            Prompt::Incompatible { body, .. } => (
                t(M::ModsIncompatibleTitle).to_string(),
                body.clone(),
                t(M::ModsInstallAnyway).to_string(),
            ),
        };
        Dialog::new("mods-dialog", title, shown.focus.clone())
            .on_close({
                let this = cx.entity().downgrade();
                move |_, cx| {
                    this.update(cx, |this, cx| {
                        this.dialog.close(cx);
                    })
                    .ok();
                }
            })
            .child(
                div()
                    .text_size(text::MEDIUM)
                    .line_height(line_height(text::MEDIUM, leading::NORMAL))
                    .text_color(ink::secondary())
                    .child(body),
            )
            .footer(
                Button::new("mods-dialog-confirm", confirm)
                    .variant(ButtonVariant::Danger)
                    .on_click(cx.listener(|this, _, _, cx| this.confirm_prompt(cx))),
            )
            .into_any_element()
    }

    fn confirm_prompt(&mut self, cx: &mut Context<Self>) {
        let Some(shown) = self.dialog.close(cx) else {
            return;
        };
        let prompt = std::mem::replace(
            &mut shown.prompt,
            Prompt::Incompatible {
                version_id: String::new(),
                body: String::new(),
            },
        );
        match prompt {
            Prompt::Delete { file_name, .. } => self.delete_mod(file_name, cx),
            Prompt::Overwrite { files, .. } => self.upload(files, true, cx),
            Prompt::Incompatible { version_id, .. } => self.install_version(version_id, true, cx),
        }
    }
}

/// A yellow banner, such as "Restart the server to apply mod changes".
fn warning(text_: impl Into<SharedString>) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .p(px(8.))
        .rounded(radius::MD)
        .border_1()
        .border_color(colors::YELLOW)
        .bg(colors::YELLOW_FADED.opacity(25))
        .text_color(ink::text())
        .child(Icon::TriangleExclamation.render(px(12.), colors::YELLOW))
        .child(text_.into())
        .into_any_element()
}

/// A short muted line, such as an empty state.
fn muted(text_: impl Into<SharedString>) -> AnyElement {
    div()
        .text_color(ink::muted())
        .child(text_.into())
        .into_any_element()
}

impl Render for ModsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let target = self.target(cx);
        // Browse is made when it first shows, then kept
        if self.view == SubView::Browse
            && self.browse.is_none()
            && let Some(target) = &target
        {
            let admin = self.state.read(cx).admin.clone();
            self.browse = Some(Browse::new(target, admin, window, cx));
        }
        let body: AnyElement = match &target {
            None => muted(t(M::ModsUnsupported)),
            Some(_) if !self.can_read(cx) => muted(t(M::ModsNoRead)),
            Some(target) => {
                let restart = self
                    .list
                    .as_ref()
                    .is_some_and(|l| l.restart_required)
                    .then(|| {
                        warning(t(if target.is_plugin() {
                            M::ModsRestartPlugin
                        } else {
                            M::ModsRestartMod
                        }))
                    });
                let tabs = Tabs::new(
                    "mods-views",
                    [t(M::ModsInstalledView), t(M::ModsBrowseView)],
                )
                .selected(match self.view {
                    SubView::Installed => 0,
                    SubView::Browse => 1,
                })
                .on_select({
                    let this = cx.entity().downgrade();
                    move |index, window, cx| {
                        this.update(cx, |this, cx| {
                            this.view = if *index == 0 {
                                SubView::Installed
                            } else {
                                SubView::Browse
                            };
                            if this.view == SubView::Browse
                                && this.browse.is_none()
                                && let Some(target) = this.target(cx)
                            {
                                let admin = this.state.read(cx).admin.clone();
                                this.browse = Some(Browse::new(&target, admin, window, cx));
                            }
                            cx.notify();
                        })
                        .ok();
                    }
                });
                let content = match self.view {
                    SubView::Installed => self.render_installed(target, cx),
                    SubView::Browse => match &self.project {
                        Some(_) => self.render_project(target, cx),
                        None => self.render_browse(target, cx),
                    },
                };
                div()
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .children(restart)
                    .child(tabs)
                    .child(content)
                    .into_any_element()
            }
        };
        let dialogs = self
            .dialog
            .render(window, |shown| Some(self.render_dialog(shown, cx)));
        div()
            .id("mods")
            .size_full()
            .overflow_y_scroll()
            .text_size(text::MEDIUM)
            .line_height(line_height(text::MEDIUM, leading::NORMAL))
            .child(div().w_full().max_w(px(900.)).pb(px(64.)).child(body))
            .children(window_layer(window, dialogs))
    }
}
