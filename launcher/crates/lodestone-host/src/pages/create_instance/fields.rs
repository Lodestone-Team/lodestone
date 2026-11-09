//! The form's fields, from setting manifests (`FormFromManifest.tsx`,
//! `FieldFromManifest.tsx`, `form.ts`): their inputs, their starting values and their
//! checks, which are the dashboard's (`generateValidationSchema`).

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, Focusable as _, IntoElement, ParentElement,
    SharedString, Styled, div, prelude::*, uniform_list,
};
use launcher_core::cores::admin::{
    PublicUser,
    instances::{ConfigurableValue, ConfigurableValueType, SettingManifest},
    models::GlobalSettingsData,
};
use lodestone_ui::{
    components::{
        Accessible as _, KeyboardFocus as _, TextInput, Toggle,
        settings::{popover, popover_panel, select_button},
    },
    theme::{colors, ink, px, radius, text, weights},
};

use crate::i18n::{InstancesMsg as M, t, tr};

/// What a field holds while it is edited.
pub enum Input {
    Text(Entity<TextInput>),
    Bool(bool),
    Choice {
        value: Option<String>,
        open: bool,
        filter: Entity<TextInput>,
    },
    /// A kind of setting this Host doesn't know: sent as it came.
    Unknown,
}

/// One field of the form.
pub struct Field {
    pub setting: SettingManifest,
    pub input: Input,
    /// Why it can't be changed (`java_cmd` and `cmd_args` for most users).
    pub locked: Option<String>,
}

/// The settings that choose the program a server runs and its arguments.
pub const COMMAND_LINE_SETTINGS: [&str; 2] = ["java_cmd", "cmd_args"];

/// Why `me` can't change `java_cmd` / `cmd_args`, or `None` if they can
/// (`useCommandLineBlockedReason`): the owner can, a user who may write global files
/// can with safe mode off, and nobody while code execution is disabled.
pub fn command_line_blocked(
    me: &PublicUser,
    settings: Option<&GlobalSettingsData>,
) -> Option<&'static str> {
    let enabled = settings.is_none_or(|s| !s.code_execution_locked && s.code_execution_enabled);
    if !enabled {
        return Some(t(M::CommandLineReadOnly));
    }
    if me.is_owner {
        return None;
    }
    if me.permissions.can_write_global_file && settings.is_some_and(|s| !s.safe_mode) {
        return None;
    }
    Some(t(M::CommandLineOwnerOnly))
}

/// The value a field starts with: its default, else its value, else the type's zero
/// (`generateInitialValues`).
pub fn initial_value(setting: &SettingManifest) -> Option<ConfigurableValue> {
    setting
        .default_value
        .clone()
        .or_else(|| setting.value.clone())
        .or(match setting.value_type {
            ConfigurableValueType::Boolean => Some(ConfigurableValue::Boolean(false)),
            ConfigurableValueType::Integer { .. } => Some(ConfigurableValue::Integer(0)),
            ConfigurableValueType::UnsignedInteger { .. } => {
                Some(ConfigurableValue::UnsignedInteger(0))
            }
            ConfigurableValueType::Float { .. } => Some(ConfigurableValue::Float(0.0)),
            ConfigurableValueType::String { .. } => Some(ConfigurableValue::String(String::new())),
            _ => None,
        })
}

/// A value as text in a field.
pub fn value_text(value: &Option<ConfigurableValue>) -> String {
    match value {
        Some(ConfigurableValue::String(s) | ConfigurableValue::Enum(s)) => s.clone(),
        Some(ConfigurableValue::Integer(n)) => n.to_string(),
        Some(ConfigurableValue::UnsignedInteger(n)) => n.to_string(),
        Some(ConfigurableValue::Float(n)) => n.to_string(),
        Some(ConfigurableValue::Boolean(b)) => b.to_string(),
        None => String::new(),
    }
}

