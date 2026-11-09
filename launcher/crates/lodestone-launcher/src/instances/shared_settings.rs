//! Shared game settings: merged into an instance's `options.txt` before Java starts
//! and read back after the game exits ([`launcher_core::shared_options`]), and the
//! "Shared game settings" section of an instance's Settings tab.

use crate::components::a11y::Accessible as _;
use crate::theme::px;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use gpui::{
    AnyElement, Context, Entity, IntoElement, Render, SharedString, Subscription, Window, div,
    prelude::*,
};
use launcher_core::{
    game::log4j::Level,
    paths::DataDir,
    shared_options::{
        self, Category, ExitOutcome, InstanceSharing, Profile, Row,
        format::{Control, slider_text},
    },
};

use super::{Instances, shared_options_i18n};
use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, Slider, TextInput, Toggle, segmented,
        settings::{SettingRow, card, link, muted_link, section, tag, value},
    },
    i18n::{
        CommonMsg, InstanceSettingsMsg as M, InstanceSettingsPlural as P, Text, format, t, tr, trn,
    },
    theme::{self, colors, radius, text},
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
            Ok(outcome) => match (&outcome.skipped, outcome.written) {
                (Some(reason), _) => Some((Level::Warn, tr!(M::SharedNotApplied, error = reason))),
                (None, 0) => None,
                (None, written) => {
                    let message = if outcome.created {
                        P::SharedWrittenNewFile
                    } else {
                        P::SharedWritten
                    };
                    Some((Level::Info, trn!(message, written)))
                }
            },
            Err(err) => Some((Level::Warn, tr!(M::SharedNotApplied, error = err))),
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
            Ok(ExitOutcome::Emitted(keys)) => {
                Some((Level::Info, trn!(P::SharedEmitted, keys.len())))
            }
            Ok(ExitOutcome::Changed(keys)) => {
                Some((Level::Info, trn!(P::SharedChangedNote, keys.len())))
            }
            Err(err) => Some((Level::Warn, tr!(M::SharedNotReadBack, error = err))),
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
    /// A message under the section, and whether it is an error.
    message: Option<(Text, bool)>,
    /// Whether the game was running at the last look, to reload when it stops.
    active: bool,
    /// [`Instances::shared_revision`] at the last load.
    shared_revision: u64,
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
            let instances = instances.read(cx);
            let active = instances.is_active(&this.instance_id);
            let revision = instances.shared_revision();
            if active != this.active || revision != this.shared_revision {
                this.active = active;
                this.shared_revision = revision;
                this.reload(cx);
            }
        });
        let active = instances.read(cx).is_active(&instance_id);
        let shared_revision = instances.read(cx).shared_revision();
        let mut this = Self {
            instances,
            instance_id,
            game_version,
            state: State::default(),
            expanded: BTreeSet::new(),
            editing: None,
            message: None,
            active,
            shared_revision,
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

    /// The files this section changes, as they are now, for Undo: the shared
    /// settings, the instance's sharing state and its `options.txt`.
    fn undo_files(&self, cx: &Context<Self>) -> crate::undo::Files {
        let dir = self.dir(cx);
        let data = self.instances.read(cx).data_dir().clone();
        crate::undo::Files::read(&[
            shared_options::profile_path(&data),
            shared_options::instance_path(&dir),
            dir.join("minecraft").join("options.txt"),
        ])
    }

    fn record_undo(&mut self, before: crate::undo::Files, cx: &mut Context<Self>) {
        let id = self.instance_id.clone();
        self.instances.update(cx, |instances, cx| {
            instances.record_shared(before, Some(id), cx)
        });
    }

    fn set_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let before = self.undo_files(cx);
        let dir = self.dir(cx);
        let result = shared_options::update_instance(&dir, |s| s.enabled = enabled);
        self.message = None;
        self.record_undo(before, cx);
        self.after(result, cx);
    }

    /// "Make main": straight away, or after asking when another instance is main.
    fn make_main(&mut self, cx: &mut Context<Self>) {
        let id = self.instance_id.clone();
        self.message = None;
        self.instances
            .update(cx, |instances, cx| instances.request_main(&id, cx));
    }

    fn set_overrides(&mut self, keys: Vec<String>, overridden: bool, cx: &mut Context<Self>) {
        let before = self.undo_files(cx);
        let dir = self.dir(cx);
        let result =
            shared_options::set_overrides(&dir, keys.iter().map(String::as_str), overridden);
        self.record_undo(before, cx);
        self.after(result, cx);
    }

    fn keep_changes(&mut self, cx: &mut Context<Self>) {
        let before = self.undo_files(cx);
        let dir = self.dir(cx);
        let result = shared_options::keep_changes_as_overrides(&dir);
        self.record_undo(before, cx);
        self.after(result, cx);
    }

    fn dismiss_changes(&mut self, cx: &mut Context<Self>) {
        let before = self.undo_files(cx);
        let dir = self.dir(cx);
        let result = shared_options::dismiss_changes(&dir);
        self.record_undo(before, cx);
        self.after(result, cx);
    }

    fn edit(&mut self, row: &Row, cx: &mut Context<Self>) {
        let text = row.local.clone().unwrap_or_default();
        let input = super::fields::labeled(
            cx,
            text,
            M::Value,
            "shared-option-value",
            shared_options_i18n::label(row),
        );
        self.editing = Some((row.key.clone(), input));
        cx.notify();
    }

    /// Saves this instance's value, picked with a switch, choice or slider.
    fn set_now(&mut self, key: &str, text: &str, cx: &mut Context<Self>) {
        let before = self.undo_files(cx);
        let dir = self.dir(cx);
        let jar = self.instances.read(cx).client_jar(&self.game_version);
        let result =
            shared_options::set_instance_value(&dir, &self.game_version, Some(&jar), key, text);
        self.message = None;
        self.record_undo(before, cx);
        self.after(result, cx);
    }

    fn save_edit(&mut self, cx: &mut Context<Self>) {
        let Some((key, input)) = self.editing.take() else {
            return;
        };
        let text = input.read(cx).text().to_string();
        let before = self.undo_files(cx);
        let dir = self.dir(cx);
        let jar = self.instances.read(cx).client_jar(&self.game_version);
        let result =
            shared_options::set_instance_value(&dir, &self.game_version, Some(&jar), &key, &text);
        if result.is_err() {
            self.editing = Some((key, input));
        }
        self.message = None;
        self.record_undo(before, cx);
        self.after(result, cx);
    }

    /// A category's row: its name, how many values are shared or overridden, and
    /// "Override all" or "Share all".
    fn category(
        &self,
        category: Category,
        rows: &[&Row],
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let expanded = self.expanded.contains(&category);
        let shared = self.state.profile.shares(category);
        let overridden = rows.iter().filter(|r| r.overridden).count();
        let with_value = rows.iter().filter(|r| r.shared.is_some()).count();
        let summary = if !shared {
            t(M::NotShared).to_string()
        } else {
            let mut parts = vec![trn!(P::SharedCount, with_value)];
            if overridden > 0 {
                parts.push(trn!(P::OverriddenCount, overridden));
            }
            parts.join(" · ")
        };
        let keys: Vec<String> = rows.iter().map(|r| r.key.clone()).collect();
        let all_overridden = !rows.is_empty() && overridden == rows.len();
        let can_open = shared && !rows.is_empty();
        SettingRow::new(category_label(category))
            .child(
                div()
                    .text_size(theme::text_size(text::SMALL))
                    .text_color(crate::theme::ink::muted())
                    .child(summary),
            )
            .when(can_open && expanded, |row| {
                row.child(
                    muted_link(
                        ("shared-category-bulk", category as usize),
                        t(if all_overridden {
                            M::ShareAll
                        } else {
                            M::OverrideAll
                        }),
                    )
                    .when(!busy, |link| {
                        link.on_press(cx.listener(move |this, _, _, cx| {
                            this.set_overrides(keys.clone(), !all_overridden, cx)
                        }))
                    }),
                )
            })
            .when(can_open, |row| {
                row.child(
                    link(
                        ("shared-category", category as usize),
                        t(if expanded { M::Hide } else { M::Show }),
                    )
                    .on_press(cx.listener(move |this, _, _, cx| {
                        if !this.expanded.remove(&category) {
                            this.expanded.insert(category);
                        }
                        cx.notify();
                    })),
                )
            })
            .into_any_element()
    }

    /// One value of an opened category: the shared value, or this instance's own.
    fn row(
        &self,
        category: Category,
        index: usize,
        row: &Row,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = category as usize * 10_000 + index;
        let key = row.key.clone();
        let editing = self
            .editing
            .as_ref()
            .filter(|(k, _)| *k == row.key)
            .map(|(_, input)| input.clone());
        let shared_text = row.shared.as_deref().map_or_else(
            || t(M::NotSharedYet).to_string(),
            |v| shared_options_i18n::show(row, v),
        );
        let local_text = row.local.as_deref().map_or_else(
            || t(M::NotInOptions).to_string(),
            |v| shared_options_i18n::show(row, v),
        );
        let edit_row = row.clone();
        let line = SettingRow::new(shared_options_i18n::label(row))
            .key(SharedString::from(format!("shared-option-{}", row.key)))
            .indent();
        let line =
            match (row.overridden, editing) {
                (true, Some(input)) => line
                    .child(div().w(px(180.)).child(input))
                    .child(
                        Button::new(("shared-save", id), t(CommonMsg::Save))
                            .variant(ButtonVariant::Primary)
                            .size(ButtonSize::Slim)
                            .on_click(cx.listener(|this, _, _, cx| this.save_edit(cx))),
                    )
                    .child(
                        muted_link(("shared-cancel", id), t(CommonMsg::Cancel)).on_press(
                            cx.listener(|this, _, _, cx| {
                                this.editing = None;
                                cx.notify();
                            }),
                        ),
                    ),
                (true, None) => {
                    let control = (!busy)
                        .then(|| {
                            let key = row.key.clone();
                            value_control(
                                ("shared-local", id),
                                row,
                                row.local.as_deref(),
                                cx.listener(move |this, text: &String, _, cx| {
                                    this.set_now(&key, text, cx)
                                }),
                            )
                        })
                        .flatten();
                    let typed = control.is_none();
                    line.map(|line| match control {
                        Some(control) => line.child(control),
                        None => line.child(value(local_text, false)),
                    })
                    .child(tag(t(M::ThisInstance), crate::theme::accent::light()))
                    .when(!busy, |line| {
                        line.when(typed, |line| {
                            line.child(link(("shared-edit", id), t(CommonMsg::Edit)).on_press(
                                cx.listener(move |this, _, _, cx| this.edit(&edit_row, cx)),
                            ))
                        })
                        .child(
                            muted_link(("shared-use", id), tr!(M::UseShared, value = shared_text))
                                .on_press(cx.listener(move |this, _, _, cx| {
                                    this.set_overrides(vec![key.clone()], false, cx)
                                })),
                        )
                    })
                }
                (false, _) => line
                    .child(value(shared_text, true))
                    .child(tag(t(M::Shared), colors::GRAY_500))
                    .when(!busy, |line| {
                        line.child(link(("shared-override", id), t(M::Override)).on_press(
                            cx.listener(move |this, _, _, cx| {
                                this.set_overrides(vec![key.clone()], true, cx)
                            }),
                        ))
                    }),
            };
        line.into_any_element()
    }
}

