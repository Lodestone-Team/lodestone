//! The modpack panel of the Installed view, for a server made from a modpack: which
//! version it runs, a check for a newer one on Modrinth, and an update from Modrinth or
//! from an `.mrpack` file. The core works out what the update changes first; the dialog
//! shows that, and the update runs once the user confirms, with the server stopped.

use gpui::{
    AnyElement, AppContext as _, Context, IntoElement, ParentElement, PathPromptOptions, Styled,
    div, prelude::*,
};
use launcher_core::cores::admin::{
    instances::ModpackSource,
    models::{InstanceState, ModpackOrigin},
    mods::{ModpackUpdatePlan, ModrinthVersion, loader_title},
};
use lodestone_ui::{
    components::{Button, ButtonVariant, Icon, Toast},
    theme::{colors, ink, leading, line_height, px, radius, text, weights},
};

use super::{ModsPage, Prompt, installed::why_not, muted, warning};
use crate::i18n::{InstanceMsg as M, InstancePlural as P, t, tr, trn};

/// Where the check for a newer version stands.
#[derive(Clone, Debug, Default)]
pub(super) enum ModpackCheck {
    #[default]
    Idle,
    Checking,
    Newest,
    Available(Box<ModrinthVersion>),
    Failed(String),
}

/// "Name 1.2.3", or the name alone.
fn origin_label(origin: &ModpackOrigin) -> String {
    match &origin.version_number {
        Some(version) => format!("{} {version}", origin.name),
        None => origin.name.clone(),
    }
}

/// At most `shown` paths, then "…".
fn short_list(paths: &[String], shown: usize) -> String {
    let mut list: Vec<&str> = paths.iter().take(shown).map(String::as_str).collect();
    if paths.len() > shown {
        list.push("…");
    }
    list.join(", ")
}

impl ModsPage {
    fn modpack_origin(&self, cx: &Context<Self>) -> Option<ModpackOrigin> {
        self.state.read(cx).instance(&self.uuid)?.modpack.clone()
    }

    fn is_stopped(&self, cx: &Context<Self>) -> bool {
        self.state
            .read(cx)
            .instance(&self.uuid)
            .is_some_and(|i| i.state == InstanceState::Stopped)
    }

