//! "How to add a friend": the Friends section's steps, as a timeline that follows
//! where the person is. Each step is ticked once the friend list shows it done, the
//! first one that isn't is the current one, with what to do next and its button, and
//! the steps go once all are done. "Hide steps" hides them for good (the settings'
//! `friend_steps_hidden`), and "Show steps" in the section's header brings them back.

use gpui::{AnyElement, Context, IntoElement, SharedString, div, prelude::*};
use launcher_core::{
    cores::friends::is_friend_list_id,
    friend_list::{self, FriendStatus},
};

use super::{Servers, extras::note, friend_list::FriendList};
use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, Icon,
        a11y::{Accessible, Role},
    },
    i18n::{FriendListMsg as M, t, tr},
    theme::{self, colors, px, text, weights},
};

/// The steps, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Choose the name friends see.
    Name,
    /// Make a friend link and send it, or open the one a friend sent.
    Link,
    /// The sender accepts the answer.
    Accept,
    /// Compare safety codes.
    Compare,
    /// A friend shares a server from Lodestone Host.
    Share,
    /// Join it from the friend list.
    Play,
}

impl Step {
    pub const ALL: [Step; 6] = [
        Step::Name,
        Step::Link,
        Step::Accept,
        Step::Compare,
        Step::Share,
        Step::Play,
    ];

    fn title(self) -> &'static str {
        t(match self {
            Step::Name => M::StepName,
            Step::Link => M::StepLink,
            Step::Accept => M::StepAccept,
            Step::Compare => M::StepCompare,
            Step::Share => M::StepShare,
            Step::Play => M::StepPlay,
        })
    }
}

/// How far the person is, and the names the current step's text needs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    /// Whether each of [`Step::ALL`] is done.
    pub done: [bool; 6],
    /// One of this person's links is open.
    pub links_open: bool,
    /// Someone answered one of this person's links: their name.
    pub reply_from: Option<String>,
    /// A friend who hasn't confirmed yet (this person answered their link).
    pub pending: Option<String>,
    /// A confirmed friend whose codes weren't compared: key and name.
    pub unverified: Option<(String, String)>,
    /// A confirmed friend who shares nothing with this person yet.
    pub share_from: Option<String>,
    /// A friend who shares a server with this person, online first: key and name.
    pub play_with: Option<(String, String)>,
    /// That friend is online now.
    pub play_online: bool,
}

impl Progress {
    /// From the friend list as it is, and whether a server was ever joined from it
    /// (`played`).
    pub fn of(list: &FriendList, played: bool) -> Self {
        let book = &list.snapshot.book;
        let now = friend_list::now();
        let live: Vec<_> = book
            .friends
            .iter()
            .filter(|f| f.status != FriendStatus::Retired)
            .collect();
        let active: Vec<_> = live
            .iter()
            .filter(|f| f.status == FriendStatus::Active)
            .copied()
            .collect();
        let sharing: Vec<_> = active
            .iter()
            .filter(|f| !f.joinable_cores(now).is_empty())
            .copied()
            .collect();
        let replies = list.waiting_replies();
        let name = |f: &friend_list::Friend| f.display_name().to_string();
        let play_with = sharing
            .iter()
            .find(|f| list.online(f) == Some(true))
            .or(sharing.first())
            .copied();
        Self {
            done: [
                !book.name.is_empty(),
                !live.is_empty() || !replies.is_empty(),
                !active.is_empty(),
                active.iter().any(|f| f.verified),
                !sharing.is_empty(),
                played,
            ],
            links_open: !book.links.is_empty(),
            reply_from: replies.first().map(|r| r.name.clone()),
            pending: live
                .iter()
                .find(|f| f.status == FriendStatus::Pending)
                .map(|f| name(f)),
            unverified: active
                .iter()
                .find(|f| !f.verified)
                .map(|f| (f.key.clone(), name(f))),
            share_from: active
                .iter()
                .find(|f| f.joinable_cores(now).is_empty())
                .or(active.first())
                .map(|f| name(f)),
            play_online: play_with.is_some_and(|f| list.online(f) == Some(true)),
            play_with: play_with.map(|f| (f.key.clone(), name(f))),
        }
    }

