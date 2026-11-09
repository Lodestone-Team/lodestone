//! Keyboard focus: the Tab order, the focus ring, and the scopes that keep focus in
//! a dialog, a popover or an overlay while it is open.
//!
//! - Tab and Shift-Tab move through the tab stops in the order they are drawn. A
//!   control becomes one with [`KeyboardFocus::keyboard_focus`], which also gives it
//!   the focus ring. GPUI clicks a focused element on Enter or Space, so its
//!   `on_click` handler is all a control needs to be used from the keyboard.
//! - The ring shows only for focus from the keyboard: GPUI's `focus_visible` style
//!   applies while the last input was a key press. Moving the mouse hides it.
//! - A [`Dialog`](super::Dialog) or a [`FocusScope`] takes focus when it opens, keeps
//!   Tab and Shift-Tab inside it, and closes on Escape (the innermost first, since the
//!   action goes to the focused element first). When it closes, focus goes back to
//!   what had it before, if that is still shown.
//! - The arrow keys move between the items of an [`ArrowGroup`] (and of
//!   [`Tabs`](super::Tabs)).
//! - [`install`] makes the root of the window take focus whenever focus is lost, so
//!   the keys always reach the window's handlers.

use crate::theme::px;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use gpui::{
    AnyElement, App, BoxShadow, Context, Div, ElementId, FocusHandle, InteractiveElement,
    IntoElement, KeyBinding, ParentElement, RenderOnce, Stateful, StyleRefinement, Styled,
    Subscription, WeakFocusHandle, Window, actions, div, point, prelude::FluentBuilder as _,
};

use crate::theme::focus_ring_width;

actions!(
    focus,
    [
        /// Tab: the next tab stop.
        FocusNext,
        /// Shift-Tab: the previous tab stop.
        FocusPrev,
        /// Escape: closes the innermost dialog, popover or overlay.
        Dismiss,
        /// Right or Down in an [`ArrowGroup`] or [`Tabs`](super::Tabs).
        SelectNext,
        /// Left or Up in an [`ArrowGroup`] or [`Tabs`](super::Tabs).
        SelectPrev,
    ]
);

/// The key context of the elements that take the arrow keys.
pub const ARROW_KEYS: &str = "ArrowKeys";

pub(super) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", FocusNext, None),
        KeyBinding::new("shift-tab", FocusPrev, None),
        KeyBinding::new("escape", Dismiss, None),
        KeyBinding::new("right", SelectNext, Some(ARROW_KEYS)),
        KeyBinding::new("down", SelectNext, Some(ARROW_KEYS)),
        KeyBinding::new("left", SelectPrev, Some(ARROW_KEYS)),
        KeyBinding::new("up", SelectPrev, Some(ARROW_KEYS)),
    ]);
}

/// The dashboard's `focus-visible:ring-4 ring-blue-faded/50`, around the element.
///
/// GPUI draws a shadow as a filled shape under the element, so this ring only reads
/// as a ring around an element with an opaque background; see [`inner_ring`].
pub fn ring(style: StyleRefinement) -> StyleRefinement {
    style.shadow(vec![BoxShadow {
        color: crate::theme::accent::faded().opacity(50).into(),
        offset: point(px(0.), px(0.)),
        blur_radius: px(0.),
        spread_radius: focus_ring_width(),
        inset: false,
    }])
}

/// The focus ring inside the element's edge, half as wide and solid, for an element
/// without a background of its own (a link, a row, a tab), where a ring around it
/// would show as a tint over its whole area.
pub fn inner_ring(style: StyleRefinement) -> StyleRefinement {
    style.shadow(vec![BoxShadow {
        color: crate::theme::accent::faded().into(),
        offset: point(px(0.), px(0.)),
        blur_radius: px(0.),
        spread_radius: focus_ring_width() / 2.,
        inset: true,
    }])
}

/// Makes an element a control that Tab reaches, with a focus ring.
///
/// The element needs an id. It becomes a tab stop, in the order it is drawn, that
/// shows the ring while it has focus from the keyboard; Enter and Space then call its
/// `on_click` handler.
pub trait KeyboardFocus: InteractiveElement + Sized {
    /// A tab stop with the [`inner_ring`], for any element.
    fn keyboard_focus(self) -> Self {
        self.tab_index(0).focus_visible(inner_ring)
    }

