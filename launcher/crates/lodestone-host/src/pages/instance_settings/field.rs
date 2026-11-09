//! One setting of the core's manifest as a control, after the dashboard's
//! `SettingField` and its `Config` atoms (`InputBox`, `SelectBox`, `ToggleBox`): the
//! text fields with their save and revert buttons, the list of an enum's options, the
//! checks a typed value must pass, and the MOTD's preview.

use gpui::{
    AnyElement, AppContext as _, Context, Entity, IntoElement, KeyDownEvent, ParentElement,
    SharedString, Styled, div, prelude::*,
};
use launcher_core::cores::admin::instance_settings::{
    COMMAND_LINE_SETTINGS, ConfigurableValue, ConfigurableValueType,
};
use lodestone_ui::{
    components::{
        Icon, IconButton, ListRow, LoadingDots, RichText, TextInput,
        a11y::Accessible,
        settings::{muted_link, popover, popover_panel, select_button},
    },
    theme::{colors, fonts, px, radius, text},
};

use super::{InstanceSettingsPage, Key};
use crate::i18n::{CommonMsg, InstanceMsg as M, t, tr};

/// Whether a setting chooses the program the instance runs, or its arguments.
pub fn is_command_line(setting_id: &str) -> bool {
    COMMAND_LINE_SETTINGS.contains(&setting_id)
}

/// What kind of field a setting is typed into. Switches and lists aren't fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    Text,
    /// A secret: masked, with Show and Hide.
    Password,
    Number,
}

impl FieldKind {
    pub fn of(value_type: &ConfigurableValueType, is_secret: bool) -> Option<Self> {
        match value_type {
            ConfigurableValueType::String { .. } if is_secret => Some(FieldKind::Password),
            ConfigurableValueType::String { .. } | ConfigurableValueType::Float { .. } => {
                Some(FieldKind::Text)
            }
            ConfigurableValueType::Integer { .. }
            | ConfigurableValueType::UnsignedInteger { .. } => Some(FieldKind::Number),
            _ => None,
        }
    }
}

/// Why a typed value can't be saved (`InputBox`'s checks).
#[derive(Clone, Debug, PartialEq)]
pub enum Problem {
    Empty,
    NotANumber,
    BelowMin(String),
    AboveMax(String),
}

impl Problem {
    pub fn text(&self) -> String {
        match self {
            Problem::Empty => t(M::CannotBeEmpty).to_string(),
            Problem::NotANumber => t(M::MustBeNumber).to_string(),
            Problem::BelowMin(min) => tr!(M::MustBeGreaterThan, min = min),
            Problem::AboveMax(max) => tr!(M::MustBeLessThan, max = max),
        }
    }
}

/// An integer as the dashboard's `parseintStrict` reads it: digits, with an optional
/// sign, and nothing else.
fn strict_int(text_: &str) -> Option<i64> {
    let digits = text_.strip_prefix(['-', '+']).unwrap_or(text_);
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    text_.parse().ok()
}

