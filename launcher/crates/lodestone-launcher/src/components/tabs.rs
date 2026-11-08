use gpui::{App, ElementId, IntoElement, RenderOnce, SharedString, Window, div, prelude::*, px};

use crate::theme::{self, colors, leading, text, weights};

type SelectHandler = Box<dyn Fn(&usize, &mut Window, &mut App) + 'static>;

/// A row of tabs over a rule, after the tab list on the dashboard's settings page.
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
        self.on_select = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for Tabs {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let on_select = self.on_select.map(std::rc::Rc::new);
        div()
            .id(self.id)
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
