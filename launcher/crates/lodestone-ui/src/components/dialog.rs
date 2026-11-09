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

use crate::skin::Skinned;
use crate::theme::px;
use launcher_core::ui_skin::SurfaceId;
use std::{rc::Rc, time::Duration};

use gpui::{
    Animation, AnimationExt, AnyElement, App, BoxShadow, ElementId, FocusHandle, Hsla, IntoElement,
    MouseButton, ParentElement, RenderOnce, SharedString, Window, div, point, prelude::*,
};

use super::a11y::{Accessible, Role, child_id};
use super::{Icon, IconButton, focus};
use crate::i18n::{CommonMsg, t};
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
/// Render it as the last child of a full-window, relatively positioned element. With
/// an [`on_close`](Self::on_close) handler (every dialog has one), it closes on
/// Escape, on a click on the backdrop and with its × button, at the end of the title's
/// row (at the top right corner of a frameless dialog). So no dialog lacks a way out
/// for the mouse, and none needs a footer button that only closes it; see
/// [`Self::footer`] for the footer's order. A dialog whose work a stray click
/// shouldn't lose (signing in) can [ignore the backdrop](Self::keep_open_on_backdrop).
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
    /// Beside the title, at the end of its row.
    header_end: Vec<AnyElement>,
    /// At the start of the footer; the footer's other elements then sit at its end.
    footer_start: Vec<AnyElement>,
    footer: Vec<AnyElement>,
    on_close: Option<CloseHandler>,
    /// A click on the backdrop calls `on_close` (yes by default).
    backdrop_closes: bool,
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
            header_end: Vec::new(),
            footer_start: Vec::new(),
            footer: Vec::new(),
            on_close: None,
            backdrop_closes: true,
            width: layout::dialog_width(),
            frameless: None,
            edge_to_edge: None,
        }
    }

    /// Fills the window from `top` down (leaving the top bar and the window buttons
    /// clear), for a frameless dialog in a window too small for it to float.
    pub fn edge_to_edge(mut self, top: gpui::Pixels) -> Self {
        self.edge_to_edge = Some(top);
        self
    }

    /// A panel of `width` × `height` that its body fills, without the title, the
    /// padding or the footer, for a page such as Settings. The × sits over the body's
    /// top right corner, so the body leaves that corner clear.
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

    /// Adds an element at the end of the title's row, such as a switch, before the ×.
    pub fn header_end(mut self, element: impl IntoElement) -> Self {
        self.header_end.push(element.into_any_element());
        self
    }

    /// Adds an element to the start of the row at the bottom, apart from the actions,
    /// such as "Import .mrpack"; the elements added with [`Self::footer`] then line
    /// up at its end.
    pub fn footer_start(mut self, element: impl IntoElement) -> Self {
        self.footer_start.push(element.into_any_element());
        self
    }

    /// Adds an element, usually a button, to the row at the bottom of the dialog, in
    /// order: the main action first, then any others, and Cancel (or "Not now",
    /// "Keep editing", …) last. Don't add a button that only closes the dialog, such
    /// as a lone Close: the × does that. A dialog with nothing here has no footer.
    pub fn footer(mut self, element: impl IntoElement) -> Self {
        self.footer.push(element.into_any_element());
        self
    }

    /// A click on the backdrop does nothing; Escape and the × still call `on_close`.
    pub fn keep_open_on_backdrop(mut self) -> Self {
        self.backdrop_closes = false;
        self
    }

    /// What closing does, by Escape, a click on the backdrop or the ×. Without it the
    /// dialog has no × and doesn't close on its own.
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
        let close = self.on_close.clone().map(close_button);
        let on_backdrop = self.on_close.filter(|_| self.backdrop_closes);
        let reduce_motion = prefers_reduced_motion();
        let enter = Animation::new(if reduce_motion { ENTER_REDUCED } else { ENTER })
            .with_easing(ease_out_cubic);
        let scrim = Hsla::from(colors::GRAY_900.opacity(60));

        let frameless = self.frameless;
        let edge_to_edge = self.edge_to_edge;
        let name = self.title.clone();
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
            .surface(SurfaceId::Dialog)
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
                    .children(self.body)
                    // over the body, at the corner (the dashboard's Settings ×)
                    .children(close.map(|close| div().absolute().top_3().right_3().child(close))),
                None => panel
                    .flex_col()
                    .gap_6()
                    .p_12()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_4()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(theme::text_size(text::H1))
                                    .line_height(theme::line_height(text::H1, leading::TIGHT))
                                    .font_weight(weights::EXTRABOLD.into())
                                    .text_color(crate::theme::ink::text())
                                    .child(self.title),
                            )
                            .children(self.header_end)
                            .children(close),
                    )
                    .child(
                        div()
                            .text_size(theme::text_size(text::H3))
                            .line_height(theme::line_height(text::H3, leading::NORMAL))
                            .text_color(crate::theme::ink::text())
                            .children(self.body),
                    )
                    .when(!self.footer.is_empty(), |this| {
                        let split = !self.footer_start.is_empty();
                        this.child(
                            div()
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .items_center()
                                .gap_2()
                                .children(self.footer_start)
                                .when(split, |row| row.child(div().flex_1()))
                                .children(self.footer),
                        )
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

        let root = div()
            .id(self.id.clone())
            .accessible(Role::Dialog, child_id(&self.id, "dialog"), name)
            .key_context("Dialog");
        focus::trap(root, &focus_handle, on_dismiss)
            .absolute()
            .inset_0()
            // The backdrop covers the window: it follows the window's own corners.
            .map(|this| super::window_frame::fit_top_corners(this, window))
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

/// The ×, which calls `on_close` like Escape and a click on the backdrop.
fn close_button(on_close: CloseHandler) -> impl IntoElement {
    div()
        .flex_none()
        .debug_selector(|| "dialog-close".into())
        .child(
            IconButton::new("dialog-close", Icon::Xmark)
                .label(t(CommonMsg::Close))
                .on_click(move |_, window, cx| on_close(window, cx)),
        )
}

/// Whether macOS asks apps to reduce motion (Accessibility > Display > Reduce
/// motion), from `NSWorkspace.accessibilityDisplayShouldReduceMotion`. The dialog
/// then fades in without the slide.
///
/// GPUI does not read this setting, and the launcher has no Objective-C crate, so it
/// is read through the Objective-C runtime that AppKit already loads.
#[cfg(target_os = "macos")]
pub fn prefers_reduced_motion() -> bool {
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

/// Whether Windows' "Animation effects" (Settings > Accessibility > Visual effects)
/// is off, which is how Windows asks apps to reduce motion.
#[cfg(windows)]
pub fn prefers_reduced_motion() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW,
    };
    let mut animate: i32 = 1;
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes one BOOL to the pointer it is given.
    let ok = unsafe {
        SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&raw mut animate).cast(), 0)
    };
    ok != 0 && animate == 0
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn prefers_reduced_motion() -> bool {
    false
}

