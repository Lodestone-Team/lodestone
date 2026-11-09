//! The notification at start that offers "Keep the narrator off" (Settings > Shared
//! game settings > Narrator) to people who don't have it on. It sits in the top right,
//! under the top bar, and comes back at every start until it's accepted (a click on it,
//! or its button) or dismissed (its ×).

use std::time::Duration;

use gpui::{
    AnimationExt as _, AnyElement, Context, ElementId, IntoElement, MouseButton, Task, div,
    prelude::*,
};
use launcher_core::ui_skin::SurfaceId;

use super::{LauncherShell, TOP_BAR_HEIGHT};
use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, Icon, IconButton,
        a11y::{Accessible, Role},
    },
    i18n::{ShellMsg as M, t},
    skin::Skinned,
    theme::{self, colors, px, radius, text, weights},
};

/// How long after the window opens the notification comes in.
const DELAY: Duration = Duration::from_millis(600);
/// How long it takes to come in and to go.
const ENTER: Duration = Duration::from_millis(200);
const LEAVE: Duration = Duration::from_millis(150);
/// How far it slides as it comes in (and back as it goes).
const SLIDE: f32 = 12.;

/// The notification while it's shown.
pub(super) struct NarratorOffer {
    /// Animating out, after it was answered or the narrator was kept off elsewhere.
    leaving: bool,
    /// [`crate::instances::Instances::shared_revision`] when last checked, to go away
    /// when "Keep the narrator off" is turned on in Settings.
    revision: u64,
    _task: Option<Task<()>>,
}

