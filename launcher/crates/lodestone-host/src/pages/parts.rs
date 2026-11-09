//! What the Settings pages (General, Users, Profile) are built from, beyond
//! `lodestone_ui::components::settings`: a text setting that saves on Enter, the
//! confirm dialog, notices, form fields, the user's avatar, and the "Saved" toast.
//! Pages keep their dialogs in a `DialogSlot` (so they fade out when closed) and draw
//! them over the whole window with `window_layer`. Work package D.

use std::rc::Rc;

use gpui::{
    AnyElement, App, AppContext as _, ClipboardItem, Context, Entity, EventEmitter, FocusHandle,
    IntoElement, KeyDownEvent, ParentElement, Render, SharedString, Styled, Window, div,
    prelude::*,
};
use lodestone_ui::{
    components::{
        Button, ButtonSize, ButtonVariant, Dialog, TextInput, Toast, ToastLevel,
        settings::SettingRow,
    },
    theme::{Color, colors, fonts, ink, leading, px, radius, text, weights},
};

use crate::{
    i18n::{CommonMsg, t},
    state::HostState,
};

/// How a toast looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Success,
    Warning,
    Error,
}

/// Shows a toast over the window.
pub fn toast(
    state: &Entity<HostState>,
    level: Level,
    message: impl Into<SharedString>,
    cx: &mut App,
) {
    let level = match level {
        Level::Info => ToastLevel::Info,
        Level::Success => ToastLevel::Success,
        Level::Warning => ToastLevel::Warning,
        Level::Error => ToastLevel::Error,
    };
    let toast = Toast::new(level, message);
    state.update(cx, |state, cx| state.toast(toast, cx));
}

/// The small "Saved" toast after a setting changed (`savedToast.ts`): one at a time,
/// for 1.5 s.
pub fn saved(state: &Entity<HostState>, cx: &mut App) {
    let toast = Toast::success(t(CommonMsg::Saved))
        .key("settings-saved")
        .duration(std::time::Duration::from_millis(1500));
    state.update(cx, |state, cx| state.toast(toast, cx));
}

/// Copies `text`. Never put what was copied in a toast: it may be a key.
pub fn copy(text: &str, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
}

/// The dashboard's `ConfirmDialog`: a title, a sentence, and the action with Cancel.
pub struct Confirm {
    pub id: &'static str,
    pub title: SharedString,
    pub body: SharedString,
    pub action: SharedString,
    pub danger: bool,
    pub busy: bool,
}

