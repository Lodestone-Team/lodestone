//! Shared game settings: merged into an instance's `options.txt` before Java starts
//! and read back after the game exits ([`launcher_core::shared_options`]), and the
//! "Shared game settings" section of an instance's Settings tab.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use gpui::{
    Context, Entity, IntoElement, Render, SharedString, Subscription, Window, div, prelude::*, px,
};
use launcher_core::{
    game::log4j::Level,
    paths::DataDir,
    shared_options::{self, Category, ExitOutcome, InstanceSharing, Profile, Row},
};

use super::{Instances, fields::section};
use crate::{
    components::{Button, ButtonSize, ButtonVariant, Icon, TextInput, Toggle},
    theme::{self, colors, radius, text, weights},
};

impl Instances {
    pub fn data_dir(&self) -> &DataDir {
        &self.data_dir
    }

    pub fn instance_dir(&self, id: &str) -> PathBuf {
        self.store.dir(id)
    }

    /// The vanilla client jar of `game_version`, for its data version.
    pub fn client_jar(&self, game_version: &str) -> PathBuf {
        shared_options::client_jar(&self.data_dir, game_version)
    }

    /// The name of an instance, for messages.
    pub fn instance_name(&self, id: &str) -> Option<String> {
        self.list
            .iter()
            .find(|i| i.id == id)
            .map(|i| i.name.clone())
    }

    fn game_version_of(&self, id: &str) -> Option<String> {
        self.list
            .iter()
            .find(|i| i.id == id)
            .map(|i| i.game_version.clone())
            .or_else(|| self.store.get(id).ok().map(|i| i.game_version))
    }

    /// Before Java starts: merges the shared settings into the instance's
    /// `options.txt`. A failure is noted in the console and doesn't stop the game.
    pub(super) fn merge_shared_settings(&mut self, id: &str) {
        let Some(version) = self.game_version_of(id) else {
            return;
        };
        let jar = self.client_jar(&version);
        let result = shared_options::before_launch(
            &self.data_dir,
            id,
            &self.store.dir(id),
            &version,
            Some(&jar),
        );
        let note = match result {
            Ok(outcome) => outcome.note().map(|note| (Level::Info, note)),
            Err(err) => Some((
                Level::Warn,
                format!("Shared game settings weren't applied: {err}"),
            )),
        };
        if let (Some((level, note)), Some(run)) = (note, self.runs.get_mut(id)) {
            run.log.note(level, note);
        }
    }

    /// After the game exits: reads the changed shared settings back.
    pub(super) fn read_back_shared_settings(&mut self, id: &str) {
        let Some(version) = self.game_version_of(id) else {
            return;
        };
        let result = shared_options::after_exit(&self.data_dir, id, &self.store.dir(id), &version);
        let note = match result {
            Ok(ExitOutcome::Nothing) => None,
            Ok(ExitOutcome::Emitted(keys)) => Some((
                Level::Info,
                format!(
                    "Shared game settings: {} value(s) from this instance now apply to every instance",
                    keys.len()
                ),
            )),
            Ok(ExitOutcome::Changed(keys)) => Some((
                Level::Info,
                format!(
                    "Shared game settings: {} setting(s) changed in-game will be replaced by the shared ones next launch (see Settings)",
                    keys.len()
                ),
            )),
            Err(err) => Some((
                Level::Warn,
                format!("Shared game settings couldn't be read back: {err}"),
            )),
        };
        if let (Some((level, note)), Some(run)) = (note, self.runs.get_mut(id)) {
            run.log.note(level, note);
        }
    }
}

/// What the section shows, read from disk.
#[derive(Default)]
struct State {
    profile: Profile,
    sharing: InstanceSharing,
    /// The instance's canonical values.
    local: BTreeMap<String, String>,
    /// The data version of the instance's `options.txt`, when known.
    dv: Option<u32>,
    error: Option<SharedString>,
}

/// The "Shared game settings" section of an instance's Settings tab.
pub struct InstanceSharedSettings {
    instances: Entity<Instances>,
    instance_id: String,
    game_version: String,
    state: State,
    expanded: BTreeSet<Category>,
    editing: Option<(String, Entity<TextInput>)>,
    message: Option<(SharedString, bool)>,
    /// Whether the game was running at the last look, to reload when it stops.
    active: bool,
    _observe: Subscription,
}

impl InstanceSharedSettings {
    pub fn new(
        instances: Entity<Instances>,
        instance_id: String,
        game_version: String,
        cx: &mut Context<Self>,
    ) -> Self {
        let observe = cx.observe(&instances, |this: &mut Self, instances, cx| {
            let active = instances.read(cx).is_active(&this.instance_id);
            if active != this.active {
                this.active = active;
                this.reload(cx);
            }
        });
        let active = instances.read(cx).is_active(&instance_id);
        let mut this = Self {
            instances,
            instance_id,
            game_version,
            state: State::default(),
            expanded: BTreeSet::new(),
            editing: None,
            message: None,
            active,
            _observe: observe,
        };
        this.reload(cx);
        this
    }