    /// A tab stop with the [`ring`] around it, for an element with an opaque
    /// background, such as a button.
    fn keyboard_focus_outer(self) -> Self {
        self.tab_index(0).focus_visible(ring)
    }
}

impl<E: InteractiveElement> KeyboardFocus for E {}

/// A scope that has been opened: a dialog, a popover or an overlay.
struct Opened {
    scope: WeakFocusHandle,
    /// What had focus when the scope opened, outside it.
    opener: Option<WeakFocusHandle>,
    /// Whether the scope had focus when it was last drawn.
    had_focus: Rc<Cell<bool>>,
    /// A dialog, which focus falls back to while it is open.
    modal: bool,
    /// The scope's element id, for the debug traces.
    name: String,
}

/// What [`install`], the scopes and [`DialogSlot`](super::DialogSlot) share. The
/// launcher draws on the main thread only.
#[derive(Default)]
struct Registry {
    /// The root of the window: everything shown is inside it.
    root: Option<WeakFocusHandle>,
    opened: Vec<Opened>,
    /// The layers of dialogs that are fading out.
    closing: Vec<WeakFocusHandle>,
}

thread_local! {
    static REGISTRY: RefCell<Registry> = RefCell::default();
}

fn with_registry<R>(f: impl FnOnce(&mut Registry) -> R) -> R {
    REGISTRY.with(|registry| f(&mut registry.borrow_mut()))
}

/// Whether `handle` was in the last frame drawn.
fn shown(handle: &FocusHandle, window: &Window) -> bool {
    let root = with_registry(|r| r.root.as_ref().and_then(WeakFocusHandle::upgrade));
    match root {
        Some(root) => root.contains(handle, window),
        None => true,
    }
}

/// Whether `handle` is in a dialog that is fading out.
fn in_closing(handle: &FocusHandle, window: &Window) -> bool {
    let closing: Vec<FocusHandle> = with_registry(|r| {
        r.closing.retain(|scope| scope.upgrade().is_some());
        r.closing
            .iter()
            .filter_map(WeakFocusHandle::upgrade)
            .collect()
    });
    closing.iter().any(|scope| scope.contains(handle, window))
}

/// Somewhere focus can go: shown, and not in a dialog that is fading out.
fn can_take_focus(handle: &FocusHandle, window: &Window) -> bool {
    shown(handle, window) && !in_closing(handle, window)
}

/// The dialog opened last that is still open, if any.
fn top_modal(window: &Window) -> Option<FocusHandle> {
    let modals: Vec<FocusHandle> = with_registry(|r| {
        r.opened
            .iter()
            .rev()
            .filter(|opened| opened.modal)
            .filter_map(|opened| opened.scope.upgrade())
            .collect()
    });
    modals
        .into_iter()
        .find(|scope| can_take_focus(scope, window))
}

/// Forgets the scopes that are gone.
fn prune(window: &Window) {
    let opened = with_registry(|r| std::mem::take(&mut r.opened));
    let kept: Vec<Opened> = opened
        .into_iter()
        .filter(|opened| opened.scope.upgrade().is_some_and(|s| shown(&s, window)))
        .collect();
    with_registry(|r| {
        let newer = std::mem::replace(&mut r.opened, kept);
        r.opened.extend(newer);
    });
}

/// Makes `root`, the outermost element of the window, take focus whenever focus is
/// lost: when the focused element is no longer drawn, or nothing is focused. Without
/// a focused element, keys go only to the window's own root and never reach the
/// launcher's handlers. Call it from the root view's first render, and keep the
/// subscription; `root` must be tracked by the outermost element.
///
/// Focus goes, in order, to what opened a popover or dialog that just closed while it
/// had focus, to the nearest element around the one that had focus that is still
/// shown, to the dialog opened last, or to `root`.
pub fn install<V: 'static>(
    root: &FocusHandle,
    window: &mut Window,
    cx: &mut Context<V>,
) -> Subscription {
    with_registry(|r| r.root = Some(root.downgrade()));
    if window.focused(cx).is_none() {
        let root = root.clone();
        // After the frame is drawn: focus can't change while the window draws.
        window.defer(cx, move |window, cx| {
            if window.focused(cx).is_none() {
                root.focus(window, cx);
            }
        });
    }
    let root = root.clone();
    cx.on_focus_lost(window, move |_, window, cx| recover(&root, window, cx))
}