    /// The first step that isn't done; `None` once all are.
    pub fn current(&self) -> Option<Step> {
        Step::ALL
            .into_iter()
            .zip(self.done)
            .find_map(|(step, done)| (!done).then_some(step))
    }

    pub fn done_count(&self) -> usize {
        self.done.iter().filter(|done| **done).count()
    }
}

/// The diameter of a step's circle.
const DOT: f32 = 20.0;

impl Servers {
    fn friend_steps_progress(&self) -> Progress {
        let played = self
            .friends
            .list
            .iter()
            .any(|entry| is_friend_list_id(&entry.saved.id));
        Progress::of(&self.friend_list, played)
    }

    fn friend_steps_hidden(&self, cx: &Context<Self>) -> bool {
        self.instances.read(cx).settings().friend_steps_hidden
    }

    fn set_friend_steps_hidden(&mut self, hidden: bool, cx: &mut Context<Self>) {
        self.instances.update(cx, |instances, cx| {
            instances.set_friend_steps_hidden(hidden, cx)
        });
        cx.notify();
    }

    /// "Show steps", for the section's header, while the steps are hidden and not
    /// all done.
    pub(super) fn friend_steps_show_button(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.friend_steps_hidden(cx) || self.friend_steps_progress().current().is_none() {
            return None;
        }
        Some(
            Button::new("friend-steps-show", t(M::ShowSteps))
                .size(ButtonSize::Slim)
                .variant(ButtonVariant::Ghost)
                .on_click(cx.listener(|this, _, _, cx| this.set_friend_steps_hidden(false, cx)))
                .into_any_element(),
        )
    }

    /// The steps, under the section's header: `None` when hidden or all done.
    pub(super) fn render_friend_steps(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.friend_steps_hidden(cx) {
            return None;
        }
        let progress = self.friend_steps_progress();
        let current = progress.current()?;
        let total = Step::ALL.len();
        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .child(
                div()
                    .id("friend-steps-title")
                    .role(Role::Heading)
                    .aria_level(3)
                    .aria_label(t(M::StepsTitle))
                    .font_weight(weights::BOLD.into())
                    .text_color(crate::theme::ink::text())
                    .child(t(M::StepsTitle)),
            )
            .child(
                note(tr!(
                    M::StepsProgress,
                    done = progress.done_count(),
                    total = total
                ))
                .flex_1()
                .min_w_0(),
            )
            .child(
                Button::new("friend-steps-hide", t(M::HideSteps))
                    .size(ButtonSize::Slim)
                    .variant(ButtonVariant::Ghost)
                    .on_click(cx.listener(|this, _, _, cx| this.set_friend_steps_hidden(true, cx))),
            );
        let steps = Step::ALL.into_iter().enumerate().map(|(index, step)| {
            let done = progress.done[index];
            let is_current = step == current;
            let last = index + 1 == total;
            self.friend_step(index, step, done, is_current, last, &progress, cx)
        });
        Some(
            div()
                .id("friend-steps")
                .flex()
                .flex_col()
                .gap_2()
                .px_4()
                .pt_3()
                .pb_1()
                .border_t_1()
                .border_color(crate::theme::ink::border())
                .child(header)
                .child(
                    div()
                        .id("friend-steps-list")
                        .role(Role::List)
                        .aria_label(t(M::StepsTitle))
                        .flex()
                        .flex_col()
                        .children(steps),
                )
                .into_any_element(),
        )
    }