impl Confirm {
    pub fn render(
        self,
        focus: FocusHandle,
        on_confirm: impl Fn(&mut Window, &mut App) + 'static,
        on_close: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Dialog {
        let on_close = Rc::new(on_close);
        let cancel = on_close.clone();
        Dialog::new(self.id, self.title, focus)
            .on_close(move |window, cx| on_close(window, cx))
            .child(paragraph(self.body))
            .footer(
                Button::new((self.id, 1usize), self.action)
                    .variant(if self.danger {
                        ButtonVariant::Danger
                    } else {
                        ButtonVariant::Primary
                    })
                    .loading(self.busy)
                    .on_click(move |_, window, cx| on_confirm(window, cx)),
            )
            .footer(
                Button::new((self.id, 2usize), t(CommonMsg::Cancel))
                    .on_click(move |_, window, cx| cancel(window, cx)),
            )
    }
}

/// A sentence of muted text.
pub fn paragraph(text_: impl Into<SharedString>) -> AnyElement {
    div()
        .text_size(text::MEDIUM)
        .line_height(lodestone_ui::theme::line_height(
            text::MEDIUM,
            leading::NORMAL,
        ))
        .text_color(ink::secondary())
        .child(text_.into())
        .into_any_element()
}

/// A notice in a coloured box: a warning (`WarningAlert`) or an error.
pub fn notice(text_: impl Into<SharedString>, color: Color) -> AnyElement {
    div()
        .px(px(12.))
        .py(px(8.))
        .rounded(radius::LG)
        .border_1()
        .border_color(color.opacity(40))
        .bg(color.opacity(10))
        .text_size(text::MEDIUM)
        .line_height(lodestone_ui::theme::line_height(
            text::MEDIUM,
            leading::NORMAL,
        ))
        .text_color(color)
        .child(text_.into())
        .into_any_element()
}

pub fn warning(text_: impl Into<SharedString>) -> AnyElement {
    notice(text_, colors::YELLOW)
}

pub fn error(text_: impl Into<SharedString>) -> AnyElement {
    notice(text_, colors::RED_200)
}

/// A card's only row, saying it is empty (`SettingsEmptyRow`).
pub fn empty_row(text_: impl Into<SharedString>) -> AnyElement {
    div()
        .px_4()
        .py_3()
        .text_size(text::MEDIUM)
        .text_color(ink::muted())
        .child(text_.into())
        .into_any_element()
}

/// A labelled text field in a form.
pub fn field(label: impl Into<SharedString>, input: &Entity<TextInput>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(
            div()
                .text_size(text::MEDIUM)
                .font_weight(weights::MEDIUM.face())
                .text_color(ink::secondary())
                .child(label.into()),
        )
        .child(input.clone())
        .into_any_element()
}

/// A text field for a form.
pub fn input(label: impl Into<SharedString>, masked: bool, cx: &mut App) -> Entity<TextInput> {
    let label: SharedString = label.into();
    cx.new(|cx| {
        let input = TextInput::new(cx).with_label(label);
        if masked { input.with_masked() } else { input }
    })
}

/// A value in monospace, such as a fingerprint or a UUID.
pub fn mono(text_: impl Into<SharedString>) -> AnyElement {
    div()
        .font_family(fonts::mono())
        .text_size(text::SMALL)
        .text_color(ink::muted())
        .child(text_.into())
        .into_any_element()
}

/// The palette of the avatars, after `boring-avatars`' "beam" colours.
const AVATAR_COLORS: [u32; 5] = [0x92A1C6, 0x146A7C, 0xF0AB3D, 0xC271B4, 0xC20D90];

/// A user's avatar: a coloured circle with the first letter of their name, the colour
/// picked from their id, so it stays the same everywhere (the dashboard's `Avatar`).
pub fn avatar(uid: &str, name: &str, size: f32) -> AnyElement {
    let hash = uid
        .bytes()
        .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    let color = Color::rgb(AVATAR_COLORS[hash as usize % AVATAR_COLORS.len()]);
    let initial: String = name
        .chars()
        .next()
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default();
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded_full()
        .bg(color)
        .text_size(px(size * 0.45))
        .font_weight(weights::BOLD.face())
        .text_color(colors::WHITE)
        .child(initial)
        .into_any_element()
}

/// What a [`TextSetting`] tells its page.
pub enum TextSettingEvent {
    /// Enter or Save, with the trimmed text that passed the check: the page saves it
    /// and calls [`TextSetting::finish`].
    Submit(String),
}

/// Checks a typed value: why it can't be saved, or `None`.
type Check = Box<dyn Fn(&str) -> Option<String>>;

/// A setting typed as text, after the dashboard's `InputBox`: Enter (or Save) checks
/// and saves it, Escape (or Reset) puts back the saved value.
pub struct TextSetting {
    id: SharedString,
    label: SharedString,
    hint: Option<SharedString>,
    input: Entity<TextInput>,
    saved: String,
    error: Option<String>,
    saving: bool,
    disabled: bool,
    check: Check,
}

impl EventEmitter<TextSettingEvent> for TextSetting {}

impl TextSetting {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        value: &str,
        check: impl Fn(&str) -> Option<String> + 'static,
        cx: &mut Context<Self>,
    ) -> Self {
        let label: SharedString = label.into();
        let id: SharedString = id.into();
        let value_shared = SharedString::from(value.to_string());
        let (label_, id_) = (label.clone(), id.clone());
        let input = cx.new(|cx| {
            TextInput::new(cx)
                .with_label(label_)
                .with_automation_id(id_)
                .with_text(value_shared)
        });
        cx.observe(&input, |_, _, cx| cx.notify()).detach();
        Self {
            id,
            label,
            hint: None,
            input,
            saved: value.to_string(),
            error: None,
            saving: false,
            disabled: false,
            check: Box::new(check),
        }
    }

    pub fn with_hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn with_placeholder(self, placeholder: impl Into<SharedString>, cx: &mut App) -> Self {
        let placeholder: SharedString = placeholder.into();
        self.input
            .update(cx, |input, cx| input.set_placeholder(placeholder, cx));
        self
    }

