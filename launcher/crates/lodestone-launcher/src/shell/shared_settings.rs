//! Settings > Shared game settings: which categories are shared, the instance they sync
//! from, "Copy from instance…", "Reset", and the shared values themselves.

use crate::theme::px;
use std::{collections::BTreeSet, path::PathBuf};

use gpui::{
    AnyElement, Context, Entity, IntoElement, Render, SharedString, Subscription, Window, div,
    prelude::*,
};
use launcher_core::shared_options::{self, Category, Profile, Row};

use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, Icon, ListRow, TextInput, Toggle,
        a11y::Accessible,
        settings::{
            SettingRow, Toast, card, link, muted_link, popover, popover_panel, section,
            select_button, value,
        },
    },
    i18n::{CommonMsg, ShellMsg as M, ShellPlural as P, t, tr, trn},
    instances::{Instances, shared_options_i18n, shared_settings::value_control},
    theme::{self, colors, text, weights},
};

/// A category's name, in the current language.
fn category_label(category: Category) -> &'static str {
    t(match category {
        Category::Controls => M::CategoryControls,
        Category::Video => M::CategoryVideo,
        Category::Sound => M::CategorySound,
        Category::Accessibility => M::CategoryAccessibility,
        Category::Language => M::CategoryLanguage,
    })
}

/// What a category holds, in the current language.
fn category_description(category: Category) -> &'static str {
    t(match category {
        Category::Controls => M::CategoryControlsHint,
        Category::Video => M::CategoryVideoHint,
        Category::Sound => M::CategorySoundHint,
        Category::Accessibility => M::CategoryAccessibilityHint,
        Category::Language => M::CategoryLanguageHint,
    })
}

/// Which instance list is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Picker {
    SyncFrom,
    CopyFrom,
}

pub struct SharedSettings {
    instances: Entity<Instances>,
    profile: Profile,
    picker: Option<Picker>,
    /// The value lists that are open.
    expanded: BTreeSet<Category>,
    editing: Option<(String, Entity<TextInput>)>,
    confirm_reset: bool,
    toast: Toast,
    /// The instances and how many are running at the last look, and
    /// [`Instances::shared_revision`], to reload when a game stops, an instance is
    /// added or deleted, or Undo puts the shared settings back.
    signature: (Vec<String>, usize, u64),
    _observe: Subscription,
}

fn signature(instances: &Instances) -> (Vec<String>, usize, u64) {
    let ids: Vec<String> = instances.list().iter().map(|i| i.id.clone()).collect();
    let active = ids.iter().filter(|id| instances.is_active(id)).count();
    (ids, active, instances.shared_revision())
}

