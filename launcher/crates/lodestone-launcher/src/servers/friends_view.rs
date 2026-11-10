//! How friends' servers look: their section on the Servers page, and the join dialog
//! (see [`super::friends`]).

use crate::skin::Skinned;
use crate::theme::px;
use gpui::{
    AnyElement, App, Context, Entity, IntoElement, KeyDownEvent, SharedString, div, prelude::*,
};
use launcher_core::{
    cores::{
        ServerState,
        connect::{JoinError, Stage},
        friends::InstanceMatch,
    },
    ui_skin::SurfaceId,
};

use super::{
    Servers,
    extras::note,
    friends::{
        FriendEntry, FriendStatus, JoinPrompt, JoinStage, connection_label, join_error_text,
    },
    status_label,
    view::server_detail,
};
use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, Dialog, Icon, LoadingDots,
        a11y::{Accessible, Role},
    },
    i18n::{CommonMsg, ServersMsg as M, t, tr},
    theme::{self, colors, fonts, leading, radius, text, weights},
};

fn paragraph(value: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_size(theme::text_size(text::MEDIUM))
        .line_height(theme::line_height(text::MEDIUM, leading::NORMAL))
        .text_color(crate::theme::ink::secondary())
        .child(value.into())
}

/// "Running · 2 / 20 players · over the local network, 12 ms".
fn online_line(status: &FriendStatus) -> Option<String> {
    let FriendStatus::Online { server, kind, rtt } = status else {
        return None;
    };
    let state = status_label(&launcher_core::cores::connect::server_instance(server));
    Some(format!(
        "{state} · {}",
        tr!(
            M::ConnectedVia,
            how = connection_label(*kind),
            ms = rtt.as_millis().to_string()
        )
    ))
}