    /// Opens categories, for the offscreen screenshots.
    pub fn preview_expand(&mut self, categories: &[Category], cx: &mut Context<Self>) {
        self.expanded.extend(categories.iter().copied());
        cx.notify();
    }

    fn dir(&self, cx: &Context<Self>) -> PathBuf {
        self.instances.read(cx).instance_dir(&self.instance_id)
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let instances = self.instances.read(cx);
        let data = instances.data_dir().clone();
        let dir = instances.instance_dir(&self.instance_id);
        let jar = instances.client_jar(&self.game_version);
        let mut state = State::default();
        match shared_options::load_profile(&data) {
            Ok(profile) => state.profile = profile,
            Err(err) => state.error = Some(err.to_string().into()),
        }
        match shared_options::load_instance(&dir) {
            Ok(sharing) => state.sharing = sharing,
            Err(err) => state.error = Some(err.to_string().into()),
        }
        if let Ok((file, Some(target))) =
            shared_options::read_instance_options(&dir, &self.game_version, Some(&jar))
        {
            state.local = shared_options::instance_values(&file, target.dv);
            state.dv = Some(target.dv);
        }
        self.state = state;
        cx.notify();
    }

    fn after(&mut self, result: Result<impl Sized, impl ToString>, cx: &mut Context<Self>) {
        if let Err(err) = result {
            self.message = Some((err.to_string().into(), true));
        }
        self.reload(cx);
    }