/// What was typed or picked.
pub enum Raw<'a> {
    Text(&'a str),
    Bool(bool),
    Choice(Option<&'a str>),
    Unknown,
}

/// The value to send for `raw`, or why it can't be sent (the dashboard's yup schema).
pub fn parse(setting: &SettingManifest, raw: Raw) -> Result<Option<ConfigurableValue>, String> {
    let name = setting.name.as_str();
    let required = || tr!(M::ValidationRequired, name = name);
    let number = || tr!(M::ValidationNumber, name = name);
    let integer = || tr!(M::ValidationInteger, name = name);
    match (&setting.value_type, raw) {
        (ConfigurableValueType::String { .. }, Raw::Text(text)) => {
            if setting.is_required && text.is_empty() {
                return Err(required());
            }
            Ok(Some(ConfigurableValue::String(text.to_string())))
        }
        (
            kind @ (ConfigurableValueType::Integer { .. }
            | ConfigurableValueType::UnsignedInteger { .. }
            | ConfigurableValueType::Float { .. }),
            Raw::Text(text),
        ) => {
            let text = text.trim();
            if text.is_empty() {
                return if setting.is_required {
                    Err(required())
                } else {
                    Ok(None)
                };
            }
            let value: f64 = text.parse().map_err(|_| number())?;
            if !value.is_finite() {
                return Err(number());
            }
            let (min, max) = match kind {
                ConfigurableValueType::Integer { min, max } => {
                    (min.map(|m| m as f64), max.map(|m| m as f64))
                }
                ConfigurableValueType::UnsignedInteger { min, max } => {
                    (min.map(|m| m as f64), max.map(|m| m as f64))
                }
                ConfigurableValueType::Float { min, max } => (*min, *max),
                _ => (None, None),
            };
            let is_integer = !matches!(kind, ConfigurableValueType::Float { .. });
            if is_integer && value.fract() != 0.0 {
                return Err(integer());
            }
            if matches!(kind, ConfigurableValueType::UnsignedInteger { .. }) && value < 0.0 {
                return Err(tr!(M::ValidationPositive, name = name));
            }
            // as the dashboard, a bound of 0 isn't checked (`if (min)`)
            if let Some(min) = min.filter(|m| *m != 0.0)
                && value < min
            {
                return Err(tr!(M::ValidationMin, name = name, min = min));
            }
            if let Some(max) = max.filter(|m| *m != 0.0)
                && value > max
            {
                return Err(tr!(M::ValidationMax, name = name, max = max));
            }
            Ok(Some(match kind {
                ConfigurableValueType::Integer { .. } => {
                    if value < i32::MIN as f64 || value > i32::MAX as f64 {
                        return Err(tr!(M::ValidationMax, name = name, max = i32::MAX));
                    }
                    ConfigurableValue::Integer(value as i64)
                }
                ConfigurableValueType::UnsignedInteger { .. } => {
                    if value > u32::MAX as f64 {
                        return Err(tr!(M::ValidationMax, name = name, max = u32::MAX));
                    }
                    ConfigurableValue::UnsignedInteger(value as u64)
                }
                _ => ConfigurableValue::Float(value),
            }))
        }
        (ConfigurableValueType::Boolean, Raw::Bool(value)) => {
            Ok(Some(ConfigurableValue::Boolean(value)))
        }
        (ConfigurableValueType::Enum { options }, Raw::Choice(choice)) => match choice {
            None if setting.is_required => Err(required()),
            None => Ok(None),
            Some(choice) if !options.iter().any(|o| o == choice) => {
                Err(tr!(M::ValidationOneOf, name = name))
            }
            Some(choice) => Ok(Some(ConfigurableValue::Enum(choice.to_string()))),
        },
        _ => Ok(initial_value(setting)),
    }
}

/// The options of an enum whose start matches what was typed, ignoring case
/// (`filterOptions`).
pub fn filter_options<'a>(options: &'a [String], query: &str) -> Vec<&'a String> {
    let query = query.trim().to_lowercase();
    options
        .iter()
        .filter(|o| query.is_empty() || o.to_lowercase().starts_with(&query))
        .collect()
}

