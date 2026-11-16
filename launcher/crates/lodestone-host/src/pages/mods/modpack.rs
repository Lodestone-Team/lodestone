//! The modpack panel of the Installed view, for a server made from a modpack: which
//! version it runs, whether a newer one is out (the core checks on its own, and on
//! "Check for an update"), an update from Modrinth or from an `.mrpack` file, and the
//! history: a timeline of the server's content images (every install, update and restore,
//! and every other change to its mods and configs), with what each changed, and a button
//! to go back to any of them. Each mod row says since when its file is as it is
//! ([`ModsPage::blame_label`]).
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
        AvailableUpdate, ChangeInfo, ChangeKind, HistoryEntryInfo, ModpackUpdatePlan, loader_title,
    },
};
use lodestone_ui::{
    components::{
        Button, ButtonVariant, DiffValue, Fact, FactLine, FactList, FactValue, Icon, ItemIcon,
        Kind, NameGroup, NameItem, NameList, NameStyle, Num, ObjectCard, ObjectChip, ProductPhrase,
        StatusBadge, Toast, VersionTag, VersionTone, readable::object_name, text_tooltip,
    },
    i18n::format,
    theme::{Color, Tone, colors, ink, leading, line_height, px, radius, text, weights},
    values::Product,
};

use super::{ModsPage, Prompt, installed::why_not, muted, warning};
use crate::i18n::{FactMsg, InstanceMsg as M, InstancePlural as P, t, tr, trn};

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