    fn set_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let dir = self.dir(cx);
        let result = shared_options::update_instance(&dir, |s| s.enabled = enabled);
        self.message = None;
        self.after(result, cx);
    }

    fn make_preferred(&mut self, cx: &mut Context<Self>) {
        let dir = self.dir(cx);
        let data = self.instances.read(cx).data_dir().clone();
        let result = shared_options::set_preferred(&data, Some(&self.instance_id), Some(&dir));
        self.message = Some((
            "In-game changes in this instance now update every instance.".into(),
            false,
        ));
        self.after(result, cx);
    }

    fn set_overrides(&mut self, keys: Vec<String>, overridden: bool, cx: &mut Context<Self>) {
        let dir = self.dir(cx);
        let result =
            shared_options::set_overrides(&dir, keys.iter().map(String::as_str), overridden);
        self.after(result, cx);
    }

    fn keep_changes(&mut self, cx: &mut Context<Self>) {
        let dir = self.dir(cx);
        let result = shared_options::keep_changes_as_overrides(&dir);
        self.after(result, cx);
    }

    fn dismiss_changes(&mut self, cx: &mut Context<Self>) {
        let dir = self.dir(cx);
        let result = shared_options::dismiss_changes(&dir);
        self.after(result, cx);
    }

    fn edit(&mut self, row: &Row, cx: &mut Context<Self>) {
        let text = row.local.clone().unwrap_or_default();
        let input = super::fields::input(cx, text, "Value");
        self.editing = Some((row.key.clone(), input));
        cx.notify();
    }

    fn save_edit(&mut self, cx: &mut Context<Self>) {
        let Some((key, input)) = self.editing.take() else {
            return;
        };
        let text = input.read(cx).text().to_string();
        let dir = self.dir(cx);
        let jar = self.instances.read(cx).client_jar(&self.game_version);
        let result =
            shared_options::set_instance_value(&dir, &self.game_version, Some(&jar), &key, &text);
        if result.is_err() {
            self.editing = Some((key, input));
        }
        self.message = None;
        self.after(result, cx);
    }

    fn category(
        &self,
        category: Category,
        rows: &[&Row],
        busy: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let expanded = self.expanded.contains(&category);
        let shared = self.state.profile.shares(category);
        let overridden = rows.iter().filter(|r| r.overridden).count();
        let with_value = rows.iter().filter(|r| r.shared.is_some()).count();
        let summary = if !shared {
            "Not shared (launcher Settings)".to_string()
        } else {
            let mut parts = vec![format!("{with_value} shared")];
            if overridden > 0 {
                parts.push(format!("{overridden} overridden"));
            }
            parts.join(" · ")
        };
        let keys: Vec<String> = rows.iter().map(|r| r.key.clone()).collect();
        let all_overridden = !rows.is_empty() && overridden == rows.len();
        let mut body = div().flex().flex_col();
        if expanded && shared {
            for (index, row) in rows.iter().enumerate() {
                body = body.child(self.row(category, index, row, busy, cx));
            }
        }
        div()
            .flex()
            .flex_col()
            .rounded(radius::LG)
            .border_1()
            .border_color(colors::FADE_700)
            .bg(colors::GRAY_850)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .child(
                        Button::new(("shared-category", category as usize), category.label())
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Slim)
                            .icon_right(if expanded {
                                Icon::ArrowUp
                            } else {
                                Icon::ArrowDown
                            })
                            .disabled(!shared || rows.is_empty())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if !this.expanded.remove(&category) {
                                    this.expanded.insert(category);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(theme::text_size(text::SMALL))
                            .text_color(colors::GRAY_500)
                            .child(summary),
                    )
                    .when(shared && !rows.is_empty(), |this| {
                        this.child(
                            Button::new(
                                ("shared-category-bulk", category as usize),
                                if all_overridden {
                                    "Share all"
                                } else {
                                    "Override all"
                                },
                            )
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Slim)
                            .disabled(busy)
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.set_overrides(keys.clone(), !all_overridden, cx)
                                },
                            )),
                        )
                    }),
            )
            .child(body)
    }

    fn row(
        &self,
        category: Category,
        index: usize,
        row: &Row,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let id = category as usize * 10_000 + index;
        let key = row.key.clone();
        let overridden = row.overridden;
        let editing = self
            .editing
            .as_ref()
            .filter(|(k, _)| *k == row.key)
            .map(|(_, input)| input.clone());
        let shared_text = row
            .shared
            .as_deref()
            .map_or("Not shared yet".to_string(), |v| row.show(v));
        let local_text = row
            .local
            .as_deref()
            .map_or("Not in options.txt".to_string(), |v| row.show(v));
        let edit_row = row.clone();
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_3()
            .py_1()
            .border_t_1()
            .border_color(colors::FADE_700)
            .child(
                div()
                    .w(px(190.))
                    .flex_none()
                    .font_weight(weights::BOLD.into())
                    .text_color(colors::GRAY_300)
                    .child(row.label.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(match (overridden, editing) {
                        (true, Some(input)) => div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(div().w(px(180.)).child(input))
                            .child(
                                Button::new(("shared-save", id), "Save")
                                    .variant(ButtonVariant::Primary)
                                    .size(ButtonSize::Slim)
                                    .on_click(cx.listener(|this, _, _, cx| this.save_edit(cx))),
                            )
                            .child(
                                Button::new(("shared-cancel", id), "Cancel")
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Slim)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.editing = None;
                                        cx.notify();
                                    })),
                            )
                            .into_any_element(),
                        (true, None) => div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(value_pill(local_text, true))
                            .child(
                                div()
                                    .text_size(theme::text_size(text::SMALL))
                                    .text_color(colors::GRAY_500)
                                    .child(format!("shared: {shared_text}")),
                            )
                            .child(
                                Button::new(("shared-edit", id), "Edit")
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Slim)
                                    .disabled(busy)
                                    .on_click(
                                        cx.listener(move |this, _, _, cx| this.edit(&edit_row, cx)),
                                    ),
                            )
                            .into_any_element(),
                        (false, _) => value_pill(shared_text, false).into_any_element(),
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_size(theme::text_size(text::SMALL))
                            .text_color(colors::GRAY_500)
                            .child("Override"),
                    )
                    .child(
                        Toggle::new(("shared-override", id), overridden)
                            .disabled(busy)
                            .on_change(cx.listener(move |this, value: &bool, _, cx| {
                                this.set_overrides(vec![key.clone()], *value, cx)
                            })),
                    ),
            )
    }
}

/// A value in the shared settings lists.
pub fn value_pill(text_: impl Into<SharedString>, own: bool) -> impl IntoElement {
    div()
        .px_2()
        .rounded(radius::MD)
        .border_1()
        .border_color(if own {
            colors::BLUE_400
        } else {
            colors::TRANSPARENT
        })
        .bg(colors::GRAY_800)
        .text_color(if own {
            colors::BLUE_200
        } else {
            colors::GRAY_300
        })
        .font_weight(weights::BOLD.into())
        .text_size(theme::text_size(text::SMALL))
        .child(text_.into())
}

/// A line of text in a box, like the inherited values of the other sections.
pub fn note_box(text_: impl Into<SharedString>) -> impl IntoElement {
    div()
        .px_3()
        .py_2()
        .rounded(radius::LG)
        .border_1()
        .border_color(colors::FADE_700)
        .bg(colors::GRAY_850)
        .text_color(colors::GRAY_400)
        .child(text_.into())
}