fn recover(root: &FocusHandle, window: &mut Window, cx: &mut App) {
    // A scope that had focus and is gone: back to what opened it.
    let returns: Vec<FocusHandle> = with_registry(|r| {
        let mut returns = Vec::new();
        r.opened.retain(|opened| {
            let gone = opened
                .scope
                .upgrade()
                .is_none_or(|s| !root.contains(&s, window));
            if gone && opened.had_focus.get() {
                returns.extend(opened.opener.as_ref().and_then(WeakFocusHandle::upgrade));
            }
            !gone
        });
        returns
    });
    let target = returns
        .into_iter()
        .rev()
        .find(|opener| can_take_focus(opener, window))
        .or_else(|| {
            window
                .focus_lost_restore_target(cx)
                .filter(|target| target != root && !in_closing(target, window))
        })
        .or_else(|| top_modal(window))
        .unwrap_or_else(|| root.clone());
    target.focus(window, cx);
}

/// Called by a dialog layer that starts fading out: it no longer takes focus, and
/// focus leaves it, for what opened the dialog, the dialog under it, or the root.
pub(super) fn mark_closing(layer: &FocusHandle) {
    with_registry(|r| {
        if !r
            .closing
            .iter()
            .any(|scope| scope.upgrade().as_ref() == Some(layer))
        {
            r.closing.push(layer.downgrade());
        }
    });
}

/// Moves focus out of `layer`, a dialog's layer that is fading out with focus in it.
pub(super) fn leave(layer: &FocusHandle, window: &mut Window, cx: &mut App) {
    mark_closing(layer);
    let openers: Vec<FocusHandle> = with_registry(|r| {
        r.opened
            .iter()
            .filter_map(|opened| {
                let scope = opened.scope.upgrade()?;
                let opener = opened.opener.as_ref()?.upgrade()?;
                Some((scope, opener))
            })
            .filter(|(scope, _)| layer.contains(scope, window))
            .map(|(_, opener)| opener)
            .collect()
    });
    let target = openers
        .into_iter()
        .find(|opener| can_take_focus(opener, window))
        .or_else(|| top_modal(window))
        .or_else(|| with_registry(|r| r.root.as_ref().and_then(WeakFocusHandle::upgrade)));
    match target {
        Some(target) => target.focus(window, cx),
        None => window.blur(cx),
    }
}

/// Tab or Shift-Tab outside every scope: the next tab stop of the window, or of the
/// dialog opened last while one is open.
pub fn step(forward: bool, window: &mut Window, cx: &mut App) {
    match top_modal(window) {
        Some(modal) => cycle_within(&modal, forward, window, cx),
        None if forward => window.focus_next(cx),
        None => window.focus_prev(cx),
    }
}

/// The most tab stops a step looks through to find one inside a scope.
const MAX_STEPS: usize = 4096;

/// Moves focus to the next (or previous) tab stop inside `scope`, wrapping around at
/// its ends. With no tab stop inside, focus stays on `scope`.
pub fn cycle_within(scope: &FocusHandle, forward: bool, window: &mut Window, cx: &mut App) {
    let start = window.focused(cx);
    let mut first = None;
    for _ in 0..MAX_STEPS {
        if forward {
            window.focus_next(cx);
        } else {
            window.focus_prev(cx);
        }
        let Some(now) = window.focused(cx) else {
            break;
        };
        if &now != scope && scope.contains(&now, window) {
            return;
        }
        // Around the whole order without finding one.
        if Some(&now) == start.as_ref() || Some(&now) == first.as_ref() {
            break;
        }
        first.get_or_insert(now);
    }
    scope.focus(window, cx);
}

/// What a scope keeps between frames.
struct ScopeState {
    handle: FocusHandle,
    had_focus: Rc<Cell<bool>>,
}

