//! A fact: a value with its key, the unit [`FactLine`](super::FactLine)s,
//! [`FactList`](super::FactList)s and cards are made of.
//!
//! A value that isn't self-explanatory gets a key: a short translated label
//! ("Last played") or, where room is tight, a 12px icon whose tooltip and accessible
//! name give "key: value". A size with its unit, or a version tag right after its
//! name, needs none ([`Fact::bare`]). Key and value are two elements, never one
//! translated string, and code never joins facts with separator characters.

use gpui::{AnyElement, IntoElement, SharedString, div, prelude::*};

use super::{
    DiffValue, Num, ObjectChip, PathText, ProductPhrase, StatusBadge, VersionTag, key_value,
};
use crate::{
    components::{
        Icon, RichText,
        a11y::{Accessible, Role as A11yRole},
    },
    theme::{self, colors, fonts, ink, px, text, weights},
    values::{
        Role,
        truncate::{MAX_SHOWN, sanitize},
    },
};

/// What names a fact's value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FactKey {
    /// A translated label: "Added", "Last played".
    Text(SharedString),
    /// An icon in place of the label, where room is tight; the label is its tooltip
    /// and accessible name.
    Icon(Icon, SharedString),
    /// No key shown, for a self-explanatory value: a size with its unit, a version
    /// tag after its name. The label, if any, is for screen readers only.
    Bare(Option<SharedString>),
}

impl FactKey {
    /// The key's label, if it has one.
    pub fn label(&self) -> Option<&SharedString> {
        match self {
            FactKey::Text(label) | FactKey::Icon(_, label) => Some(label),
            FactKey::Bare(label) => label.as_ref(),
        }
    }
}

/// A fact's value, in its role.
pub enum FactValue {
    /// Text in a role: plain, an object's name, a time, an exact value.
    Text(SharedString, Role),
    Num(Num),
    Version(VersionTag),
    Product(ProductPhrase),
    Path(PathText),
    Status(StatusBadge),
    Chip(ObjectChip),
    Diff(DiffValue),
    /// Anything else, with its plain text for tooltips and screen readers.
    Element(AnyElement, SharedString),
}

impl FactValue {
    /// Plain text.
    pub fn text(text: impl Into<SharedString>) -> Self {
        Self::role(text, Role::Plain)
    }

    /// An object's name.
    pub fn object(name: impl Into<SharedString>) -> Self {
        Self::role(name, Role::Object)
    }

    /// A time or a duration, already formatted.
    pub fn time(time: impl Into<SharedString>) -> Self {
        Self::role(time, Role::Time)
    }

    /// A short exact value: a port, an id.
    pub fn mono(value: impl Into<SharedString>) -> Self {
        Self::role(value, Role::Exact)
    }

    fn role(text: impl Into<SharedString>, role: Role) -> Self {
        FactValue::Text(sanitize(&text.into(), MAX_SHOWN).into(), role)
    }

    /// Any element, with its plain text.
    pub fn element(element: impl IntoElement, plain: impl Into<SharedString>) -> Self {
        FactValue::Element(element.into_any_element(), plain.into())
    }

    /// The value as plain text, for tooltips and accessible names.
    pub fn plain(&self) -> SharedString {
        match self {
            FactValue::Text(text, _) => text.clone(),
            FactValue::Num(num) => num.plain().into(),
            FactValue::Version(tag) => tag.version().clone(),
            FactValue::Product(phrase) => phrase.plain().into(),
            FactValue::Path(path) => path.full().clone(),
            FactValue::Status(badge) => badge.label().clone(),
            FactValue::Chip(chip) => chip.name().clone(),
            FactValue::Diff(diff) => diff.plain().into(),
            FactValue::Element(_, plain) => plain.clone(),
        }
    }

    /// The value as drawn in `place`.
    pub(super) fn render(self, place: Place) -> AnyElement {
        match self {
            FactValue::Text(text, role) => role_text(text, role, place).into_any_element(),
            FactValue::Num(num) => match place {
                Place::Row => num.into_any_element(),
                Place::Strip | Place::List => num.inherit_color().into_any_element(),
            },
            FactValue::Version(tag) => tag.into_any_element(),
            FactValue::Product(phrase) => phrase.into_any_element(),
            FactValue::Path(path) => path.into_any_element(),
            FactValue::Status(badge) => badge.into_any_element(),
            FactValue::Chip(chip) => chip.into_any_element(),
            FactValue::Diff(diff) => diff.into_any_element(),
            FactValue::Element(element, _) => element,
        }
    }
}

/// Where a fact is drawn: a row's 12px line, a header's key-over-value strip, or a
/// two-column fact table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Place {
    Row,
    Strip,
    List,
}

/// Text in `role`, as `place` draws it. In a row, plain text is ink-secondary; in a
/// strip or a table, values are in the main ink. Times are muted and exact values
/// mono everywhere.
fn role_text(text: SharedString, role: Role, place: Place) -> gpui::Div {
    let main = match place {
        Place::Row => ink::secondary(),
        Place::Strip | Place::List => ink::text(),
    };
    let base = div().flex_none().whitespace_nowrap();
    let styled = match role {
        Role::Object => base
            .font_weight(weights::MEDIUMBOLD.into())
            .text_color(ink::text()),
        Role::Version | Role::Exact => base
            .font_family(fonts::mono())
            .text_size(theme::text_size(text::SMALL))
            .text_color(ink::secondary()),
        Role::Number => base.font_features(fonts::tabular()).text_color(main),
        Role::Time => base.text_color(ink::muted()),
        Role::Key | Role::Unit => base.text_color(ink::faint()),
        Role::Plain => base.text_color(main),
    };
    styled.child(RichText::new(text))
}