impl Field {
    /// A field for `setting`, starting at its initial value.
    pub fn new(setting: SettingManifest, locked: Option<String>, cx: &mut App) -> Self {
        let initial = initial_value(&setting);
        let input = match &setting.value_type {
            ConfigurableValueType::Boolean => {
                Input::Bool(matches!(initial, Some(ConfigurableValue::Boolean(true))))
            }
            ConfigurableValueType::Enum { .. } => Input::Choice {
                value: match initial {
                    Some(ConfigurableValue::Enum(v) | ConfigurableValue::String(v)) => Some(v),
                    _ => None,
                },
                open: false,
                filter: cx.new(|cx| {
                    TextInput::new(cx).with_placeholder(lodestone_ui::i18n::CommonMsg::Search)
                }),
            },
            ConfigurableValueType::Unknown => Input::Unknown,
            _ => {
                let text = SharedString::from(value_text(&initial));
                let masked = setting.is_secret;
                let disabled = locked.is_some();
                let label = SharedString::from(setting.name.clone());
                Input::Text(cx.new(|cx| {
                    let input = TextInput::new(cx)
                        .with_text(text)
                        .with_label(lodestone_ui::i18n::Text::from(label))
                        .with_disabled(disabled);
                    if masked { input.with_masked() } else { input }
                }))
            }
        };
        Self {
            setting,
            input,
            locked,
        }
    }

    /// The value to send, or why it can't be.
    pub fn value(&self, cx: &App) -> Result<Option<ConfigurableValue>, String> {
        let raw = match &self.input {
            Input::Text(input) => {
                return parse(&self.setting, Raw::Text(input.read(cx).text()));
            }
            Input::Bool(value) => Raw::Bool(*value),
            Input::Choice { value, .. } => Raw::Choice(value.as_deref()),
            Input::Unknown => Raw::Unknown,
        };
        parse(&self.setting, raw)
    }
}

/// A form field's row: its name (and "Optional"), what it is, its control on the
/// right, and the error under it (`FormInputField.tsx`).
pub fn row(
    name: impl Into<SharedString>,
    description: impl Into<SharedString>,
    optional: bool,
    control: AnyElement,
    error: Option<String>,
) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_start()
        .justify_between()
        .gap(px(16.))
        .px(px(16.))
        .py(px(12.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(6.))
                        .items_baseline()
                        .text_size(text::MEDIUM)
                        .font_weight(weights::BOLD.face())
                        .text_color(ink::text())
                        .child(name.into())
                        .when(optional, |this| {
                            this.child(
                                div()
                                    .text_size(text::SMALL)
                                    .font_weight(weights::MEDIUM.face())
                                    .text_color(ink::faint())
                                    .child(t(M::Optional)),
                            )
                        }),
                )
                .child(
                    div()
                        .text_size(text::SMALL)
                        .text_color(ink::muted())
                        .child(description.into()),
                ),
        )
        .child(
            div()
                .flex_none()
                .w(px(240.))
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(control)
                .when_some(error, |this, error| {
                    this.child(
                        div()
                            .text_size(text::SMALL)
                            .text_color(colors::RED_200)
                            .child(error),
                    )
                }),
        )
        .into_any_element()
}

/// A box of rows, as the dashboard's form sections.
pub fn section(rows: Vec<AnyElement>) -> AnyElement {
    let count = rows.len();
    div()
        .flex()
        .flex_col()
        .rounded(radius::LG)
        .border_1()
        .border_color(ink::border())
        .bg(colors::GRAY_800)
        .children(rows.into_iter().enumerate().map(|(i, row)| {
            div()
                .when(i + 1 < count, |this| {
                    this.border_b_1().border_color(ink::border())
                })
                .child(row)
        }))
        .into_any_element()
}

/// A view that holds fields: how its fields are reached by index.
pub trait HasFields: Sized + 'static {
    fn field_mut(&mut self, key: usize) -> Option<&mut Field>;
}

