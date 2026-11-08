//! The launcher's Settings page: the settings every instance uses unless its own
//! Settings tab overrides them. Each section is its own function, so later sections
//! (such as a Developer section) slot in beside them.

use gpui::{
    Context, Entity, IntoElement, Render, SharedString, Subscription, Window, div, prelude::*, px,
};
use launcher_core::settings::{DEFAULT_MIN_RAM_MB, InstanceDefaults, default_max_ram_mb};
use launcher_core::wording;

use crate::{
    components::{Button, ButtonVariant},
    instances::{
        Instances,
        fields::{self, Fields, Values, field, section},
    },
    theme::{self, colors, fonts, leading, radius, text, weights},
};

pub struct LauncherSettings {
    instances: Entity<Instances>,
    fields: Fields,
    message: Option<(SharedString, bool)>,
    _observe: Vec<Subscription>,
}

impl LauncherSettings {
    pub fn new(instances: Entity<Instances>, cx: &mut Context<Self>) -> Self {
        let (defaults, total) = {
            let instances = instances.read(cx);
            (instances.defaults().clone(), instances.total_memory())
        };
        let fields = Fields::new(
            &Values {
                memory: defaults.memory(total),
                jvm_args: &defaults.jvm_args,
                game_args: &defaults.game_args,
                window: defaults.window(),
                env: &defaults.env,
            },
            cx,
        );
        let observe = vec![
            cx.observe(&fields.min_ram, |_, _, cx| cx.notify()),
            cx.observe(&fields.max_ram, |_, _, cx| cx.notify()),
        ];
        Self {
            instances,
            fields,
            message: None,
            _observe: observe,
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let result = self.read(cx).and_then(|defaults| {
            self.instances
                .update(cx, |instances, cx| instances.save_defaults(defaults, cx))
        });
        self.message = Some(match result {
            Ok(()) => (
                "Saved. Instances that don't override these use them from the next Play.".into(),
                false,
            ),
            Err(err) => (err.into(), true),
        });
        cx.notify();
    }

    fn read(&self, cx: &Context<Self>) -> Result<InstanceDefaults, String> {
        let instances = self.instances.read(cx);
        let total = instances.total_memory();
        let mut defaults = instances.defaults().clone();
        let check = self.fields.memory(total, cx);
        let (min, max) = check
            .values
            .ok_or_else(|| format!("Memory: {}", check.error.unwrap_or_default()))?;
        // The launcher's own defaults stay unset, so they follow this Mac.
        defaults.min_ram = (min != DEFAULT_MIN_RAM_MB).then_some(min);
        defaults.max_ram = (max != default_max_ram_mb(total)).then_some(max);
        defaults.jvm_args = self.fields.jvm_args(cx);
        defaults.game_args = self.fields.game_args(cx);
        let window = self.fields.window(cx)?;
        defaults.resolution = window.resolution;
        defaults.fullscreen = window.fullscreen;
        defaults.env = self.fields.env(cx)?;
        Ok(defaults)
    }

    fn memory_section(&self, cx: &Context<Self>) -> impl IntoElement {
        let total = self.instances.read(cx).total_memory();
        let check = self.fields.memory(total, cx);
        section("Memory", fields::total_memory_label(total))
            .child(
                div()
                    .text_size(theme::text_size(text::SMALL))
                    .text_color(colors::GRAY_500)
                    .child(format!(
                        "The default is {} MB to {} MB: half this {}'s memory, at most 4 GB.",
                        DEFAULT_MIN_RAM_MB,
                        crate::instances::thousands(default_max_ram_mb(total).into()),
                        wording::COMPUTER
                    )),
            )
            .child(fields::memory_inputs(&self.fields, &check))
    }

    fn arguments_section(&self) -> impl IntoElement {
        section(
            "Arguments",
            Some("Separated by spaces; quote values with spaces.".into()),
        )
        .child(field("JVM ARGUMENTS", self.fields.jvm_args.clone()))
        .child(field("GAME ARGUMENTS", self.fields.game_args.clone()))
    }

    fn window_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        section(
            "Game window",
            Some("Leave the size empty for the game's default.".into()),
        )
        .child(fields::window_inputs(
            &self.fields,
            "default-fullscreen",
            cx.listener(|this, value: &bool, _, cx| {
                this.fields.fullscreen = *value;
                cx.notify();
            }),
        ))
    }

    fn env_section(&self) -> impl IntoElement {
        section(
            "Environment variables",
            Some("NAME=value, separated by spaces.".into()),
        )
        .child(self.fields.env.clone())
    }
}

impl Render for LauncherSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("launcher-settings")
            .flex()
            .flex_col()
            .gap_6()
            .w_full()
            .px_8()
            .pt_6()
            .line_height(theme::line_height(text::MEDIUM, leading::NORMAL))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .font_family(fonts::TITLE)
                            .text_size(theme::text_size(text::H1))
                            .line_height(theme::line_height(text::H1, leading::TIGHT))
                            .font_weight(weights::BOLD.into())
                            .child("Settings"),
                    )
                    .child(
                        div()
                            .text_color(colors::GRAY_500)
                            .font_weight(weights::BOLD.into())
                            .child("Every instance uses these unless its own Settings tab overrides them."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_8()
                    .w_full()
                    .max_w(px(1000.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_6()
                            .flex_1()
                            .min_w_0()
                            .child(self.memory_section(cx))
                            .child(self.window_section(cx)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_6()
                            .flex_1()
                            .min_w_0()
                            .child(self.arguments_section())
                            .child(self.env_section()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(
                        Button::new("save-launcher-settings", "Save")
                            .variant(ButtonVariant::Primary)
                            .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                    )
                    .when_some(self.message.clone(), |this, (message, error)| {
                        this.child(
                            div()
                                .px_3()
                                .py_1()
                                .rounded(radius::MD)
                                .text_size(theme::text_size(text::SMALL))
                                .font_weight(weights::BOLD.into())
                                .text_color(if error {
                                    colors::RED_200
                                } else {
                                    colors::GREEN_300
                                })
                                .child(message),
                        )
                    }),
            )
    }
}
