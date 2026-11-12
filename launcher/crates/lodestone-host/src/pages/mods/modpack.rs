//! The modpack panel of the Installed view, for a server made from a modpack: which
//! version it runs, whether a newer one is out (the core checks on its own, and on
//! "Check for an update"), an update from Modrinth or from an `.mrpack` file, and the
//! history: a timeline of every install, update and restore, with what each changed, and
//! a button to go back to any of them. Each mod row says which version its file came
//! with ([`ModsPage::blame_label`]).
//!
//! The core works out what an update or restore changes first; the dialog shows that,
//! and it runs once the user confirms, with the server stopped.

use gpui::{
    AnyElement, AppContext as _, Context, IntoElement, ParentElement, PathPromptOptions,
    SharedString, Styled, div, prelude::*,
};
use launcher_core::cores::admin::{
    instances::ModpackSource,
    models::{InstanceState, ModpackOrigin},
    mods::{
        AvailableUpdate, ChangeInfo, ChangeKind, HistoryEntryInfo, HistoryKind, ModpackHistory,
        ModpackUpdatePlan, loader_title,
    },
};
use lodestone_ui::{
    components::{Button, ButtonVariant, Icon, Toast, text_tooltip},
    i18n::format,
    theme::{Color, colors, ink, leading, line_height, px, radius, text, weights},
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
    Available(AvailableUpdate),
    Failed(String),
}

/// How many changes an entry lists before "and N more".
const CHANGES_SHOWN: usize = 40;

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

/// The sign and color of a change: `+` added, `↑` updated, `~` replaced, `−` removed.
fn change_mark(kind: ChangeKind) -> (&'static str, Color) {
    match kind {
        ChangeKind::Added => ("+", colors::GREEN),
        ChangeKind::Updated => ("↑", colors::BLUE_200),
        ChangeKind::Replaced => ("~", colors::YELLOW),
        ChangeKind::Removed | ChangeKind::Unknown => ("−", colors::RED_200),
    }
}

fn entry_version(entry: &HistoryEntryInfo) -> String {
    entry
        .version_number
        .clone()
        .unwrap_or_else(|| t(M::ModpackUnknownVersion).to_string())
}