/// The game and the loader a modpack version runs on, as plain phrases ("Minecraft
/// 1.21.1", "Fabric 0.16.9"); no loader for a vanilla pack.
fn software(
    game: &str,
    loader: &str,
    version: Option<&str>,
) -> (ProductPhrase, Option<ProductPhrase>) {
    (
        ProductPhrase::new(Product::Minecraft, game.to_string()),
        ProductPhrase::from_name(
            &loader_title(loader),
            version.unwrap_or_default().to_string(),
        ),
    )
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

/// A count of changes as a fact: an icon whose tooltip says "Added: 3".
fn change_fact(kind: ChangeKind, count: usize) -> Fact {
    let (icon, key) = match kind {
        ChangeKind::Added => (Icon::Plus, t(FactMsg::Added)),
        ChangeKind::Updated => (Icon::ArrowUp, t(FactMsg::Updated)),
        ChangeKind::Replaced => (Icon::ArrowsRotate, t(M::ModpackReplacedKey)),
        ChangeKind::Removed | ChangeKind::Unknown => (Icon::Xmark, t(FactMsg::Removed)),
    };
    Fact::icon(icon, key, Num::count(count as u64))
}

/// A changelog (Markdown from Modrinth), in a box that scrolls when it is long.
fn changelog_box(id: impl Into<SharedString>, changelog: &str) -> AnyElement {
    let blocks = launcher_core::markdown::parse(changelog);
    div()
        .id(id.into())
        .max_h(px(320.))
        .overflow_y_scroll()
        .p(px(10.))
        .rounded(radius::MD)
        .bg(colors::GRAY_900)
        .text_size(text::SMALL)
        .child(lodestone_ui::markdown::render_sized(
            "changelog",
            &blocks,
            &std::collections::HashMap::new(),
            "",
            480.,
        ))
        .into_any_element()
}

/// "What's new", with a chevron that says whether it is open: a quiet link that
/// shows a changelog only when it is asked for.
fn whats_new_toggle(id: impl Into<SharedString>, open: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.into())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.))
        .cursor_pointer()
        .text_size(text::SMALL)
        .text_color(colors::BLUE_200)
        .hover(|style| style.underline())
        .child(
            if open {
                Icon::ChevronDown
            } else {
                Icon::ChevronRight
            }
            .render(px(8.), colors::BLUE_200),
        )
        .child(t(M::ModpackWhatsNew))
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
                    this.toast(
                        Toast::error(t(M::ModpackNotMrpack))
                            .object(ObjectChip::new(name).kind(Kind::File).mono()),
                        cx,
                    )
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
                        // a restore's target is named by the entry it goes back to
                        let pack = plan.target.name.clone();
                        let mut lines = vec![if restore {
                            t(M::ModpackRestored).to_string()
                        } else {
                            t(M::ModpackUpdated).to_string()
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
                        lines.extend(counts);
                        lines.extend(done.skipped);
                        this.modpack_check = ModpackCheck::Idle;
                        let toast = if restore {
                            Toast::success(lines.join("\n"))
                                .object(ObjectChip::new(pack).icon(Icon::CodeCompare))
                        } else {
                            Toast::success(lines.join("\n"))
                                .object(ObjectChip::new(pack).kind(Kind::Modpack))
                                .version(VersionTag::new(version))
                        };
                        toast
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

    /// When the entry a file of the mods folder came with was recorded ("3 Mar"), with
    /// the tooltip's text, which names that entry.
    pub(super) fn blame_label(&self, file_name: &str) -> Option<(String, String)> {
        let history = self.modpack_history.as_ref()?;
        // a server that never changed its pack has nothing to tell apart
        if history.entries.len() < 2 {
            return None;
        }
        let directory = self.list.as_ref().map_or("mods", |l| l.directory.as_str());
        let entry = history.blame_of(&format!("{directory}/{file_name}"))?;
        Some((
            entry_time(entry.time)?,
            tr!(M::ModpackSinceTooltip, change = entry.label),
        ))
    }

    /// The panel, for a server made from a modpack.
    pub(super) fn modpack_panel(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let origin = self.modpack_origin(cx)?;
        let can_write = self.can_write(cx);
        let on_modrinth = !origin.project_id.is_empty();
        let status = match &self.modpack_check {
            ModpackCheck::Newest => Some(muted(t(M::ModpackUpToDate))),
            ModpackCheck::Available(update) if update.changelog.is_some() => {
                let open = self.update_changelog_open;
                Some(
                    whats_new_toggle("modpack-update-changelog", open)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.update_changelog_open = !this.update_changelog_open;
                            cx.notify();
                        }))
                        .into_any_element(),
                )
            }
            ModpackCheck::Failed(error) => Some(
                div()
                    .text_color(colors::RED_200)
                    .child(tr!(M::ModpackCheckFailed, error = error))
                    .into_any_element(),
            ),
            _ => None,
        };
        // the version it runs, and the newer one when one is out
        let available = match &self.modpack_check {
            ModpackCheck::Available(update) => Some(update.version_number.clone()),
            _ => None,
        };
        let versions = FactLine::strip("modpack-versions")
            .facts(
                origin
                    .version_number
                    .clone()
                    .map(|version| Fact::new(t(FactMsg::Installed), VersionTag::new(version))),
            )
            .facts(available.clone().map(|version| {
                Fact::new(
                    t(FactMsg::Available),
                    VersionTag::new(version).tone(VersionTone::To),
                )
            }));
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
        let mut history_button = Button::new("modpack-history", t(M::ModpackHistory));
        if entries > 0 {
            history_button = history_button.count(entries as u64);
        }
        let history_button =
            history_button
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
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                object_name(origin.name.clone())
                                    .text_size(text::H3)
                                    .font_weight(weights::BOLD.face()),
                            )
                            .when(available.is_some(), |this| {
                                this.child(StatusBadge::new(
                                    t(M::ModpackUpdateAvailable),
                                    Tone::Info,
                                ))
                            }),
                    )
                    .child(muted(t(M::ModpackFrom)))
                    .child(versions)
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
                .when_some(
                    match &self.modpack_check {
                        ModpackCheck::Available(update) if self.update_changelog_open => {
                            update.changelog.clone()
                        }
                        _ => None,
                    },
                    |this, changelog| {
                        this.child(changelog_box("modpack-update-changelog-text", &changelog))
                    },
                )
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
            .map(|(index, entry)| self.history_row(entry, index == last, can_write, cx))
            .collect();
        div()
            .id("modpack-history-list")
            .flex()
            .flex_col()
            .pt(px(12.))
            .border_t_1()
            .border_color(ink::border())
            .children(rows)
            .into_any_element()
    }

    fn history_row(
        &self,
        entry: &HistoryEntryInfo,
        last: bool,
        can_write: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = entry.id.clone();
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
        // what it changed, the game and loader it ran, and when it was recorded
        let (game, loader) = software(
            &entry.game_version,
            &entry.loader,
            entry.loader_version.as_deref(),
        );
        let facts: Vec<Fact> = [
            ChangeKind::Added,
            ChangeKind::Updated,
            ChangeKind::Replaced,
            ChangeKind::Removed,
        ]
        .into_iter()
        .filter_map(|kind| {
            let n = entry.count(kind);
            (n > 0).then(|| change_fact(kind, n).priority(2))
        })
        .chain(Some(Fact::bare_named(game.key(), game).priority(1)))
        .chain(loader.map(|loader| Fact::bare_named(loader.key(), loader).priority(1)))
        .chain(
            entry_time(entry.time)
                .map(|time| Fact::new(t(M::ModpackRecorded), FactValue::time(time))),
        )
        .collect();
        let restore = (!entry.current).then(|| {
            let missing = entry.missing > 0;
            let button = Button::new(
                SharedString::from(format!("modpack-restore-{id}")),
                t(M::ModpackRestore),
            )
            .icon(Icon::ArrowsRotate)
            .disabled(!can_write || missing || self.modpack_busy)
            .on_click({
                let id = id.clone();
                cx.listener(move |this, _, _, cx| {
                    this.plan_modpack(
                        ModpackSource::History {
                            entry_id: id.clone(),
                        },
                        cx,
                    )
                })
            });
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
        let has_changes = !entry.changes.is_empty();
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
                    .on_click({
                        let id = id.clone();
                        cx.listener(move |this, _, _, cx| {
                            if !this.history_expanded.remove(&id) {
                                this.history_expanded.insert(id.clone());
                            }
                            cx.notify();
                        })
                    })
            })
            .child(object_name(entry.label.clone()))
            .when(entry.current, |this| {
                this.child(StatusBadge::new(t(M::ModpackCurrent), Tone::Success))
            });
        let details =
            FactLine::row(SharedString::from(format!("modpack-entry-facts-{id}"))).facts(facts);
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
                                    .flex_1()
                                    .min_w_0()
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
        restore: bool,
        cx: &Context<Self>,
    ) -> (AnyElement, bool) {
        let unknown = || t(M::ModpackUnknownVersion).to_string();
        let current = self
            .modpack_origin(cx)
            .and_then(|o| o.version_number)
            .unwrap_or_else(unknown);
        let target = &plan.target;
        let mut lines: Vec<AnyElement> = Vec::new();
        // what changes: the pack (a restore's target is named by the entry it goes back
        // to), then its version from and to
        let to = target.version_number.clone().unwrap_or_else(unknown);
        if restore {
            lines.push(
                ObjectCard::new(target.name.clone())
                    .thumbnail(
                        div()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(colors::GRAY_750)
                            .child(Icon::CodeCompare.render(px(16.), colors::GRAY_500)),
                    )
                    .fact(Fact::bare_named(
                        t(FactMsg::Version),
                        VersionTag::new(to.clone()),
                    ))
                    .into_any_element(),
            );
            lines.push(
                div()
                    .child(t(M::ModpackDialogRestoreIntro))
                    .into_any_element(),
            );
        } else {
            lines.push(
                ObjectCard::new(target.name.clone())
                    .kind(Kind::Modpack)
                    .into_any_element(),
            );
        }
        let mut facts = FactList::new("modpack-plan");
        if !restore {
            facts = facts.row(t(FactMsg::Version), DiffValue::versions(current, to));
        }
        if plan.reinstalls_server {
            let (game, loader) = software(
                &target.game_version,
                &target.loader,
                target.loader_version.as_deref(),
            );
            facts = facts.row(game.key(), game);
            if let Some(loader) = loader {
                facts = facts.row(loader.key(), loader);
            }
        }
        for (n, key) in [
            (plan.added.len(), t(FactMsg::Added)),
            (plan.replaced.len(), t(M::ModpackReplacedKey)),
            (plan.removed.len(), t(FactMsg::Removed)),
        ] {
            if n > 0 {
                facts = facts.row(key, Num::count(n as u64));
            }
        }
        if let Some(reason) = &target.unsupported_reason {
            lines.push(
                div()
                    .text_color(colors::RED_200)
                    .child(tr!(M::ModpackDialogUnsupported, reason = reason))
                    .into_any_element(),
            );
        }
        if plan.reinstalls_server {
            lines.push(div().child(t(M::ModpackDialogReinstall)).into_any_element());
        }
        lines.push(facts.into_any_element());
        if plan.added.is_empty() && plan.replaced.is_empty() && plan.removed.is_empty() {
            lines.push(div().child(t(M::ModpackDialogNothing)).into_any_element());
        }
        // the files removed, and the ones kept as the person changed them
        let files = |heading: &str, paths: &[String], icon: ItemIcon| {
            NameGroup::new(heading.to_string()).items(paths.iter().map(|path| {
                NameItem::new(path.clone())
                    .icon(icon.clone())
                    .style(NameStyle::Path)
            }))
        };
        if !plan.removed.is_empty() || !plan.kept.is_empty() {
            let mut list = NameList::new("modpack-plan-files").collapsed_after(5);
            if !plan.removed.is_empty() {
                list = list.group(files(
                    t(FactMsg::Removed),
                    &plan.removed,
                    ItemIcon::Kind(Kind::File),
                ));
            }
            if !plan.kept.is_empty() {
                list = list.group(files(
                    t(M::ModpackDialogKept),
                    &plan.kept,
                    ItemIcon::Kind(Kind::File),
                ));
            }
            lines.push(list.into_any_element());
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
        let history: launcher_core::cores::admin::mods::ModpackHistory = serde_json::from_value(serde_json::json!({
            "entries": [
                {
                    "id": "c".repeat(64), "time": now - 2 * 60 * 60 * 1000,
                    "label": "Updated Fabulously Optimized to 6.4.0",
                    "game_version": "1.21.4", "loader": "fabric", "loader_version": "0.16.10",
                    "current": true, "file_count": 64, "missing": 0,
                    "changes": [
                        { "kind": "added", "path": "mods/entityculling-1.7.3.jar" },
                        { "kind": "updated", "path": "mods/sodium-0.6.9.jar", "old_path": "mods/sodium-0.6.6.jar" },
                        { "kind": "updated", "path": "mods/lithium-0.15.0.jar", "old_path": "mods/lithium-0.14.7.jar" },
                        { "kind": "replaced", "path": "config/modmenu.json" },
                        { "kind": "removed", "path": "mods/starlight-1.1.3.jar" }
                    ]
                },
                {
                    "id": "b".repeat(64), "time": now - 9 * day,
                    "label": "Added Starlight; Updated Sodium",
                    "game_version": "1.21.4", "loader": "fabric", "loader_version": "0.16.9",
                    "file_count": 63, "missing": 0,
                    "changes": [
                        { "kind": "updated", "path": "mods/sodium-0.6.6.jar", "old_path": "mods/sodium-0.6.5.jar" },
                        { "kind": "added", "path": "mods/starlight-1.1.3.jar" }
                    ]
                },
                {
                    "id": "a".repeat(64), "time": now - 30 * day,
                    "label": "Installed Fabulously Optimized 6.2.0",
                    "game_version": "1.21.3", "loader": "fabric", "loader_version": "0.16.7",
                    "file_count": 61, "missing": 2, "changes": []
                }
            ],
            "blame": {
                "mods/sodium-0.6.9.jar": "c".repeat(64),
                "mods/lithium-0.15.0.jar": "c".repeat(64),
                "mods/entityculling-1.7.3.jar": "c".repeat(64),
                "mods/fabric-api-0.110.5.jar": "a".repeat(64),
                "mods/modmenu-13.0.0.jar": "b".repeat(64)
            },
            "update": {
                "version_id": "fo-650", "version_number": "6.5.0", "name": "6.5.0",
                "game_versions": ["1.21.4"], "date_published": "2026-11-10T00:00:00Z",
                "changelog": "## 6.5.0\n\n- Minecraft 1.21.4 hotfixes\n- Lithium 0.15.1"
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
        self.history_expanded.insert("c".repeat(64));
        // fetches still under way are for a server the core doesn't have
        self.history_serial += 1;
        self.list_serial += 1;
        self.list = Some(list);
        self.list_error = None;
        self.loading = false;
        cx.notify();
    }
}
