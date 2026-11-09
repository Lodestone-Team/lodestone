use crate::theme::px;
use std::rc::Rc;

use gpui::{
    App, ElementId, FocusHandle, IntoElement, RenderOnce, SharedString, Window, div, prelude::*,
};

use super::focus::{self, SelectNext, SelectPrev};
use crate::theme::{self, colors, leading, text, weights};

type SelectHandler = Box<dyn Fn(&usize, &mut Window, &mut App) + 'static>;

/// A row of tabs over a rule, after the tab list on the dashboard's settings page.
///
/// Each tab is a tab stop; Enter or Space selects the focused one, and Left and Right
/// move focus to the tab beside it and select it.
#[derive(IntoElement)]
pub struct Tabs {
    id: ElementId,
    labels: Vec<SharedString>,
    selected: usize,
    on_select: Option<SelectHandler>,
}

impl Tabs {
    pub fn new(
        id: impl Into<ElementId>,
        labels: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        Self {
            id: id.into(),
            labels: labels.into_iter().map(Into::into).collect(),
            selected: 0,
            on_select: None,
        }
    }

    pub fn selected(mut self, index: usize) -> Self {
        self.selected = index;
        self
    }

    /// Called with the index of the tab that was clicked.
    pub fn on_select(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        let (id, labels) = (self.id.clone(), self.labels.clone());
        self.on_select = Some(Box::new(move |index, window, cx| {
            let label = labels.get(*index).map(|l| l.to_string());
            super::trace_ui(
                "ui.select",
                "tabs",
                &id,
                serde_json::json!({ "index": index, "label": label }),
            );
            handler(index, window, cx)
        }));
        self
    }
}

impl RenderOnce for Tabs {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let on_select = self.on_select.map(Rc::new);
        // One handle per tab, kept while the tabs are shown, so the arrow keys can move
        // focus to a given tab.
        let handles: Rc<Vec<FocusHandle>> = Rc::new(
            (0..self.labels.len())
                .map(|index| {
                    let key = ElementId::from((self.id.clone(), format!("tab-focus-{index}")));
                    window
                        .use_keyed_state(key, cx, |_, cx| cx.focus_handle().tab_stop(true))
                        .read(cx)
                        .clone()
                })
                .collect(),
        );
        let arrow = |forward: bool| {
            let handles = handles.clone();
            let on_select = on_select.clone();
            move |window: &mut Window, cx: &mut App| {
                let Some(current) = handles.iter().position(|h| h.is_focused(window)) else {
                    return;
                };
                let Some(next) = neighbour(current, handles.len(), forward) else {
                    return;
                };
                handles[next].focus(window, cx);
                if let Some(on_select) = &on_select {
                    on_select(&next, window, cx);
                }
            }
        };
        let (next, prev) = (arrow(true), arrow(false));
        div()
            .id(self.id)
            .key_context(focus::ARROW_KEYS)
            .on_action(move |_: &SelectNext, window, cx| next(window, cx))
            .on_action(move |_: &SelectPrev, window, cx| prev(window, cx))
            .flex()
            .flex_row()
            .flex_wrap()
            .items_end()
            .gap_4()
            .w_full()
            .border_b_2()
            .border_color(colors::GRAY_700)
            .text_size(theme::text_size(text::H3))
            .line_height(theme::line_height(text::H3, leading::NORMAL))
            .font_weight(weights::BOLD.into())
            .children(self.labels.into_iter().enumerate().map(|(index, label)| {
                let selected = index == self.selected;
                let on_select = on_select.clone();
                div()
                    .id(("tab", index))
                    .track_focus(&handles[index])
                    .focus_visible(focus::inner_ring)
                    .cursor_pointer()
                    .map(|this| {
                        if selected {
                            this.border_b_2()
                                .border_color(colors::BLUE_200)
                                .text_color(colors::BLUE_200)
                        } else {
                            // mb-0.5 keeps unselected labels level with the selected one.
                            this.mb(px(2.))
                                .text_color(colors::GRAY_500)
                                .hover(|style| style.text_color(colors::GRAY_400))
                        }
                    })
                    .when_some(on_select, |this, on_select| {
                        this.on_click(move |_, window, cx| on_select(&index, window, cx))
                    })
                    .child(label)
            }))
    }
}

/// The tab beside `current` of `len`, after it or before it; none past either end.
fn neighbour(current: usize, len: usize, forward: bool) -> Option<usize> {
    if forward {
        Some(current + 1).filter(|next| *next < len)
    } else {
        current.checked_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::neighbour;

    #[test]
    fn arrows_stop_at_the_ends() {
        assert_eq!(neighbour(0, 5, true), Some(1));
        assert_eq!(neighbour(3, 5, true), Some(4));
        assert_eq!(neighbour(4, 5, true), None);
        assert_eq!(neighbour(4, 5, false), Some(3));
        assert_eq!(neighbour(0, 5, false), None);
        assert_eq!(neighbour(0, 1, true), None);
    }
}
