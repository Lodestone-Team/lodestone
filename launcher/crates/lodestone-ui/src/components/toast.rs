//! Toasts: short messages in a stack at the bottom right of a window, as the
//! dashboard shows them (react-toastify): "Saved", "Couldn't send the command: …".
//!
//! An app keeps one [`Toasts`] view per window, draws it last (over the page), and
//! pushes [`Toast`]s to it. Each toast:
//!
//! - has a level ([`ToastLevel`]), which picks its icon and colour;
//! - goes away by itself after its duration, but not while the pointer is on it;
//! - closes when clicked, or with its close button;
//! - may have one action button ("Undo", "Open folder"), which closes it too;
//! - may have a key: a new toast with the same key replaces the old one, so a
//!   repeated "Saved" doesn't stack up.
//!
//! Screen readers hear each toast: it is reported with the `Status` role.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    Animation, AnimationExt as _, App, BoxShadow, Context, ElementId, IntoElement, Render,
    SharedString, Task, Window, div, point, prelude::*,
};
use launcher_core::ui_skin::SurfaceId;

use super::{
    Icon,
    a11y::{Accessible, Role},
    dialog::prefers_reduced_motion,
    motion::ease_out_cubic,
};
use crate::{
    i18n::{CommonMsg, t},
    skin::Skinned,
    theme::{self, Color, colors, leading, px, radius, shadow, text, weights},
};

/// How long a toast stays, unless it says otherwise.
pub const DEFAULT_DURATION: Duration = Duration::from_secs(5);
/// How long an error stays: it may be longer to read.
pub const ERROR_DURATION: Duration = Duration::from_secs(8);
/// The most toasts shown at once; a new one pushes the oldest out.
pub const MAX_SHOWN: usize = 5;
/// How often the time left is counted down.
const TICK: Duration = Duration::from_millis(200);

/// What a toast is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl ToastLevel {
    fn icon(self) -> Icon {
        match self {
            ToastLevel::Info => Icon::CircleInfo,
            ToastLevel::Success => Icon::CircleCheck,
            ToastLevel::Warning => Icon::TriangleExclamation,
            ToastLevel::Error => Icon::CircleExclamation,
        }
    }

    /// The dashboard's toast colours: info gray, success green, warning yellow,
    /// error red.
    fn color(self) -> Color {
        match self {
            ToastLevel::Info => colors::GRAY_400,
            ToastLevel::Success => colors::GREEN,
            ToastLevel::Warning => colors::YELLOW,
            ToastLevel::Error => colors::RED_200,
        }
    }
}

type ActionHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A toast's button.
#[derive(Clone)]
pub struct ToastAction {
    label: SharedString,
    handler: ActionHandler,
}

/// One toast, before it is shown.
#[derive(Clone)]
pub struct Toast {
    pub level: ToastLevel,
    pub text: SharedString,
    /// A toast with the same key replaces this one.
    pub key: Option<SharedString>,
    pub duration: Duration,
    action: Option<ToastAction>,
}

impl std::fmt::Debug for Toast {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Toast")
            .field("level", &self.level)
            .field("text", &self.text)
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

impl Toast {
    pub fn new(level: ToastLevel, text: impl Into<SharedString>) -> Self {
        Self {
            level,
            text: text.into(),
            key: None,
            duration: if level == ToastLevel::Error {
                ERROR_DURATION
            } else {
                DEFAULT_DURATION
            },
            action: None,
        }
    }

    pub fn info(text: impl Into<SharedString>) -> Self {
        Self::new(ToastLevel::Info, text)
    }

    pub fn success(text: impl Into<SharedString>) -> Self {
        Self::new(ToastLevel::Success, text)
    }

    pub fn warning(text: impl Into<SharedString>) -> Self {
        Self::new(ToastLevel::Warning, text)
    }

    pub fn error(text: impl Into<SharedString>) -> Self {
        Self::new(ToastLevel::Error, text)
    }

