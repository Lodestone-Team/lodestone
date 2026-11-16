//! How friends' servers look: their section on the Servers page, and the join dialog
//! (see [`super::friends`]).

use crate::skin::Skinned;
use crate::theme::px;
use gpui::{
    AnyElement, App, Context, Entity, IntoElement, KeyDownEvent, SharedString, div, prelude::*,
};
use launcher_core::{
    cores::{
        connect::{JoinError, Stage},
        friends::{InstanceMatch, Placement},
    },
    ui_skin::SurfaceId,
};

use lodestone_ui::values::truncate::sanitize;

use super::{
    Servers,
    extras::note,
    facts,
    friends::{
        FriendEntry, FriendStatus, JoinPrompt, JoinStage, Liveness, connection_label, game_address,
        join_error_text, port_taken_text, port_taken_values,
    },
};
use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, Dialog, Fact, FactLine, FactList, FactValue, Icon, Kind,
        LoadingDots, ObjectCard, ObjectChip, StatusBadge, UntitledName, ValueField, ValueKind,
        a11y::{Accessible, Role},
    },
    i18n::{CommonMsg, FactMsg, ServersMsg as M, format, t},
    theme::Tone,
    theme::{self, colors, fonts, leading, radius, text, weights},
};

/// A public invite link without its scheme: `connect.lodestone.cc/j/ABCDEFGH23`.
fn short_link(link: &str) -> &str {
    link.strip_prefix("https://")
        .or_else(|| link.strip_prefix("http://"))
        .unwrap_or(link)
}

fn paragraph(value: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_size(theme::text_size(text::MEDIUM))
        .line_height(theme::line_height(text::MEDIUM, leading::NORMAL))
        .text_color(crate::theme::ink::secondary())
        .child(value.into())
}

/// While a friend's server answers: its players, how this computer reaches it, and
/// the round trip, for its row's second line.
fn online_facts(status: &FriendStatus) -> Vec<Fact> {
    let FriendStatus::Online { server, kind, rtt } = status else {
        return Vec::new();
    };
    let server = launcher_core::cores::connect::server_instance(server);
    facts::server_players(&server)
        .into_iter()
        .chain([
            facts::connection(connection_label(*kind)),
            facts::ping(rtt.as_millis() as u64),
        ])
        .collect()
}