/// The focus of a scope keyed by `key`, under the element being drawn: `given`, or a
/// handle of its own. The first time, once the frame is drawn, the scope remembers
/// what had focus and takes focus, unless it already has it.
pub(super) fn open_scope(
    key: impl Into<ElementId>,
    given: Option<&FocusHandle>,
    modal: bool,
    window: &mut Window,
    cx: &mut App,
) -> FocusHandle {
    let given = given.cloned();
    let first = given.clone();
    let key = key.into();
    let name = key.to_string();
    let state = window.use_keyed_state(key, cx, move |window, cx| {
        let handle = first.unwrap_or_else(|| cx.focus_handle());
        let had_focus = Rc::new(Cell::new(false));
        let (scope, had) = (handle.clone(), had_focus.clone());
        // Once the frame that first shows the scope is drawn.
        window.defer(cx, move |window, cx| {
            let opener = window
                .focused(cx)
                .filter(|focused| !scope.contains(focused, window));
            prune(window);
            with_registry(|r| {
                r.opened.push(Opened {
                    scope: scope.downgrade(),
                    opener: opener.map(|opener| opener.downgrade()),
                    had_focus: had,
                    modal,
                    name: name.clone(),
                })
            });
            launcher_core::trace::emit(
                "ui.scope",
                || serde_json::json!({ "action": "open", "scope": name, "dialog": modal }),
            );
            if !scope.contains_focused(window, cx) {
                scope.focus(window, cx);
            }
        });
        ScopeState { handle, had_focus }
    });
    let state = state.read(cx);
    let handle = given.unwrap_or_else(|| state.handle.clone());
    state.had_focus.set(handle.contains_focused(window, cx));
    handle
}

type DismissHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// Adds what every scope does to its outermost element: track `handle`, keep Tab and
/// Shift-Tab inside, and call `on_dismiss` on Escape.
pub(super) fn trap(
    element: Stateful<Div>,
    handle: &FocusHandle,
    on_dismiss: Option<DismissHandler>,
) -> Stateful<Div> {
    let (next, prev) = (handle.clone(), handle.clone());
    element
        .track_focus(handle)
        .on_action(move |_: &FocusNext, window, cx| cycle_within(&next, true, window, cx))
        .on_action(move |_: &FocusPrev, window, cx| cycle_within(&prev, false, window, cx))
        .when_some(on_dismiss, |this, on_dismiss| {
            this.on_action(move |_: &Dismiss, window, cx| on_dismiss(window, cx))
        })
}

/// A popover, a menu or an overlay that keeps focus while it is open: it takes focus
/// when it is first drawn, keeps Tab and Shift-Tab inside, and calls its
/// `on_dismiss` handler on Escape, after giving focus back to what opened it.
///
/// Style it like a `div`. Its id must stay the same while it is open.
#[derive(IntoElement)]
pub struct FocusScope {
    id: ElementId,
    base: Stateful<Div>,
    on_dismiss: Option<DismissHandler>,
    arrow_keys: bool,
}

impl FocusScope {
    pub fn new(id: impl Into<ElementId>) -> Self {
        let id = id.into();
        Self {
            base: div().id(id.clone()),
            id,
            on_dismiss: None,
            arrow_keys: false,
        }
    }

    /// Down and Up (and Right and Left) also move between its tab stops, wrapping
    /// around, as in a menu or a list.
    pub fn arrow_keys(mut self) -> Self {
        self.arrow_keys = true;
        self
    }

    pub fn on_dismiss(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }
    /// Keeps the mouse off what is under it, as `div().occlude()`.
    pub fn occlude(mut self) -> Self {
        self.base = self.base.occlude();
        self
    }
}

impl Styled for FocusScope {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl ParentElement for FocusScope {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements);
    }
}

impl RenderOnce for FocusScope {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let handle = open_scope((self.id, "focus-scope"), None, false, window, cx);
        let on_dismiss = self.on_dismiss.map(|on_dismiss| {
            let scope = handle.clone();
            Rc::new(move |window: &mut Window, cx: &mut App| {
                give_back(&scope, window, cx);
                on_dismiss(window, cx);
            }) as DismissHandler
        });
        let (next, prev) = (handle.clone(), handle.clone());
        trap(self.base, &handle, on_dismiss).when(self.arrow_keys, |this| {
            this.key_context(ARROW_KEYS)
                .on_action(move |_: &SelectNext, window, cx| cycle_within(&next, true, window, cx))
                .on_action(move |_: &SelectPrev, window, cx| cycle_within(&prev, false, window, cx))
        })
    }
}

