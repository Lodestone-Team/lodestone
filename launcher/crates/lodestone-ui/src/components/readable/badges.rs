//! Small badges: a mod's version ([`VersionTag`]), a state ([`StatusBadge`]), a count
//! ([`CountBadge`]), why something was made ([`ReasonTag`]), and the placeholder name
//! of something nobody named ([`UntitledName`]).
//!
//! Colour means status and nothing else: only the status badge is coloured. Tags
//! and counts are grays, so they read apart from `settings::tag`'s coloured outline.

use std::{panic::Location, rc::Rc};

use gpui::{
    App, ClickEvent, ElementId, IntoElement, KeyDownEvent, RenderOnce, SharedString, Window, div,
    prelude::*,
};

use super::{auto_id, key_value};
use crate::{
    components::{
        Icon,
        a11y::{Accessible, Role},
        focus::KeyboardFocus,
        text_tooltip,
    },
    i18n::{FactMsg, format, t},
    theme::{self, Tone, colors, fonts, ink, outline, pill, px, radius, text, weights},
    values::truncate::{MAX_SHOWN, sanitize},
};

/// Versions longer than this are cut in the middle and have a tooltip: what fits a
/// tag's 160px at 11px JetBrains Mono.
pub const VERSION_TOOLTIP_OVER: usize = 22;

/// How a [`VersionTag`] reads beside another.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VersionTone {
    #[default]
    Normal,
    /// The version something changes from: fainter.
    From,
    /// The version it changes to: the main ink.
    To,
}

/// A mod's, a plugin's or a modpack's version, verbatim, after its name:
/// JetBrains Mono 11px on gray-900 with a thin outline, at most 160px wide and cut
/// in the middle, so `mc1.21.1-0.6.0-fabric` keeps both ends. Never for the game or
/// a loader, which read as one phrase ([`ProductPhrase`](super::ProductPhrase)).
#[derive(Clone, Debug, IntoElement)]
pub struct VersionTag {
    id: ElementId,
    version: SharedString,
    tone: VersionTone,
}

impl VersionTag {
    #[track_caller]
    pub fn new(version: impl Into<SharedString>) -> Self {
        let version: SharedString = sanitize(&version.into(), 256).into();
        Self {
            id: auto_id(Location::caller(), &version),
            version,
            tone: VersionTone::Normal,
        }
    }

    pub fn tone(mut self, tone: VersionTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    pub fn version(&self) -> &SharedString {
        &self.version
    }

    /// Whether it is long enough to be cut, and so has a tooltip.
    pub fn has_tooltip(&self) -> bool {
        self.version.chars().count() > VERSION_TOOLTIP_OVER
    }
}

impl RenderOnce for VersionTag {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let color = match self.tone {
            VersionTone::Normal => ink::secondary(),
            VersionTone::From => ink::faint(),
            VersionTone::To => ink::text(),
        };
        let label = key_value(t(FactMsg::Version), &self.version);
        let tooltip = self.has_tooltip().then(|| self.version.clone());
        div()
            .id(self.id.clone())
            .accessible_id(Role::Label, &self.id, label)
            .when_some(tooltip, |this, version| this.tooltip(text_tooltip(version)))
            .flex()
            .flex_none()
            .items_center()
            .h(px(18.))
            .max_w(px(160.))
            .px(px(5.))
            .rounded(radius::DEFAULT)
            .bg(colors::GRAY_900)
            .border_1()
            .border_color(outline())
            .font_family(fonts::mono())
            .text_size(theme::text_size(text::VERSION))
            .font_weight(weights::MEDIUM.into())
            .text_color(color)
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis_middle()
                    .child(self.version),
            )
    }
}

/// A state, always drawn the same way: "Online", "Running", "Failed". A pill in the
/// tone's colours (10px, weight 550), with an optional icon.
#[derive(Clone, Debug, IntoElement)]
pub struct StatusBadge {
    id: ElementId,
    label: SharedString,
    tone: Tone,
    icon: Option<Icon>,
}

impl StatusBadge {
    #[track_caller]
    pub fn new(label: impl Into<SharedString>, tone: Tone) -> Self {
        let label = label.into();
        Self {
            id: auto_id(Location::caller(), &label),
            label,
            tone,
            icon: None,
        }
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    pub fn label(&self) -> &SharedString {
        &self.label
    }

    pub fn tone(&self) -> Tone {
        self.tone
    }
}

impl RenderOnce for StatusBadge {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let fg = pill::fg(self.tone);
        div()
            .id(self.id.clone())
            .accessible_id(
                Role::Label,
                &self.id,
                key_value(t(FactMsg::Status), &self.label),
            )
            .flex()
            .flex_none()
            .items_center()
            .gap(px(4.))
            .h(px(18.))
            .px(px(6.))
            .rounded_full()
            .bg(pill::bg(self.tone))
            .text_size(theme::text_size(text::CAPTION))
            .font_weight(weights::BOLD.into())
            .text_color(fg)
            .whitespace_nowrap()
            .when_some(self.icon, |this, icon| this.child(icon.render(px(10.), fg)))
            .child(self.label)
    }
}