    /// One step: its circle (ticked, numbered in the accent colour while current, or
    /// grey), the line to the next one, its title, and for the current step what to do
    /// and its button.
    #[allow(clippy::too_many_arguments)]
    fn friend_step(
        &self,
        index: usize,
        step: Step,
        done: bool,
        current: bool,
        last: bool,
        progress: &Progress,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let accent = crate::theme::accent::light();
        let number = SharedString::from((index + 1).to_string());
        let dot = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(DOT))
            .rounded_full()
            .text_size(theme::text_size(text::SMALL))
            .font_weight(weights::BOLD.into())
            .map(|dot| {
                if done {
                    dot.bg(colors::GREEN_400)
                        .child(Icon::Check.render(px(11.), colors::WHITE))
                } else if current {
                    dot.border_2()
                        .border_color(accent)
                        .text_color(accent)
                        .child(number)
                } else {
                    dot.border_1()
                        .border_color(colors::GRAY_600)
                        .text_color(colors::GRAY_500)
                        .child(number)
                }
            });
        let rail = div()
            .flex()
            .flex_col()
            .flex_none()
            .items_center()
            .w(px(DOT))
            .child(dot)
            .when(!last, |rail| {
                rail.child(
                    div()
                        .flex_1()
                        .min_h(px(8.))
                        .w(px(2.))
                        .my_1()
                        .rounded_full()
                        .bg(if done {
                            colors::GREEN_400.opacity(60)
                        } else {
                            colors::GRAY_700
                        }),
                )
            });
        let state = t(if done {
            M::StepDone
        } else if current {
            M::StepNext
        } else {
            M::StepLater
        });
        let label = format!("{}. {}: {state}", index + 1, step.title());
        let title = div()
            .text_size(theme::text_size(text::MEDIUM))
            .when(current, |title| {
                title
                    .font_weight(weights::BOLD.into())
                    .text_color(crate::theme::ink::text())
            })
            .when(!current, |title| {
                title.text_color(crate::theme::ink::muted())
            })
            .child(step.title());
        let body = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap_1()
            .pb_3()
            // the title's line sits level with the circle
            .min_h(px(DOT))
            .child(title)
            .when(current, |body| {
                body.child(note(step_detail(step, progress)))
                    .children(self.friend_step_action(step, progress, cx))
            });
        div()
            .id(SharedString::from(format!("friend-step-{}", index + 1)))
            .accessible(Role::ListItem, format!("friend-step-{}", index + 1), label)
            .flex()
            .flex_row()
            .items_stretch()
            .gap_3()
            .child(rail)
            .child(body)
            .into_any_element()
    }

    /// The current step's button, where it has one.
    fn friend_step_action(
        &self,
        step: Step,
        progress: &Progress,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let button = |label: &'static str| {
            Button::new("friend-step-action", label)
                .size(ButtonSize::Slim)
                .variant(ButtonVariant::Primary)
        };
        let action = match step {
            Step::Name => button(t(M::ChooseName))
                .on_click(cx.listener(|this, _, _, cx| this.open_friend_name(cx))),
            // a link is out: wait for it (the header's Add friend opens a friend's)
            Step::Link if progress.links_open => return None,
            Step::Link => button(t(M::AddFriend))
                .on_click(cx.listener(|this, _, _, cx| this.open_add_friend(None, cx))),
            Step::Compare => {
                let (key, _) = progress.unverified.clone()?;
                button(t(M::CompareCodes))
                    .on_click(cx.listener(move |this, _, _, cx| this.open_friend_verify(&key, cx)))
            }
            Step::Play => {
                let (key, _) = progress.play_with.clone()?;
                if self.friend_list.expanded.contains(&key) {
                    return None;
                }
                button(t(M::ShowServers))
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_friend(&key, cx)))
            }
            // Accept is on the answer's row below; sharing happens in the friend's Host
            Step::Accept | Step::Share => return None,
        };
        Some(div().flex().child(action).into_any_element())
    }
}