impl From<Num> for FactValue {
    fn from(num: Num) -> Self {
        FactValue::Num(num)
    }
}

impl From<VersionTag> for FactValue {
    fn from(tag: VersionTag) -> Self {
        FactValue::Version(tag)
    }
}

impl From<ProductPhrase> for FactValue {
    fn from(phrase: ProductPhrase) -> Self {
        FactValue::Product(phrase)
    }
}

impl From<PathText> for FactValue {
    fn from(path: PathText) -> Self {
        FactValue::Path(path)
    }
}

impl From<StatusBadge> for FactValue {
    fn from(badge: StatusBadge) -> Self {
        FactValue::Status(badge)
    }
}

impl From<ObjectChip> for FactValue {
    fn from(chip: ObjectChip) -> Self {
        FactValue::Chip(chip)
    }
}

impl From<DiffValue> for FactValue {
    fn from(diff: DiffValue) -> Self {
        FactValue::Diff(diff)
    }
}

impl From<&'static str> for FactValue {
    fn from(text: &'static str) -> Self {
        FactValue::text(text)
    }
}

impl From<String> for FactValue {
    fn from(text: String) -> Self {
        FactValue::text(text)
    }
}

impl From<SharedString> for FactValue {
    fn from(text: SharedString) -> Self {
        FactValue::text(text)
    }
}

/// A value and its key; see the module docs.
pub struct Fact {
    pub(super) key: FactKey,
    pub(super) value: FactValue,
    /// Lower is kept longer when a line runs out of room; the fact's place by
    /// default.
    pub(super) priority: Option<u8>,
    /// Cut in the middle before any fact is left out, and at most half the line: a
    /// file name.
    pub(super) shrink: bool,
}

impl Fact {
    /// A value under a label: `Fact::new(t(FactMsg::Added), FactValue::time(when))`.
    pub fn new(key: impl Into<SharedString>, value: impl Into<FactValue>) -> Self {
        Self::with_key(FactKey::Text(key.into()), value)
    }

    /// A value after an icon, whose tooltip and accessible name give the label.
    pub fn icon(icon: Icon, key: impl Into<SharedString>, value: impl Into<FactValue>) -> Self {
        Self::with_key(FactKey::Icon(icon, key.into()), value)
    }

    /// A self-explanatory value, with no key shown: a size with its unit.
    pub fn bare(value: impl Into<FactValue>) -> Self {
        Self::with_key(FactKey::Bare(None), value)
    }

    /// A self-explanatory value whose key screen readers still hear.
    pub fn bare_named(key: impl Into<SharedString>, value: impl Into<FactValue>) -> Self {
        Self::with_key(FactKey::Bare(Some(key.into())), value)
    }

    fn with_key(key: FactKey, value: impl Into<FactValue>) -> Self {
        Self {
            key,
            value: value.into(),
            priority: None,
            shrink: false,
        }
    }

    /// Kept longer than facts with a higher number when the line is short.
    pub fn priority(mut self, priority: u8) -> Self {
        self.priority = Some(priority);
        self
    }

    /// Cut in the middle before anything is left out (a file name).
    pub fn shrink(mut self) -> Self {
        self.shrink = true;
        self
    }

    pub fn key(&self) -> &FactKey {
        &self.key
    }

    /// The value as plain text.
    pub fn plain(&self) -> SharedString {
        self.value.plain()
    }

    /// "key: value", or the value alone when it has no key.
    pub fn label(&self) -> SharedString {
        match self.key.label() {
            Some(key) => key_value(key, &self.value.plain()),
            None => self.value.plain(),
        }
    }

    /// The fact in a row: the key (label or icon) then the value, 4px apart.
    pub(super) fn render_in_row(self, index: usize) -> AnyElement {
        let label = self.label();
        let shrink = self.shrink;
        let id = ("fact", index);
        let key = match &self.key {
            FactKey::Text(key) => Some(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .font_weight(weights::MEDIUM.into())
                    .text_color(ink::faint())
                    .child(key.clone())
                    .into_any_element(),
            ),
            FactKey::Icon(icon, _) => {
                Some(icon.render(px(12.), colors::GRAY_500).into_any_element())
            }
            FactKey::Bare(_) => None,
        };
        let value = self.value.render(Place::Row);
        div()
            .id(id)
            .accessible(A11yRole::Label, format!("fact-{index}"), label)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.))
            // a shrinking fact fills the width it is laid out in, and its value
            // gives way; the others are as wide as they need
            .when(shrink, |this| this.w_full().min_w_0())
            .when(!shrink, |this| this.flex_none())
            .children(key)
            .child(
                div()
                    .flex()
                    .items_center()
                    .when(shrink, |this| this.flex_1().min_w_0())
                    .child(value),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_read_key_colon_value() {
        let fact = Fact::new("Size", FactValue::text("35 MB"));
        assert_eq!(fact.label(), "Size: 35 MB");
        let fact = Fact::icon(Icon::Clock, "Last played", FactValue::time("2 hours ago"));
        assert_eq!(fact.label(), "Last played: 2 hours ago");
        assert_eq!(Fact::bare("1.2 GB").label(), "1.2 GB");
        assert_eq!(Fact::bare_named("Size", "1.2 GB").label(), "Size: 1.2 GB");
        // values are made safe
        assert_eq!(FactValue::object("a\nb").plain(), "a b");
    }
}
