use gpui::{
    App, ClickEvent, ElementId, IntoElement, RenderOnce, SharedString, Window, div, prelude::*, px,
    rems,
};

use super::{ClickHandler, Icon, focus::KeyboardFocus};
use crate::theme::{self, colors, radius, text, weights};

/// A selectable row with an icon, a label and optional trailing detail, after the
/// dashboard's instance pills and instance tab list.
#[derive(IntoElement)]
pub struct ListRow {
    id: ElementId,
    label: SharedString,
    icon: Option<Icon>,
    detail: Option<SharedString>,
    selected: bool,
    on_click: Option<ClickHandler>,
}

impl ListRow {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            detail: None,
            selected: false,
            on_click: None,
        }
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Short text at the end of the row, such as a status.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for ListRow {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let icon_color = if self.selected {
            colors::WHITE.opacity(50)
        } else {
            colors::GRAY_FADED.opacity(30)
        };
        div()
            .id(self.id)
            .keyboard_focus()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .w_full()
            .px_2()
            .py_1()
            .rounded(radius::MD)
            // A transparent border keeps rows the same size whether or not they are selected.
            .border_1()
            .border_color(colors::TRANSPARENT)
            .text_size(theme::text_size(text::MEDIUM))
            .line_height(rems(1.25))
            .font_weight(weights::MEDIUM.into())
            .text_color(colors::GRAY_300)
            .cursor_pointer()
            .hover(|style| style.bg(colors::GRAY_800))
            .when(self.selected, |this| {
                this.bg(colors::GRAY_800).border_color(colors::FADE_700)
            })
            .when_some(self.icon, |this, icon| {
                this.child(icon.render(px(14.), icon_color))
            })
            .child(div().flex_1().min_w_0().truncate().child(self.label))
            .when_some(self.detail, |this, detail| {
                this.child(
                    div()
                        .flex_none()
                        .text_size(theme::text_size(text::SMALL))
                        .text_color(colors::GRAY_FADED.opacity(30))
                        .child(detail),
                )
            })
            .when_some(self.on_click, |this, on_click| this.on_click(on_click))
    }
}
