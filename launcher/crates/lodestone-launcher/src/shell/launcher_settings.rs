//! Settings > Instance defaults: the memory, window, arguments and environment
//! variables every instance uses unless its own Settings tab changes them. Changes
//! are kept until Save; the Settings dialog shows the bar with Save and Discard, and
//! asks before closing over unsaved ones.

use crate::theme::px;
use gpui::{
    App, Context, Entity, IntoElement, Render, SharedString, Subscription, Window, div, prelude::*,
};
use launcher_core::settings::{DEFAULT_MIN_RAM_MB, InstanceDefaults, default_max_ram_mb};

use crate::{
    components::{
        TextInput, Toggle,
        settings::{SettingRow, Toast, card},
    },
    i18n::{CommonMsg, FormatMsg, ShellMsg as M, format, t, tr},
    instances::{
        Instances,
        fields::{Fields, Values},
    },
    theme::{self, colors, leading, text},
};

pub struct LauncherSettings {
    instances: Entity<Instances>,
    fields: Fields,
    toast: Toast,
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
        .map(|input| cx.observe(input, |_, _, cx| cx.notify()))
        .collect();
        Self {
            instances,
            fields,
            toast: Toast::default(),
            _observe: observe,
        }
    }

    /// Whether the fields differ from the saved defaults (or aren't valid).
    pub fn has_changes(&self, cx: &App) -> bool {
        match self.read(cx) {
            Ok(defaults) => &defaults != self.instances.read(cx).defaults(),
            Err(_) => true,
        }
    }

    /// Whether the fields can be saved: every value is valid.
    pub fn can_save(&self, cx: &App) -> bool {
        self.read(cx).is_ok()
    }

    /// Saves the fields. Returns whether they are saved now; invalid ones say why
    /// under their rows.
    pub fn save(&mut self, cx: &mut Context<Self>) -> bool {
        let Ok(defaults) = self.read(cx) else {
            cx.notify();
            return false;
        };
        if &defaults == self.instances.read(cx).defaults() {
            return true;
        }
        let result = self
            .instances
            .update(cx, |instances, cx| instances.save_defaults(defaults, cx));
        let (message, error): (SharedString, bool) = match &result {
            Ok(()) => (t(CommonMsg::Saved).into(), false),
            Err(err) => (err.clone().into(), true),
        };
        self.toast
            .show(message, error, |this: &mut Self| &mut this.toast, cx);
        result.is_ok()
    }

    /// Changes a field without saving, for the offscreen screenshots.
    pub fn preview_change(&mut self, cx: &mut Context<Self>) {
        self.fields
            .jvm_args
            .update(cx, |input, cx| input.set_text("-XX:+UseZGC", cx));
    }

    /// Puts the saved defaults back in the fields.
    pub fn discard(&mut self, cx: &mut Context<Self>) {
        let (defaults, total) = {
            let instances = self.instances.read(cx);
            (instances.defaults().clone(), instances.total_memory())
        };
        self.fields.set(
            &Values {
                memory: defaults.memory(total),
                jvm_args: &defaults.jvm_args,
                game_args: &defaults.game_args,
                window: defaults.window(),
                env: &defaults.env,
            },
            cx,
        );
        cx.notify();
    }

    fn read(&self, cx: &App) -> Result<InstanceDefaults, String> {
        let instances = self.instances.read(cx);
        let total = instances.total_memory();
        let mut defaults = instances.defaults().clone();
        let check = self.fields.memory(total, cx);
        let (min, max) = check
            .values
            .ok_or_else(|| tr!(M::MemoryError, error = check.error.unwrap_or_default()))?;
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
        let megabytes = |mb: u64| tr!(FormatMsg::Megabytes, size = format::number(mb));
        let memory_hint = tr!(
            M::MemoryDefaultHint,
            min = megabytes(DEFAULT_MIN_RAM_MB.into()),
            max = megabytes(default_max_ram_mb(total).into()),
        );
        let window_error = self.fields.window(cx).err();
        let env_error = self.fields.env(cx).err();
        div()
            .flex()
            .flex_col()
            .w_full()
            .line_height(theme::line_height(text::MEDIUM, leading::NORMAL))
            .child(card([
                SettingRow::new(t(M::Memory))
                    .hint(memory_hint)
                    .child(sized(&self.fields.min_ram, 84.))
                    .child("–")
                    .child(sized(&self.fields.max_ram, 84.))
                    .child(
                        div()
                            .text_color(colors::GRAY_500)
                            .child(t(M::MegabytesUnit)),
                    )
                    .note_opt(
                        memory_check
                            .error
                            .map(|e| (e.into(), colors::RED_200))
                            .or(memory_check.warning.map(|w| (w.into(), colors::YELLOW))),
                    )
                    .into_any_element(),
                SettingRow::new(t(M::Window))
                    .child(sized(&self.fields.width, 72.))
                    .child("×")
                    .child(sized(&self.fields.height, 72.))
                    .child(
                        div()
                            .ml_2()
                            .text_color(colors::GRAY_400)
                            .child(t(M::Fullscreen)),
                    )
                    .child(
                        Toggle::new("default-fullscreen", self.fields.fullscreen).on_change(
                            cx.listener(|this, value: &bool, _, cx| {
                                this.fields.fullscreen = *value;
                                cx.notify();
                            }),
                        ),
                    )
                    .note_opt(window_error.map(|e| (e.into(), colors::RED_200)))
                    .into_any_element(),
                SettingRow::new(t(M::JvmArguments))
                    .child(sized(&self.fields.jvm_args, 280.))
                    .into_any_element(),
                SettingRow::new(t(M::GameArguments))
                    .child(sized(&self.fields.game_args, 280.))
                    .into_any_element(),
                SettingRow::new(t(M::EnvironmentVariables))
                    .child(sized(&self.fields.env, 280.))
                    .note_opt(env_error.map(|e| (e.into(), colors::RED_200)))
                    .into_any_element(),
            ]))
    }
}