/// A count after a label, instead of "(7)": a small gray pill, blue when its tab or
/// row is chosen. Large counts are compacted ("12.3k") with the exact number in a
/// tooltip.
#[derive(Clone, Debug, IntoElement)]
pub struct CountBadge {
    id: ElementId,
    count: u64,
    active: bool,
}

impl CountBadge {
    #[track_caller]
    pub fn new(count: u64) -> Self {
        Self {
            id: auto_id(Location::caller(), &count.to_string()),
            count,
            active: false,
        }
    }

    /// On the chosen tab or row.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// The shown text, and the exact number when that is compacted.
    pub fn texts(&self) -> (String, Option<String>) {
        let shown = format::compact(self.count);
        let exact = format::number(self.count);
        let tooltip = (shown != exact).then_some(exact);
        (shown, tooltip)
    }
}

impl RenderOnce for CountBadge {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (shown, exact) = self.texts();
        let label = exact.clone().unwrap_or_else(|| shown.clone());
        let (bg, fg) = if self.active {
            (pill::bg(Tone::Info), pill::fg(Tone::Info))
        } else {
            (colors::GRAY_700, ink::secondary())
        };
        div()
            .id(self.id.clone())
            .accessible_id(Role::Label, &self.id, label)
            .when_some(exact, |this, exact| this.tooltip(text_tooltip(exact)))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .min_w(px(16.))
            .h(px(16.))
            .px(px(5.))
            .rounded_full()
            .bg(bg)
            .text_size(theme::text_size(text::CAPTION))
            .font_weight(weights::BOLD.into())
            .font_features(fonts::tabular())
            .text_color(fg)
            .whitespace_nowrap()
            .child(shown)
    }
}

/// Why something was made, on a snapshot's or a copy's second line: "Before update",
/// "Copy of". A gray tag (10px, weight 550, ink-secondary on gray-700), apart from
/// the coloured status badges.
#[derive(Clone, Debug, IntoElement)]
pub struct ReasonTag {
    label: SharedString,
    icon: Option<Icon>,
}

impl ReasonTag {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            icon: None,
        }
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn label(&self) -> &SharedString {
        &self.label
    }
}

impl RenderOnce for ReasonTag {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(4.))
            .h(px(16.))
            .px(px(6.))
            .py(px(1.))
            .rounded(radius::MD)
            // gray-700: the light theme draws gray-750 as the cards it sits on
            .bg(colors::GRAY_700)
            .text_size(theme::text_size(text::CAPTION))
            .font_weight(weights::BOLD.into())
            .text_color(ink::secondary())
            .whitespace_nowrap()
            .when_some(self.icon, |this, icon| {
                this.child(icon.render(px(10.), ink::secondary()))
            })
            .child(self.label)
    }
}

type RenameHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The name of something nobody named, such as "Untitled snapshot": 14px, weight
/// 450, ink-muted, so it reads as a placeholder rather than a name. Double-clicking
/// it, or Enter or F2 while it has focus, renames it.
#[derive(IntoElement)]
pub struct UntitledName {
    id: ElementId,
    text: SharedString,
    on_rename: Option<RenameHandler>,
}

impl UntitledName {
    /// `text` is the translated placeholder (`ValueMsg::UntitledSnapshot`).
    #[track_caller]
    pub fn new(text: impl Into<SharedString>) -> Self {
        let text: SharedString = sanitize(&text.into(), MAX_SHOWN).into();
        Self {
            id: auto_id(Location::caller(), &text),
            text,
            on_rename: None,
        }
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    pub fn on_rename(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_rename = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for UntitledName {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let rename = self.on_rename;
        div()
            .id(self.id.clone())
            .accessible_id(Role::Label, &self.id, self.text.clone())
            .min_w_0()
            .truncate()
            .text_size(theme::text_size(text::MEDIUM))
            .font_weight(weights::MEDIUM.into())
            .text_color(ink::muted())
            .when_some(rename, |this, rename| {
                let on_key = rename.clone();
                this.keyboard_focus()
                    .on_click(move |event: &ClickEvent, window, cx| {
                        if event.click_count() >= 2 {
                            rename(window, cx);
                        }
                    })
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "f2") {
                            cx.stop_propagation();
                            on_key(window, cx);
                        }
                    })
            })
            .child(self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_versions_have_a_tooltip() {
        assert!(!VersionTag::new("0.116.17+1.21.1").has_tooltip());
        assert!(VersionTag::new("mc1.21.1-0.6.0-fabric-build.42+rev").has_tooltip());
        // a version can't break a line or flip around
        assert_eq!(VersionTag::new("1.0\n\u{202E}x").version(), "1.0 x");
    }
}
