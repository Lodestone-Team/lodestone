use crate::theme::px;
use gpui::{
    App, ClickEvent, ElementId, IntoElement, RenderOnce, SharedString, Window, div, prelude::*,
    rems,
};

use super::{
    ClickHandler, Icon,
    a11y::{Accessible, Role},
    focus::KeyboardFocus,
};
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
        let (id, label) = (self.id.clone(), self.label.clone());
        self.on_click = Some(Box::new(move |event, window, cx| {
            super::trace_ui(
                "ui.click",
                "list_row",
                &id,
                serde_json::json!({ "label": label.as_ref() }),
            );
            handler(event, window, cx)
        }));
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
            .id(self.id.clone())
            .accessible_id(Role::ListItem, &self.id, self.label.clone())
            .aria_selected(self.selected)
            .when_some(self.detail.clone(), |this, detail| {
                this.aria_description(detail)
            })
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
            .text_color(crate::theme::ink::text())
            .cursor_pointer()
            .hover(|style| style.bg(colors::GRAY_700).text_color(colors::WHITE))
            .when(self.selected, |this| {
                this.bg(colors::GRAY_750)
                    .border_color(crate::theme::ink::border())
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
            .when_some(self.on_click, |this, on_click| this.on_press(on_click))
    }
}
