//! The modal dialog, and how to make one animate closed.
//!
//! [`Dialog`] animates open by itself. To animate closed, the dialog must still be
//! rendered after its owner closes it, so the owner keeps it in a
//! [`DialogSlot`](super::DialogSlot) instead of an `Option`:
//!
//! 1. Change the field from `Option<T>` to `DialogSlot<Owner, T>`, and build it with
//!    `DialogSlot::new(|this: &mut Owner| &mut this.field)`. `T` is whatever renders the
//!    dialog: a prompt struct, a `Modal` enum, or an `Entity` of a dialog view.
//! 2. `self.field = Some(value)` becomes `self.field.open(value, cx)` (`cx` is the
//!    owner's `Context`); it notifies.
//! 3. `self.field = None` becomes `self.field.close(cx);`, and
//!    `let Some(x) = self.field.take()` becomes `let Some(x) = self.field.close(cx)`,
//!    which gives `&mut T`: clone what you need, or `take()` fields that rendering
//!    doesn't use, since the closing dialog is rendered from this state.
//! 4. `&self.field` / `&mut self.field` become `self.field.current()` /
//!    `self.field.current_mut()`, and `is_some()` becomes `is_open()`.
//! 5. Where the dialog was rendered, use
//!    `.children(self.field.render(window, |value| Some(build(value))))`, where `build`
//!    makes the `Dialog` (or clones the entity) from `value` alone, not from
//!    `self.field`. The closure may borrow the owner and its `cx`. Add the elements as the
//!    last children of a full-window, relatively positioned element, as for a bare
//!    `Dialog`.
//! 6. A subscription to an entity dialog's "dismissed" event should check that the
//!    emitter is still the open dialog, so a dialog that is fading out can't close the one
//!    that replaced it.
//! 7. Give each opened dialog its own `FocusHandle` (`cx.focus_handle()` when it opens,
//!    kept in `T`), not one handle the owner reuses: a dialog fading out must not share
//!    its handle with the one opened after it, or the slot can't tell which has focus.
//!
//! A closing dialog fades out and rises for [`EXIT`](super::EXIT) (a fade only, over
//! [`EXIT_REDUCED`](super::EXIT_REDUCED), with reduced motion). It takes no clicks, and
//! blocks them from the page until it is gone; if it had focus, focus leaves it. Nothing
//! changes in the dialog's own code: an unmigrated `Option` still compiles and opens
//! animated, and closes at once.

use std::{rc::Rc, time::Duration};

use gpui::{
    Animation, AnimationExt, AnyElement, App, BoxShadow, ElementId, FocusHandle, Hsla, IntoElement,
    MouseButton, ParentElement, RenderOnce, SharedString, Window, div, point, prelude::*, px,
};

use super::focus;
use crate::theme::{self, colors, layout, leading, radius, shadow, text, weights};

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
/// closes on Escape and on a click on the backdrop.
///
/// The dialog tracks `focus_handle`, which should be its own (not a text field's).
/// When it first renders it takes focus, unless focus is already in it (say, in a
/// field the opener focused); Tab and Shift-Tab then stay inside it until it closes,
/// and focus goes back to what had it before (see [`focus`](super::focus)).
///
/// When it appears, the backdrop's dim fades in and the panel fades in while it
/// slides down into place. The animation is keyed by the dialog's id, so each dialog
/// animates when it opens and not again when it re-renders. A bare `Dialog` closes at
/// once, as GPUI drops an element as soon as it is no longer rendered; rendered from a
/// [`DialogSlot`](super::DialogSlot), it animates closed (see the module docs).
#[derive(IntoElement)]
pub struct Dialog {
    id: ElementId,
    title: SharedString,
    focus_handle: FocusHandle,
    body: Vec<AnyElement>,
    footer: Vec<AnyElement>,
    on_close: Option<CloseHandler>,
    width: gpui::Pixels,
    /// No title, padding or footer: the body fills a panel of this height.
    frameless: Option<gpui::Pixels>,
    /// Fills the window below `top` instead of floating: no rounding, border or
    /// shadow, for a window too small for the dialog.
    edge_to_edge: Option<gpui::Pixels>,
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
            frameless: None,
            edge_to_edge: None,
        }
    }

    /// A panel of `width` × `height` that its body fills, without the title, the
    /// padding or the footer, for a page such as Settings.
    /// Fills the window from `top` down (leaving the top bar and the window buttons
    /// clear), for a frameless dialog in a window too small for it to float.
    pub fn edge_to_edge(mut self, top: gpui::Pixels) -> Self {
        self.edge_to_edge = Some(top);
        self
    }

    pub fn frameless(mut self, width: gpui::Pixels, height: gpui::Pixels) -> Self {
        self.width = width;
        self.frameless = Some(height);
        self
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
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus_handle = focus::open_scope(
            ElementId::from((self.id.clone(), "focus")),
            Some(&self.focus_handle),
            true,
            window,
            cx,
        );
        let on_dismiss = self.on_close.clone();
        let on_backdrop = self.on_close;
        let reduce_motion = prefers_reduced_motion();
        let enter = Animation::new(if reduce_motion { ENTER_REDUCED } else { ENTER })
            .with_easing(ease_out_cubic);
        let scrim = Hsla::from(colors::GRAY_900.opacity(60));

        let frameless = self.frameless;
        let edge_to_edge = self.edge_to_edge;
        let panel = div()
            .id("dialog-panel")
            .relative()
            .occlude()
            .flex()
            .w(self.width)
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
            // (An empty shadow list here would also drop the background.)
            .when(edge_to_edge.is_some(), |panel| {
                panel.rounded_none().border_0()
            })
            .map(|panel| match frameless {
                Some(height) => panel
                    .flex_row()
                    .h(height)
                    .overflow_hidden()
                    .children(self.body),
                None => panel
                    .flex_col()
                    .gap_6()
                    .p_12()
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
                    }),
            })
            .with_animation(
                ElementId::from((self.id.clone(), "panel-enter")),
                enter.clone(),
                move |panel, delta| {
                    let slide = if reduce_motion { 0. } else { SLIDE };
                    panel.opacity(delta).top(px(-slide * (1. - delta)))
                },
            );

        let root = div().id(self.id.clone()).key_context("Dialog");
        focus::trap(root, &focus_handle, on_dismiss)
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .justify_center()
            .map(|this| match edge_to_edge {
                Some(top) => this.items_start().pt(top),
                None => this.items_center(),
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
pub(super) fn prefers_reduced_motion() -> bool {
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
pub(super) fn prefers_reduced_motion() -> bool {
    false
}