/// A finite decimal number, such as `0.5` or `-2`.
fn strict_float(text_: &str) -> Option<f64> {
    let ok = text_
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'));
    if !ok {
        return None;
    }
    text_.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// The value to save for `text_` typed into a setting of `value_type`, or why it
/// can't be. `submit` is false while typing: an empty number is then only "not a
/// number". `None` for a setting that isn't typed (a switch or a list).
pub fn value_of(
    value_type: &ConfigurableValueType,
    text_: &str,
    submit: bool,
) -> Result<Option<ConfigurableValue>, Problem> {
    let number = matches!(
        value_type,
        ConfigurableValueType::Integer { .. }
            | ConfigurableValueType::UnsignedInteger { .. }
            | ConfigurableValueType::Float { .. }
    );
    if number && text_.is_empty() && submit {
        return Err(Problem::Empty);
    }
    fn bounds<T: PartialOrd + ToString>(
        n: T,
        min: Option<T>,
        max: Option<T>,
    ) -> Result<T, Problem> {
        if let Some(min) = min
            && n < min
        {
            return Err(Problem::BelowMin(min.to_string()));
        }
        if let Some(max) = max
            && n > max
        {
            return Err(Problem::AboveMax(max.to_string()));
        }
        Ok(n)
    }
    Ok(Some(match value_type {
        ConfigurableValueType::String { .. } => ConfigurableValue::String(text_.to_string()),
        ConfigurableValueType::Integer { min, max } => {
            let n = strict_int(text_).ok_or(Problem::NotANumber)?;
            ConfigurableValue::Integer(bounds(n, *min, *max)?)
        }
        ConfigurableValueType::UnsignedInteger { min, max } => {
            let n = strict_int(text_).ok_or(Problem::NotANumber)?;
            let min = Some(min.map(|m| m as i64).unwrap_or(0));
            let n = bounds(n, min, max.map(|m| m as i64))?;
            ConfigurableValue::UnsignedInteger(n as u64)
        }
        ConfigurableValueType::Float { min, max } => {
            let n = strict_float(text_).ok_or(Problem::NotANumber)?;
            ConfigurableValue::Float(bounds(n, *min, *max)?)
        }
        _ => return Ok(None),
    }))
}

/// A setting's text field, and what it remembers between renders.
pub struct Field {
    pub input: Entity<TextInput>,
    /// The value the core has.
    saved_text: String,
    /// Why the last save failed, or the typed value can't be saved.
    pub error: Option<String>,
    pub saving: bool,
    pub disabled: bool,
    pub kind: FieldKind,
    /// A password field shows its text.
    pub revealed: bool,
}

impl Field {
    pub fn new(
        kind: FieldKind,
        text_: String,
        disabled: bool,
        cx: &mut Context<InstanceSettingsPage>,
    ) -> Self {
        let input = Self::make_input(&text_, kind == FieldKind::Password, disabled, cx);
        Self {
            input,
            saved_text: text_,
            error: None,
            saving: false,
            disabled,
            kind,
            revealed: false,
        }
    }

    fn make_input(
        text_: &str,
        masked: bool,
        disabled: bool,
        cx: &mut Context<InstanceSettingsPage>,
    ) -> Entity<TextInput> {
        let text_ = SharedString::from(text_.to_string());
        cx.new(|cx| {
            let input = TextInput::new(cx).with_text(text_).with_disabled(disabled);
            if masked { input.with_masked() } else { input }
        })
    }

    pub fn text(&self, cx: &gpui::App) -> String {
        self.input.read(cx).text().to_string()
    }

    /// The typed text differs from the saved value.
    pub fn touched(&self, cx: &gpui::App) -> bool {
        self.text(cx) != self.saved_text
    }

    /// The value was saved.
    pub fn saved(&mut self, text_: String, _cx: &mut Context<InstanceSettingsPage>) {
        self.saved_text = text_;
        self.error = None;
    }

    /// Escape or the revert button: the saved value again.
    fn revert(&mut self, cx: &mut Context<InstanceSettingsPage>) {
        let saved = self.saved_text.clone();
        self.input.update(cx, |input, cx| input.set_text(saved, cx));
        self.error = None;
    }

    /// Shows or masks a password field's text, keeping what was typed.
    fn toggle_reveal(&mut self, cx: &mut Context<InstanceSettingsPage>) -> Entity<TextInput> {
        self.revealed = !self.revealed;
        let typed = self.text(cx);
        self.input = Self::make_input(&typed, !self.revealed, self.disabled, cx);
        self.input.clone()
    }

    pub fn render(&self, key: Key, cx: &mut Context<InstanceSettingsPage>) -> AnyElement {
        let id = SharedString::from(format!("field-{}-{}", key.0, key.1));
        let touched = self.touched(cx) && !self.disabled;
        let (on_key, save, revert, reveal) = (key.clone(), key.clone(), key.clone(), key);
        div()
            .id(id.clone())
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .w(px(280.))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                match event.keystroke.key.as_str() {
                    "enter" => this.submit(on_key.clone(), cx),
                    "escape" => {
                        if let Some(field) = this.fields.get_mut(&on_key) {
                            field.revert(cx);
                        }
                        cx.notify();
                    }
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(div().flex_1().min_w_0().child(self.input.clone()))
            .when(self.saving, |this| this.child(LoadingDots))
            .when(touched && !self.saving, |this| {
                this.child(
                    IconButton::new(sub(&id, "save"), Icon::Check)
                        .label(t(CommonMsg::Save))
                        .on_click(cx.listener(move |this, _, _, cx| this.submit(save.clone(), cx))),
                )
                .child(
                    IconButton::new(sub(&id, "revert"), Icon::Xmark)
                        .label(t(M::Revert))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(field) = this.fields.get_mut(&revert) {
                                field.revert(cx);
                            }
                            cx.notify();
                        })),
                )
            })
            .when(self.kind == FieldKind::Password, |this| {
                this.child(
                    muted_link(
                        sub(&id, "reveal"),
                        t(if self.revealed {
                            M::HidePassword
                        } else {
                            M::ShowPassword
                        }),
                    )
                    .on_press(cx.listener(move |this, _, _, cx| {
                        if let Some(field) = this.fields.get_mut(&reveal) {
                            let input = field.toggle_reveal(cx);
                            this._subscriptions
                                .push(cx.observe(&input, |_, _, cx| cx.notify()));
                        }
                        cx.notify();
                    })),
                )
            })
            .into_any_element()
    }
}