/// A category's name, in the current language.
pub fn category_label(category: Category) -> &'static str {
    t(match category {
        Category::Controls => M::CategoryControls,
        Category::Video => M::CategoryVideo,
        Category::Sound => M::CategorySound,
        Category::Accessibility => M::CategoryAccessibility,
        Category::Language => M::CategoryLanguage,
    })
}

/// What a category holds, in the current language.
pub fn category_description(category: Category) -> &'static str {
    t(match category {
        Category::Controls => M::CategoryControlsDescription,
        Category::Video => M::CategoryVideoDescription,
        Category::Sound => M::CategorySoundDescription,
        Category::Accessibility => M::CategoryAccessibilityDescription,
        Category::Language => M::CategoryLanguageDescription,
    })
}

/// The labels of up to three keys, for messages.
fn labels(rows: &[Row], keys: &BTreeMap<String, String>) -> String {
    let mut names: Vec<String> = keys
        .keys()
        .map(|key| {
            rows.iter()
                .find(|r| &r.key == key)
                .map_or_else(|| key.clone(), shared_options_i18n::label)
        })
        .collect();
    let more = names.len().saturating_sub(3);
    names.truncate(3);
    let list = format::list(&names);
    if more > 0 {
        trn!(P::AndMore, more, names = list)
    } else {
        list
    }
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

        let source = if !enabled {
            t(M::SharingOff).to_string()
        } else if is_preferred {
            t(M::SharingMain).to_string()
        } else {
            match &preferred_name {
                Some(name) => tr!(M::SharingFrom, name = name),
                None => t(M::SharingNoMain).to_string(),
            }
        };

        let mut items: Vec<AnyElement> = vec![
            SettingRow::new(t(M::UseSharedSettings))
                .hint(source)
                .child(
                    Toggle::new("use-shared-settings", enabled)
                        .label(t(M::UseSharedSettings))
                        .disabled(busy)
                        .on_change(
                            cx.listener(|this, value: &bool, _, cx| this.set_enabled(*value, cx)),
                        ),
                )
                .when(busy, |row| {
                    row.note(t(M::StopGameToChange), colors::GRAY_400)
                })
                .into_any_element(),
        ];
        if enabled {
            // The main instance: its in-game changes are copied to the others.
            items.push(
                SettingRow::new(t(M::MainInstance))
                    .hint(t(M::MainInstanceHint))
                    .when(is_preferred, |row| {
                        row.child(tag(t(M::IsMainInstance), colors::GREEN_300))
                    })
                    .when(!is_preferred, |row| {
                        row.child(
                            Button::new("make-main", t(M::MakeMain))
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Slim)
                                .on_click(cx.listener(|this, _, _, cx| this.make_main(cx))),
                        )
                    })
                    .into_any_element(),
            );
            for category in Category::ALL {
                let in_category: Vec<&Row> =
                    rows.iter().filter(|r| r.category == category).collect();
                items.push(self.category(category, &in_category, busy, cx));
                if self.expanded.contains(&category) && self.state.profile.shares(category) {
                    for (index, row) in in_category.iter().enumerate() {
                        items.push(self.row(category, index, row, busy, cx));
                    }
                }
            }
        }

        let mut body = section(t(M::SharedGameSettings), Some(t(M::SharedLead).into()));
        if !changed.is_empty() && enabled {
            let count = changed.len();
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .px_4()
                    .py_3()
                    .rounded(radius::XL)
                    .border_1()
                    .border_color(colors::YELLOW.opacity(30))
                    .bg(colors::YELLOW.opacity(5))
                    .child(div().text_color(crate::theme::ink::text()).child(trn!(
                        P::ChangedInGame,
                        count,
                        names = labels(&rows, &changed)
                    )))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .child(
                                Button::new("keep-changes", t(M::KeepForInstance))
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Slim)
                                    .on_click(cx.listener(|this, _, _, cx| this.keep_changes(cx))),
                            )
                            .child(
                                muted_link("dismiss-changes", t(M::Dismiss)).on_press(
                                    cx.listener(|this, _, _, cx| this.dismiss_changes(cx)),
                                ),
                            ),
                    ),
            );
        }
        let message = self
            .message
            .as_ref()
            .map(|(message, error)| (message.get(), *error))
            .or(self.state.error.clone().map(|e| (e, true)));
        body.child(card(items))
            .when_some(message, |this, (message, error)| {
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

/// A switch, choice or slider for `row`'s `value`; `None` for a typed setting. Each
/// calls `on_set` with the new canonical value.
pub(crate) fn value_control(
    id: (&'static str, usize),
    row: &Row,
    value: Option<&str>,
    on_set: impl Fn(&String, &mut Window, &mut gpui::App) + 'static,
) -> Option<AnyElement> {
    let (name, index) = id;
    let on_set = std::rc::Rc::new(on_set);
    let label = shared_options_i18n::label(row);
    Some(match row.control() {
        Control::Toggle => {
            let on = value == Some("true");
            Toggle::new((name, index), on)
                .label(label)
                .on_change(move |on: &bool, window, cx| {
                    on_set(&if *on { "true" } else { "false" }.to_string(), window, cx)
                })
                .into_any_element()
        }
        Control::Choice(choices) => {
            let selected = value.map(|v| v.trim().to_string());
            segmented(
                format!("{name}-{index}"),
                choices
                    .iter()
                    .map(|(value, _)| {
                        (
                            value.to_string(),
                            SharedString::from(shared_options_i18n::show(row, value)),
                        )
                    })
                    .collect(),
                selected.as_ref(),
                false,
                move |value: String, window, cx| on_set(&value, window, cx),
            )
            .into_any_element()
        }
        Control::Slider { min, max, step } => {
            let current = value
                .and_then(|v| v.trim().parse::<f64>().ok())
                .unwrap_or(min);
            let shown = row.clone();
            Slider::new((name, index), current, min, max, step)
                .label(label)
                .format(move |n| {
                    shared_options_i18n::show(&shown, &slider_text(min, max, step, n)).into()
                })
                .on_change(move |n, window, cx| on_set(&slider_text(min, max, step, n), window, cx))
                .into_any_element()
        }
        Control::Text => return None,
    })
}