/// What to do for the current step.
fn step_detail(step: Step, progress: &Progress) -> String {
    let friend = |name: &Option<String>| name.clone().unwrap_or_default();
    match step {
        Step::Name => t(M::StepNameDetail).to_string(),
        Step::Link if progress.links_open => t(M::StepLinkWaiting).to_string(),
        Step::Link => t(M::StepLinkDetail).to_string(),
        Step::Accept => match (&progress.reply_from, &progress.pending) {
            (Some(name), _) => tr!(M::StepAcceptReply, name = name),
            (None, pending) => tr!(M::StepAcceptPending, name = friend(pending)),
        },
        Step::Compare => tr!(
            M::StepCompareDetail,
            name = progress
                .unverified
                .as_ref()
                .map(|(_, name)| name.clone())
                .unwrap_or_default()
        ),
        Step::Share => tr!(M::StepShareDetail, name = friend(&progress.share_from)),
        Step::Play => tr!(
            if progress.play_online {
                M::StepPlayOnline
            } else {
                M::StepPlayDetail
            },
            name = progress
                .play_with
                .as_ref()
                .map(|(_, name)| name.clone())
                .unwrap_or_default()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn progress(done: [bool; 6]) -> Progress {
        Progress {
            done,
            ..Progress::default()
        }
    }

    #[test]
    fn the_current_step_is_the_first_not_done() {
        assert_eq!(progress([false; 6]).current(), Some(Step::Name));
        assert_eq!(
            progress([true, true, false, false, false, false]).current(),
            Some(Step::Accept)
        );
        // a skipped step stays current, even with later ones done
        assert_eq!(
            progress([true, true, true, false, true, true]).current(),
            Some(Step::Compare)
        );
        assert_eq!(progress([true; 6]).current(), None);
        assert_eq!(
            progress([true, true, true, false, true, true]).done_count(),
            5
        );
    }

    fn friend(byte: u8, name: &str, status: FriendStatus, verified: bool) -> friend_list::Friend {
        friend_list::Friend {
            key: format!("{byte:02x}").repeat(32),
            name: name.into(),
            status,
            verified,
            ..friend_list::Friend::default()
        }
    }

    fn list(name: &str, friends: Vec<friend_list::Friend>) -> FriendList {
        let mut list = FriendList::new(&std::env::temp_dir());
        list.snapshot.book.name = name.into();
        list.snapshot.book.friends = friends;
        list
    }

    #[test]
    fn the_steps_follow_the_friend_list() {
        let fresh = Progress::of(&list("", Vec::new()), false);
        assert_eq!(fresh.current(), Some(Step::Name));

        let named = Progress::of(&list("Ana", Vec::new()), false);
        assert_eq!(named.current(), Some(Step::Link));
        assert!(!named.links_open);

        // Ana answered Ben's link: waiting for Ben
        let answered = Progress::of(
            &list("Ana", vec![friend(1, "Ben", FriendStatus::Pending, false)]),
            false,
        );
        assert_eq!(answered.current(), Some(Step::Accept));
        assert_eq!(answered.pending.as_deref(), Some("Ben"));

        let confirmed = Progress::of(
            &list("Ana", vec![friend(1, "Ben", FriendStatus::Active, false)]),
            false,
        );
        assert_eq!(confirmed.current(), Some(Step::Compare));
        assert_eq!(
            confirmed.unverified.as_ref().map(|(_, name)| name.as_str()),
            Some("Ben")
        );

        let verified = Progress::of(
            &list("Ana", vec![friend(1, "Ben", FriendStatus::Active, true)]),
            false,
        );
        assert_eq!(verified.current(), Some(Step::Share));
        assert_eq!(verified.share_from.as_deref(), Some("Ben"));

        // Ben shares from one core, which is online
        let mut ben = friend(1, "Ben", FriendStatus::Active, true);
        let fingerprint = "ab".repeat(32);
        ben.state = Some(friend_list::FriendState {
            seq: 1,
            expires: friend_list::now() + 86_400,
            cores: vec![
                serde_json::from_value(
                    serde_json::json!({ "fingerprint": fingerprint, "name": "Ben's PC" }),
                )
                .unwrap(),
            ],
        });
        let mut sharing = list("Ana", vec![ben]);
        sharing
            .presence
            .insert(("01".repeat(32), fingerprint), true);
        let shared = Progress::of(&sharing, false);
        assert_eq!(shared.current(), Some(Step::Play));
        assert!(shared.play_online);
        assert_eq!(Progress::of(&sharing, true).current(), None);

        // a retired friend doesn't count
        let retired = Progress::of(
            &list("Ana", vec![friend(1, "Ben", FriendStatus::Retired, true)]),
            false,
        );
        assert_eq!(retired.current(), Some(Step::Link));
    }
}