    /// A new toast with the same key replaces the shown one, such as "Saved".
    pub fn key(mut self, key: impl Into<SharedString>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// A button on the toast. Pressing it runs `handler` and closes the toast.
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some(ToastAction {
            label: label.into(),
            handler: Rc::new(handler),
        });
        self
    }
}

struct Shown {
    id: u64,
    toast: Toast,
    /// Counts down while the pointer isn't on the toast.
    remaining: Duration,
    hovered: bool,
}

/// The toasts of a window. Draw it as the window's last child.
pub struct Toasts {
    shown: Vec<Shown>,
    next_id: u64,
    ticker: Option<Task<()>>,
}

impl Default for Toasts {
    fn default() -> Self {
        Self::new()
    }
}

impl Toasts {
    pub fn new() -> Self {
        Self {
            shown: Vec::new(),
            next_id: 0,
            ticker: None,
        }
    }

    /// Shows `toast`, and returns its id (for [`Self::dismiss`]).
    pub fn push(&mut self, toast: Toast, cx: &mut Context<Self>) -> u64 {
        if let Some(key) = &toast.key {
            self.shown
                .retain(|shown| shown.toast.key.as_ref() != Some(key));
        }
        let id = self.next_id;
        self.next_id += 1;
        self.shown.push(Shown {
            id,
            remaining: toast.duration,
            toast,
            hovered: false,
        });
        if self.shown.len() > MAX_SHOWN {
            self.shown.drain(..self.shown.len() - MAX_SHOWN);
        }
        self.tick(cx);
        cx.notify();
        id
    }

    pub fn dismiss(&mut self, id: u64, cx: &mut Context<Self>) {
        self.shown.retain(|shown| shown.id != id);
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.shown.clear();
        cx.notify();
    }

    /// The texts shown, oldest first: for tests.
    pub fn texts(&self) -> Vec<SharedString> {
        self.shown.iter().map(|s| s.toast.text.clone()).collect()
    }

    /// Counts the toasts' time down while any are shown.
    fn tick(&mut self, cx: &mut Context<Self>) {
        if self.ticker.is_some() {
            return;
        }
        self.ticker = Some(cx.spawn(async move |this, cx| {
            let mut last = Instant::now();
            loop {
                cx.background_executor().timer(TICK).await;
                let now = Instant::now();
                let elapsed = now - last;
                last = now;
                let more = this
                    .update(cx, |this, cx| {
                        this.elapse(elapsed);
                        cx.notify();
                        let more = !this.shown.is_empty();
                        if !more {
                            this.ticker = None;
                        }
                        more
                    })
                    .unwrap_or(false);
                if !more {
                    break;
                }
            }
        }));
    }

