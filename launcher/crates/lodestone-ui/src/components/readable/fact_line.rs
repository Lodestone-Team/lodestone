//! A line of facts, in two forms:
//!
//! - **row** ([`FactLine::row`]): the 12px line under a list row's name. It never
//!   wraps: facts sit 12px apart in the order given, and when they don't fit, a fact
//!   marked [`shrink`](Fact::shrink) (a file name) is cut in the middle first, then the
//!   facts with the highest [`priority`](Fact::priority) number are left out. The
//!   line's tooltip lists every fact with its key, the ones left out and the icon
//!   facts' keys included.
//! - **strip** ([`FactLine::strip`]): a header's key-over-value cells, 24px apart with
//!   a thin divider between them. It wraps rather than cutting a key, so long
//!   translations stay whole.

use std::panic::Location;

use gpui::{
    AnyElement, App, AvailableSpace, Bounds, ContentMask, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, RenderOnce, SharedString, Size, Style,
    Window, div, point, prelude::*, relative,
};

use super::{Fact, FactKey, FactTooltip, Place};
use crate::{
    components::a11y::{Accessible, Role},
    i18n::format,
    theme::{self, fonts, ink, px, text, weights},
};

/// The gap between a row's facts.
const ROW_GAP: f32 = 12.;
/// The least a shrinking fact is cut to before facts are left out instead.
const MIN_SHRINK: f32 = 80.;
/// The most of the line a shrinking fact takes.
const MAX_SHRINK: f32 = 0.5;

/// A line of facts; see the module docs.
#[derive(IntoElement)]
pub struct FactLine {
    id: ElementId,
    strip: bool,
    facts: Vec<Fact>,
}

impl FactLine {
    /// The 12px line under a row's name.
    pub fn row(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            strip: false,
            facts: Vec::new(),
        }
    }

    /// A header's key-over-value strip.
    pub fn strip(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            strip: true,
            facts: Vec::new(),
        }
    }

    pub fn fact(mut self, fact: Fact) -> Self {
        self.facts.push(fact);
        self
    }

    pub fn facts(mut self, facts: impl IntoIterator<Item = Fact>) -> Self {
        self.facts.extend(facts);
        self
    }

    /// Every fact as "key: value", for its accessible name.
    fn label(&self) -> SharedString {
        let labels: Vec<SharedString> = self.facts.iter().map(Fact::label).collect();
        format::list(&labels).into()
    }
}

impl RenderOnce for FactLine {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let label = self.label();
        if self.strip {
            return strip(self.id, label, self.facts).into_any_element();
        }
        let rows: Vec<(Option<SharedString>, SharedString)> = self
            .facts
            .iter()
            .map(|fact| (fact.key.label().cloned(), fact.plain()))
            .collect();
        div()
            .id(self.id.clone())
            .accessible_id(Role::Group, &self.id, label)
            .tooltip(FactTooltip::builder(rows))
            .w_full()
            .min_w_0()
            .h(px(18.))
            .text_size(theme::text_size(text::SMALL))
            .font_weight(weights::MEDIUM.into())
            .text_color(ink::secondary())
            .child(FactRow { facts: self.facts })
            .into_any_element()
    }
}

/// The strip's cells. Every cell starts with its divider and the row is pulled 13px
/// (the divider and a cell's padding) to the left inside a clipping frame, so the
/// first cell of each wrapped line lines up with the edge and its divider is cut
/// off, and no line ends with a divider.
fn strip(id: ElementId, label: SharedString, facts: Vec<Fact>) -> impl IntoElement {
    const PULL: f32 = 13.;
    div()
        .id(id.clone())
        .accessible_id(Role::Group, &id, label)
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(
            div()
                .ml(px(-PULL))
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_y(px(8.))
                .children(facts.into_iter().enumerate().map(|(index, fact)| {
                    let key = match &fact.key {
                        FactKey::Text(key) | FactKey::Icon(_, key) => Some(key.clone()),
                        FactKey::Bare(_) => None,
                    };
                    let cell_label = fact.label();
                    let cell = div()
                        .id(("cell", index))
                        .accessible(Role::Label, format!("cell-{index}"), cell_label)
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .px(px(12.))
                        .when_some(key, |this, key| {
                            this.child(
                                div()
                                    .text_size(theme::text_size(text::SMALL))
                                    .font_weight(weights::MEDIUM.into())
                                    .text_color(ink::faint())
                                    .child(key),
                            )
                        })
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .text_size(theme::text_size(text::MEDIUM))
                                .font_weight(weights::MEDIUMBOLD.into())
                                .font_features(fonts::tabular())
                                .text_color(ink::text())
                                .child(fact.value.render(Place::Strip)),
                        );
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .child(div().flex_none().w(px(1.)).h(px(16.)).bg(ink::border()))
                        .child(cell)
                })),
        )
}

/// The row's facts, laid out each frame at the width the row has.
struct FactRow {
    facts: Vec<Fact>,
}