impl SharedSettings {
    pub fn new(instances: Entity<Instances>, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&instances, |this: &mut Self, instances, cx| {
            let now = signature(instances.read(cx));
            if now != this.signature {
                this.signature = now;
                this.reload(cx);
            }
        });
        let mut this = Self {
            signature: signature(instances.read(cx)),
            instances,
            profile: Profile::default(),
            picker: None,
            expanded: BTreeSet::new(),
            editing: None,
            confirm_reset: false,
            toast: Toast::default(),
            _observe: observe,
        };
        this.reload(cx);
        this
    }

    /// Opens value lists, for the offscreen screenshots.
    pub fn preview_expand(&mut self, categories: &[Category], cx: &mut Context<Self>) {
        self.expanded.extend(categories.iter().copied());
        cx.notify();
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let data = self.instances.read(cx).data_dir().clone();
        match shared_options::load_profile(&data) {
            Ok(profile) => self.profile = profile,
            Err(err) => self.say(err.to_string(), true, cx),
        }
        cx.notify();
    }

    /// The shared settings as they are now (and `instance`'s sharing state), for Undo.
    fn undo_files(&self, instance: Option<PathBuf>, cx: &Context<Self>) -> crate::undo::Files {
        let data = self.instances.read(cx).data_dir().clone();
        let mut paths = vec![shared_options::profile_path(&data)];
        paths.extend(instance.map(|dir| shared_options::instance_path(&dir)));
        crate::undo::Files::read(&paths)
    }

    fn record_undo(&mut self, before: crate::undo::Files, cx: &mut Context<Self>) {
        self.instances.update(cx, |instances, cx| {
            instances.record_shared(before, None, cx)
        });
    }

    fn say(&mut self, message: impl Into<SharedString>, error: bool, cx: &mut Context<Self>) {
        self.toast
            .show(message, error, |this: &mut Self| &mut this.toast, cx);
    }

    fn done(
        &mut self,
        result: Result<Profile, shared_options::SharedError>,
        ok: Option<String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(profile) => {
                self.profile = profile;
                self.instances
                    .update(cx, |instances, cx| instances.shared_settings_changed(cx));
                if let Some(message) = ok {
                    self.say(message, false, cx);
                }
            }
            Err(err) => self.say(err.to_string(), true, cx),
        }
        cx.notify();
    }

    fn set_category(&mut self, category: Category, shared: bool, cx: &mut Context<Self>) {
        let before = self.undo_files(None, cx);
        let data = self.instances.read(cx).data_dir().clone();
        let result = shared_options::update_profile(&data, |profile| {
            if shared {
                profile.disabled.remove(&category);
            } else {
                profile.disabled.insert(category);
            }
        });
        self.record_undo(before, cx);
        self.done(result, None, cx);
    }

    /// Picks the main instance. Another instance taking over from the current main
    /// asks first (in the shell), so it isn't changed here.
    fn sync_from(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        self.picker = None;
        if let Some(id) = &id {
            let name = self
                .instances
                .read(cx)
                .instance_name(id)
                .unwrap_or_default();
            let done = self
                .instances
                .update(cx, |instances, cx| instances.request_main(id, cx));
            if done {
                self.say(tr!(M::SyncedFrom, name = name), false, cx);
            }
            self.reload(cx);
            return;
        }
        let instances = self.instances.read(cx);
        let data = instances.data_dir().clone();
        let dir = id.as_deref().map(|id| instances.instance_dir(id));
        let message = match id.as_deref().and_then(|id| instances.instance_name(id)) {
            Some(name) => tr!(M::SyncedFrom, name = name),
            None => t(M::SyncedFromNone).into(),
        };
        let before = self.undo_files(dir.clone(), cx);
        let result = shared_options::set_preferred(&data, id.as_deref(), dir.as_deref());
        self.record_undo(before, cx);
        self.done(result, Some(message), cx);
    }

    fn copy_from(&mut self, id: String, cx: &mut Context<Self>) {
        let before = self.undo_files(None, cx);
        self.picker = None;
        let instances = self.instances.read(cx);
        let data = instances.data_dir().clone();
        let dir = instances.instance_dir(&id);
        let name = instances.instance_name(&id).unwrap_or_default();
        let version = instances
            .list()
            .iter()
            .find(|i| i.id == id)
            .map(|i| i.game_version.clone())
            .unwrap_or_default();
        let jar = instances.client_jar(&version);
        match shared_options::copy_from_instance(&data, &dir, &version, Some(&jar)) {
            Ok(count) => {
                self.record_undo(before, cx);
                self.say(trn!(P::CopiedValues, count, name = name), false, cx);
                self.reload(cx);
            }
            Err(err) => self.say(err.to_string(), true, cx),
        }
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        let before = self.undo_files(None, cx);
        self.confirm_reset = false;
        let data = self.instances.read(cx).data_dir().clone();
        let result = shared_options::reset(&data);
        self.record_undo(before, cx);
        self.done(result, Some(t(M::SharedCleared).into()), cx);
    }

    fn save_edit(&mut self, cx: &mut Context<Self>) {
        let Some((key, input)) = self.editing.take() else {
            return;
        };
        let text = input.read(cx).text().to_string();
        let data = self.instances.read(cx).data_dir().clone();
        let before = self.undo_files(None, cx);
        let result = shared_options::set_value(&data, &key, &text);
        if result.is_err() {
            self.editing = Some((key, input));
        }
        self.record_undo(before, cx);
        self.done(result, None, cx);
    }

    fn set_narrator_off(&mut self, off: bool, cx: &mut Context<Self>) {
        let before = self.undo_files(None, cx);
        let data = self.instances.read(cx).data_dir().clone();
        let result = shared_options::set_narrator_off(&data, off);
        let message = if off {
            t(M::NarratorKeptOff)
        } else {
            t(M::NarratorPerInstance)
        };
        self.record_undo(before, cx);
        self.done(result, Some(message.into()), cx);
    }

    /// Saves a value picked with a switch, choice or slider straight away.
    fn set_now(&mut self, key: &str, text: &str, cx: &mut Context<Self>) {
        let before = self.undo_files(None, cx);
        let data = self.instances.read(cx).data_dir().clone();
        let result = shared_options::set_value(&data, key, text);
        self.record_undo(before, cx);
        self.done(result, None, cx);
    }

    fn picker_list(&self, picker: Picker, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let instances = self.instances.read(cx);
        let mut list = popover_panel(match picker {
            Picker::SyncFrom => "sync-from-list",
            Picker::CopyFrom => "copy-from-list",
        })
        .w(px(320.))
        .max_h(px(320.))
        .overflow_y_scroll()
        .on_mouse_down_out(cx.listener(|this, _, _, cx| {
            this.picker = None;
            cx.notify();
        }));
        if picker == Picker::SyncFrom {
            list = list.child(
                ListRow::new("sync-none", t(M::NoInstance))
                    .detail(t(M::EditHereOnly))
                    .selected(self.profile.preferred.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.sync_from(None, cx))),
            );
        }
        for (index, instance) in instances.list().iter().enumerate() {
            let id = instance.id.clone();
            let detail = format!(
                "{} · {}",
                instance.game_version,
                super::loader_name(instance.loader)
            );
            let row = ListRow::new(("shared-pick", index), instance.name.clone())
                .icon(Icon::Cube)
                .detail(detail);
            list = list.child(match picker {
                Picker::SyncFrom => row
                    .selected(self.profile.preferred.as_deref() == Some(id.as_str()))
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.sync_from(Some(id.clone()), cx)),
                    ),
                Picker::CopyFrom => {
                    row.on_click(cx.listener(move |this, _, _, cx| this.copy_from(id.clone(), cx)))
                }
            });
        }
        list
    }

    fn value_row(&self, index: usize, row: &Row, cx: &mut Context<Self>) -> AnyElement {
        let editing = self
            .editing
            .as_ref()
            .filter(|(key, _)| *key == row.key)
            .map(|(_, input)| input.clone());
        let key = row.key.clone();
        let text_ = row.shared.clone().unwrap_or_default();
        let shown = row
            .shared
            .as_deref()
            .map(|v| shared_options_i18n::show(row, v))
            .unwrap_or_default();
        let label = shared_options_i18n::label(row);
        let line = SettingRow::new(label.clone())
            .key(SharedString::from(format!("shared-option-{key}")))
            .indent();
        if editing.is_none()
            && let Some(control) = value_control(
                ("shared-value", index),
                row,
                row.shared.as_deref(),
                cx.listener({
                    let key = key.clone();
                    move |this, text: &String, _, cx| this.set_now(&key, text, cx)
                }),
            )
        {
            return line.child(control).into_any_element();
        }
        match editing {
            Some(input) => line
                .child(div().w(px(200.)).child(input))
                .child(
                    Button::new(("shared-value-save", index), t(CommonMsg::Save))
                        .variant(ButtonVariant::Primary)
                        .size(ButtonSize::Slim)
                        .on_click(cx.listener(|this, _, _, cx| this.save_edit(cx))),
                )
                .child(
                    muted_link(("shared-value-cancel", index), t(CommonMsg::Cancel)).on_press(
                        cx.listener(|this, _, _, cx| {
                            this.editing = None;
                            cx.notify();
                        }),
                    ),
                ),
            None => line.child(value(shown, false)).child(
                link(("shared-value-edit", index), t(CommonMsg::Edit)).on_press(cx.listener(
                    move |this, _, _, cx| {
                        let text = text_.clone();
                        let (label, automation_id) = (
                            label.clone(),
                            format!("shared-value-input-{}", key.as_str()),
                        );
                        let input = cx.new(|cx| {
                            TextInput::new(cx)
                                .with_placeholder(M::Value)
                                .with_label(label)
                                .with_automation_id(automation_id)
                                .with_text(text)
                        });
                        this.editing = Some((key.clone(), input));
                        cx.notify();
                    },
                )),
            ),
        }
        .into_any_element()
    }
}

