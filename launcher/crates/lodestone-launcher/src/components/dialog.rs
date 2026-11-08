use std::{rc::Rc, time::Duration};

use gpui::{
    Animation, AnimationExt, AnyElement, App, BoxShadow, ElementId, FocusHandle, Hsla, IntoElement,
    KeyBinding, MouseButton, ParentElement, RenderOnce, SharedString, Window, actions, div, point,
    prelude::*, px,
};

use crate::theme::{self, colors, layout, leading, radius, shadow, text, weights};

actions!(dialog, [Dismiss]);

pub(super) fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("escape", Dismiss, Some("Dialog"))]);
}

type CloseHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// How long a dialog takes to appear, like the dashboard's modals (`duration-200`).
const ENTER: Duration = Duration::from_millis(200);
/// The shorter fade that replaces the slide when the system asks for reduced motion.
const ENTER_REDUCED: Duration = Duration::from_millis(150);
/// How far above its place the panel starts (the dashboard's `-translate-y-3`).
const SLIDE: f32 = 12.;

/// A cubic ease-out, close to the CSS `ease-out` the dashboard's modals use.
fn ease_out_cubic(delta: f32) -> f32 {
    1. - (1. - delta).powi(3)
}

/// A modal dialog over a dimmed backdrop, after the dashboard's `ConfirmDialog`.
///
/// Render it as the last child of a full-window, relatively positioned element. It
/// closes on Escape (while `focus_handle` is focused) and on a click on the backdrop.
///
/// When it appears, the backdrop's dim fades in and the panel fades in while it
/// slides down into place. The animation is keyed by the dialog's id, so each dialog
/// animates when it opens and not again when it re-renders. It closes at once:
/// GPUI drops an element as soon as it is no longer rendered.
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
        let reduce_motion = prefers_reduced_motion();
        let enter = Animation::new(if reduce_motion { ENTER_REDUCED } else { ENTER })
            .with_easing(ease_out_cubic);
        let scrim = Hsla::from(colors::GRAY_900.opacity(60));

        let panel = div()
            .id("dialog-panel")
            .relative()
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
            })
            .with_animation(
                ElementId::from((self.id.clone(), "panel-enter")),
                enter.clone(),
                move |panel, delta| {
                    let slide = if reduce_motion { 0. } else { SLIDE };
                    panel.opacity(delta).top(px(-slide * (1. - delta)))
                },
            );

        div()
            .id(self.id.clone())
            .key_context("Dialog")
            .track_focus(&self.focus_handle)
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .when_some(on_dismiss, |this, on_close| {
                this.on_action(move |_: &Dismiss, window, cx| on_close(window, cx))
            })
            .when_some(on_backdrop, |this, on_close| {
                this.on_mouse_down(MouseButton::Left, move |_, window, cx| on_close(window, cx))
            })
            .child(panel)
            .with_animation(
                ElementId::from((self.id, "backdrop-enter")),
                enter,
                move |backdrop, delta| backdrop.bg(scrim.opacity(delta)),
            )
    }
}

/// Whether macOS asks apps to reduce motion (Accessibility > Display > Reduce
/// motion), from `NSWorkspace.accessibilityDisplayShouldReduceMotion`. The dialog
/// then fades in without the slide.
///
/// GPUI does not read this setting, and the launcher has no Objective-C crate, so it
/// is read through the Objective-C runtime that AppKit already loads.
#[cfg(target_os = "macos")]
fn prefers_reduced_motion() -> bool {
    use std::ffi::{c_char, c_void};

    type Id = *mut c_void;
    type Sel = *mut c_void;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn objc_msgSend();
    }

    // SAFETY: `objc_msgSend` is called through function types that match the two
    // methods: `+[NSWorkspace sharedWorkspace]` returns an object and takes no
    // arguments, and `-accessibilityDisplayShouldReduceMotion` returns a `BOOL`
    // (read as a byte) and takes none. Both exist since macOS 10.12, and a missing class is
    // checked before any message is sent.
    unsafe {
        let class = objc_getClass(c"NSWorkspace".as_ptr());
        if class.is_null() {
            return false;
        }
        let send_id = std::mem::transmute::<
            unsafe extern "C" fn(),
            unsafe extern "C" fn(Id, Sel) -> Id,
        >(objc_msgSend);
        // `BOOL` is a `signed char` on Intel Macs and a `bool` on Apple silicon; read it
        // as a byte and compare, so no value is ever reinterpreted as a Rust `bool`.
        let send_bool = std::mem::transmute::<
            unsafe extern "C" fn(),
            unsafe extern "C" fn(Id, Sel) -> i8,
        >(objc_msgSend);
        let workspace = send_id(class, sel_registerName(c"sharedWorkspace".as_ptr()));
        !workspace.is_null()
            && send_bool(
                workspace,
                sel_registerName(c"accessibilityDisplayShouldReduceMotion".as_ptr()),
            ) != 0
    }
}

#[cfg(not(target_os = "macos"))]
fn prefers_reduced_motion() -> bool {
    false
}