/// The labels of up to three keys, for messages.
fn labels(rows: &[Row], keys: &BTreeMap<String, String>) -> String {
    let mut names: Vec<String> = keys
        .keys()
        .map(|key| {
            rows.iter()
                .find(|r| &r.key == key)
                .map_or_else(|| key.clone(), |r| r.label.clone())
        })
        .collect();
    let more = names.len().saturating_sub(3);
    names.truncate(3);
    let mut text_ = names.join(", ");
    if more > 0 {
        text_.push_str(&format!(" and {more} more"));
    }
    text_
}

impl Render for InstanceSharedSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (busy, preferred_name) = {
            let instances = self.instances.read(cx);
            (
                instances.is_active(&self.instance_id),
                self.state
                    .profile
                    .preferred
                    .as_deref()
                    .and_then(|id| instances.instance_name(id)),
            )
        };
        let enabled = self.state.sharing.enabled;
        let is_preferred = self.state.profile.preferred.as_deref() == Some(&self.instance_id);
        // Only what this version has: a value in its file, or a key it knows.
        let dv = self.state.dv;
        let rows: Vec<Row> = shared_options::rows(
            &self.state.profile,
            Some((&self.state.sharing, &self.state.local)),
        )
        .into_iter()
        .filter(|row| {
            row.local.is_some() || dv.is_none_or(|dv| shared_options::exists_at(&row.key, dv))
        })
        .collect();
        let changed = self.state.sharing.changed.clone();

        let source = if is_preferred {
            "This is the preferred instance: settings you change in-game here update every instance."
                .to_string()
        } else {
            match &preferred_name {
                Some(name) => format!(
                    "Settings changed in-game in {name} reach this instance at its next launch."
                ),
                None => {
                    "No instance is preferred, so shared values change only in the launcher's Settings."
                        .to_string()
                }
            }
        };

        let mut section = section(
            "Shared game settings",
            Some("Mouse, key bindings, video, sound and more from options.txt.".into()),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            Toggle::new("use-shared-settings", enabled)
                                .disabled(busy)
                                .on_change(cx.listener(|this, value: &bool, _, cx| {
                                    this.set_enabled(*value, cx)
                                })),
                        )
                        .child(
                            div()
                                .font_weight(weights::BOLD.into())
                                .text_color(colors::GRAY_400)
                                .child("Use shared settings"),
                        ),
                )
                .child(if is_preferred {
                    div()
                        .px_2()
                        .py(px(2.))
                        .rounded(radius::MD)
                        .bg(colors::GREEN_FADED)
                        .text_color(colors::GREEN_300)
                        .text_size(theme::text_size(text::SMALL))
                        .font_weight(weights::BOLD.into())
                        .child("Preferred")
                        .into_any_element()
                } else {
                    Button::new("make-preferred", "Make preferred")
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Slim)
                        .on_click(cx.listener(|this, _, _, cx| this.make_preferred(cx)))
                        .into_any_element()
                }),
        )
        .child(
            div()
                .text_size(theme::text_size(text::SMALL))
                .text_color(colors::GRAY_500)
                .child(source),
        );

        if !changed.is_empty() && enabled {
            let count = changed.len();
            section = section.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .rounded(radius::LG)
                    .border_1()
                    .border_color(colors::YELLOW)
                    .bg(colors::GRAY_850)
                    .child(
                        div()
                            .text_color(colors::YELLOW_300)
                            .font_weight(weights::BOLD.into())
                            .child(format!(
                                "{count} setting{} you changed in-game ({}) will be replaced by the shared ones next launch.",
                                if count == 1 { "" } else { "s" },
                                labels(&rows, &changed)
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(
                                Button::new("keep-changes", "Keep them as overrides for this instance")
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Slim)
                                    .on_click(cx.listener(|this, _, _, cx| this.keep_changes(cx))),
                            )
                            .child(
                                Button::new("dismiss-changes", "Dismiss")
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Slim)
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.dismiss_changes(cx)),
                                    ),
                            ),
                    ),
            );
        }

        if busy {
            section = section.child(
                div()
                    .text_size(theme::text_size(text::SMALL))
                    .text_color(colors::GRAY_500)
                    .child("Stop the game to change these: it rewrites options.txt when it quits."),
            );
        }

        if enabled {
            let mut list = div().flex().flex_col().gap_2();
            for category in Category::ALL {
                let in_category: Vec<&Row> =
                    rows.iter().filter(|r| r.category == category).collect();
                list = list.child(self.category(category, &in_category, busy, cx));
            }
            section = section.child(list);
        } else {
            section = section.child(note_box(
                "Off: this instance keeps its own options.txt and nothing is read back from it.",
            ));
        }

        let message = self
            .message
            .clone()
            .or(self.state.error.clone().map(|e| (e, true)));
        section.when_some(message, |this, (message, error)| {
            this.child(super::fields::status(
                message,
                if error {
                    colors::RED_200
                } else {
                    colors::GREEN_300
                },
            ))
        })
    }
}
