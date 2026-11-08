//! Settings > Shared game settings: which categories are shared, the instance they sync
//! from, "Copy from instance…", "Reset", and the shared values themselves.

use std::collections::BTreeSet;

use gpui::{
    Context, Entity, IntoElement, Render, SharedString, Subscription, Window, div, prelude::*, px,
};
use launcher_core::shared_options::{self, Category, Profile, Row};

use crate::{
    components::{Button, ButtonSize, ButtonVariant, Icon, ListRow, TextInput, Toggle},
    instances::{
        Instances,
        fields::{self, section},
        shared_settings::{note_box, value_pill},
    },
    theme::{self, colors, radius, text, weights},
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
    message: Option<(SharedString, bool)>,
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
            message: None,
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
            Err(err) => self.message = Some((err.to_string().into(), true)),
        }
        cx.notify();
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
                self.message = ok.map(|m| (m.into(), false));
            }
            Err(err) => self.message = Some((err.to_string().into(), true)),
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
                self.message = Some((format!("Copied {count} values from {name}.").into(), false));
                self.reload(cx);
            }
            Err(err) => {
                self.message = Some((err.to_string().into(), true));
                cx.notify();
            }
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
        let mut list = div()
            .flex()
            .flex_col()
            .p_1()
            .max_w(px(360.))
            .rounded(radius::LG)
            .border_1()
            .border_color(colors::FADE_700)
            .bg(colors::GRAY_800);
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

    fn value_row(
        &self,
        index: usize,
        row: &Row,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
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
                    .w(px(220.))
                    .flex_none()
                    .font_weight(weights::BOLD.into())
                    .text_color(colors::GRAY_300)
                    .child(row.label.clone()),
            )
            .child(match editing {
                Some(input) => div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(200.)).child(input))
                    .child(
                        Button::new(("shared-value-save", index), "Save")
                            .variant(ButtonVariant::Primary)
                            .size(ButtonSize::Slim)
                            .on_click(cx.listener(|this, _, _, cx| this.save_edit(cx))),
                    )
                    .child(
                        Button::new(("shared-value-cancel", index), "Cancel")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Slim)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.editing = None;
                                cx.notify();
                            })),
                    )
                    .into_any_element(),
                None => div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .items_center()
                    .gap_2()
                    .child(value_pill(shown, false))
                    .child(div().flex_1())
                    .child(
                        Button::new(("shared-value-edit", index), "Edit")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Slim)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let input = fields::input(cx, text_.clone(), "Value");
                                this.editing = Some((key.clone(), input));
                                cx.notify();
                            })),
                    )
                    .into_any_element(),
            })
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

        let mut categories = div().flex().flex_col().gap_2();
        for category in Category::ALL {
            let count = rows.iter().filter(|r| r.category == category).count();
            let shared = self.profile.shares(category);
            categories = categories.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(
                        Toggle::new(("share-category", category as usize), shared).on_change(
                            cx.listener(move |this, value: &bool, _, cx| {
                                this.set_category(category, *value, cx)
                            }),
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .font_weight(weights::BOLD.into())
                                    .text_color(colors::GRAY_300)
                                    .child(category.label()),
                            )
                            .child(
                                div()
                                    .text_size(theme::text_size(text::SMALL))
                                    .text_color(colors::GRAY_500)
                                    .child(category.description()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(theme::text_size(text::SMALL))
                            .text_color(colors::GRAY_500)
                            .child(format!(
                                "{count} value{}",
                                if count == 1 { "" } else { "s" }
                            )),
                    ),
            );
        }
        // TODO(M5b): share servers.dat once the NBT reader lands.
        categories = categories.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .child(Toggle::new("share-servers", false).disabled(true))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .child(
                            div()
                                .font_weight(weights::BOLD.into())
                                .text_color(colors::GRAY_500)
                                .child("Server list"),
                        )
                        .child(
                            div()
                                .text_size(theme::text_size(text::SMALL))
                                .text_color(colors::GRAY_500)
                                .child("Coming soon. Resource and shader packs stay per instance."),
                        ),
                ),
        );

        let sync_label = match &preferred_name {
            Some(name) => format!("Sync from: {name}"),
            None => "Sync from: no instance".to_string(),
        };
        let sync_hint = if preferred_name.is_some() {
            "Settings you change in-game in this instance update the shared ones. Other instances only receive them."
        } else {
            "Nothing is read back from the games: shared values change only here."
        };

        let left = div()
            .flex()
            .flex_col()
            .gap_4()
            .flex_1()
            .min_w_0()
            .child(categories);

        let mut right = div().flex().flex_col().gap_3().flex_1().min_w_0().child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(fields::caption_text("SYNC FROM"))
                .child(
                    div().flex().flex_row().child(
                        Button::new("sync-from", sync_label)
                            .variant(ButtonVariant::Secondary)
                            .icon_right(Icon::CaretDown)
                            .disabled(!has_instances)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.picker = match this.picker {
                                    Some(Picker::SyncFrom) => None,
                                    _ => Some(Picker::SyncFrom),
                                };
                                cx.notify();
                            })),
                    ),
                )
                .child(
                    div()
                        .text_size(theme::text_size(text::SMALL))
                        .text_color(colors::GRAY_500)
                        .child(sync_hint),
                ),
        );
        if self.picker == Some(Picker::SyncFrom) {
            right = right.child(self.picker_list(Picker::SyncFrom, cx));
        }
        right = right.child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(
                    Button::new("copy-from-instance", "Copy from instance…")
                        .variant(ButtonVariant::Secondary)
                        .disabled(!has_instances)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.picker = match this.picker {
                                Some(Picker::CopyFrom) => None,
                                _ => Some(Picker::CopyFrom),
                            };
                            cx.notify();
                        })),
                )
                .child(if self.confirm_reset {
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("confirm-reset-shared", "Clear every shared value")
                                .variant(ButtonVariant::Danger)
                                .on_click(cx.listener(|this, _, _, cx| this.reset(cx))),
                        )
                        .child(
                            Button::new("cancel-reset-shared", "Cancel")
                                .variant(ButtonVariant::Ghost)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.confirm_reset = false;
                                    cx.notify();
                                })),
                        )
                        .into_any_element()
                } else {
                    Button::new("reset-shared", "Reset")
                        .variant(ButtonVariant::Danger)
                        .disabled(rows.is_empty())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.confirm_reset = true;
                            cx.notify();
                        }))
                        .into_any_element()
                }),
        );
        if self.picker == Some(Picker::CopyFrom) {
            right = right.child(self.picker_list(Picker::CopyFrom, cx));
        }

        let mut values = div().flex().flex_col().gap_2();
        if rows.is_empty() {
            values = values.child(note_box(
                "No shared values yet. Copy them from an instance, or pick one to sync from and play it.",
            ));
        }
        let mut index = 0;
        for category in Category::ALL {
            let in_category: Vec<&Row> = rows.iter().filter(|r| r.category == category).collect();
            if in_category.is_empty() {
                continue;
            }
            let mut group = div()
                .flex()
                .flex_col()
                .rounded(radius::LG)
                .border_1()
                .border_color(colors::FADE_700)
                .bg(colors::GRAY_850)
                .child(
                    div()
                        .px_3()
                        .py_2()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new(("shared-values", category as usize), category.label())
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Slim)
                                .icon_right(if self.expanded.contains(&category) {
                                    Icon::ArrowUp
                                } else {
                                    Icon::ArrowDown
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if !this.expanded.remove(&category) {
                                        this.expanded.insert(category);
                                    }
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .text_size(theme::text_size(text::SMALL))
                                .text_color(colors::GRAY_500)
                                .child(format!("{} values", in_category.len())),
                        )
                        .when(!self.profile.shares(category), |this| {
                            this.child(
                                div()
                                    .text_size(theme::text_size(text::SMALL))
                                    .text_color(colors::GRAY_500)
                                    .child("not shared"),
                            )
                        }),
                );
            if self.expanded.contains(&category) {
                for row in in_category {
                    group = group.child(self.value_row(index, row, cx));
                    index += 1;
                }
            }
            values = values.child(group);
        }

        div()
            .id("shared-game-settings")
            .flex()
            .flex_col()
            .gap_4()
            .w_full()
            .max_w(px(1000.))
            .px_8()
            .pt_6()
            .child(
                section(
                    "Shared game settings",
                    Some(
                        "One set of options.txt settings for every instance. An instance can override single values in its Settings tab."
                            .into(),
                    ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_8()
                        .w_full()
                        .child(left)
                        .child(right),
                ),
            )
            .when_some(self.message.clone(), |this, (message, error)| {
                this.child(fields::status(
                    message,
                    if error {
                        colors::RED_200
                    } else {
                        colors::GREEN_300
                    },
                ))
            })
            .child(values)
    }
}