    /// Read-only for users who may not change it.
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled != disabled {
            self.disabled = disabled;
            let text = self.input.read(cx).text().clone();
            self.input = cx.new(|cx| {
                TextInput::new(cx)
                    .with_label(self.label.clone())
                    .with_automation_id(self.id.clone())
                    .with_text(text)
                    .with_disabled(disabled)
            });
            cx.observe(&self.input, |_, _, cx| cx.notify()).detach();
            cx.notify();
        }
    }

    /// The value the core has now, unless the user is editing it.
    pub fn set_value(&mut self, value: &str, cx: &mut Context<Self>) {
        if self.saving || self.saved == value {
            return;
        }
        let editing = self.input.read(cx).text().as_ref() != self.saved;
        self.saved = value.to_string();
        if !editing {
            let value = SharedString::from(value.to_string());
            self.input.update(cx, |input, cx| input.set_text(value, cx));
        }
        cx.notify();
    }

    fn changed(&self, cx: &App) -> bool {
        self.input.read(cx).text().trim() != self.saved
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.saving || self.disabled || !self.changed(cx) {
            return;
        }
        let text = self.input.read(cx).text().trim().to_string();
        self.error = (self.check)(&text);
        if self.error.is_none() {
            self.saving = true;
            cx.emit(TextSettingEvent::Submit(text));
        }
        cx.notify();
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        let saved = SharedString::from(self.saved.clone());
        self.input.update(cx, |input, cx| input.set_text(saved, cx));
        self.error = None;
        cx.notify();
    }

    /// The page saved it (`Ok` with the value now saved), or couldn't (`Err`).
    pub fn finish(&mut self, result: Result<String, String>, cx: &mut Context<Self>) {
        self.saving = false;
        match result {
            Ok(value) => {
                self.saved = value;
                self.error = None;
            }
            Err(err) => self.error = Some(err),
        }
        cx.notify();
    }
}

impl Render for TextSetting {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let changed = self.changed(cx);
        let id = self.id.clone();
        div()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                match event.keystroke.key.as_str() {
                    "enter" => this.submit(cx),
                    "escape" => this.reset(cx),
                    _ => {}
                }
            }))
            .child(
                SettingRow::new(self.label.clone())
                    .key(self.id.clone())
                    .hint_opt(self.hint.clone())
                    .child(div().w(px(240.)).child(self.input.clone()))
                    .when(changed && !self.disabled, |row| {
                        row.child(
                            Button::new((id.clone(), 1usize), t(CommonMsg::Save))
                                .variant(ButtonVariant::Primary)
                                .size(ButtonSize::Slim)
                                .loading(self.saving)
                                .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
                        )
                        .child(
                            Button::new((id.clone(), 2usize), t(CommonMsg::Reset))
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Slim)
                                .on_click(cx.listener(|this, _, _, cx| this.reset(cx))),
                        )
                    })
                    .note_opt(
                        self.error
                            .clone()
                            .map(|error| (SharedString::from(error), colors::RED_200)),
                    ),
            )
    }
}

/// Runs `work` (a request to the core) in the background, then `done` with its result
/// on the view, if it is still there.
pub fn spawn<V: 'static, T: Send + 'static>(
    cx: &mut Context<V>,
    work: impl std::future::Future<Output = T> + Send + 'static,
    done: impl FnOnce(&mut V, T, &mut Context<V>) + 'static,
) {
    let task = cx.background_spawn(work);
    cx.spawn(async move |this, cx| {
        let result = task.await;
        this.update(cx, |this, cx| {
            done(this, result, cx);
            cx.notify();
        })
        .ok();
    })
    .detach();
}

/// A date from unix seconds, as "Mar 12" (with the year when it isn't this year's).
pub fn date(seconds: i64) -> String {
    use chrono::{Datelike as _, Local, TimeZone as _};
    let Some(time) = Local.timestamp_opt(seconds, 0).single() else {
        return String::new();
    };
    let day = time.date_naive();
    lodestone_ui::i18n::format::short_date(day, day.year() != Local::now().year())
}

/// How long ago unix second `seconds` was: "Just now", "5 minutes ago", "Yesterday",
/// "3 days ago", or the date.
pub fn ago(seconds: i64) -> String {
    use crate::i18n::{FormatMsg, FormatPlural, trn};
    let now = chrono::Local::now().timestamp();
    let elapsed = (now - seconds).max(0) as u64;
    match elapsed {
        0..60 => t(FormatMsg::JustNow).to_string(),
        60..3600 => trn!(FormatPlural::MinutesAgo, elapsed / 60),
        3600..86_400 => trn!(FormatPlural::HoursAgo, elapsed / 3600),
        86_400..172_800 => t(FormatMsg::Yesterday).to_string(),
        _ if elapsed < 7 * 86_400 => trn!(FormatPlural::DaysAgo, elapsed / 86_400),
        _ => date(seconds),
    }
}

/// The address of the core `state` is signed in to, as `display_address` writes it.
pub fn current_address(state: &Entity<HostState>, cx: &App) -> Option<String> {
    Some(launcher_core::cores::client::display_address(
        state.read(cx).admin.core().base(),
    ))
}

/// A page's sections, one under the other, as wide as the settings column.
pub fn page(children: impl IntoIterator<Item = AnyElement>) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(32.))
        .w_full()
        .max_w(lodestone_ui::components::settings::content_width())
        .pb(px(32.))
        .children(children)
}