    /// Asks Modrinth (through the core) for the pack's newest version.
    fn check_modpack(&mut self, cx: &mut Context<Self>) {
        let Some(origin) = self.modpack_origin(cx).filter(|o| !o.project_id.is_empty()) else {
            return;
        };
        self.modpack_check = ModpackCheck::Checking;
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let task = cx.spawn(async move |this, cx| {
            let project = origin.project_id.clone();
            let versions = cx
                .background_spawn(
                    async move { admin.modrinth_versions(&project, None, None).await },
                )
                .await;
            this.update(cx, |this, cx| {
                this.modpack_check = match versions {
                    Ok(versions) => match versions.into_iter().next() {
                        Some(newest) if newest.id != origin.version_id => {
                            ModpackCheck::Available(Box::new(newest))
                        }
                        _ => ModpackCheck::Newest,
                    },
                    Err(err) => ModpackCheck::Failed(err.to_string()),
                };
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    /// Asks the core what updating to `source` does, then shows it.
    fn plan_modpack(&mut self, source: ModpackSource, cx: &mut Context<Self>) {
        if self.modpack_busy {
            return;
        }
        self.modpack_busy = true;
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let plan = {
                let source = source.clone();
                cx.background_spawn(async move { admin.plan_modpack_update(&uuid, &source).await })
                    .await
            };
            this.update(cx, |this, cx| {
                this.modpack_busy = false;
                match plan {
                    Ok(plan) => this.open_prompt(
                        Prompt::ModpackUpdate {
                            source,
                            plan: Box::new(plan),
                        },
                        cx,
                    ),
                    Err(err) => {
                        this.toast(Toast::error(tr!(M::ModpackPlanFailed, error = err)), cx)
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    /// A file dialog for an `.mrpack`, which is uploaded to the core, then planned.
    fn choose_modpack_file(&mut self, cx: &mut Context<Self>) {
        let dialog = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(t(M::ModpackFilePrompt).into()),
        });
        let admin = self.state.read(cx).admin.clone();
        let task = cx.spawn(async move |this, cx| {
            let path = match dialog.await {
                Ok(Ok(Some(mut paths))) if !paths.is_empty() => paths.remove(0),
                Ok(Err(_)) => {
                    this.update(cx, |this, cx| {
                        this.toast(
                            Toast::error(t(crate::i18n::CommonMsg::FileDialogUnavailable)),
                            cx,
                        )
                    })
                    .ok();
                    return;
                }
                _ => return,
            };
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if !name.to_lowercase().ends_with(".mrpack") {
                this.update(cx, |this, cx| {
                    this.toast(Toast::error(tr!(M::ModpackNotMrpack, name = name)), cx)
                })
                .ok();
                return;
            }
            this.update(cx, |this, cx| {
                this.modpack_busy = true;
                cx.notify();
            })
            .ok();
            let uploaded = cx
                .background_spawn(async move {
                    let bytes = std::fs::read(&path)
                        .map_err(|err| launcher_core::cores::CoreError::Invalid(err.to_string()))?;
                    admin.upload_modpack(&name, &bytes).await
                })
                .await;
            this.update(cx, |this, cx| {
                this.modpack_busy = false;
                match uploaded {
                    Ok(info) => {
                        // the core gives no upload id for a pack it can't use
                        if let Some(reason) = info.unsupported_reason {
                            this.toast(
                                Toast::error(tr!(M::ModpackDialogUnsupported, reason = reason)),
                                cx,
                            );
                        } else if let Some(upload_id) = info.upload_id {
                            this.plan_modpack(ModpackSource::Upload { upload_id }, cx);
                        }
                    }
                    Err(err) => {
                        this.toast(Toast::error(tr!(M::ModpackPlanFailed, error = err)), cx)
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    /// Runs the update the user confirmed.
    pub(super) fn run_modpack_update(
        &mut self,
        source: ModpackSource,
        plan: ModpackUpdatePlan,
        cx: &mut Context<Self>,
    ) {
        if self.modpack_busy {
            return;
        }
        self.modpack_busy = true;
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { admin.update_modpack(&uuid, &source).await })
                .await;
            this.update(cx, |this, cx| {
                this.modpack_busy = false;
                let toast = match result {
                    Ok(done) => {
                        let version = plan
                            .target
                            .version_number
                            .clone()
                            .unwrap_or_else(|| t(M::ModpackUnknownVersion).to_string());
                        let mut lines = vec![tr!(
                            M::ModpackUpdated,
                            pack = plan.target.name,
                            version = version
                        )];
                        let counts = [
                            (done.added, P::ModpackAdded),
                            (done.replaced, P::ModpackReplaced),
                            (done.removed, P::ModpackRemoved),
                        ];
                        let counts: Vec<String> = counts
                            .into_iter()
                            .filter(|(n, _)| *n > 0)
                            .map(|(n, message)| trn!(message, n as usize))
                            .collect();
                        if !counts.is_empty() {
                            lines.push(counts.join(", "));
                        }
                        lines.extend(done.skipped);
                        this.modpack_check = ModpackCheck::Idle;
                        Toast::success(lines.join("\n"))
                    }
                    Err(err) => Toast::error(tr!(M::ModpackUpdateFailed, error = err)),
                };
                this.toast(toast, cx);
                this.reload(cx);
                this.state.update(cx, |state, cx| state.reload(cx));
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    /// The panel, for a server made from a modpack.
    pub(super) fn modpack_panel(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let origin = self.modpack_origin(cx)?;
        let can_write = self.can_write(cx);
        let on_modrinth = !origin.project_id.is_empty();
        let status = match &self.modpack_check {
            ModpackCheck::Newest => Some(muted(t(M::ModpackUpToDate))),
            ModpackCheck::Failed(error) => Some(
                div()
                    .text_color(colors::RED_200)
                    .child(tr!(M::ModpackCheckFailed, error = error))
                    .into_any_element(),
            ),
            _ => None,
        };
        let main_button = match &self.modpack_check {
            ModpackCheck::Available(version) if on_modrinth => {
                let source = ModpackSource::Modrinth {
                    version_id: version.id.clone(),
                };
                Some(why_not(
                    "modpack-update-why",
                    can_write,
                    Button::new(
                        "modpack-update",
                        tr!(M::ModpackUpdateTo, version = version.version_number),
                    )
                    .variant(ButtonVariant::Primary)
                    .icon(Icon::CircleUp)
                    .loading(self.modpack_busy)
                    .disabled(!can_write || self.modpack_busy)
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.plan_modpack(source.clone(), cx)),
                    ),
                ))
            }
            _ if on_modrinth => Some(
                Button::new("modpack-check", t(M::ModpackCheck))
                    .loading(matches!(self.modpack_check, ModpackCheck::Checking))
                    .disabled(matches!(self.modpack_check, ModpackCheck::Checking))
                    .on_click(cx.listener(|this, _, _, cx| this.check_modpack(cx)))
                    .into_any_element(),
            ),
            _ => None,
        };
        let may_upload = can_write && self.can_create(cx);
        let file_button = why_not(
            "modpack-file-why",
            may_upload,
            Button::new("modpack-file", t(M::ModpackFromFile))
                .icon(Icon::Upload)
                .disabled(!may_upload || self.modpack_busy)
                .on_click(cx.listener(|this, _, _, cx| this.choose_modpack_file(cx))),
        );
        Some(
            div()
                .id("modpack-panel")
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .p(px(16.))
                .rounded(radius::XL)
                .border_1()
                .border_color(ink::border())
                .bg(colors::GRAY_850)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(
                            div()
                                .text_size(text::H3)
                                .font_weight(weights::BOLD.face())
                                .text_color(ink::text())
                                .child(origin_label(&origin)),
                        )
                        .child(muted(tr!(M::ModpackFrom, pack = origin.name)))
                        .children(status),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap(px(8.))
                        .children(main_button)
                        .child(file_button),
                )
                .into_any_element(),
        )
    }

    /// The dialog's text for an update: what changes, and why it can't run, if so.
    /// Returns the body and whether the update may run.
    pub(super) fn modpack_dialog_body(
        &self,
        plan: &ModpackUpdatePlan,
        cx: &Context<Self>,
    ) -> (AnyElement, bool) {
        let unknown = || t(M::ModpackUnknownVersion).to_string();
        let current = self
            .modpack_origin(cx)
            .and_then(|o| o.version_number)
            .unwrap_or_else(unknown);
        let target = &plan.target;
        let mut lines: Vec<AnyElement> = vec![
            div()
                .child(tr!(
                    M::ModpackDialogIntro,
                    pack = target.name,
                    from = current,
                    to = target.version_number.clone().unwrap_or_else(unknown)
                ))
                .into_any_element(),
        ];
        if let Some(reason) = &target.unsupported_reason {
            lines.push(
                div()
                    .text_color(colors::RED_200)
                    .child(tr!(M::ModpackDialogUnsupported, reason = reason))
                    .into_any_element(),
            );
        }
        if plan.reinstalls_server {
            let software = format!(
                "{} {} · Minecraft {}",
                loader_title(&target.loader),
                target.loader_version.clone().unwrap_or_default(),
                target.game_version
            );
            lines.push(
                div()
                    .child(tr!(M::ModpackDialogReinstall, software = software))
                    .into_any_element(),
            );
        }
        let counts: Vec<String> = [
            (plan.added.len(), P::ModpackAdded),
            (plan.replaced.len(), P::ModpackReplaced),
            (plan.removed.len(), P::ModpackRemoved),
        ]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, message)| trn!(message, n))
        .collect();
        lines.push(
            div()
                .child(if counts.is_empty() {
                    t(M::ModpackDialogNothing).to_string()
                } else {
                    counts.join(", ")
                })
                .into_any_element(),
        );
        if !plan.removed.is_empty() {
            lines.push(muted(short_list(&plan.removed, 8)));
        }
        if !plan.kept.is_empty() {
            lines.push(muted(tr!(
                M::ModpackDialogKept,
                files = short_list(&plan.kept, 8)
            )));
        }
        if let Some(note) = &plan.note {
            lines.push(muted(note.clone()));
        }
        let stopped = self.is_stopped(cx);
        if !stopped {
            lines.push(warning(t(M::ModpackDialogStop)));
        }
        let may_run = stopped && target.unsupported_reason.is_none() && self.can_write(cx);
        (
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .text_size(text::MEDIUM)
                .line_height(line_height(text::MEDIUM, leading::NORMAL))
                .text_color(ink::secondary())
                .children(lines)
                .into_any_element(),
            may_run,
        )
    }
}