/// The control of field `key` of `view`.
pub fn control<V: HasFields>(
    field: &Field,
    key: usize,
    id_prefix: &str,
    cx: &mut Context<V>,
) -> AnyElement {
    let disabled = field.locked.is_some() || !field.setting.is_mutable;
    match &field.input {
        Input::Text(input) => input.clone().into_any_element(),
        Input::Bool(value) => Toggle::new(SharedString::from(format!("{id_prefix}-{key}")), *value)
            .label(field.setting.name.clone())
            .disabled(disabled)
            .on_change(cx.listener(move |view, value: &bool, _, cx| {
                if let Some(Field {
                    input: Input::Bool(v),
                    ..
                }) = view.field_mut(key)
                {
                    *v = *value;
                }
                cx.notify();
            }))
            .into_any_element(),
        Input::Choice {
            value,
            open,
            filter,
        } => {
            let options = match &field.setting.value_type {
                ConfigurableValueType::Enum { options } => options.clone(),
                _ => Vec::new(),
            };
            let label = value.clone().unwrap_or_default();
            let open = *open;
            let filter = filter.clone();
            div()
                .flex()
                .flex_col()
                .child(
                    select_button(
                        SharedString::from(format!("{id_prefix}-{key}")),
                        div().truncate().child(label.clone()),
                    )
                    .aria_label(field.setting.name.clone())
                    .w_full()
                    .justify_between()
                    .when(!disabled, |this| {
                        let filter = filter.clone();
                        this.on_press(cx.listener(move |view, _, window, cx| {
                            if let Some(Field {
                                input: Input::Choice { open, .. },
                                ..
                            }) = view.field_mut(key)
                            {
                                *open = !*open;
                                if *open {
                                    filter.focus_handle(cx).focus(window, cx);
                                }
                            }
                            cx.notify();
                        }))
                    }),
                )
                .when(open, |this| {
                    let close = cx.listener(move |view, _: &(), _, cx| {
                        if let Some(Field {
                            input: Input::Choice { open, .. },
                            ..
                        }) = view.field_mut(key)
                        {
                            *open = false;
                        }
                        cx.notify();
                    });
                    let query = filter.read(cx).text().to_string();
                    let shown: Vec<String> = filter_options(&options, &query)
                        .into_iter()
                        .cloned()
                        .collect();
                    let count = shown.len();
                    let selected = value.clone();
                    let list = if count == 0 {
                        div()
                            .p(px(8.))
                            .text_size(text::MEDIUM)
                            .text_color(ink::muted())
                            .child(t(M::NothingFound))
                            .into_any_element()
                    } else {
                        uniform_list(
                            SharedString::from(format!("{id_prefix}-{key}-options")),
                            count,
                            cx.processor(move |_, range: std::ops::Range<usize>, _, cx| {
                                range
                                    .filter_map(|i| shown.get(i).map(|o| (i, o.clone())))
                                    .map(|(i, option)| {
                                        let chosen = selected.as_deref() == Some(option.as_str());
                                        let pick = option.clone();
                                        div()
                                            .id(i)
                                            .accessible(
                                                gpui::Role::ListItem,
                                                format!("option-{i}"),
                                                option.clone(),
                                            )
                                            .keyboard_focus()
                                            .h(px(30.))
                                            .px(px(10.))
                                            .flex()
                                            .items_center()
                                            .rounded(radius::MD)
                                            .cursor_pointer()
                                            .text_size(text::MEDIUM)
                                            .text_color(ink::text())
                                            .when(chosen, |this| this.bg(colors::GRAY_700))
                                            .hover(|style| style.bg(colors::GRAY_750))
                                            .child(option)
                                            .on_press(cx.listener(move |view: &mut V, _, _, cx| {
                                                if let Some(Field {
                                                    input: Input::Choice { value, open, .. },
                                                    ..
                                                }) = view.field_mut(key)
                                                {
                                                    *value = Some(pick.clone());
                                                    *open = false;
                                                }
                                                cx.notify();
                                            }))
                                    })
                                    .collect()
                            }),
                        )
                        .h(px((count.min(8) * 30) as f32))
                        .into_any_element()
                    };
                    let panel =
                        popover_panel(SharedString::from(format!("{id_prefix}-{key}-panel")))
                            .w(px(240.))
                            .gap(px(4.))
                            .on_mouse_down_out(cx.listener(move |view, _, _, cx| {
                                if let Some(Field {
                                    input: Input::Choice { open, .. },
                                    ..
                                }) = view.field_mut(key)
                                {
                                    *open = false;
                                }
                                cx.notify();
                            }))
                            .child(filter.clone())
                            .child(list);
                    this.child(div().child(popover(
                        SharedString::from(format!("{id_prefix}-{key}-popover")),
                        panel,
                        move |window, cx| close(&(), window, cx),
                    )))
                })
                .into_any_element()
        }
        Input::Unknown => div()
            .text_size(text::SMALL)
            .text_color(ink::muted())
            .child(t(M::UnknownSetting))
            .into_any_element(),
    }
}