/// When an entry happened, as the overview shows times.
fn entry_time(millis: i64) -> Option<String> {
    let time = chrono::DateTime::from_timestamp_millis(millis)?;
    let local = time.with_timezone(&chrono::Local);
    Some(format::day_and_time(
        local,
        chrono::Local::now().date_naive(),
    ))
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

    /// Fetches the history (and what the core's last check found).
    pub(super) fn load_modpack_history(&mut self, cx: &mut Context<Self>) {
        if !self.can_read(cx) {
            return;
        }
        self.history_serial += 1;
        let serial = self.history_serial;
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let history = cx
                .background_spawn(async move { admin.modpack_history(&uuid).await })
                .await;
            this.update(cx, |this, cx| {
                if serial != this.history_serial {
                    return;
                }
                match history {
                    Ok(history) => {
                        if let Some(update) = history.update.clone() {
                            this.modpack_check = ModpackCheck::Available(update);
                        }
                        this.modpack_history = Some(history);
                        this.modpack_history_error = None;
                    }
                    Err(err) => this.modpack_history_error = Some(err.to_string()),
                }
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    /// Asks the core to look for a newer version now.
    fn check_modpack(&mut self, cx: &mut Context<Self>) {
        self.modpack_check = ModpackCheck::Checking;
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let found = cx
                .background_spawn(async move { admin.check_modpack(&uuid).await })
                .await;
            this.update(cx, |this, cx| {
                this.modpack_check = match found {
                    Ok(Some(update)) => ModpackCheck::Available(update),
                    Ok(None) => ModpackCheck::Newest,
                    Err(err) => ModpackCheck::Failed(err.to_string()),
                };
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    /// Asks the core what updating (or restoring) to `source` does, then shows it.
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

    /// Runs the update or restore the user confirmed.
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
        let restore = matches!(source, ModpackSource::History { .. });
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
                        let pack = plan.target.name.clone();
                        let mut lines = vec![if restore {
                            tr!(M::ModpackRestored, pack = pack, version = version)
                        } else {
                            tr!(M::ModpackUpdated, pack = pack, version = version)
                        }];
                        let counts: Vec<String> = [
                            (done.added, P::ModpackAdded),
                            (done.replaced, P::ModpackReplaced),
                            (done.removed, P::ModpackRemoved),
                        ]
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
                    Err(err) if restore => Toast::error(tr!(M::ModpackRestoreFailed, error = err)),
                    Err(err) => Toast::error(tr!(M::ModpackUpdateFailed, error = err)),
                };
                this.toast(toast, cx);
                this.reload(cx);
                this.load_modpack_history(cx);
                this.state.update(cx, |state, cx| state.reload(cx));
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    /// "since 6.4.0" for a file of the mods folder that came with the pack: the version
    /// its current file came with. With the tooltip's text.
    pub(super) fn blame_label(&self, file_name: &str) -> Option<(String, String)> {
        let history = self.modpack_history.as_ref()?;
        // a server that never changed its pack has nothing to tell apart
        if history.entries.len() < 2 {
            return None;
        }
        let directory = self.list.as_ref().map_or("mods", |l| l.directory.as_str());
        let entry = history.blame_of(&format!("{directory}/{file_name}"))?;
        let version = entry_version(entry);
        Some((
            tr!(M::ModpackSince, version = version),
            tr!(M::ModpackSinceTooltip, version = version),
        ))
    }

    /// The panel, for a server made from a modpack.
    pub(super) fn modpack_panel(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let origin = self.modpack_origin(cx)?;
        let can_write = self.can_write(cx);
        let on_modrinth = !origin.project_id.is_empty();
        let status = match &self.modpack_check {
            ModpackCheck::Newest => Some(muted(t(M::ModpackUpToDate))),
            ModpackCheck::Available(update) => Some(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .text_color(colors::GREEN)
                    .child(div().size(px(6.)).rounded_full().bg(colors::GREEN))
                    .child(tr!(M::ModpackOut, version = update.version_number))
                    .into_any_element(),
            ),
            ModpackCheck::Failed(error) => Some(
                div()
                    .text_color(colors::RED_200)
                    .child(tr!(M::ModpackCheckFailed, error = error))
                    .into_any_element(),
            ),
            _ => None,
        };
        let main_button = match &self.modpack_check {
            ModpackCheck::Available(update) if on_modrinth => {
                let source = ModpackSource::Modrinth {
                    version_id: update.version_id.clone(),
                };
                Some(why_not(
                    "modpack-update-why",
                    can_write,
                    Button::new(
                        "modpack-update",
                        tr!(M::ModpackUpdateTo, version = update.version_number),
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
        let entries = self.modpack_history.as_ref().map_or(0, |h| h.entries.len());
        let history_button = Button::new(
            "modpack-history",
            if entries > 0 {
                format!("{} ({entries})", t(M::ModpackHistory))
            } else {
                t(M::ModpackHistory).to_string()
            },
        )
        .icon(Icon::CodeCompare)
        .on_click(cx.listener(|this, _, _, cx| {
            this.history_open = !this.history_open;
            if this.history_open && this.modpack_history.is_none() {
                this.load_modpack_history(cx);
            }
            cx.notify();
        }));
        let header = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(12.))
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
                    .child(file_button)
                    .child(history_button),
            );
        Some(
            div()
                .id("modpack-panel")
                .flex()
                .flex_col()
                .gap(px(16.))
                .p(px(16.))
                .rounded(radius::XL)
                .border_1()
                .border_color(ink::border())
                .bg(colors::GRAY_850)
                .child(header)
                .when(self.history_open, |this| {
                    this.child(self.render_history(can_write, cx))
                })
                .into_any_element(),
        )
    }

    /// The timeline: newest first, a dot per entry on a line, what it changed when it is
    /// opened, and a Restore button on every entry but the current one.
    fn render_history(&self, can_write: bool, cx: &mut Context<Self>) -> AnyElement {
        if let Some(error) = &self.modpack_history_error {
            return div()
                .text_color(colors::RED_200)
                .child(tr!(M::ModpackHistoryFailed, error = error))
                .into_any_element();
        }
        let Some(history) = &self.modpack_history else {
            return div()
                .h(px(48.))
                .flex()
                .items_center()
                .justify_center()
                .child(lodestone_ui::components::LoadingDots)
                .into_any_element();
        };
        if history.entries.is_empty() {
            return muted(t(M::ModpackHistoryEmpty));
        }
        let last = history.entries.len() - 1;
        let rows: Vec<AnyElement> = history
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| self.history_row(history, entry, index == last, can_write, cx))
            .collect();
        div()
            .id("modpack-history-list")
            .flex()
            .flex_col()
            .pt(px(12.))
            .border_t_1()
            .border_color(ink::border())
            .children(rows)
            .when(history.stored_bytes > 0, |this| {
                this.child(
                    div()
                        .pt(px(8.))
                        .text_size(text::SMALL)
                        .text_color(ink::faint())
                        .child(tr!(
                            M::ModpackStoreSize,
                            size = format::bytes(history.stored_bytes)
                        )),
                )
            })
            .into_any_element()
    }

    fn history_row(
        &self,
        history: &ModpackHistory,
        entry: &HistoryEntryInfo,
        last: bool,
        can_write: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = entry.id;
        let open = self.history_expanded.contains(&id);
        let dot_color = if entry.current {
            colors::GREEN
        } else {
            ink::faint()
        };
        // the dot and the line down to the next entry
        let rail = div()
            .flex_none()
            .w(px(16.))
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .mt(px(5.))
                    .size(px(10.))
                    .rounded_full()
                    .border_2()
                    .border_color(dot_color)
                    .when(entry.current, |this| this.bg(dot_color)),
            )
            .when(!last, |this| {
                this.child(div().flex_1().w(px(2.)).bg(ink::border()))
            });
        let kind = match &entry.kind {
            HistoryKind::Install => t(M::ModpackKindInstall).to_string(),
            HistoryKind::Baseline => t(M::ModpackKindBaseline).to_string(),
            HistoryKind::Restore { to } => tr!(
                M::ModpackKindRestore,
                version = history
                    .entry(*to)
                    .map(entry_version)
                    .unwrap_or_else(|| t(M::ModpackUnknownVersion).to_string())
            ),
            HistoryKind::Update | HistoryKind::Unknown => t(M::ModpackKindUpdate).to_string(),
        };
        let counts: Vec<AnyElement> = [
            ChangeKind::Added,
            ChangeKind::Updated,
            ChangeKind::Replaced,
            ChangeKind::Removed,
        ]
        .into_iter()
        .filter_map(|kind| {
            let n = entry.count(kind);
            (n > 0).then(|| {
                let (sign, color) = change_mark(kind);
                lodestone_ui::components::settings::tag(format!("{sign}{n}"), color)
                    .into_any_element()
            })
        })
        .collect();
        let software = format!(
            "Minecraft {} · {} {}",
            entry.game_version,
            loader_title(&entry.loader),
            entry.loader_version.clone().unwrap_or_default()
        );
        let restore = (!entry.current).then(|| {
            let missing = entry.missing > 0;
            let button = Button::new(
                SharedString::from(format!("modpack-restore-{id}")),
                t(M::ModpackRestore),
            )
            .icon(Icon::ArrowsRotate)
            .disabled(!can_write || missing || self.modpack_busy)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.plan_modpack(ModpackSource::History { entry_id: id }, cx)
            }));
            if missing {
                div()
                    .id(SharedString::from(format!("modpack-restore-why-{id}")))
                    .child(button)
                    .tooltip(text_tooltip(t(M::ModpackRestoreMissing)))
                    .into_any_element()
            } else {
                why_not(format!("modpack-restore-why-{id}"), can_write, button)
            }
        });
        let has_changes = !entry.changes.is_empty() || !entry.kept.is_empty();
        let header = div()
            .id(SharedString::from(format!("modpack-entry-{id}")))
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .when(has_changes, |this| {
                this.cursor_pointer()
                    .child(
                        if open {
                            Icon::ChevronDown
                        } else {
                            Icon::ChevronRight
                        }
                        .render(px(10.), ink::muted()),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.history_expanded.remove(&id) {
                            this.history_expanded.insert(id);
                        }
                        cx.notify();
                    }))
            })
            .child(
                div()
                    .font_weight(weights::BOLD.face())
                    .text_color(ink::text())
                    .child(entry_version(entry)),
            )
            .child(div().text_color(ink::secondary()).child(kind))
            .when(entry.current, |this| {
                this.child(lodestone_ui::components::settings::tag(
                    t(M::ModpackCurrent),
                    colors::GREEN,
                ))
            })
            .children(counts);
        let details = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(8.))
            .text_size(text::SMALL)
            .text_color(ink::faint())
            .children(entry_time(entry.time))
            .child(software)
            .when(!entry.kept.is_empty(), |this| {
                this.child(trn!(P::ModpackKeptCount, entry.kept.len()))
            });
        let changes = open.then(|| self.change_list(entry));
        div()
            .flex()
            .flex_row()
            .gap(px(10.))
            .child(rail)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .pb(px(14.))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_start()
                            .justify_between()
                            .gap(px(8.))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(header)
                                    .child(details),
                            )
                            .children(restore),
                    )
                    .children(changes),
            )
            .into_any_element()
    }

    /// The files an entry changed, one per line: `+ new.jar`, `↑ old.jar → new.jar`.
    fn change_list(&self, entry: &HistoryEntryInfo) -> AnyElement {
        let line = |change: &ChangeInfo| {
            let (sign, color) = change_mark(change.kind);
            let name = match change.old_file_name() {
                Some(old) => format!("{old} → {}", change.file_name()),
                None => change.file_name().to_string(),
            };
            div()
                .flex()
                .flex_row()
                .gap(px(8.))
                .child(
                    div()
                        .w(px(10.))
                        .flex_none()
                        .text_color(color)
                        .font_weight(weights::BOLD.face())
                        .child(sign),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(ink::secondary())
                        .child(name),
                )
                .into_any_element()
        };
        let mut lines: Vec<AnyElement> =
            entry.changes.iter().take(CHANGES_SHOWN).map(line).collect();
        if entry.changes.len() > CHANGES_SHOWN {
            lines.push(muted(trn!(
                P::ModpackMoreChanges,
                entry.changes.len() - CHANGES_SHOWN
            )));
        }
        if !entry.kept.is_empty() {
            lines.push(muted(tr!(
                M::ModpackDialogKept,
                files = short_list(&entry.kept, 8)
            )));
        }
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .mt(px(4.))
            .p(px(8.))
            .rounded(radius::MD)
            .bg(colors::GRAY_900)
            .font_family(lodestone_ui::theme::fonts::mono())
            .text_size(text::SMALL)
            .children(lines)
            .into_any_element()
    }

    /// The dialog's text for an update or restore: what changes, and why it can't run,
    /// if so. Returns the body and whether it may run.
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