impl Servers {
    /// "Servers from invite links", in "Other hosts and servers": the ones joined
    /// before with an invite link from anyone. Those shared from a core on this
    /// computer show under it in "On this computer" instead. None when there are none.
    pub(super) fn render_invite_servers(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let local = self.local_cores();
        let rows: Vec<AnyElement> = self
            .friends
            .list
            .iter()
            .filter(|entry| entry.saved.placement(&local) == Placement::Friends)
            .map(|entry| self.friend_row(entry, false, cx))
            .collect();
        if rows.is_empty() {
            return None;
        }
        let card = div()
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
                        div().flex().flex_col().flex_1().min_w_0().child(
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
                        ),
                    ),
            )
            .children(rows)
            .into_any_element();
        Some(card)
    }

    /// A friend's server's row. `here`: it is shared from a core on this computer, and
    /// shows under that core in "On this computer".
    pub(super) fn friend_row(
        &self,
        entry: &FriendEntry,
        here: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let saved = &entry.saved;
        let id = saved.id.clone();
        // shared from this computer: as the core's owner the launcher would join it
        // directly (and the core's own list shows it, so it isn't here)
        let not_owner = here
            && !self
                .local_cores()
                .iter()
                .any(|core| core.uuid == saved.core_uuid && core.owner);
        let row_id = format!("friend-{}", saved.id.replace([':', '/', '.'], "-"));
        let playing = self
            .friends
            .sessions
            .get(&id)
            .and_then(|s| s.playing.clone());
        let shared = saved.shared();
        let as_server = launcher_core::cores::connect::server_instance(&shared);
        // the state, as a badge by the name; a server that doesn't answer says why
        let (dot, badge, problem): (theme::Color, Option<StatusBadge>, Option<String>) =
            if playing.is_some() {
                (
                    colors::GREEN_200,
                    Some(StatusBadge::new(t(M::FriendPlayingNow), Tone::Success)),
                    None,
                )
            } else {
                match &entry.status {
                    FriendStatus::Unknown => (colors::GRAY_500, None, None),
                    FriendStatus::Checking => (
                        colors::GRAY_500,
                        Some(StatusBadge::new(t(M::FriendChecking), Tone::Neutral)),
                        None,
                    ),
                    FriendStatus::Online { server, .. } => (
                        match Liveness::of_shared(server) {
                            Liveness::Stopped => colors::GRAY_500,
                            Liveness::Starting => colors::YELLOW,
                            _ => colors::GREEN_200,
                        },
                        Some(facts::state_badge(
                            launcher_core::cores::connect::server_instance(server).state,
                        )),
                        None,
                    ),
                    FriendStatus::Offline(error) => {
                        (colors::RED_200, None, Some(join_error_text(error).0))
                    }
                }
            };
        let unanswered = saved.never_answered();
        let mut line = Vec::new();
        if unanswered {
            line.push(Fact::new(
                t(M::InviteLinkKey),
                FactValue::mono(short_link(&saved.link).to_string()),
            ));
        } else {
            line.extend(facts::versions(&as_server));
            if !here {
                // under the core that shares it, the core is plain
                line.push(
                    Fact::icon(
                        Icon::Desktop,
                        t(M::SharedBy),
                        FactValue::object(sanitize(&saved.core_name, 64)),
                    )
                    .priority(3),
                );
            }
            if playing.is_none() {
                line.extend(online_facts(&entry.status));
            }
        }
        let checking = self.friend_checking(&id);
        // known to be stopped or starting: Play couldn't join it
        let blocked = playing.is_none() && self.friend_liveness(&id).blocks_play();
        let busy = matches!(
            self.friends.join.current().map(|p| &p.stage),
            Some(JoinStage::Connecting { .. } | JoinStage::Preparing { .. })
        );
        let (play_id, check_id, remove_id, untrust_id) =
            (id.clone(), id.clone(), id.clone(), id.clone());
        // an instance page's link to this server (its id is the server link's core)
        let overlay = self
            .highlight
            .as_ref()
            .is_some_and(|h| h.core == saved.id)
            .then(|| self.highlight_overlay())
            .flatten();
        div()
            .id(SharedString::from(row_id.clone()))
            .accessible(Role::ListItem, row_id.clone(), saved.name.clone())
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_4()
            .py_2p5()
            .border_t_1()
            .border_color(crate::theme::ink::border())
            .children(overlay)
            .child(div().flex_none().size(px(8.)).rounded_full().bg(dot))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .min_w_0()
                            .child(if unanswered {
                                // added without joining: its host hasn't answered yet
                                UntitledName::new(t(M::UnnamedServer)).into_any_element()
                            } else {
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .font_weight(weights::MEDIUMBOLD.into())
                                    .text_color(crate::theme::ink::text())
                                    .child(sanitize(&saved.name, 120))
                                    .into_any_element()
                            })
                            .when(unanswered, |this| {
                                this.child(StatusBadge::new(t(M::WaitingForReply), Tone::Neutral))
                            })
                            .children(badge),
                    )
                    .child(FactLine::row(SharedString::from(format!("{row_id}-facts"))).facts(line))
                    .when(not_owner, |this| {
                        this.child(note(t(M::ConnectToJoinDirectly)))
                    })
                    .children(problem.map(|line| note(line).text_color(colors::YELLOW)))
                    // the same every session: for Minecraft's server list
                    .children(saved.game_port.map(|port| {
                        let address = game_address(port);
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .pt_1()
                            .child(
                                div().flex_none().w(px(240.)).child(
                                    ValueField::new(
                                        SharedString::from(format!("magic-localhost-{row_id}")),
                                        address,
                                    )
                                    .label(t(M::MagicLocalhost))
                                    .kind(ValueKind::Code),
                                ),
                            )
                            .child(note(t(M::WhileConnected)))
                    }))
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
                            .disabled(busy || blocked)
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.play_friend(&play_id, cx)),
                            ),
                    ),
            )
            .into_any_element()
    }
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
    // pasting a link adds a server; the other stages join one
    let title = if matches!(prompt.stage, JoinStage::Paste { .. }) {
        t(M::AddServerTitle)
    } else {
        t(M::JoinFriendTitle)
    };
    let dialog = Dialog::new("join-friend", title, prompt.focus.clone())
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
                                        enter.update(cx, |this, cx| this.submit_invite_add(cx));
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
                // Add saves it without joining, even while its host is offline
                .footer(
                    Button::new("join-friend-add", t(M::AddInvite))
                        .variant(ButtonVariant::Primary)
                        .on_click({
                            let add = submit.clone();
                            move |_, _, cx| add.update(cx, |this, cx| this.submit_invite_add(cx))
                        }),
                )
                .footer(
                    Button::new("join-friend-submit", t(M::AddAndJoin))
                        .variant(ButtonVariant::Secondary)
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
                Some(_) => t(M::Connecting).to_string(),
                None => t(M::ConnectingToFriend).to_string(),
            };
            let card = name.as_ref().map(|name| {
                ObjectCard::new(sanitize(name, 120))
                    .kind(Kind::Server)
                    .compact(true)
                    .id("join-friend-connecting-server")
            });
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
                        .children(card)
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
            let liveness = Liveness::of_shared(shared);
            let stopped = liveness.blocks_play();
            let instance = this.friend_instance(id, cx);
            let instances = this.instances.read(cx);
            let instance_name = |instance_id: &str| {
                instances
                    .list()
                    .iter()
                    .find(|i| i.id == instance_id)
                    .map_or_else(|| instance_id.to_string(), |i| i.name.clone())
            };
            // the instance Play uses, with a badge for why it fits; or what Play makes
            let instance_value = |instance_id: &str, badge: M| {
                let name = sanitize(&instance_name(instance_id), 120);
                let plain = format::list(&[name.clone(), t(badge).to_string()]);
                FactValue::element(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(ObjectChip::new(name).kind(Kind::Instance))
                        .child(StatusBadge::new(t(badge), Tone::Success)),
                    plain,
                )
            };
            let instance_line = match &instance {
                Some(InstanceMatch::Linked(instance_id)) => {
                    instance_value(instance_id, M::MadeForThisServer)
                }
                Some(InstanceMatch::Matching(instance_id)) => {
                    instance_value(instance_id, M::SameVersion)
                }
                Some(InstanceMatch::Create) | None => FactValue::text(t(if shared.modpack {
                    M::MakesInstanceWithMods
                } else {
                    M::MakesInstance
                })),
            };
            let mut list = FactList::new("join-friend-facts")
                .row(
                    t(FactMsg::Host),
                    ObjectChip::new(sanitize(&shared.core_name, 64))
                        .kind(Kind::Computer)
                        .muted(),
                )
                .row(t(FactMsg::Game), facts::game(&server.game_version));
            if let Some(loader) = facts::loader(server.loader, server.loader_version.as_deref()) {
                list = list.row(t(FactMsg::Loader), loader);
            }
            list = list.row(
                t(FactMsg::Mods),
                t(if shared.modpack {
                    M::FriendHasMods
                } else {
                    M::FriendNoMods
                }),
            );
            list = list.row(t(FactMsg::Status), facts::state_badge(server.state));
            if let Some(players) =
                facts::players_value(server.state, server.player_count, server.max_player_count)
                    .filter(|_| !stopped)
            {
                list = list.row(t(FactMsg::Players), players);
            }
            if session.is_direct() {
                list = list.row(t(FactMsg::Connection), t(M::DirectConnection));
            } else {
                list = list
                    .row(t(FactMsg::Connection), connection_label(session.kind()))
                    .row(
                        t(FactMsg::Ping),
                        facts::ping_value(session.rtt().as_millis() as u64),
                    );
            }
            list = list.row(t(FactMsg::Instance), instance_line);
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
                .child(div().h(px(4.)))
                .child(list)
                .child(
                    ValueField::new("join-friend-address", session.address.clone())
                        .label(t(FactMsg::Address))
                        .kind(ValueKind::Code),
                );
            dialog
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(card)
                        .children(session.port_taken.map(|saved| {
                            let (title, body) = port_taken_text();
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(problem(title, body, colors::YELLOW))
                                .children(
                                    port_taken_values(saved, &session.address)
                                        .into_iter()
                                        .enumerate()
                                        .map(|(i, (key, value))| {
                                            div().pt_1().child(
                                                crate::components::ValueField::new(
                                                    ("join-port-taken", i),
                                                    value,
                                                )
                                                .label(key)
                                                .kind(crate::components::ValueKind::Code),
                                            )
                                        }),
                                )
                        }))
                        .when(stopped, |this| {
                            let (title, body) = if liveness == Liveness::Starting {
                                (M::FriendServerStarting, M::FriendServerStartingBody)
                            } else {
                                (M::FriendServerStopped, M::FriendServerStoppedBody)
                            };
                            this.child(
                                div()
                                    .id("join-friend-stopped")
                                    .accessible(Role::Status, "join-friend-stopped", t(title))
                                    .child(problem(
                                        t(title).into(),
                                        t(body).into(),
                                        colors::YELLOW,
                                    )),
                            )
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
                        // on again once the server runs (the dialog follows its state)
                        .disabled(stopped)
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