impl Servers {
    /// "Friends' servers": the ones joined before, and "Join a friend".
    pub(super) fn render_friends(&self, cx: &mut Context<Self>) -> AnyElement {
        let rows: Vec<AnyElement> = self
            .friends
            .list
            .iter()
            .map(|entry| self.friend_row(entry, cx))
            .collect();
        let empty = rows.is_empty();
        div()
            .id("friends-servers")
            .flex()
            .flex_col()
            .w_full()
            .rounded(radius::XL)
            .border_1()
            .border_color(crate::theme::ink::border())
            .bg(colors::GRAY_850)
            .surface(SurfaceId::Card)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .child(Icon::User.render(px(16.), colors::GRAY_400))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .id("friends-title")
                                    .role(Role::Heading)
                                    .aria_level(2)
                                    .aria_label(t(M::FriendsTitle))
                                    .font_family(fonts::heading())
                                    .text_size(theme::text_size(text::H3))
                                    .font_weight(weights::BOLD.into())
                                    .text_color(crate::theme::ink::text())
                                    .child(t(M::FriendsTitle)),
                            )
                            .child(note(t(M::FriendsHint))),
                    )
                    .child(
                        Button::new("join-friend", t(M::JoinAFriend))
                            .size(ButtonSize::Slim)
                            .icon(Icon::Plus)
                            .on_click(cx.listener(|this, _, _, cx| this.open_join(None, cx))),
                    ),
            )
            .when(empty, |this| {
                this.child(
                    note(t(M::NoFriendsYet))
                        .px_4()
                        .py_3()
                        .border_t_1()
                        .border_color(crate::theme::ink::border()),
                )
            })
            .children(rows)
            .into_any_element()
    }

    fn friend_row(&self, entry: &FriendEntry, cx: &mut Context<Self>) -> AnyElement {
        let saved = &entry.saved;
        let id = saved.id.clone();
        let row_id = format!("friend-{}", saved.id.replace([':', '/', '.'], "-"));
        let playing = self
            .friends
            .sessions
            .get(&id)
            .and_then(|s| s.playing.clone());
        let shared = saved.shared();
        let as_server = launcher_core::cores::connect::server_instance(&shared);
        let (dot, status): (theme::Color, Option<(String, theme::Color)>) = if playing.is_some() {
            (
                colors::GREEN_200,
                Some((t(M::FriendPlayingNow).into(), colors::GREEN_200)),
            )
        } else {
            match &entry.status {
                FriendStatus::Unknown => (colors::GRAY_500, None),
                FriendStatus::Checking => (
                    colors::GRAY_500,
                    Some((t(M::FriendChecking).into(), crate::theme::ink::muted())),
                ),
                status @ FriendStatus::Online { server, .. } => (
                    match server.state.as_deref() {
                        Some("Stopped") | Some("Error") => colors::GRAY_500,
                        _ => colors::GREEN_200,
                    },
                    online_line(status).map(|line| (line, crate::theme::ink::secondary())),
                ),
                FriendStatus::Offline(error) => (
                    colors::RED_200,
                    Some((join_error_text(error).0, colors::YELLOW)),
                ),
            }
        };
        let checking = matches!(entry.status, FriendStatus::Checking);
        let busy = matches!(
            self.friends.join.current().map(|p| &p.stage),
            Some(JoinStage::Connecting { .. } | JoinStage::Preparing { .. })
        );
        let (play_id, check_id, remove_id, untrust_id) =
            (id.clone(), id.clone(), id.clone(), id.clone());
        div()
            .id(SharedString::from(row_id.clone()))
            .accessible(Role::ListItem, row_id.clone(), saved.name.clone())
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_4()
            .py_2p5()
            .border_t_1()
            .border_color(crate::theme::ink::border())
            .child(div().flex_none().size(px(8.)).rounded_full().bg(dot))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .font_weight(weights::BOLD.into())
                            .text_color(crate::theme::ink::text())
                            .child(saved.name.clone()),
                    )
                    .child(note(format!(
                        "{} · {}",
                        server_detail(&as_server),
                        tr!(M::SharedBy, name = saved.core_name)
                    )))
                    .children(status.map(|(line, color)| note(line).text_color(color)))
                    .when(saved.is_trusted(), |this| {
                        this.child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .child(note(t(M::TrustedForMods)))
                                .child(
                                    Button::new(
                                        SharedString::from(format!("untrust-{row_id}")),
                                        t(M::StopTrusting),
                                    )
                                    .size(ButtonSize::Slim)
                                    .variant(ButtonVariant::Ghost)
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| this.untrust_friend(&untrust_id, cx),
                                    )),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .flex_none()
                    .when(checking, |this| this.child(LoadingDots))
                    .child(
                        Button::new(
                            SharedString::from(format!("check-{row_id}")),
                            t(M::FriendCheck),
                        )
                        .size(ButtonSize::Slim)
                        .variant(ButtonVariant::Ghost)
                        .disabled(checking || playing.is_some())
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.check_friend(&check_id, cx)),
                        ),
                    )
                    .child(
                        Button::new(
                            SharedString::from(format!("remove-{row_id}")),
                            t(CommonMsg::Remove),
                        )
                        .size(ButtonSize::Slim)
                        .variant(ButtonVariant::Ghost)
                        .disabled(playing.is_some())
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.remove_friend(&remove_id, cx)),
                        ),
                    )
                    .child(
                        Button::new(SharedString::from(format!("play-{row_id}")), t(M::Play))
                            .size(ButtonSize::Slim)
                            .variant(ButtonVariant::Primary)
                            .disabled(busy)
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.play_friend(&play_id, cx)),
                            ),
                    ),
            )
            .into_any_element()
    }
}

/// A line with a label and its value, in the server's card.
fn fact(label: &'static str, value: impl Into<SharedString>) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .gap_3()
        .child(
            div()
                .flex_none()
                .w(px(120.))
                .text_size(theme::text_size(text::SMALL))
                .line_height(theme::line_height(text::SMALL, leading::NORMAL))
                .text_color(crate::theme::ink::muted())
                .child(label),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(theme::text_size(text::SMALL))
                .line_height(theme::line_height(text::SMALL, leading::NORMAL))
                .text_color(crate::theme::ink::text())
                .child(value.into()),
        )
}

/// A warning or error box: a title and what to do.
fn problem(title: String, body: String, color: theme::Color) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_start()
        .gap_3()
        .px_3()
        .py_2()
        .rounded(radius::LG)
        .border_1()
        .border_color(color.opacity(40))
        .bg(color.opacity(8))
        .child(
            div()
                .pt_0p5()
                .child(Icon::TriangleExclamation.render(px(14.), color)),
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
                        .font_weight(weights::BOLD.into())
                        .text_color(crate::theme::ink::text())
                        .child(title),
                )
                .child(
                    div()
                        .text_size(theme::text_size(text::SMALL))
                        .line_height(theme::line_height(text::SMALL, leading::NORMAL))
                        .text_color(crate::theme::ink::secondary())
                        .child(body),
                ),
        )
}