impl ModsPage {
    /// A sample history and mod list, for the development screenshots' `--demo`.
    #[cfg(feature = "screenshot")]
    pub fn demo(&mut self, cx: &mut Context<Self>) {
        let day = 24 * 60 * 60 * 1000_i64;
        let now = chrono::Utc::now().timestamp_millis();
        let history: ModpackHistory = serde_json::from_value(serde_json::json!({
            "entries": [
                {
                    "id": 3, "time": now - 2 * 60 * 60 * 1000, "kind": { "type": "update" },
                    "name": "Fabulously Optimized", "version_number": "6.4.0",
                    "game_version": "1.21.4", "loader": "fabric", "loader_version": "0.16.10",
                    "current": true, "file_count": 64, "missing": 0,
                    "kept": ["config/sodium-options.json"],
                    "changes": [
                        { "kind": "added", "path": "mods/entityculling-1.7.3.jar" },
                        { "kind": "updated", "path": "mods/sodium-0.6.9.jar", "old_path": "mods/sodium-0.6.6.jar" },
                        { "kind": "updated", "path": "mods/lithium-0.15.0.jar", "old_path": "mods/lithium-0.14.7.jar" },
                        { "kind": "replaced", "path": "config/modmenu.json" },
                        { "kind": "removed", "path": "mods/starlight-1.1.3.jar" }
                    ]
                },
                {
                    "id": 2, "time": now - 9 * day, "kind": { "type": "update" },
                    "name": "Fabulously Optimized", "version_number": "6.3.1",
                    "game_version": "1.21.4", "loader": "fabric", "loader_version": "0.16.9",
                    "file_count": 63, "missing": 0,
                    "changes": [
                        { "kind": "updated", "path": "mods/sodium-0.6.6.jar", "old_path": "mods/sodium-0.6.5.jar" },
                        { "kind": "added", "path": "mods/starlight-1.1.3.jar" }
                    ]
                },
                {
                    "id": 1, "time": now - 30 * day, "kind": { "type": "install" },
                    "name": "Fabulously Optimized", "version_number": "6.2.0",
                    "game_version": "1.21.3", "loader": "fabric", "loader_version": "0.16.7",
                    "file_count": 61, "missing": 2, "changes": []
                }
            ],
            "blame": {
                "mods/sodium-0.6.9.jar": 3,
                "mods/lithium-0.15.0.jar": 3,
                "mods/entityculling-1.7.3.jar": 3,
                "mods/fabric-api-0.110.5.jar": 1,
                "mods/modmenu-13.0.0.jar": 2
            },
            "stored_bytes": 48_213,
            "update": {
                "version_id": "fo-650", "version_number": "6.5.0", "name": "6.5.0",
                "game_versions": ["1.21.4"], "date_published": "2026-11-10T00:00:00Z"
            }
        }))
        .expect("the sample history reads");
        let list: launcher_core::cores::admin::mods::InstalledModList =
            serde_json::from_value(serde_json::json!({
                "kind": "mod", "loader": "fabric", "game_version": "1.21.4",
                "directory": "mods", "loader_version": "0.16.10",
                "mods": [
                    { "file_name": "sodium-0.6.9.jar", "enabled": true, "size": 1_048_576,
                      "metadata": { "name": "Sodium", "version": "0.6.9", "authors": [] } },
                    { "file_name": "fabric-api-0.110.5.jar", "enabled": true, "size": 2_202_009,
                      "metadata": { "name": "Fabric API", "version": "0.110.5", "authors": [] } },
                    { "file_name": "modmenu-13.0.0.jar", "enabled": true, "size": 811_008,
                      "metadata": { "name": "Mod Menu", "version": "13.0.0", "authors": [] } }
                ]
            }))
            .expect("the sample mods read");
        self.modpack_check = ModpackCheck::Available(history.update.clone().unwrap_or_default());
        self.modpack_history = Some(history);
        self.modpack_history_error = None;
        self.history_open = true;
        self.history_expanded.insert(3);
        // fetches still under way are for a server the core doesn't have
        self.history_serial += 1;
        self.list_serial += 1;
        self.list = Some(list);
        self.list_error = None;
        self.loading = false;
        cx.notify();
    }
}
