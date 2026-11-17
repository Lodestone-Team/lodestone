//! A two-column table of facts ([`FactList`]): keys in a column as wide as the widest
//! translated key, values beside them, all left-aligned. And the tooltip that lists
//! a row's facts ([`FactTooltip`]).

use gpui::{
    AnyView, App, AppContext as _, Context, ElementId, IntoElement, Pixels, Render, RenderOnce,
    SharedString, Window, div, prelude::*, relative,
};
use launcher_core::ui_skin::SurfaceId;

use super::{FactValue, Place, key_value};
use crate::{
    components::a11y::{Accessible, Role, automation_id},
    skin::Skinned,
    theme::{self, colors, fonts, ink, px, radius, text, weights},
    values::truncate::{MAX_SHOWN, sanitize},
};

/// The narrowest a key column is.
const MIN_KEY_WIDTH: f32 = 96.;

/// Facts as a table: "Minecraft | 1.20.1", "Loader | Fabric 0.16.14", "Play time |
/// 2 h 5 min". Keys are 12px ink-faint in a column sized to the widest key (96px at
/// least, 40% of the width at most, wrapping to two lines before they are cut);
/// values are 14px in the main ink with tabular figures. Rows are 28px with a line
/// between them. [`framed`](Self::framed) puts it on a card, as in dialogs; settings
/// panes leave it bare.
#[derive(IntoElement)]
pub struct FactList {
    id: ElementId,
    rows: Vec<(SharedString, FactValue)>,
    framed: bool,
}

impl FactList {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            rows: Vec::new(),
            framed: false,
        }
    }

    pub fn row(mut self, key: impl Into<SharedString>, value: impl Into<FactValue>) -> Self {
        self.rows.push((key.into(), value.into()));
        self
    }

    /// On a gray-800 card.
    pub fn framed(mut self, framed: bool) -> Self {
        self.framed = framed;
        self
    }
}

/// The width of the widest of `keys` at the key style, in the window's font.
fn widest_key(keys: &[&SharedString], window: &mut Window) -> Pixels {
    let mut style = window.text_style();
    style.font_family = fonts::body();
    style.font_size = theme::text_size(text::SMALL);
    style.font_weight = weights::MEDIUM.face();
    let font_size = style.font_size.to_pixels(window.rem_size());
    keys.iter()
        .map(|key| {
            let runs = [style.to_run(key.len())];
            window
                .text_system()
                .shape_line((*key).clone(), font_size, &runs, None)
                .width
        })
        .fold(Pixels::ZERO, |widest, width| widest.max(width))
}

impl RenderOnce for FactList {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // Keys can't hold a newline (the line shaper asserts there is none).
        let rows: Vec<(SharedString, FactValue)> = self
            .rows
            .into_iter()
            .map(|(key, value)| (sanitize(&key, MAX_SHOWN).into(), value))
            .collect();
        let keys: Vec<&SharedString> = rows.iter().map(|(key, _)| key).collect();
        // Room after the key, as the column's gap.
        // (and 2px for rounding, so a key that fits never wraps)
        let key_width = (widest_key(&keys, window) + px(18.)).max(px(MIN_KEY_WIDTH));
        let count = rows.len();
        div()
            .id(self.id.clone())
            .role(Role::List)
            .accessibility_id(automation_id(&self.id))
            .flex()
            .flex_col()
            .w_full()
            .when(self.framed, |this| {
                this.bg(colors::GRAY_800)
                    .rounded(radius::LG)
                    .px(px(12.))
                    .py(px(4.))
            })
            .children(rows.into_iter().enumerate().map(|(index, (key, value))| {
                let label = key_value(&key, &value.plain());
                div()
                    .id(("row", index))
                    .accessible(Role::ListItem, format!("row-{index}"), label)
                    .flex()
                    .flex_row()
                    .items_center()
                    .min_h(px(28.))
                    .py(px(4.))
                    .when(index + 1 < count, |this| {
                        this.border_b_1().border_color(ink::border())
                    })
                    .child(
                        div()
                            .flex_none()
                            .w(key_width)
                            .max_w(relative(0.4))
                            .pr(px(16.))
                            .line_clamp(2)
                            .text_size(theme::text_size(text::SMALL))
                            .font_weight(weights::MEDIUM.into())
                            .text_color(ink::faint())
                            .child(key),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_w_0()
                            .items_center()
                            .text_size(theme::text_size(text::MEDIUM))
                            .font_weight(weights::MEDIUM.into())
                            .font_features(fonts::tabular())
                            .text_color(ink::text())
                            .child(value.render(Place::List)),
                    )
            }))
    }
}

/// A tooltip listing facts as plain keys and values: a row's every fact, the ones
/// its line left out and its icon facts' keys included.
pub struct FactTooltip {
    rows: Vec<(Option<SharedString>, SharedString)>,
}

impl FactTooltip {
    /// For `.tooltip(…)`: shows `rows` (a key, if any, and a value).
    pub fn builder(
        rows: Vec<(Option<SharedString>, SharedString)>,
    ) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
        move |_, cx| cx.new(|_| FactTooltip { rows: rows.clone() }).into()
    }
}

impl Render for FactTooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // A tooltip is a window of its own: it sets its own text style.
        div()
            .max_w(px(360.))
            .px_2()
            .py_1()
            .rounded(radius::MD)
            .border_1()
            .border_color(ink::border())
            .bg(colors::GRAY_850)
            .surface(SurfaceId::Popover)
            .font_family(fonts::body())
            .text_size(theme::text_size(text::SMALL))
            .font_weight(weights::MEDIUM.into())
            .font_features(fonts::tabular())
            .text_color(ink::text())
            .flex()
            .flex_col()
            .gap(px(2.))
            .children(self.rows.iter().map(|(key, value)| {
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.))
                    .when_some(key.clone(), |this, key| {
                        this.child(div().flex_none().text_color(ink::faint()).child(key))
                    })
                    .child(div().min_w_0().child(value.clone()))
            }))
    }
}
