//! Right-aligned columns for lists whose rows all have the same facts (mods, worlds,
//! backups, snapshots): the size and the date in fixed-width tabular columns, their
//! translated names shown once in a [`ListHeader`] instead of a key on every row.
//! On a narrow window the columns fold back into each row's fact line as keyed
//! facts.

use std::{cell::Cell, rc::Rc};

use gpui::{App, IntoElement, Pixels, RenderOnce, SharedString, Window, canvas, div, prelude::*};

use super::{Fact, FactValue, Place};
use crate::{
    components::a11y::{Accessible, Role},
    theme::{self, fonts, ink, px, text, weights},
};

/// A column: its translated header and its width.
#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    pub header: SharedString,
    pub width: Pixels,
}

impl Column {
    pub fn new(header: impl Into<SharedString>, width: Pixels) -> Self {
        Self {
            header: header.into(),
            width,
        }
    }

    /// A size column, 72px.
    pub fn size(header: impl Into<SharedString>) -> Self {
        Self::new(header, px(72.))
    }

    /// A date column ("Today, 00:54"), 112px.
    pub fn date(header: impl Into<SharedString>) -> Self {
        Self::new(header, px(112.))
    }
}

/// A list's columns, kept by the view that draws the list, so the list's width from
/// the last frame decides whether they fold. Clones share that width.
///
/// Draw [`probe`](Self::probe) inside the list (a `relative` element), a
/// [`ListHeader`] above it, and for each row either [`cells`](Self::cells) or, when
/// [`is_folded`](Self::is_folded), [`fold_into`](Self::fold_into) its fact line.
#[derive(Clone)]
pub struct ListColumns {
    columns: Rc<[Column]>,
    fold_below: Pixels,
    width: Rc<Cell<Option<Pixels>>>,
    folded: Rc<Cell<bool>>,
}

impl ListColumns {
    /// Columns that fold into the rows when the list is narrower than `fold_below`.
    pub fn new(columns: Vec<Column>, fold_below: Pixels) -> Self {
        Self {
            columns: columns.into(),
            fold_below,
            width: Rc::new(Cell::new(None)),
            folded: Rc::new(Cell::new(false)),
        }
    }

    pub fn columns(&self) -> &[Column] {
        &self.columns
    }

    /// Whether the list was too narrow for the columns in the last frame.
    pub fn is_folded(&self) -> bool {
        self.folded.get()
    }

    /// The list's width in the last frame, once drawn.
    pub fn width(&self) -> Option<Pixels> {
        self.width.get()
    }

    /// Records the list's width each frame. When that folds or unfolds the columns
    /// it asks for one more frame, so the change shows one frame late (and only
    /// then, so it can't loop).
    pub fn probe(&self) -> impl IntoElement {
        let (width, folded, fold_below) =
            (self.width.clone(), self.folded.clone(), self.fold_below);
        canvas(
            move |bounds, window, _| {
                let now = bounds.size.width;
                width.set(Some(now));
                let fold = now < fold_below;
                if fold != folded.get() {
                    folded.set(fold);
                    window.request_animation_frame();
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }

    /// A row's values in the columns: fixed widths, right-aligned, tabular figures.
    pub fn cells(&self, values: Vec<FactValue>) -> impl IntoElement + use<> {
        let columns = self.columns.clone();
        div()
            .flex()
            .flex_row()
            .flex_none()
            .items_center()
            .text_size(theme::text_size(text::SMALL))
            .font_weight(weights::MEDIUM.into())
            .font_features(fonts::tabular())
            .text_color(ink::secondary())
            .children(columns.iter().zip(values).map(|(column, value)| {
                div()
                    .flex()
                    .flex_none()
                    .justify_end()
                    .w(column.width)
                    .pl(px(12.))
                    .overflow_hidden()
                    .child(value.render(Place::Row))
            }))
    }

    /// A row's values as keyed facts, for its fact line while the columns are folded.
    pub fn fold_into(&self, values: Vec<FactValue>) -> Vec<Fact> {
        self.columns
            .iter()
            .zip(values)
            .map(|(column, value)| Fact::new(column.header.clone(), value))
            .collect()
    }
}

/// The columns' translated names, once, over a line: 12px, weight 450, ink-faint,
/// aligned with the cells. Hidden while the columns are folded. GPUI has no sticky
/// position, so draw it above the list's scrolling area.
#[derive(IntoElement)]
pub struct ListHeader {
    columns: ListColumns,
    /// What sits left of the columns (the name's header), if anything.
    leading: Option<SharedString>,
}

impl ListHeader {
    pub fn new(columns: &ListColumns) -> Self {
        Self {
            columns: columns.clone(),
            leading: None,
        }
    }

    /// A header over the rows' names.
    pub fn leading(mut self, header: impl Into<SharedString>) -> Self {
        self.leading = Some(header.into());
        self
    }
}

impl RenderOnce for ListHeader {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        if self.columns.is_folded() {
            return div().into_any_element();
        }
        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .pb(px(4.))
            .border_b_1()
            .border_color(ink::border())
            .text_size(theme::text_size(text::SMALL))
            .font_weight(weights::MEDIUM.into())
            .text_color(ink::faint())
            .child(div().flex_1().min_w_0().truncate().children(self.leading))
            .children(
                self.columns
                    .columns
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        div()
                            .id(("column", index))
                            .accessible(
                                Role::ColumnHeader,
                                format!("column-{index}"),
                                column.header.clone(),
                            )
                            .flex()
                            .flex_none()
                            .justify_end()
                            .w(column.width)
                            .pl(px(12.))
                            .whitespace_nowrap()
                            .child(column.header.clone())
                    }),
            )
            .into_any_element()
    }
}