    /// Takes `elapsed` off every toast the pointer isn't on, and drops those whose
    /// time is up.
    fn elapse(&mut self, elapsed: Duration) {
        for shown in &mut self.shown {
            if !shown.hovered {
                shown.remaining = shown.remaining.saturating_sub(elapsed);
            }
        }
        self.shown.retain(|shown| !shown.remaining.is_zero());
    }
}

impl Render for Toasts {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let reduce_motion = prefers_reduced_motion();
        div()
            .absolute()
            .bottom(px(16.))
            .right(px(16.))
            .flex()
            .flex_col()
            .items_end()
            .gap(px(8.))
            .children(self.shown.iter().map(|shown| {
                let id = shown.id;
                let toast = &shown.toast;
                let color = toast.level.color();
                let this = cx.entity().downgrade();
                let close = {
                    let this = this.clone();
                    move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                        this.update(cx, |this, cx| this.dismiss(id, cx)).ok();
                    }
                };
                let action = toast.action.clone().map(|action| {
                    let this = this.clone();
                    div()
                        .id(("toast-action", id))
                        .accessible(
                            Role::Button,
                            format!("toast-{id}-action"),
                            action.label.clone(),
                        )
                        .flex_none()
                        .px(px(8.))
                        .py(px(2.))
                        .rounded(radius::DEFAULT)
                        .cursor_pointer()
                        .text_color(theme::accent::light())
                        .font_weight(weights::BOLD.into())
                        .hover(|style| style.bg(colors::GRAY_700))
                        .child(action.label.clone())
                        .on_press(move |_, window, cx| {
                            cx.stop_propagation();
                            (action.handler)(window, cx);
                            this.update(cx, |this, cx| this.dismiss(id, cx)).ok();
                        })
                });
                div()
                    .id(("toast", id))
                    .accessible(Role::Status, format!("toast-{id}"), toast.text.clone())
                    .occlude()
                    .w(px(360.))
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(px(10.))
                    .px(px(12.))
                    .py(px(10.))
                    .rounded(radius::LG)
                    .border_1()
                    .border_color(colors::GRAY_FADED.opacity(30))
                    .bg(colors::GRAY_850)
                    .surface(SurfaceId::Popover)
                    .shadow(vec![BoxShadow {
                        color: shadow::COLOR.into(),
                        offset: point(px(0.), px(4.)),
                        blur_radius: px(16.),
                        spread_radius: px(0.),
                        inset: false,
                    }])
                    .text_size(theme::text_size(text::MEDIUM))
                    .line_height(theme::line_height(text::MEDIUM, leading::SNUG))
                    .font_weight(weights::MEDIUM.into())
                    .text_color(theme::ink::text())
                    .cursor_pointer()
                    .on_hover({
                        let this = this.clone();
                        move |hovered, _, cx| {
                            let hovered = *hovered;
                            this.update(cx, |this, _| {
                                if let Some(shown) = this.shown.iter_mut().find(|s| s.id == id) {
                                    shown.hovered = hovered;
                                }
                            })
                            .ok();
                        }
                    })
                    .on_press(close.clone())
                    .child(
                        div()
                            .pt(px(2.))
                            .child(toast.level.icon().render(px(16.), color)),
                    )
                    .child(div().flex_1().min_w_0().child(toast.text.clone()))
                    .children(action)
                    .child(
                        div()
                            .id(("toast-close", id))
                            .accessible(
                                Role::Button,
                                format!("toast-{id}-close"),
                                t(CommonMsg::Close),
                            )
                            .flex_none()
                            .p(px(3.))
                            .rounded(radius::DEFAULT)
                            .cursor_pointer()
                            .hover(|style| style.bg(colors::GRAY_700))
                            .child(Icon::Xmark.render(px(12.), colors::GRAY_500))
                            .on_press(move |event, window, cx| {
                                cx.stop_propagation();
                                close(event, window, cx);
                            }),
                    )
                    .with_animation(
                        ElementId::from(("toast-enter", id)),
                        Animation::new(Duration::from_millis(if reduce_motion { 1 } else { 180 }))
                            .with_easing(ease_out_cubic),
                        move |toast, delta| {
                            let slide = if reduce_motion { 0. } else { 12. };
                            toast.opacity(delta).ml(px(slide * (1. - delta)))
                        },
                    )
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(toasts: &mut Toasts, toast: Toast) {
        let id = toasts.next_id;
        toasts.next_id += 1;
        toasts.shown.push(Shown {
            id,
            remaining: toast.duration,
            toast,
            hovered: false,
        });
    }

    #[test]
    fn toasts_go_away_unless_hovered() {
        let mut toasts = Toasts::new();
        shown(&mut toasts, Toast::info("a"));
        shown(&mut toasts, Toast::error("b"));
        toasts.shown[1].hovered = true;
        toasts.elapse(DEFAULT_DURATION);
        assert_eq!(toasts.texts(), vec![SharedString::from("b")]);
        toasts.elapse(ERROR_DURATION);
        assert_eq!(toasts.texts().len(), 1, "a hovered toast stays");
        toasts.shown[0].hovered = false;
        toasts.elapse(ERROR_DURATION);
        assert!(toasts.texts().is_empty());
    }

    #[test]
    fn errors_stay_longer() {
        assert_eq!(Toast::error("x").duration, ERROR_DURATION);
        assert_eq!(Toast::success("x").duration, DEFAULT_DURATION);
    }

    #[cfg(feature = "screenshot")]
    #[gpui::test]
    fn keys_replace_and_the_stack_is_capped(cx: &mut gpui::TestAppContext) {
        use gpui::AppContext as _;
        let toasts = cx.new(|_| Toasts::new());
        toasts.update(cx, |toasts, cx| {
            toasts.push(Toast::success("Saved").key("saved"), cx);
            toasts.push(Toast::success("Saved again").key("saved"), cx);
            assert_eq!(toasts.texts(), vec![SharedString::from("Saved again")]);
            for i in 0..10 {
                toasts.push(Toast::info(format!("{i}")), cx);
            }
            assert_eq!(toasts.texts().len(), MAX_SHOWN);
            assert_eq!(toasts.texts()[MAX_SHOWN - 1], SharedString::from("9"));
        });
    }
}
