//! Comparisons, instead of sentences: a value changing from one to another
//! ([`DiffValue`], a FactList row's value) and two things side by side with what
//! differs marked ([`CompareTable`]).

use std::panic::Location;

use gpui::{App, ElementId, IntoElement, RenderOnce, SharedString, Window, div, prelude::*};

use super::{FactValue, Place, VersionTag, VersionTone, auto_id, key_value};
use crate::{
    components::{
        Icon,
        a11y::{Accessible, Role, automation_id},
    },
    i18n::{ValueMsg, interpolate, locale, t},
    theme::{self, Tone, colors, fonts, ink, pill, px, radius, text, weights},
    values::truncate::sanitize,
};

#[derive(Clone, Debug)]
enum Side {
    Version(SharedString),
    Text(SharedString),
}

impl Side {
    fn text(&self) -> &SharedString {
        match self {
            Side::Version(text) | Side::Text(text) => text,
        }
    }
}

/// A value changing: the old one muted (a fainter version tag for versions), an
/// arrow icon (mirrored right to left, never a translated "→"), then the new one in
/// the main ink. Screen readers hear "Changes from 6.1.0 to 6.2.0".
#[derive(Clone, Debug, IntoElement)]
pub struct DiffValue {
    id: ElementId,
    from: Side,
    to: Side,
}

impl DiffValue {
    /// From one version to another, as two tags.
    #[track_caller]
    pub fn versions(from: impl Into<SharedString>, to: impl Into<SharedString>) -> Self {
        Self::new(
            Side::Version(clean(from)),
            Side::Version(clean(to)),
            Location::caller(),
        )
    }

    /// From one text to another.
    #[track_caller]
    pub fn text(from: impl Into<SharedString>, to: impl Into<SharedString>) -> Self {
        Self::new(
            Side::Text(clean(from)),
            Side::Text(clean(to)),
            Location::caller(),
        )
    }

    fn new(from: Side, to: Side, caller: &'static Location<'static>) -> Self {
        Self {
            id: auto_id(caller, &format!("{}>{}", from.text(), to.text())),
            from,
            to,
        }
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// "6.1.0 → 6.2.0", for tooltips.
    pub fn plain(&self) -> String {
        if locale().is_rtl() {
            format!("{} ← {}", self.to.text(), self.from.text())
        } else {
            format!("{} → {}", self.from.text(), self.to.text())
        }
    }

    /// "Changes from 6.1.0 to 6.2.0", its accessible name.
    pub fn label(&self) -> String {
        interpolate(
            t(ValueMsg::FromTo),
            &[
                ("from", self.from.text() as &dyn std::fmt::Display),
                ("to", self.to.text() as &dyn std::fmt::Display),
            ],
        )
    }
}

fn clean(text: impl Into<SharedString>) -> SharedString {
    sanitize(&text.into(), 256).into()
}

impl RenderOnce for DiffValue {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let label = self.label();
        let side = |side: Side, from: bool| match side {
            Side::Version(version) => VersionTag::new(version)
                .tone(if from {
                    VersionTone::From
                } else {
                    VersionTone::To
                })
                .into_any_element(),
            Side::Text(text) => div()
                .min_w_0()
                .truncate()
                .when(from, |this| this.text_color(ink::muted()))
                .when(!from, |this| {
                    this.text_color(ink::text())
                        .font_weight(weights::MEDIUMBOLD.into())
                })
                .child(text)
                .into_any_element(),
        };
        let arrow = if locale().is_rtl() {
            Icon::ArrowLeft
        } else {
            Icon::ArrowRight
        };
        div()
            .id(self.id.clone())
            .accessible_id(Role::Label, &self.id, label)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .min_w_0()
            .child(side(self.from, true))
            .child(arrow.render(px(12.), colors::GRAY_500))
            .child(side(self.to, false))
    }
}

/// Two things side by side, as a version mismatch is: column headers ("Server",
/// "This instance") at 12px ink-faint, a row per fact, and only the cells that
/// differ marked in yellow with a warning icon.
#[derive(IntoElement)]
pub struct CompareTable {
    id: ElementId,
    columns: [SharedString; 2],
    rows: Vec<(SharedString, FactValue, FactValue, bool)>,
}

impl CompareTable {
    pub fn new(id: impl Into<ElementId>, columns: [SharedString; 2]) -> Self {
        Self {
            id: id.into(),
            columns,
            rows: Vec::new(),
        }
    }

    /// A fact's two values; `mismatch` marks both.
    pub fn row(
        mut self,
        key: impl Into<SharedString>,
        a: impl Into<FactValue>,
        b: impl Into<FactValue>,
        mismatch: bool,
    ) -> Self {
        self.rows.push((key.into(), a.into(), b.into(), mismatch));
        self
    }
}

impl RenderOnce for CompareTable {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let [first, second] = self.columns;
        let header = |index: usize, text: SharedString| {
            div()
                .id(("header", index))
                .flex_1()
                .min_w_0()
                .truncate()
                .role(Role::ColumnHeader)
                .child(text)
        };
        let cell = |column: &SharedString, value: FactValue, mismatch: bool, id: SharedString| {
            let label = key_value(column, &value.plain());
            div().flex().flex_1().min_w_0().child(
                div()
                    .id(ElementId::Name(id.clone()))
                    .accessible(Role::Cell, id, label)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.))
                    .when(mismatch, |this| {
                        this.flex_none()
                            .w_auto()
                            .max_w_full()
                            .px(px(6.))
                            .py(px(2.))
                            .rounded(radius::DEFAULT)
                            .bg(pill::bg(Tone::Warning))
                            .text_color(pill::fg(Tone::Warning))
                            .child(
                                Icon::TriangleExclamation.render(px(12.), pill::fg(Tone::Warning)),
                            )
                    })
                    .child(value.render(Place::List)),
            )
        };
        let table = automation_id(&self.id);
        div()
            .id(self.id.clone())
            .role(Role::Table)
            .accessibility_id(table.clone())
            .flex()
            .flex_col()
            .w_full()
            .text_size(theme::text_size(text::MEDIUM))
            .font_weight(weights::MEDIUM.into())
            .font_features(fonts::tabular())
            .text_color(ink::text())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(12.))
                    .pb(px(4.))
                    .border_b_1()
                    .border_color(ink::border())
                    .text_size(theme::text_size(text::SMALL))
                    .text_color(ink::faint())
                    .child(div().w(px(96.)).flex_none())
                    .child(header(0, first.clone()))
                    .child(header(1, second.clone())),
            )
            .children(
                self.rows
                    .into_iter()
                    .enumerate()
                    .map(|(index, (key, a, b, mismatch))| {
                        div()
                            .id(("compare-row", index))
                            .role(Role::Row)
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(12.))
                            .min_h(px(32.))
                            .child(
                                div()
                                    .w(px(96.))
                                    .flex_none()
                                    .line_clamp(2)
                                    .text_size(theme::text_size(text::SMALL))
                                    .text_color(ink::faint())
                                    .child(key),
                            )
                            .child(cell(
                                &first,
                                a,
                                mismatch,
                                format!("{table}-{index}-a").into(),
                            ))
                            .child(cell(
                                &second,
                                b,
                                mismatch,
                                format!("{table}-{index}-b").into(),
                            ))
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_reads_from_and_to() {
        let diff = DiffValue::versions("6.1.0", "6.2.0");
        assert_eq!(diff.plain(), "6.1.0 → 6.2.0");
        assert_eq!(diff.label(), "Changes from 6.1.0 to 6.2.0");
    }
}