impl IntoElement for FactRow {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for FactRow {
    type RequestLayoutState = ();
    type PrepaintState = Vec<AnyElement>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<AnyElement> {
        let height = AvailableSpace::Definite(bounds.size.height);
        let natural = Size {
            width: AvailableSpace::MaxContent,
            height,
        };
        // Each fact at the width it needs.
        let mut items: Vec<(AnyElement, Size<Pixels>)> = Vec::new();
        let mut priorities = Vec::new();
        let mut shrink = None;
        for (index, fact) in std::mem::take(&mut self.facts).into_iter().enumerate() {
            priorities.push(fact.priority.unwrap_or(index.min(u8::MAX as usize) as u8));
            if fact.shrink && shrink.is_none() {
                shrink = Some(index);
            }
            let mut element = fact.render_in_row(index);
            let size = element.layout_as_root(natural, window, cx);
            items.push((element, size));
        }
        let widths: Vec<f32> = items
            .iter()
            .map(|(_, size)| f32::from(size.width))
            .collect();
        let plan = plan(
            &widths,
            &priorities,
            shrink,
            f32::from(bounds.size.width),
            f32::from(px(ROW_GAP)),
            f32::from(px(MIN_SHRINK)),
        );
        // Place the kept facts in their order, the shrinking one at its new width.
        let mut kept = Vec::new();
        let mut x = bounds.origin.x;
        for (index, (mut element, mut size)) in items.into_iter().enumerate() {
            if !plan.kept[index] {
                continue;
            }
            if Some(index) == shrink
                && let Some(width) = plan.shrink_width
            {
                size = element.layout_as_root(
                    Size {
                        width: AvailableSpace::Definite(gpui::px(width)),
                        height,
                    },
                    window,
                    cx,
                );
            }
            let y = bounds.origin.y + (bounds.size.height - size.height) / 2.;
            element.prepaint_at(point(x, y), window, cx);
            x += size.width + px(ROW_GAP);
            kept.push(element);
        }
        kept
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        kept: &mut Vec<AnyElement>,
        window: &mut Window,
        cx: &mut App,
    ) {
        // A single fact wider than the line is clipped rather than drawn past it.
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for element in kept {
                element.paint(window, cx);
            }
        });
    }
}

/// Which facts a row keeps, and the width of the one that shrinks.
#[derive(Debug, PartialEq)]
struct Plan {
    kept: Vec<bool>,
    shrink_width: Option<f32>,
}

/// Fits facts of `widths` into `width`, `gap` apart. The shrinking fact (if any) is
/// capped at half the line, then cut down to `min_shrink` before anything is left
/// out; then the facts with the highest priority number go first. The last fact is
/// always kept.
fn plan(
    widths: &[f32],
    priorities: &[u8],
    shrink: Option<usize>,
    width: f32,
    gap: f32,
    min_shrink: f32,
) -> Plan {
    let mut kept = vec![true; widths.len()];
    let mut shown: Vec<f32> = widths.to_vec();
    if let Some(index) = shrink {
        let cap = (width * MAX_SHRINK).max(min_shrink.min(widths[index]));
        shown[index] = shown[index].min(cap);
    }
    loop {
        let count = kept.iter().filter(|kept| **kept).count();
        let total: f32 = shown
            .iter()
            .zip(&kept)
            .filter(|(_, kept)| **kept)
            .map(|(width, _)| width)
            .sum::<f32>()
            + gap * count.saturating_sub(1) as f32;
        if total <= width + 0.5 {
            break;
        }
        // Cut the shrinking fact as far as it may go.
        if let Some(index) = shrink.filter(|index| kept[*index]) {
            let room = width - (total - shown[index]);
            if room >= min_shrink.min(widths[index]) {
                shown[index] = room;
                break;
            }
        }
        if count <= 1 {
            break;
        }
        // Leave out the kept fact with the highest priority number (the last of
        // equals).
        let drop = (0..widths.len())
            .filter(|index| kept[*index])
            .max_by_key(|index| (priorities[*index], *index));
        match drop {
            Some(index) => kept[index] = false,
            None => break,
        }
    }
    let shrink_width = shrink
        .filter(|index| kept[*index] && shown[*index] < widths[*index])
        .map(|index| shown[index].max(0.));
    Plan { kept, shrink_width }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_fits_on_a_wide_line() {
        let fit = plan(&[60., 40., 50.], &[0, 1, 2], None, 400., 12., 80.);
        assert_eq!(fit.kept, [true, true, true]);
        assert_eq!(fit.shrink_width, None);
    }

    #[test]
    fn the_lowest_priority_facts_go_first() {
        // a version (0), a size (1), a time (2) and an author (3) on 200px
        let fit = plan(&[50., 40., 60., 70.], &[0, 1, 2, 3], None, 200., 12., 80.);
        assert_eq!(fit.kept, [true, true, true, false]);
        let fit = plan(&[50., 40., 60., 70.], &[0, 1, 2, 3], None, 110., 12., 80.);
        assert_eq!(fit.kept, [true, true, false, false]);
        // the order of priorities, not of places
        let fit = plan(&[50., 40., 60.], &[2, 0, 1], None, 120., 12., 80.);
        assert_eq!(fit.kept, [false, true, true]);
        // the last fact stays, even when it doesn't fit
        let fit = plan(&[500., 40.], &[0, 1], None, 100., 12., 80.);
        assert_eq!(fit.kept, [true, false]);
    }

    #[test]
    fn a_file_name_shrinks_before_anything_is_left_out() {
        // file name 200px (lowest priority), size 40, time 60 on 240px
        let fit = plan(&[200., 40., 60.], &[9, 1, 2], Some(0), 240., 12., 80.);
        assert_eq!(fit.kept, [true, true, true]);
        let width = fit.shrink_width.unwrap();
        assert_eq!(width, 240. - 40. - 60. - 24.);
        // capped at half the line even when there is room
        let fit = plan(&[300., 40.], &[9, 1], Some(0), 400., 12., 80.);
        assert_eq!(fit.shrink_width, Some(200.));
        // too little room even at its least: the file name goes
        let fit = plan(&[200., 80., 60.], &[9, 1, 2], Some(0), 200., 12., 80.);
        assert_eq!(fit.kept, [false, true, true]);
        assert_eq!(fit.shrink_width, None);
    }
}