#[cfg(all(test, feature = "screenshot"))]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use gpui::{
        Context, Entity, FocusHandle, IntoElement, Modifiers, Render, TestAppContext,
        VisualTestContext, Window, div, prelude::*,
    };

    use super::Dialog;
    use crate::components::Button;
    use crate::theme::px;

    #[derive(Clone, Copy, Debug)]
    enum Variant {
        Normal,
        Wide,
        EdgeToEdge,
        Frameless,
        FramelessEdgeToEdge,
    }

    const VARIANTS: [Variant; 5] = [
        Variant::Normal,
        Variant::Wide,
        Variant::EdgeToEdge,
        Variant::Frameless,
        Variant::FramelessEdgeToEdge,
    ];

    /// A window with one dialog of `variant`, with or without an `on_close` handler
    /// that counts its calls.
    struct Harness {
        focus: FocusHandle,
        variant: Variant,
        closable: bool,
        backdrop_closes: bool,
        closed: Rc<Cell<usize>>,
    }

    impl Render for Harness {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let closed = self.closed.clone();
            let dialog = Dialog::new("dialog", "Dialog", self.focus.clone())
                .map(|dialog| match self.variant {
                    Variant::Normal => dialog,
                    Variant::Wide => dialog.width(px(760.)),
                    Variant::EdgeToEdge => dialog.edge_to_edge(px(40.)),
                    Variant::Frameless => dialog.frameless(px(600.), px(400.)),
                    Variant::FramelessEdgeToEdge => {
                        dialog.frameless(px(600.), px(400.)).edge_to_edge(px(40.))
                    }
                })
                .when(self.closable, |dialog| {
                    dialog.on_close(move |_, _| closed.set(closed.get() + 1))
                })
                .when(!self.backdrop_closes, Dialog::keep_open_on_backdrop)
                .child(div().child("Body"))
                .footer(Button::new("ok", "OK"));
            div().relative().size_full().child(dialog)
        }
    }

    fn open(
        variant: Variant,
        closable: bool,
        cx: &mut TestAppContext,
    ) -> (Entity<Harness>, &mut VisualTestContext) {
        cx.update(crate::components::init);
        let (view, cx) = cx.add_window_view(|_, cx| Harness {
            focus: cx.focus_handle(),
            variant,
            closable,
            backdrop_closes: true,
            closed: Rc::new(Cell::new(0)),
        });
        cx.run_until_parked();
        (view, cx)
    }

    fn closed(view: &Entity<Harness>, cx: &mut VisualTestContext) -> usize {
        view.read_with(cx, |this, _| this.closed.get())
    }

    #[test]
    fn every_variant_has_a_close_button_that_calls_on_close() {
        for variant in VARIANTS {
            let mut cx = TestAppContext::single();
            let (view, cx) = open(variant, true, &mut cx);
            let bounds = cx
                .debug_bounds("dialog-close")
                .unwrap_or_else(|| panic!("a {variant:?} dialog with on_close shows a ×"));
            cx.simulate_click(bounds.center(), Modifiers::none());
            assert_eq!(
                closed(&view, cx),
                1,
                "the × of a {variant:?} dialog calls on_close, once"
            );
        }
    }

    #[test]
    fn no_close_button_without_on_close() {
        for variant in VARIANTS {
            let mut cx = TestAppContext::single();
            let (_, cx) = open(variant, false, &mut cx);
            assert!(
                cx.debug_bounds("dialog-close").is_none(),
                "a {variant:?} dialog without on_close has no ×"
            );
        }
    }

    #[test]
    fn the_close_button_works_from_the_keyboard() {
        let mut cx = TestAppContext::single();
        let (view, cx) = open(Variant::Normal, true, &mut cx);
        // The dialog took focus; Tab reaches the ×, first in the title's row, and
        // Enter clicks it.
        cx.simulate_keystrokes("tab");
        assert_eq!(closed(&view, cx), 0);
        crate::components::focus::press_enter(cx);
        assert_eq!(closed(&view, cx), 1);
    }

    #[test]
    fn escape_and_the_backdrop_call_the_same_handler() {
        let mut cx = TestAppContext::single();
        let (view, cx) = open(Variant::Normal, true, &mut cx);
        cx.simulate_keystrokes("escape");
        assert_eq!(closed(&view, cx), 1);
        cx.simulate_click(gpui::point(px(4.), px(4.)), Modifiers::none());
        assert_eq!(closed(&view, cx), 2);
    }

    #[test]
    fn a_dialog_kept_open_on_the_backdrop_still_closes_on_escape_and_the_x() {
        let mut cx = TestAppContext::single();
        let (view, cx) = open(Variant::Normal, true, &mut cx);
        view.update(cx, |this, cx| {
            this.backdrop_closes = false;
            cx.notify();
        });
        cx.run_until_parked();
        cx.simulate_click(gpui::point(px(4.), px(4.)), Modifiers::none());
        assert_eq!(closed(&view, cx), 0, "a click on the backdrop does nothing");
        cx.simulate_keystrokes("escape");
        assert_eq!(closed(&view, cx), 1);
        let bounds = cx.debug_bounds("dialog-close").expect("it has a ×");
        cx.simulate_click(bounds.center(), Modifiers::none());
        assert_eq!(closed(&view, cx), 2);
    }
}
