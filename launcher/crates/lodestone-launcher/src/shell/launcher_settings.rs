//! Settings > Instance defaults: the memory, window, arguments and environment
//! variables every instance uses unless its own Settings tab changes them. Changes
//! save once typing stops and the values are valid.

use std::time::Duration;

use gpui::{
    Context, Entity, IntoElement, Render, SharedString, Subscription, Task, Window, div,
    prelude::*, px,
};
use launcher_core::settings::{DEFAULT_MIN_RAM_MB, InstanceDefaults, default_max_ram_mb};

use crate::{
    components::{
        TextInput, Toggle,
        settings::{SettingRow, Toast, card},
    },
    instances::{
        Instances,
        fields::{self, Fields, Values},
    },
    theme::{self, colors, leading, text},
};

/// How long typing has to stop before the change is saved.
const SAVE_AFTER: Duration = Duration::from_millis(600);

pub struct LauncherSettings {
    instances: Entity<Instances>,
    fields: Fields,
    toast: Toast,
    _save: Option<Task<()>>,
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
        let observe = [
            &fields.min_ram,
            &fields.max_ram,
            &fields.jvm_args,
            &fields.game_args,
            &fields.width,
            &fields.height,
            &fields.env,
        ]
        .into_iter()
        .map(|input| cx.observe(input, |this, _, cx| this.schedule_save(cx)))
        .collect();
        Self {
            instances,
            fields,
            toast: Toast::default(),
            _save: None,
            _observe: observe,
        }
    }

    fn schedule_save(&mut self, cx: &mut Context<Self>) {
        self._save = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SAVE_AFTER).await;
            this.update(cx, |this, cx| this.save(cx)).ok();
        }));
        cx.notify();
    }

    /// Saves the fields if they are valid and changed; invalid ones say why under
    /// their rows.
    fn save(&mut self, cx: &mut Context<Self>) {
        let Ok(defaults) = self.read(cx) else {
            cx.notify();
            return;
        };
        if &defaults == self.instances.read(cx).defaults() {
            return;
        }
        let result = self
            .instances
            .update(cx, |instances, cx| instances.save_defaults(defaults, cx));
        let (message, error): (SharedString, bool) = match result {
            Ok(()) => ("Saved".into(), false),
            Err(err) => (err.into(), true),
        };
        self.toast
            .show(message, error, |this: &mut Self| &mut this.toast, cx);
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
}

fn sized(input: &Entity<TextInput>, width: f32) -> impl IntoElement {
    div().w(px(width)).child(input.clone())
}

impl LauncherSettings {
    /// Drawn by the Settings dialog, at the bottom of its page.
    pub fn toast(&self) -> &Toast {
        &self.toast
    }
}

impl Render for LauncherSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let total = self.instances.read(cx).total_memory();
        let memory_check = self.fields.memory(total, cx);
        let memory_hint = format!(
            "{} The default is {} MB to {} MB: half this Mac's memory, at most 4 GB.",
            fields::total_memory_label(total).unwrap_or_default(),
            DEFAULT_MIN_RAM_MB,
            crate::instances::thousands(default_max_ram_mb(total).into())
        );
        let window_error = self.fields.window(cx).err();
        let env_error = self.fields.env(cx).err();
        div()
            .flex()
            .flex_col()
            .w_full()
            .line_height(theme::line_height(text::MEDIUM, leading::NORMAL))
            .child(card([
                SettingRow::new("Memory")
                    .hint(memory_hint.trim().to_string())
                    .child(sized(&self.fields.min_ram, 84.))
                    .child("–")
                    .child(sized(&self.fields.max_ram, 84.))
                    .child(div().text_color(colors::GRAY_500).child("MB"))
                    .note_opt(
                        memory_check
                            .error
                            .map(|e| (e.into(), colors::RED_200))
                            .or(memory_check.warning.map(|w| (w.into(), colors::YELLOW))),
                    )
                    .into_any_element(),
                SettingRow::new("Window")
                    .hint("Leave the size empty for the game's default.")
                    .child(sized(&self.fields.width, 72.))
                    .child("×")
                    .child(sized(&self.fields.height, 72.))
                    .child(
                        div()
                            .ml_2()
                            .text_color(colors::GRAY_400)
                            .child("Fullscreen"),
                    )
                    .child(
                        Toggle::new("default-fullscreen", self.fields.fullscreen).on_change(
                            cx.listener(|this, value: &bool, _, cx| {
                                this.fields.fullscreen = *value;
                                this.save(cx);
                                cx.notify();
                            }),
                        ),
                    )
                    .note_opt(window_error.map(|e| (e.into(), colors::RED_200)))
                    .into_any_element(),
                SettingRow::new("JVM arguments")
                    .hint("Separated by spaces; quote values with spaces.")
                    .child(sized(&self.fields.jvm_args, 280.))
                    .into_any_element(),
                SettingRow::new("Game arguments")
                    .hint("Passed to Minecraft itself.")
                    .child(sized(&self.fields.game_args, 280.))
                    .into_any_element(),
                SettingRow::new("Environment variables")
                    .hint("NAME=value, separated by spaces.")
                    .child(sized(&self.fields.env, 280.))
                    .note_opt(env_error.map(|e| (e.into(), colors::RED_200)))
                    .into_any_element(),
            ]))
    }
}