/// Gives focus back to what opened `scope`, if it is still shown.
fn give_back(scope: &FocusHandle, window: &mut Window, cx: &mut App) {
    let (opener, name) = with_registry(|r| {
        let opened = r
            .opened
            .iter()
            .find(|opened| opened.scope.upgrade().as_ref() == Some(scope));
        (
            opened.and_then(|opened| opened.opener.as_ref()?.upgrade()),
            opened.map(|opened| opened.name.clone()),
        )
    });
    if let Some(name) = name {
        launcher_core::trace::emit(
            "ui.scope",
            || serde_json::json!({ "action": "dismiss", "scope": name }),
        );
    }
    if let Some(opener) = opener.filter(|opener| can_take_focus(opener, window)) {
        opener.focus(window, cx);
    }
}

/// A row or column of items that the arrow keys move between: Right and Down go to
/// the next tab stop inside it, Left and Up to the previous, without wrapping. Each
/// item is a tab stop of its own, so Tab also reaches them, and Enter or Space
/// chooses one.
///
/// Style it like a `div`.
#[derive(IntoElement)]
pub struct ArrowGroup {
    id: ElementId,
    base: Stateful<Div>,
}

impl ArrowGroup {
    pub fn new(id: impl Into<ElementId>) -> Self {
        let id = id.into();
        Self {
            base: div().id(id.clone()),
            id,
        }
    }
}

impl Styled for ArrowGroup {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl ParentElement for ArrowGroup {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.base.extend(elements);
    }
}

impl RenderOnce for ArrowGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let handle = window
            .use_keyed_state((self.id, "arrow-group"), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let (next, prev) = (handle.clone(), handle.clone());
        self.base
            .key_context(ARROW_KEYS)
            .track_focus(&handle)
            .on_action(move |_: &SelectNext, window, cx| {
                step_within(&next, true, window, cx);
            })
            .on_action(move |_: &SelectPrev, window, cx| {
                step_within(&prev, false, window, cx);
            })
    }
}

/// Moves focus one tab stop forward or back if that stays inside `group`; returns
/// whether it moved.
pub fn step_within(group: &FocusHandle, forward: bool, window: &mut Window, cx: &mut App) -> bool {
    let Some(start) = window.focused(cx) else {
        return false;
    };
    if forward {
        window.focus_next(cx);
    } else {
        window.focus_prev(cx);
    }
    match window.focused(cx) {
        Some(now) if now != start && &now != group && group.contains(&now, window) => true,
        _ => {
            start.focus(window, cx);
            false
        }
    }
}

/// Tab, Escape and focus on open, in a test window. GPUI's test platform comes with
/// its `test-support` feature, which the `screenshot` feature turns on:
/// `cargo test -p lodestone-launcher --features screenshot`.
#[cfg(all(test, feature = "screenshot"))]
mod tests {
    use gpui::{
        Context, Entity, FocusHandle, IntoElement, Render, Subscription, TestAppContext,
        VisualTestContext, Window, div, prelude::*,
    };

    use super::{FocusNext, FocusPrev, FocusScope, install, step};
    use crate::components::{Button, Dialog, DialogSlot};

    /// A page with two buttons, and a dialog with two buttons and a menu.
    struct Harness {
        root: FocusHandle,
        dialog: DialogSlot<Harness, FocusHandle>,
        menu_open: bool,
        focus_lost: Option<Subscription>,
    }