impl LauncherShell {
    /// At start: shows the notification shortly after the window opens, if the
    /// narrator isn't kept off and the offer hasn't been answered.
    pub fn offer_narrator(&mut self, cx: &mut Context<Self>) {
        if !self.instances.read(cx).offers_narrator() {
            return;
        }
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DELAY).await;
            this.update(cx, |this, cx| {
                let revision = this.instances.read(cx).shared_revision();
                this.narrator_offer = Some(NarratorOffer {
                    leaving: false,
                    revision,
                    _task: None,
                });
                cx.notify();
            })
            .ok();
        });
        // Kept until it fires: dropping a task cancels it.
        self.narrator_offer_pending = Some(task);
    }

    /// Shows the notification straight away, for the offscreen screenshots.
    pub fn preview_narrator_offer(&mut self, cx: &mut Context<Self>) {
        let revision = self.instances.read(cx).shared_revision();
        self.narrator_offer = Some(NarratorOffer {
            leaving: false,
            revision,
            _task: None,
        });
        cx.notify();
    }

    /// The shared game settings changed: the notification goes once the narrator is
    /// kept off, such as from Settings.
    pub(super) fn check_narrator_offer(&mut self, cx: &mut Context<Self>) {
        let revision = self.instances.read(cx).shared_revision();
        let Some(offer) = self.narrator_offer.as_mut() else {
            return;
        };
        if offer.leaving || offer.revision == revision {
            return;
        }
        offer.revision = revision;
        if !self.instances.read(cx).offers_narrator() {
            self.close_narrator_offer(cx);
        }
    }

    /// Accepted: keeps the narrator off in every instance, with Undo.
    fn accept_narrator_offer(&mut self, cx: &mut Context<Self>) {
        if !self.narrator_offer_open() {
            return;
        }
        let result = self.instances.update(cx, |instances, cx| {
            instances.answer_narrator_offer(cx);
            instances.set_narrator_off(true, cx)
        });
        if let Some(offer) = self.narrator_offer.as_mut() {
            offer.revision = self.instances.read(cx).shared_revision();
        }
        self.close_narrator_offer(cx);
        let (message, error) = match result {
            Ok(_) => (t(M::NarratorKeptOff).to_string(), false),
            Err(err) => (err.to_string(), true),
        };
        self.undo_toast
            .show(message, error, |this: &mut Self| &mut this.undo_toast, cx);
    }

    /// Dismissed: it doesn't come back. Settings still has the switch.
    fn dismiss_narrator_offer(&mut self, cx: &mut Context<Self>) {
        if !self.narrator_offer_open() {
            return;
        }
        self.instances
            .update(cx, |instances, cx| instances.answer_narrator_offer(cx));
        self.close_narrator_offer(cx);
    }

    fn narrator_offer_open(&self) -> bool {
        self.narrator_offer
            .as_ref()
            .is_some_and(|offer| !offer.leaving)
    }

    /// Animates the notification out, then forgets it.
    fn close_narrator_offer(&mut self, cx: &mut Context<Self>) {
        let Some(offer) = self.narrator_offer.as_mut() else {
            return;
        };
        offer.leaving = true;
        offer._task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(LEAVE).await;
            this.update(cx, |this, cx| {
                this.narrator_offer = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }
}

/// The notification, over the top right of the window, if it's shown.
pub(super) fn render(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> Option<AnyElement> {
    let leaving = shell.narrator_offer.as_ref()?.leaving;
    let slide = if crate::components::prefers_reduced_motion() {
        0.
    } else {
        SLIDE
    };
    let animation = if leaving {
        gpui::Animation::new(LEAVE).with_easing(|t| t * t)
    } else {
        gpui::Animation::new(ENTER).with_easing(|t| 1. - (1. - t).powi(3))
    };
    let title = t(M::NarratorOfferTitle);
    let ink = crate::skin::painted(SurfaceId::Toast)
        .and_then(|p| p.text)
        .unwrap_or_else(crate::theme::ink::text);

    let card = div()
        .id("narrator-offer")
        .accessible(Role::Group, "narrator-offer", title)
        .relative()
        .occlude()
        .flex()
        .flex_row()
        .items_start()
        .gap_3()
        .w(px(340.))
        .p_3()
        .rounded(radius::LG)
        .border_1()
        .border_color(crate::theme::ink::border())
        .bg(colors::GRAY_750)
        .surface(SurfaceId::Toast)
        .shadow(vec![gpui::BoxShadow {
            color: crate::theme::shadow::COLOR.into(),
            offset: gpui::point(px(0.), px(4.)),
            blur_radius: px(12.),
            spread_radius: px(0.),
            inset: false,
        }])
        .cursor_pointer()
        .hover(|style| style.border_color(colors::GRAY_500))
        // A click anywhere on it accepts. The buttons inside are what the keyboard and
        // assistive technology reach.
        .on_press(cx.listener(|this, _, _, cx| this.accept_narrator_offer(cx)))
        .child(
            div()
                .flex_none()
                .pt(px(2.))
                .child(Icon::CircleInfo.render(px(14.), colors::BLUE_200)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(
                    div()
                        .text_size(theme::text_size(text::MEDIUM))
                        .font_weight(weights::BOLD.into())
                        .text_color(ink)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(theme::text_size(text::SMALL))
                        .text_color(crate::theme::ink::muted())
                        .child(t(M::NarratorOfferBody)),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .pt_1()
                        // The button accepts by itself; the card mustn't accept again.
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            Button::new("narrator-offer-accept", t(M::NarratorOfferAccept))
                                .variant(ButtonVariant::Primary)
                                .size(ButtonSize::Slim)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.accept_narrator_offer(cx)),
                                ),
                        ),
                ),
        )
        .child(
            div()
                .flex_none()
                // A click on × dismisses, without the card accepting.
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    IconButton::new("narrator-offer-dismiss", Icon::Xmark)
                        .label(t(M::NarratorOfferDismiss))
                        .on_click(cx.listener(|this, _, _, cx| this.dismiss_narrator_offer(cx))),
                ),
        )
        .with_animation(
            ElementId::from(("narrator-offer", leaving as usize)),
            animation,
            move |card, delta| {
                // in: 0 → 1 slides in from the right; out: 1 → 0 slides back
                let shown = if leaving { 1. - delta } else { delta };
                card.opacity(shown).left(px(slide * (1. - shown)))
            },
        );
    Some(
        div()
            .absolute()
            .top(px(TOP_BAR_HEIGHT + 12.))
            .right(px(16.))
            .child(card)
            .into_any_element(),
    )
}