/// The row of a field, with its error.
pub fn field_row<V: HasFields>(
    field: &Field,
    key: usize,
    id_prefix: &str,
    error: Option<String>,
    cx: &mut Context<V>,
) -> AnyElement {
    let description = match &field.locked {
        Some(reason) => format!("{} {reason}", field.setting.description),
        None => field.setting.description.clone(),
    };
    row(
        field.setting.name.clone(),
        description,
        !field.setting.is_required,
        control(field, key, id_prefix, cx),
        error,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setting(kind: ConfigurableValueType, required: bool) -> SettingManifest {
        SettingManifest {
            setting_id: "s".into(),
            name: "Port".into(),
            description: String::new(),
            value: None,
            value_type: kind,
            default_value: None,
            is_secret: false,
            is_required: required,
            is_mutable: true,
        }
    }

    #[test]
    fn numbers_are_checked_as_the_dashboard_does() {
        let port = setting(
            ConfigurableValueType::UnsignedInteger {
                min: Some(1),
                max: Some(65535),
            },
            true,
        );
        assert_eq!(
            parse(&port, Raw::Text("25565")),
            Ok(Some(ConfigurableValue::UnsignedInteger(25565)))
        );
        assert!(
            parse(&port, Raw::Text(""))
                .unwrap_err()
                .contains("required")
        );
        assert!(
            parse(&port, Raw::Text("abc"))
                .unwrap_err()
                .contains("number")
        );
        assert!(
            parse(&port, Raw::Text("1.5"))
                .unwrap_err()
                .contains("integer")
        );
        assert!(
            parse(&port, Raw::Text("-3"))
                .unwrap_err()
                .contains("positive")
        );
        assert!(
            parse(&port, Raw::Text("70000"))
                .unwrap_err()
                .contains("less")
        );
        let optional = setting(
            ConfigurableValueType::Float {
                min: None,
                max: None,
            },
            false,
        );
        assert_eq!(parse(&optional, Raw::Text(" ")), Ok(None));
        assert_eq!(
            parse(&optional, Raw::Text("0.5")),
            Ok(Some(ConfigurableValue::Float(0.5)))
        );
    }

    #[test]
    fn strings_enums_and_starting_values() {
        let name = setting(ConfigurableValueType::String { regex: None }, true);
        assert!(parse(&name, Raw::Text("")).is_err());
        let version = setting(
            ConfigurableValueType::Enum {
                options: vec!["1.21".into(), "1.20.1".into()],
            },
            true,
        );
        assert!(parse(&version, Raw::Choice(None)).is_err());
        assert!(parse(&version, Raw::Choice(Some("1.8"))).is_err());
        assert_eq!(
            parse(&version, Raw::Choice(Some("1.21"))),
            Ok(Some(ConfigurableValue::Enum("1.21".into())))
        );
        assert_eq!(initial_value(&version), None);
        assert_eq!(
            initial_value(&setting(ConfigurableValueType::Boolean, true)),
            Some(ConfigurableValue::Boolean(false))
        );
        let options = vec!["1.21".to_string(), "1.20.1".into(), "24w14a".into()];
        assert_eq!(filter_options(&options, "1.2").len(), 2);
        assert_eq!(filter_options(&options, "").len(), 3);
        assert_eq!(filter_options(&options, "24W").len(), 1);
    }

    #[test]
    fn the_command_line_is_for_the_owner() {
        let mut me = PublicUser {
            uid: "1".into(),
            username: "u".into(),
            is_owner: true,
            is_admin: false,
            permissions: Default::default(),
        };
        let settings = GlobalSettingsData {
            code_execution_enabled: true,
            safe_mode: false,
            ..Default::default()
        };
        assert_eq!(command_line_blocked(&me, Some(&settings)), None);
        me.is_owner = false;
        assert!(command_line_blocked(&me, Some(&settings)).is_some());
        me.permissions.can_write_global_file = true;
        assert_eq!(command_line_blocked(&me, Some(&settings)), None);
        let off = GlobalSettingsData {
            code_execution_enabled: false,
            ..settings
        };
        me.is_owner = true;
        assert!(command_line_blocked(&me, Some(&off)).is_some());
    }
}