/// A row of dots and what is going on.
fn working(label: impl Into<SharedString>) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .py_2()
        .child(LoadingDots)
        .child(paragraph(label))
}

/// The join dialog.
pub fn join_dialog(servers: &Entity<Servers>, prompt: &JoinPrompt, cx: &App) -> AnyElement {
    let this = servers.read(cx);
    let close = servers.clone();
    let cancel = servers.clone();
    let dialog = Dialog::new("join-friend", t(M::JoinFriendTitle), prompt.focus.clone())
        .width(px(560.))
        .on_close(move |_, cx| close.update(cx, |this, cx| this.close_join(cx)));
    let cancel_button = move |label: &'static str| {
        let cancel = cancel.clone();
        Button::new("join-friend-cancel", label)
            .variant(ButtonVariant::Secondary)
            .on_click(move |_, _, cx| cancel.update(cx, |this, cx| this.close_join(cx)))
    };
    match &prompt.stage {
        JoinStage::Paste { input, error } => {
            let submit = servers.clone();
            let enter = servers.clone();
            dialog
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(paragraph(t(M::JoinFriendIntro)))
                        .child(
                            div()
                                .on_key_down(move |event: &KeyDownEvent, _, cx| {
                                    if event.keystroke.key == "enter" {
                                        enter.update(cx, |this, cx| this.submit_invite(cx));
                                        cx.stop_propagation();
                                    }
                                })
                                .child(input.clone()),
                        )
                        .children(error.clone().map(|error| {
                            note(error)
                                .font_weight(weights::BOLD.into())
                                .text_color(colors::RED_200)
                        })),
                )
                .footer(
                    Button::new("join-friend-submit", t(M::Join))
                        .variant(ButtonVariant::Primary)
                        .on_click(move |_, _, cx| {
                            submit.update(cx, |this, cx| this.submit_invite(cx))
                        }),
                )
                .footer(cancel_button(t(CommonMsg::Cancel)))
                .into_any_element()
        }
        JoinStage::Connecting {
            progress,
            since,
            name,
        } => {
            let heading = match name {
                Some(name) => tr!(M::ConnectingTo, name = name),
                None => t(M::ConnectingToFriend).to_string(),
            };
            let detail = match progress.stage() {
                Stage::Connecting if since.elapsed().as_secs() >= 12 => t(M::StageFindingSlow),
                Stage::Connecting => t(M::StageFinding),
                Stage::ReadingInfo => t(M::StageReading),
            };
            dialog
                .child(
                    div()
                        .id("join-friend-progress")
                        .accessible(Role::Status, "join-friend-progress", heading.clone())
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(working(heading))
                        .child(note(detail)),
                )
                .footer(cancel_button(t(CommonMsg::Cancel)))
                .into_any_element()
        }
        JoinStage::Ready { id, error } => {
            let Some(session) = this.friends.sessions.get(id) else {
                return dialog
                    .child(working(t(CommonMsg::Loading)))
                    .footer(cancel_button(t(CommonMsg::Cancel)))
                    .into_any_element();
            };
            let shared = session.shared();
            let server = &session.server;
            let stopped = matches!(server.state, ServerState::Stopped | ServerState::Error);
            let instance = this.friend_instance(id, cx);
            let instances = this.instances.read(cx);
            let instance_name = |instance_id: &str| {
                instances
                    .list()
                    .iter()
                    .find(|i| i.id == instance_id)
                    .map_or_else(|| instance_id.to_string(), |i| i.name.clone())
            };
            let instance_line = match &instance {
                Some(InstanceMatch::Linked(instance_id)) => {
                    tr!(M::PlaysWithLinked, name = instance_name(instance_id))
                }
                Some(InstanceMatch::Matching(instance_id)) => {
                    tr!(M::PlaysWithInstance, name = instance_name(instance_id))
                }
                Some(InstanceMatch::Create) | None => t(if shared.modpack {
                    M::MakesInstanceWithMods
                } else {
                    M::MakesInstance
                })
                .to_string(),
            };
            let players = super::players_label(server);
            let play = servers.clone();
            let card = div()
                .flex()
                .flex_col()
                .gap_1p5()
                .px_4()
                .py_3()
                .rounded(radius::LG)
                .border_1()
                .border_color(crate::theme::ink::border())
                .bg(colors::GRAY_900)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .flex_none()
                                .size(px(8.))
                                .rounded_full()
                                .bg(if stopped {
                                    colors::GRAY_500
                                } else {
                                    colors::GREEN_200
                                }),
                        )
                        .child(
                            div()
                                .id("join-friend-server")
                                .role(Role::Heading)
                                .aria_level(2)
                                .aria_label(server.name.clone())
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .font_family(fonts::heading())
                                .text_size(theme::text_size(text::H3))
                                .font_weight(weights::BOLD.into())
                                .text_color(crate::theme::ink::text())
                                .child(server.name.clone()),
                        ),
                )
                .child(note(tr!(M::SharedBy, name = shared.core_name)))
                .child(div().h(px(4.)))
                .child(fact(t(M::FactVersion), server_detail(server)))
                .child(fact(
                    t(M::FactMods),
                    t(if shared.modpack {
                        M::FriendHasMods
                    } else {
                        M::FriendNoMods
                    }),
                ))
                .child(fact(
                    t(M::FactStatus),
                    match players {
                        Some(players) if !stopped => {
                            format!("{} · {players}", super::state_label(server.state))
                        }
                        _ => status_label(server),
                    },
                ))
                .child(fact(
                    t(M::FactConnection),
                    tr!(
                        M::ConnectedVia,
                        how = connection_label(session.kind()),
                        ms = session.rtt().as_millis().to_string()
                    ),
                ))
                .child(fact(t(M::FactInstance), instance_line));
            dialog
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(card)
                        .when(stopped, |this| {
                            this.child(problem(
                                t(M::FriendServerStopped).into(),
                                t(M::FriendServerStoppedBody).into(),
                                colors::YELLOW,
                            ))
                        })
                        .children(
                            error
                                .clone()
                                .map(|(title, body)| problem(title, body, colors::RED_200)),
                        ),
                )
                .footer(
                    Button::new("join-friend-play", t(M::Play))
                        .variant(ButtonVariant::Primary)
                        .on_click(move |_, _, cx| play.update(cx, |this, cx| this.friend_play(cx))),
                )
                .footer(cancel_button(t(CommonMsg::Cancel)))
                .into_any_element()
        }
        JoinStage::Preparing { id } => {
            let label = this
                .friends
                .sessions
                .get(id)
                .and_then(|s| this.busy.get(&(id.clone(), s.server.uuid.clone())))
                .map_or_else(|| t(M::FriendStarting).to_string(), |busy| busy.label());
            dialog
                .child(
                    div()
                        .id("join-friend-preparing")
                        .accessible(Role::Status, "join-friend-preparing", label.clone())
                        .child(working(label)),
                )
                .footer(cancel_button(t(CommonMsg::Cancel)))
                .into_any_element()
        }
        JoinStage::Failed {
            error,
            source: _,
            name,
        } => {
            let (title, body) = join_error_text(error);
            let wrong_host = matches!(error, JoinError::WrongHost);
            let can_retry = !matches!(error, JoinError::WrongHost | JoinError::Ended);
            let retry = servers.clone();
            dialog
                .child(
                    div()
                        .id("join-friend-error")
                        .accessible(Role::Alert, "join-friend-error", title.clone())
                        .flex()
                        .flex_col()
                        .gap_3()
                        .children(
                            name.clone()
                                .map(|name| paragraph(name).font_weight(weights::BOLD.into())),
                        )
                        .child(problem(
                            title,
                            body,
                            if wrong_host {
                                colors::RED_200
                            } else {
                                colors::YELLOW
                            },
                        )),
                )
                .when(can_retry, |dialog| {
                    dialog.footer(
                        Button::new("join-friend-retry", t(CommonMsg::TryAgain))
                            .variant(ButtonVariant::Primary)
                            .on_click(move |_, _, cx| {
                                retry.update(cx, |this, cx| this.retry_join(cx))
                            }),
                    )
                })
                .footer(cancel_button(t(CommonMsg::Close)))
                .into_any_element()
        }
    }
}
