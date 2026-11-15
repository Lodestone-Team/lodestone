//! "Add a modpack": a Modrinth or CurseForge modpack's files added to an instance that
//! exists, such as Fabulously Optimized's client mods in an instance made for a server.
//! The browser lists only packs for the instance's Minecraft version and loader; the
//! picked pack is downloaded and checked, what adding it does is shown (what the
//! instance's own pack, the server's, keeps, and what the instance has already), and
//! only then are its files added. The work is `launcher_core::modpack::addon`'s.

use crate::theme::px;
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, SharedString,
    Task, Window, div, prelude::*,
};

use super::{
    Instances,
    modpack_browser::ModpackBrowser,
    modpacks::{AddPrompt, AddSource},
};
use crate::{
    components::{Button, ButtonVariant, Dialog, LoadingDots},
    i18n::{
        CommonMsg, InstanceSettingsMsg as M, InstanceSettingsPlural as P, InstancesPlural, format,
        t, tr, trn,
    },
    theme::{self, colors, radius, text, weights},
};

/// The dialog is done: closed, or the pack is being added.
pub struct AddModpackDismissed;

enum Stage {
    Browse,
    /// Downloading and checking the picked pack.
    Checking,
    Review(Box<AddPrompt>),
}

pub struct AddModpackDialog {
    instances: Entity<Instances>,
    instance_id: String,
    instance_name: String,
    game_version: String,
    loader: String,
    server: Option<String>,
    browser: Entity<ModpackBrowser>,
    stage: Stage,
    error: Option<SharedString>,
    focus: FocusHandle,
    _task: Option<Task<()>>,
}

impl EventEmitter<AddModpackDismissed> for AddModpackDialog {}

impl Focusable for AddModpackDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl AddModpackDialog {
    /// The dialog for the instance `id` (which the caller checked is there).
    pub fn new(instances: Entity<Instances>, id: &str, cx: &mut Context<Self>) -> Self {
        let instance = instances
            .read(cx)
            .list()
            .iter()
            .find(|i| i.id == id)
            .cloned()
            .unwrap_or_default();
        let server = launcher_core::cores::link::load_link(
            &instances.read(cx).content_dirs(&instance.id).instance,
        )
        .map(|link| link.server_name);
        let browser = {
            let instances = instances.clone();
            let (game, loader) = (instance.game_version.clone(), instance.loader);
            cx.new(|cx| {
                let mut browser = ModpackBrowser::new(instances, cx).compatible_with(game, loader);
                browser.ensure_searched(cx);
                browser
            })
        };
        cx.observe(&browser, |_, _, cx| cx.notify()).detach();
        Self {
            instances,
            instance_id: instance.id.clone(),
            instance_name: instance.name.clone(),
            game_version: instance.game_version.clone(),
            loader: crate::shell::loader_name(instance.loader).to_string(),
            server,
            browser,
            stage: Stage::Browse,
            error: None,
            focus: cx.focus_handle(),
            _task: None,
        }
    }

    /// The browser, for the offscreen screenshots.
    pub fn browser(&self) -> &Entity<ModpackBrowser> {
        &self.browser
    }

    /// Shows a review, for the offscreen screenshots.
    pub fn preview_review(&mut self, prompt: AddPrompt, cx: &mut Context<Self>) {
        self.stage = Stage::Review(Box::new(prompt));
        cx.notify();
    }

    /// Downloads and checks the picked pack, then shows what adding it does.
    fn check(&mut self, cx: &mut Context<Self>) {
        let source = {
            let browser = self.browser.read(cx);
            match (browser.cf_selection(), browser.selection()) {
                (Some(picked), _) => AddSource::CurseForge(Box::new(picked)),
                (None, Some(picked)) => AddSource::Modrinth(Box::new(picked)),
                (None, None) => return,
            }
        };
        let id = self.instance_id.clone();
        let work = self.instances.update(cx, |instances, cx| {
            instances.plan_add_modpack(&id, source, cx)
        });
        self.stage = Stage::Checking;
        self.error = None;
        self._task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(prompt) => this.stage = Stage::Review(Box::new(prompt)),
                    Err(err) => {
                        this.stage = Stage::Browse;
                        this.error = Some(err.into());
                    }
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Back to the browser, forgetting the downloaded pack.
    fn back(&mut self, cx: &mut Context<Self>) {
        self.discard(cx);
        self.stage = Stage::Browse;
        cx.notify();
    }

    /// Removes the downloaded pack of a review that wasn't answered.
    pub fn discard(&mut self, cx: &mut Context<Self>) {
        if let Stage::Review(prompt) = &self.stage {
            let prompt = prompt.clone();
            self.instances
                .update(cx, |instances, _| instances.discard_add(&prompt));
        }
    }

    /// Closes the dialog, forgetting a pack that wasn't added.
    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.discard(cx);
        self.stage = Stage::Browse;
        self._task = None;
        cx.emit(AddModpackDismissed);
    }

    fn add(&mut self, cx: &mut Context<Self>) {
        let Stage::Review(prompt) = std::mem::replace(&mut self.stage, Stage::Browse) else {
            return;
        };
        self.instances
            .update(cx, |instances, cx| instances.apply_add_modpack(*prompt, cx));
        cx.emit(AddModpackDismissed);
    }

    fn note(&self) -> String {
        match &self.server {
            Some(server) => tr!(M::AddModpackServerNote, server = server),
            None => t(M::AddModpackNote).to_string(),
        }
    }