/// An enum setting: a button showing the value, which opens the list of options
/// (`SelectBox`). Picking one saves it.
#[allow(clippy::too_many_arguments)]
pub fn enum_select(
    id: SharedString,
    name: &str,
    current: &str,
    options: &[String],
    open: bool,
    enabled: bool,
    key: Key,
    cx: &mut Context<InstanceSettingsPage>,
) -> AnyElement {
    let toggle = key.clone();
    let current_text = SharedString::from(current.to_string());
    let button = select_button(
        sub(&id, "select"),
        div().max_w(px(220.)).truncate().child(current_text.clone()),
    )
    .aria_label(SharedString::from(name.to_string()))
    .aria_value(current_text)
    .min_w(px(160.))
    .map(|this| {
        if enabled {
            let page = cx.entity().downgrade();
            let expand = toggle.clone();
            this.expandable(open, move |_, cx| {
                page.update(cx, |this, cx| {
                    this.open_menu = if open { None } else { Some(expand.clone()) };
                    cx.notify();
                })
                .ok();
            })
            .on_press(cx.listener(move |this, _, _, cx| {
                this.open_menu = if this.open_menu.as_ref() == Some(&toggle) {
                    None
                } else {
                    Some(toggle.clone())
                };
                cx.notify();
            }))
        } else {
            this.opacity(0.5).cursor_default()
        }
    });
    let page = cx.entity().downgrade();
    div()
        .flex()
        .flex_col()
        .items_end()
        .child(button)
        .when(open && enabled, |this| {
            let mut panel = popover_panel(sub(&id, "list"))
                .min_w(px(200.))
                .max_h(px(320.))
                .overflow_y_scroll();
            for (index, option) in options.iter().enumerate() {
                let picked = key.clone();
                let value = option.clone();
                panel = panel.child(
                    ListRow::new(sub(&id, &format!("option-{index}")), option.clone())
                        .selected(option == current)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.change(picked.clone(), ConfigurableValue::Enum(value.clone()), cx)
                        })),
                );
            }
            this.child(popover(sub(&id, "menu"), panel, move |_, cx| {
                page.update(cx, |this, cx| {
                    this.open_menu = None;
                    cx.notify();
                })
                .ok();
            }))
        })
        .into_any_element()
}

/// The MOTD as `server.properties` holds it, as players see it: `\uXXXX` escapes and
/// `\n` turned into their characters (`convertUnicode`).
pub fn unescape_motd(motd: &str) -> String {
    let mut out = String::with_capacity(motd.len());
    let mut chars = motd.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            Some('n') => {
                chars.next();
                out.push('\n');
            }
            Some('u') => {
                let hex: String = chars.clone().skip(1).take(4).collect();
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    Some(decoded) if hex.len() == 4 => {
                        for _ in 0..5 {
                            chars.next();
                        }
                        out.push(decoded);
                    }
                    _ => out.push('\\'),
                }
            }
            _ => out.push('\\'),
        }
    }
    out
}

/// How the MOTD looks in the server list: its colours and formatting on a dark panel
/// (the dashboard's `MOTDRender`, on dirt).
pub fn motd_preview(motd: &str) -> AnyElement {
    let motd = unescape_motd(motd);
    div()
        .px(px(10.))
        .py(px(8.))
        .rounded(radius::MD)
        .bg(colors::GRAY_900)
        .border_1()
        .border_color(colors::GRAY_700)
        .font_family(fonts::mono())
        .text_size(text::MEDIUM)
        .text_color(colors::GRAY_400)
        .children(
            motd.split('\n')
                .take(2)
                .map(|line| div().child(RichText::minecraft(line.to_string())))
                .collect::<Vec<_>>(),
        )
        .into_any_element()
}

/// A child's element id: `id` and a part, such as "setting-x-save".
fn sub(id: &SharedString, part: &str) -> SharedString {
    SharedString::from(format!("{id}-{part}"))
}
