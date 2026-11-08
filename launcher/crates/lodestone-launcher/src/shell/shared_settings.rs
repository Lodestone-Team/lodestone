//! Settings > Shared game settings: which categories are shared, the instance they sync
//! from, "Copy from instance…", "Reset", and the shared values themselves.

use std::collections::BTreeSet;

use gpui::{
    AnyElement, Context, Entity, IntoElement, Render, SharedString, Subscription, Window, div,
    prelude::*, px,
};
use launcher_core::shared_options::{self, Category, Profile, Row};

use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, Icon, ListRow, TextInput, Toggle,
        settings::{
            SettingRow, Toast, card, link, muted_link, popover, popover_panel, section,
            select_button, value,
        },
    },
    instances::{Instances, fields},
    theme::{self, colors, text, weights},
};

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
    /// The instances and how many are running at the last look, to reload when a game
    /// stops or an instance is added or deleted.
    signature: (Vec<String>, usize),
    _observe: Subscription,
}

fn signature(instances: &Instances) -> (Vec<String>, usize) {
    let ids: Vec<String> = instances.list().iter().map(|i| i.id.clone()).collect();
    let active = ids.iter().filter(|id| instances.is_active(id)).count();
    (ids, active)
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
                if let Some(message) = ok {
                    self.say(message, false, cx);
                }
            }
            Err(err) => self.say(err.to_string(), true, cx),
        }
        cx.notify();
    }

    fn set_category(&mut self, category: Category, shared: bool, cx: &mut Context<Self>) {
        let data = self.instances.read(cx).data_dir().clone();
        let result = shared_options::update_profile(&data, |profile| {
            if shared {
                profile.disabled.remove(&category);
            } else {
                profile.disabled.insert(category);
            }
        });
        self.done(result, None, cx);
    }

    fn sync_from(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        self.picker = None;
        let instances = self.instances.read(cx);
        let data = instances.data_dir().clone();
        let dir = id.as_deref().map(|id| instances.instance_dir(id));
        let message = match id.as_deref().and_then(|id| instances.instance_name(id)) {
            Some(name) => format!("Settings changed in-game in {name} now update every instance."),
            None => "No instance updates the shared settings now; edit them here.".into(),
        };
        let result = shared_options::set_preferred(&data, id.as_deref(), dir.as_deref());
        self.done(result, Some(message), cx);
    }

    fn copy_from(&mut self, id: String, cx: &mut Context<Self>) {
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
                self.say(format!("Copied {count} values from {name}."), false, cx);
                self.reload(cx);
            }
            Err(err) => self.say(err.to_string(), true, cx),
        }
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        self.confirm_reset = false;
        let data = self.instances.read(cx).data_dir().clone();
        let result = shared_options::reset(&data);
        self.done(
            result,
            Some("The shared values are cleared. Instances keep their own options.txt.".into()),
            cx,
        );
    }

    fn save_edit(&mut self, cx: &mut Context<Self>) {
        let Some((key, input)) = self.editing.take() else {
            return;
        };
        let text = input.read(cx).text().to_string();
        let data = self.instances.read(cx).data_dir().clone();
        let result = shared_options::set_value(&data, &key, &text);
        if result.is_err() {
            self.editing = Some((key, input));
        }
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
                ListRow::new("sync-none", "No instance")
                    .detail("edit here only")
                    .selected(self.profile.preferred.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.sync_from(None, cx))),
            );
        }
        for (index, instance) in instances.list().iter().enumerate() {
            let id = instance.id.clone();
            let detail = format!("{} · {}", instance.game_version, instance.loader.label());
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
            .map(|v| row.show(v))
            .unwrap_or_default();
        let line = SettingRow::new(row.label.clone()).indent();
        match editing {
            Some(input) => line
                .child(div().w(px(200.)).child(input))
                .child(
                    Button::new(("shared-value-save", index), "Save")
                        .variant(ButtonVariant::Primary)
                        .size(ButtonSize::Slim)
                        .on_click(cx.listener(|this, _, _, cx| this.save_edit(cx))),
                )
                .child(
                    muted_link(("shared-value-cancel", index), "Cancel").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.editing = None;
                            cx.notify();
                        },
                    )),
                ),
            None => line.child(value(shown, false)).child(
                link(("shared-value-edit", index), "Edit").on_click(cx.listener(
                    move |this, _, _, cx| {
                        let input = fields::input(cx, text_.clone(), "Value");
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
            "Settings you change in-game in this instance become the shared ones. Other instances only receive them."
        } else {
            "Without one, nothing is read back from the games, and shared values change only here."
        };

        let sync = SettingRow::new("Sync from").hint(sync_hint).child(
            div()
                .flex()
                .flex_col()
                .items_end()
                .child(
                    select_button(
                        "sync-from",
                        div()
                            .font_weight(weights::BOLD.into())
                            .child(preferred_name.unwrap_or_else(|| "No instance".into())),
                    )
                    .when(has_instances, |button| {
                        button.on_click(
                            cx.listener(|this, _, _, cx| this.toggle_picker(Picker::SyncFrom, cx)),
                        )
                    }),
                )
                .when(self.picker == Some(Picker::SyncFrom), |this| {
                    this.child(popover(self.picker_list(Picker::SyncFrom, cx)))
                }),
        );
        let copy = SettingRow::new("Start from an instance")
            .hint("Copies an instance's options.txt into the shared values.")
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_end()
                    .child(
                        Button::new("copy-from-instance", "Copy from instance…")
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Slim)
                            .disabled(!has_instances)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_picker(Picker::CopyFrom, cx)
                            })),
                    )
                    .when(self.picker == Some(Picker::CopyFrom), |this| {
                        this.child(popover(self.picker_list(Picker::CopyFrom, cx)))
                    }),
            );
        let reset = SettingRow::new("Clear the shared values")
            .danger()
            .hint("Instances keep their own options.txt.");
        let reset = if self.confirm_reset {
            reset
                .child(
                    Button::new("confirm-reset-shared", "Clear every shared value")
                        .variant(ButtonVariant::Danger)
                        .size(ButtonSize::Slim)
                        .on_click(cx.listener(|this, _, _, cx| this.reset(cx))),
                )
                .child(
                    muted_link("cancel-reset-shared", "Cancel").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.confirm_reset = false;
                            cx.notify();
                        },
                    )),
                )
        } else {
            reset.child(
                Button::new("reset-shared", "Clear…")
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
                SettingRow::new(category.label())
                    .hint(category.description())
                    .child(
                        div()
                            .text_size(theme::text_size(text::SMALL))
                            .text_color(colors::GRAY_500)
                            .child(match count {
                                0 => "No values yet".to_string(),
                                1 => "1 value".to_string(),
                                n => format!("{n} values"),
                            }),
                    )
                    .when(count > 0, |row| {
                        row.child(
                            link(
                                ("shared-values", category as usize),
                                if expanded { "Hide" } else { "Show" },
                            )
                            .on_click(cx.listener(
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
                        Toggle::new(("share-category", category as usize), shared).on_change(
                            cx.listener(move |this, value: &bool, _, cx| {
                                this.set_category(category, *value, cx)
                            }),
                        ),
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
            SettingRow::new("Server list")
                .hint("Coming soon. Resource and shader packs stay per instance.")
                .child(Toggle::new("share-servers", false).disabled(true))
                .into_any_element(),
        );

        div()
            .id("shared-game-settings")
            .flex()
            .flex_col()
            .gap_8()
            .w_full()
            .child(section("Source", None).child(card([
                sync.into_any_element(),
                copy.into_any_element(),
                reset.into_any_element(),
            ])))
            .child(
                section(
                    "What's shared",
                    Some("A category that is off stays separate in each instance.".into()),
                )
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