    impl Render for Harness {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            if self.focus_lost.is_none() {
                self.focus_lost = Some(install(&self.root, window, cx));
            }
            let menu_open = self.menu_open;
            let dialog = self.dialog.render(window, |focus| {
                let close = cx.listener(|this, _: &(), _, cx| {
                    this.dialog.close(cx);
                });
                let close_menu = cx.listener(|this, _: &(), _, cx| {
                    this.menu_open = false;
                    cx.notify();
                });
                Some(
                    Dialog::new("dialog", "Dialog", focus.clone())
                        .on_close(move |window, cx| close(&(), window, cx))
                        .child(div().id("dialog-body").when(menu_open, |this| {
                            this.child(
                                FocusScope::new("menu")
                                    .arrow_keys()
                                    .on_dismiss(move |window, cx| close_menu(&(), window, cx))
                                    .child(Button::new("menu-a", "A"))
                                    .child(Button::new("menu-b", "B")),
                            )
                        }))
                        .footer(Button::new("ok", "OK"))
                        .footer(Button::new("cancel", "Cancel"))
                        .into_any_element(),
                )
            });
            div()
                .id("root")
                .track_focus(&self.root)
                .on_action(|_: &FocusNext, window, cx| step(true, window, cx))
                .on_action(|_: &FocusPrev, window, cx| step(false, window, cx))
                .relative()
                .size_full()
                .child(Button::new("page-a", "Page A"))
                .child(Button::new("page-b", "Page B"))
                .children(dialog)
        }
    }

    fn harness(cx: &mut TestAppContext) -> (Entity<Harness>, &mut VisualTestContext) {
        cx.update(crate::components::init);
        cx.add_window_view(|_, cx| Harness {
            root: cx.focus_handle(),
            dialog: DialogSlot::new(|this: &mut Harness| &mut this.dialog),
            menu_open: false,
            focus_lost: None,
        })
    }

    /// Opens the dialog from a model update, without a window, as the launcher's
    /// events do.
    fn open_dialog(view: &Entity<Harness>, cx: &mut VisualTestContext) -> FocusHandle {
        let focus = view.update(cx, |this, cx| {
            let focus = cx.focus_handle();
            this.dialog.open(focus.clone(), cx);
            focus
        });
        cx.run_until_parked();
        focus
    }

    fn focus_in(handle: &FocusHandle, cx: &mut VisualTestContext) -> bool {
        cx.update(|window, cx| handle.contains_focused(window, cx))
    }

    fn focused(handle: &FocusHandle, cx: &mut VisualTestContext) -> bool {
        cx.update(|window, _| handle.is_focused(window))
    }

    #[test]
    fn the_root_takes_focus() {
        let mut cx = TestAppContext::single();
        let (view, cx) = harness(&mut cx);
        let root = view.read_with(cx, |this, _| this.root.clone());
        assert!(focused(&root, cx));
        // Tab reaches the page's buttons.
        cx.simulate_keystrokes("tab");
        assert!(focus_in(&root, cx));
        assert!(!focused(&root, cx));
    }

    #[test]
    fn a_dialog_takes_focus_and_keeps_tab_inside() {
        let mut cx = TestAppContext::single();
        let (view, cx) = harness(&mut cx);
        let dialog = open_dialog(&view, cx);
        assert!(
            focus_in(&dialog, cx),
            "the dialog takes focus when it opens"
        );
        for _ in 0..5 {
            cx.simulate_keystrokes("tab");
            assert!(focus_in(&dialog, cx), "Tab stays in the dialog");
            assert!(!focused(&dialog, cx), "Tab reaches its buttons");
        }
        for _ in 0..5 {
            cx.simulate_keystrokes("shift-tab");
            assert!(focus_in(&dialog, cx), "Shift-Tab stays in the dialog");
        }
    }

    #[test]
    fn escape_closes_a_dialog_and_focus_goes_back() {
        let mut cx = TestAppContext::single();
        let (view, cx) = harness(&mut cx);
        // A page button has focus when the dialog opens.
        cx.simulate_keystrokes("tab");
        let opener = cx
            .update(|window, cx| window.focused(cx))
            .expect("a page button");
        let dialog = open_dialog(&view, cx);
        assert!(focus_in(&dialog, cx));
        cx.simulate_keystrokes("escape");
        assert!(!view.read_with(cx, |this, _| this.dialog.is_open()));
        cx.run_until_parked();
        assert!(focused(&opener, cx), "focus is back on the page button");
    }

    #[test]
    fn escape_closes_the_innermost_first() {
        let mut cx = TestAppContext::single();
        let (view, cx) = harness(&mut cx);
        let dialog = open_dialog(&view, cx);
        view.update(cx, |this, cx| {
            this.menu_open = true;
            cx.notify();
        });
        cx.run_until_parked();
        // The menu took focus; Down moves within it, and wraps.
        for _ in 0..3 {
            cx.simulate_keystrokes("down");
            assert!(focus_in(&dialog, cx));
        }
        cx.simulate_keystrokes("escape");
        assert!(!view.read_with(cx, |this, _| this.menu_open));
        assert!(view.read_with(cx, |this, _| this.dialog.is_open()));
        assert!(focus_in(&dialog, cx), "focus stays in the dialog");
        cx.simulate_keystrokes("escape");
        assert!(!view.read_with(cx, |this, _| this.dialog.is_open()));
    }
}