impl SharedSettings {
    /// Drawn by the Settings dialog, at the bottom of its page.
    pub fn toast(&self) -> &Toast {
        &self.toast
    }
}

impl Render for SharedSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = shared_options::rows(&self.profile, None);
        let preferred_name = self
            .profile
            .preferred
            .as_deref()
            .and_then(|id| self.instances.read(cx).instance_name(id));
        let has_instances = !self.instances.read(cx).list().is_empty();
        let sync_hint = if preferred_name.is_some() {
            t(M::SyncFromHint)
        } else {
            t(M::SyncFromNoneHint)
        };

        let sync = SettingRow::new(t(M::SyncFrom)).hint(sync_hint).child(
            div()
                .flex()
                .flex_col()
                .items_end()
                .child(
                    select_button(
                        "sync-from",
                        div().font_weight(weights::BOLD.into()).child(
                            preferred_name
                                .clone()
                                .unwrap_or_else(|| t(M::NoInstance).into()),
                        ),
                    )
                    .aria_label(t(M::SyncFrom))
                    .aria_value(preferred_name.unwrap_or_else(|| t(M::NoInstance).into()))
                    .when(has_instances, |button| {
                        let shell = cx.entity().downgrade();
                        button.expandable(self.picker == Some(Picker::SyncFrom), move |_, cx| {
                            shell
                                .update(cx, |this, cx| this.toggle_picker(Picker::SyncFrom, cx))
                                .ok();
                        })
                    })
                    .when(has_instances, |button| {
                        button.on_press(
                            cx.listener(|this, _, _, cx| this.toggle_picker(Picker::SyncFrom, cx)),
                        )
                    }),
                )
                .when(self.picker == Some(Picker::SyncFrom), |this| {
                    let close = cx.listener(|this, _: &(), _, cx| {
                        this.picker = None;
                        cx.notify();
                    });
                    this.child(popover(
                        "syncfrom-popover",
                        self.picker_list(Picker::SyncFrom, cx),
                        move |window, cx| close(&(), window, cx),
                    ))
                }),
        );
        let copy = SettingRow::new(t(M::StartFromInstance)).child(
            div()
                .flex()
                .flex_col()
                .items_end()
                .child(
                    Button::new("copy-from-instance", t(M::CopyFromInstance))
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Slim)
                        .disabled(!has_instances)
                        .on_click(
                            cx.listener(|this, _, _, cx| this.toggle_picker(Picker::CopyFrom, cx)),
                        ),
                )
                .when(self.picker == Some(Picker::CopyFrom), |this| {
                    let close = cx.listener(|this, _: &(), _, cx| {
                        this.picker = None;
                        cx.notify();
                    });
                    this.child(popover(
                        "copyfrom-popover",
                        self.picker_list(Picker::CopyFrom, cx),
                        move |window, cx| close(&(), window, cx),
                    ))
                }),
        );
        let reset = SettingRow::new(t(M::ClearShared))
            .danger()
            .hint(t(M::ClearSharedHint));
        let reset = if self.confirm_reset {
            reset
                .child(
                    Button::new("confirm-reset-shared", t(M::ClearEveryShared))
                        .variant(ButtonVariant::Danger)
                        .size(ButtonSize::Slim)
                        .on_click(cx.listener(|this, _, _, cx| this.reset(cx))),
                )
                .child(
                    muted_link("cancel-reset-shared", t(CommonMsg::Cancel)).on_press(cx.listener(
                        |this, _, _, cx| {
                            this.confirm_reset = false;
                            cx.notify();
                        },
                    )),
                )
        } else {
            reset.child(
                Button::new("reset-shared", t(M::ClearEllipsis))
                    .variant(ButtonVariant::Danger)
                    .size(ButtonSize::Slim)
                    .disabled(rows.is_empty())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.confirm_reset = true;
                        cx.notify();
                    })),
            )
        };

        let mut categories: Vec<AnyElement> = Vec::new();
        let mut index = 0;
        for category in Category::ALL {
            let in_category: Vec<&Row> = rows.iter().filter(|r| r.category == category).collect();
            let count = in_category.len();
            let shared = self.profile.shares(category);
            let expanded = self.expanded.contains(&category) && count > 0;
            categories.push(
                SettingRow::new(category_label(category))
                    .hint(category_description(category))
                    .child(
                        div()
                            .text_size(theme::text_size(text::SMALL))
                            .text_color(crate::theme::ink::muted())
                            .child(match count {
                                0 => t(M::NoValuesYet).to_string(),
                                n => trn!(P::Values, n),
                            }),
                    )
                    .when(count > 0, |row| {
                        row.child(
                            link(
                                ("shared-values", category as usize),
                                if expanded { t(M::Hide) } else { t(M::Show) },
                            )
                            .expandable(expanded, {
                                let shell = cx.entity().downgrade();
                                move |_, cx| {
                                    shell
                                        .update(cx, |this, cx| {
                                            if !this.expanded.remove(&category) {
                                                this.expanded.insert(category);
                                            }
                                            cx.notify();
                                        })
                                        .ok();
                                }
                            })
                            .on_press(cx.listener(
                                move |this, _, _, cx| {
                                    if !this.expanded.remove(&category) {
                                        this.expanded.insert(category);
                                    }
                                    cx.notify();
                                },
                            )),
                        )
                    })
                    .child(
                        Toggle::new(("share-category", category as usize), shared)
                            .label(category_label(category))
                            .on_change(cx.listener(move |this, value: &bool, _, cx| {
                                this.set_category(category, *value, cx)
                            })),
                    )
                    .into_any_element(),
            );
            if expanded {
                for row in in_category {
                    categories.push(self.value_row(index, row, cx));
                    index += 1;
                }
            }
        }
        // TODO(M5b): share servers.dat once the NBT reader lands.
        categories.push(
            SettingRow::new(t(M::ServerList))
                .hint(t(M::ServerListHint))
                .child(
                    Toggle::new("share-servers", false)
                        .label(t(M::ServerList))
                        .disabled(true),
                )
                .into_any_element(),
        );

        let accessibility_shared = self.profile.shares(Category::Accessibility);
        let narrator = SettingRow::new(t(M::KeepNarratorOff))
            .hint(t(M::KeepNarratorOffHint))
            .note_opt((!accessibility_shared).then(|| {
                (
                    SharedString::from(tr!(
                        M::NarratorNotShared,
                        category = t(M::CategoryAccessibility)
                    )),
                    colors::YELLOW,
                )
            }))
            .child(
                Toggle::new("narrator-off", shared_options::narrator_off(&self.profile))
                    .label(t(M::KeepNarratorOff))
                    .disabled(!accessibility_shared)
                    .on_change(
                        cx.listener(|this, off: &bool, _, cx| this.set_narrator_off(*off, cx)),
                    ),
            );

        div()
            .id("shared-game-settings")
            .flex()
            .flex_col()
            .gap_8()
            .w_full()
            .child(section(t(M::Source), None).child(card([
                sync.into_any_element(),
                copy.into_any_element(),
                reset.into_any_element(),
            ])))
            .child(section(t(M::NarratorSection), None).child(card([narrator.into_any_element()])))
            .child(
                section(t(M::WhatsShared), Some(t(M::WhatsSharedLead).into()))
                    .child(card(categories)),
            )
    }
}

impl SharedSettings {
    fn toggle_picker(&mut self, picker: Picker, cx: &mut Context<Self>) {
        self.picker = (self.picker != Some(picker)).then_some(picker);
        cx.notify();
    }
}