    fn review(&self, prompt: &AddPrompt) -> impl IntoElement {
        const SHOWN: usize = 6;
        let plan = &prompt.plan;
        let name = |path: &String| path.rsplit('/').next().unwrap_or(path).to_string();
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
                        .text_color(crate::theme::ink::secondary())
                        .child(item)
                }))
                .when(more > 0, |this| {
                    this.child(
                        div()
                            .pl_3()
                            .text_size(theme::text_size(text::SMALL))
                            .text_color(crate::theme::ink::muted())
                            .child(trn!(P::AndMoreFiles, more)),
                    )
                })
        };
        let count = |n: usize| format::number(n as u64);
        let mut groups = Vec::new();
        if !plan.added.is_empty() {
            groups.push(list(
                tr!(M::AddGroupFiles, count = count(plan.added.len())),
                plan.added.iter().map(name).collect(),
                colors::GREEN_300,
            ));
        }
        if !plan.configs.is_empty() {
            groups.push(list(
                tr!(M::AddGroupConfigs, count = count(plan.configs.len())),
                plan.configs.clone(),
                colors::GRAY_400,
            ));
        }
        if !plan.kept_instance_pack.is_empty() {
            let title = match &prompt.server {
                Some(server) => tr!(
                    M::AddGroupServer,
                    server = server,
                    count = count(plan.kept_instance_pack.len())
                ),
                None => tr!(
                    M::AddGroupPack,
                    count = count(plan.kept_instance_pack.len())
                ),
            };
            groups.push(list(
                title,
                plan.kept_instance_pack.iter().map(name).collect(),
                crate::theme::accent::light(),
            ));
        }
        if !plan.already_there.is_empty() {
            groups.push(list(
                tr!(M::AddGroupThere, count = count(plan.already_there.len())),
                plan.already_there.iter().map(name).collect(),
                colors::GRAY_400,
            ));
        }
        let record = &plan.pack.record;
        let summary = tr!(
            M::AddModpackSummary,
            pack = record.title,
            version = record.version_number,
            size = format::bytes_binary(plan.download_size()),
        );
        div()
            .flex()
            .flex_col()
            .gap_3()
            .text_color(crate::theme::ink::secondary())
            .child(div().whitespace_normal().child(summary))
            .when(!plan.changes_files(), |this| {
                this.child(div().child(t(M::AddModpackNothing)))
            })
            .child(
                div()
                    .id("add-modpack-plan")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .max_h(px(300.))
                    .overflow_y_scroll()
                    .px_3()
                    .py_2()
                    .rounded(radius::LG)
                    .bg(colors::GRAY_900)
                    .children(groups),
            )
            .when(!prompt.blocked.is_empty(), |this| {
                this.child(
                    div()
                        .whitespace_normal()
                        .text_size(theme::text_size(text::SMALL))
                        .child(trn!(
                            InstancesPlural::PackFilesToDownload,
                            prompt.blocked.len()
                        )),
                )
            })
            .child(
                div()
                    .whitespace_normal()
                    .text_size(theme::text_size(text::SMALL))
                    .child(self.note()),
            )
    }
}

impl Render for AddModpackDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let selected = self.browser.read(cx).has_selection();
        let body = match &self.stage {
            Stage::Browse => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .whitespace_normal()
                        .text_size(theme::text_size(text::SMALL))
                        .text_color(crate::theme::ink::secondary())
                        .child(tr!(
                            M::AddModpackFilter,
                            version = self.game_version,
                            loader = self.loader,
                            note = self.note(),
                        )),
                )
                .child(self.browser.clone())
                .into_any_element(),
            Stage::Checking => div()
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .gap_2()
                .py_8()
                .text_color(crate::theme::ink::secondary())
                .child(LoadingDots)
                .child(t(M::CheckingModpack))
                .into_any_element(),
            Stage::Review(prompt) => self.review(prompt).into_any_element(),
        };
        Dialog::new(
            "add-modpack",
            tr!(M::AddModpackTitle, name = self.instance_name),
            self.focus.clone(),
        )
        .width(px(820.))
        .on_close({
            let entity = entity.clone();
            move |_, cx| entity.update(cx, |this, cx| this.dismiss(cx))
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap_4()
                .text_size(theme::text_size(text::MEDIUM))
                .child(body)
                .when_some(self.error.clone(), |this, error| {
                    this.child(
                        div()
                            .whitespace_normal()
                            .text_size(theme::text_size(text::SMALL))
                            .text_color(colors::RED_200)
                            .child(error),
                    )
                }),
        )
        .map(|dialog| match &self.stage {
            Stage::Browse => dialog.footer(
                Button::new("check-add-modpack", t(M::AddModpackButton))
                    .variant(ButtonVariant::Primary)
                    .disabled(!selected)
                    .on_click(cx.listener(|this, _, _, cx| this.check(cx))),
            ),
            Stage::Checking => dialog.footer(
                Button::new("check-add-modpack", t(M::AddModpackButton))
                    .variant(ButtonVariant::Primary)
                    .disabled(true),
            ),
            Stage::Review(prompt) => dialog
                .footer_start(
                    Button::new("back-add-modpack", t(CommonMsg::Back))
                        .variant(ButtonVariant::Ghost)
                        .on_click(cx.listener(|this, _, _, cx| this.back(cx))),
                )
                .footer(
                    Button::new("confirm-add-modpack", t(CommonMsg::Add))
                        .variant(ButtonVariant::Primary)
                        .disabled(!prompt.plan.changes_files())
                        .on_click(cx.listener(|this, _, _, cx| this.add(cx))),
                ),
        })
        .footer(
            Button::new("cancel-add-modpack", t(CommonMsg::Cancel))
                .variant(ButtonVariant::Ghost)
                .on_click(cx.listener(|this, _, _, cx| this.dismiss(cx))),
        )
    }
}
