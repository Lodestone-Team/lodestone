use std::rc::Rc;

use gpui::{
    AnyElement, App, BoxShadow, ElementId, FocusHandle, IntoElement, KeyBinding, MouseButton,
    ParentElement, RenderOnce, SharedString, Window, actions, div, point, prelude::*, px,
};

use crate::theme::{self, colors, layout, leading, radius, shadow, text, weights};

actions!(dialog, [Dismiss]);

pub(super) fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Dismiss, Some("Dialog"))]);
}

type CloseHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// A modal dialog over a dimmed backdrop, after the dashboard's `ConfirmDialog`.
///
/// Render it as the last child of a full-window, relatively positioned element. It
/// closes on Escape (while `focus_handle` is focused) and on a click on the backdrop.
#[derive(IntoElement)]
pub struct Dialog {
    id: ElementId,
    title: SharedString,
    focus_handle: FocusHandle,
    body: Vec<AnyElement>,
    footer: Vec<AnyElement>,
    on_close: Option<CloseHandler>,
    width: gpui::Pixels,
}

impl Dialog {
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        focus_handle: FocusHandle,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            focus_handle,
            body: Vec::new(),
            footer: Vec::new(),
            on_close: None,
            width: layout::DIALOG_WIDTH,
        }
    }

    /// A wider dialog than the dashboard's 500 px, for two columns.
    pub fn width(mut self, width: gpui::Pixels) -> Self {
        self.width = width;
        self
    }

    /// Adds an element, usually a button, to the row at the bottom of the dialog.
    pub fn footer(mut self, element: impl IntoElement) -> Self {
        self.footer.push(element.into_any_element());
        self
    }

    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }
}

impl ParentElement for Dialog {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.body.extend(elements);
    }
}

impl RenderOnce for Dialog {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let on_dismiss = self.on_close.clone();
        let on_backdrop = self.on_close;

        let panel = div()
            .id("dialog-panel")
            .occlude()
            .flex()
            .flex_col()
            .gap_6()
            .w(self.width)
            .p_12()
            .rounded(radius::XL_2)
            .border_1()
            .border_color(colors::GRAY_FADED.opacity(30))
            .bg(colors::GRAY_850)
            // The dashboard's `drop-shadow-lg`.
            .shadow(vec![BoxShadow {
                color: shadow::COLOR.into(),
                offset: point(px(0.), px(8.)),
                blur_radius: px(24.),
                spread_radius: px(0.),
                inset: false,
            }])
            .child(
                div()
                    .text_size(theme::text_size(text::H1))
                    .line_height(theme::line_height(text::H1, leading::TIGHT))
                    .font_weight(weights::EXTRABOLD.into())
                    .text_color(colors::GRAY_300)
                    .child(self.title),
            )
            .child(
                div()
                    .text_size(theme::text_size(text::H3))
                    .line_height(theme::line_height(text::H3, leading::NORMAL))
                    .text_color(colors::GRAY_300)
                    .children(self.body),
            )
            .when(!self.footer.is_empty(), |this| {
                this.child(div().flex().flex_row().gap_2().children(self.footer))
            });

        div()
            .id(self.id)
            .key_context("Dialog")
            .track_focus(&self.focus_handle)
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(colors::GRAY_900.opacity(60))
            .when_some(on_dismiss, |this, on_close| {
                this.on_action(move |_: &Dismiss, window, cx| on_close(window, cx))
            })
            .when_some(on_backdrop, |this, on_close| {
                this.on_mouse_down(MouseButton::Left, move |_, window, cx| on_close(window, cx))
            })
            .child(panel)
    }
}
