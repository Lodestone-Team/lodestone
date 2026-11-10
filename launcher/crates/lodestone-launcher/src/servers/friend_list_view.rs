//! How the friend list looks: its section on the Servers page, and its dialogs (see
//! [`super::friend_list`]).

use gpui::{
    AnyElement, App, Context, Entity, IntoElement, KeyDownEvent, SharedString, div, prelude::*,
};
use launcher_core::{
    cores::connect::{SharedServer, server_instance},
    friend_list::{Friend, FriendStatus as Status, ops::LinkReply},
    ui_skin::SurfaceId,
};

use super::{
    Servers,
    extras::note,
    friend_list::{AddFriend, CoreServers, FriendDialog, FriendPrompt, expiry_label, hex, short},
    friends::JoinStage,
    status_label,
    view::server_detail,
};
use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, Checkbox, Dialog, Icon, LoadingDots,
        a11y::{Accessible, Role},
    },
    i18n::{CommonMsg, FriendListMsg as M, FriendListPlural as P, ServersMsg, format, t, tr, trn},
    skin::Skinned,
    theme::{self, colors, fonts, leading, px, radius, text, weights},
};

fn paragraph(value: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_size(theme::text_size(text::MEDIUM))
        .line_height(theme::line_height(text::MEDIUM, leading::NORMAL))
        .text_color(crate::theme::ink::secondary())
        .child(value.into())
}

/// A small word in a rounded box beside a name: "Not verified", "Hidden".
fn badge(label: &'static str, color: theme::Color) -> gpui::Div {
    div()
        .flex_none()
        .px_1p5()
        .rounded(radius::MD)
        .border_1()
        .border_color(color.opacity(40))
        .bg(color.opacity(10))
        .text_size(theme::text_size(text::CAPTION))
        .line_height(theme::line_height(text::SMALL, leading::NORMAL))
        .font_weight(weights::BOLD.into())
        .text_color(color)
        .child(label)
}

/// A safety code, in a monospaced box so its groups line up.
fn code_box(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    code: &str,
) -> gpui::Stateful<gpui::Div> {
    let id: SharedString = id.into();
    let label: SharedString = label.into();
    div()
        .id(id.clone())
        .accessible(Role::Label, id, format!("{label}: {code}"))
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .gap_1()
        .px_3()
        .py_2()
        .rounded(radius::LG)
        .border_1()
        .border_color(crate::theme::ink::border())
        .bg(colors::GRAY_900)
        .child(note(label))
        .child(
            div()
                .font_family(fonts::mono())
                .text_size(theme::text_size(text::H3))
                .font_weight(weights::BOLD.into())
                .text_color(crate::theme::ink::text())
                .child(code.to_string()),
        )
}

/// A warning box with an icon.
fn warning(body: impl Into<SharedString>, color: theme::Color) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_start()
        .gap_2()
        .px_3()
        .py_2()
        .rounded(radius::LG)
        .border_1()
        .border_color(color.opacity(40))
        .bg(color.opacity(8))
        .child(
            div()
                .pt_0p5()
                .child(Icon::TriangleExclamation.render(px(13.), color)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(theme::text_size(text::SMALL))
                .line_height(theme::line_height(text::SMALL, leading::NORMAL))
                .text_color(crate::theme::ink::text())
                .child(body.into()),
        )
}

fn working(label: impl Into<SharedString>) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(LoadingDots)
        .child(note(label))
}

fn row() -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .px_4()
        .py_2p5()
        .border_t_1()
        .border_color(crate::theme::ink::border())
}

/// "Alex" and "Sam": the names of friends a new name matches.
fn names(list: &[String]) -> String {
    format::list(list)
}

impl Servers {
    /// "Friends": this person, their friend links and answers, and their friends.
    pub(super) fn render_friend_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let list = &self.friend_list;
        let book = &list.snapshot.book;
        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_4()
            .py_3()
            .child(Icon::UserGroup.render(px(16.), colors::GRAY_400))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .id("friend-list-title")
                            .role(Role::Heading)
                            .aria_level(2)
                            .aria_label(t(M::Title))
                            .font_family(fonts::heading())
                            .text_size(theme::text_size(text::H3))
                            .font_weight(weights::BOLD.into())
                            .text_color(crate::theme::ink::text())
                            .child(t(M::Title)),
                    )
                    .child(note(t(M::Hint))),
            )
            .when(list.refreshing, |this| this.child(LoadingDots))
            .when(list.opened, |this| {
                this.child(
                    Button::new("friend-list-refresh", t(CommonMsg::Refresh))
                        .size(ButtonSize::Slim)
                        .variant(ButtonVariant::Ghost)
                        .icon(Icon::ArrowsRotate)
                        .disabled(list.refreshing)
                        .on_click(cx.listener(|this, _, _, cx| this.start_friend_refresh(cx))),
                )
            })
            .child(
                Button::new("friend-list-add", t(M::AddFriend))
                    .size(ButtonSize::Slim)
                    .icon(Icon::Plus)
                    .on_click(cx.listener(|this, _, _, cx| this.open_add_friend(None, cx))),
            );

        // you
        let you = if book.name.is_empty() {
            row()
                .id("friend-list-you")
                .child(Icon::CircleUser.render(px(16.), colors::GRAY_500))
                .child(note(t(M::NoNameYet)).flex_1().min_w_0())
                .child(
                    Button::new("friend-list-choose-name", t(M::ChooseName))
                        .size(ButtonSize::Slim)
                        .variant(ButtonVariant::Primary)
                        .on_click(cx.listener(|this, _, _, cx| this.open_friend_name(cx))),
                )
        } else {
            row()
                .id("friend-list-you")
                .accessible(
                    Role::Group,
                    "friend-list-you",
                    format!("{}: {}", t(M::You), book.name),
                )
                .child(Icon::CircleUser.render(px(16.), crate::theme::accent::light()))
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
                                .gap_2()
                                .child(note(t(M::You)))
                                .child(
                                    div()
                                        .truncate()
                                        .font_weight(weights::BOLD.into())
                                        .text_color(crate::theme::ink::text())
                                        .child(book.name.clone()),
                                ),
                        )
                        .when(!list.snapshot.safety_code.is_empty(), |this| {
                            this.child(
                                note(tr!(M::YourSafetyCode, code = list.snapshot.safety_code))
                                    .font_family(fonts::mono()),
                            )
                        }),
                )
                .child(
                    Button::new("friend-list-edit-name", t(M::EditName))
                        .size(ButtonSize::Slim)
                        .variant(ButtonVariant::Ghost)
                        .on_click(cx.listener(|this, _, _, cx| this.open_friend_name(cx))),
                )
                .child(
                    Button::new("friend-list-retire", t(M::Retire))
                        .size(ButtonSize::Slim)
                        .variant(ButtonVariant::Danger)
                        .on_click(cx.listener(|this, _, _, cx| this.open_friend_retire(cx))),
                )
        };

        let mut body: Vec<AnyElement> = vec![you.into_any_element()];
        if let Some(error) = &list.open_error {
            body.push(
                row()
                    .child(
                        note(format!("{}: {error}", t(M::OpenFailed)))
                            .flex_1()
                            .text_color(colors::RED_200),
                    )
                    .child(
                        Button::new("friend-list-retry", t(CommonMsg::TryAgain))
                            .size(ButtonSize::Slim)
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.friend_me(cx).detach();
                            })),
                    )
                    .into_any_element(),
            );
        }
        if let Some(problem) = &list.problem {
            body.push(
                row()
                    .child(Icon::TriangleExclamation.render(px(13.), colors::YELLOW))
                    .child(
                        note(tr!(M::RefreshProblem, error = problem))
                            .flex_1()
                            .min_w_0(),
                    )
                    .into_any_element(),
            );
        }
        if list.refreshing {
            body.push(row().child(working(t(M::Refreshing))).into_any_element());
        }
        if !book.links.is_empty() {
            body.push(self.render_links(cx));
        }
        let friends = list.sorted();
        if friends.is_empty() && list.open_error.is_none() {
            body.push(
                note(t(M::NoFriendsYet))
                    .px_4()
                    .py_3()
                    .border_t_1()
                    .border_color(crate::theme::ink::border())
                    .into_any_element(),
            );
        }
        for friend in friends {
            body.push(self.friend_list_row(friend, cx));
        }

        div()
            .id("friend-list")
            .flex()
            .flex_col()
            .w_full()
            .rounded(radius::XL)
            .border_1()
            .border_color(crate::theme::ink::border())
            .bg(colors::GRAY_850)
            .surface(SurfaceId::Card)
            .child(header)
            .children(body)
            .into_any_element()
    }

    /// "Your friend links": each open link, and the answers waiting under it.
    fn render_links(&self, cx: &mut Context<Self>) -> AnyElement {
        let list = &self.friend_list;
        let replies = list.waiting_replies();
        let mut out = div().flex().flex_col().child(
            row()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .font_weight(weights::BOLD.into())
                        .text_color(crate::theme::ink::text())
                        .child(t(M::LinksTitle)),
                )
                .child(
                    Button::new("friend-list-check-answers", t(M::CheckAnswers))
                        .size(ButtonSize::Slim)
                        .variant(ButtonVariant::Ghost)
                        .on_click(cx.listener(|this, _, _, cx| this.check_friend_replies(cx))),
                ),
        );
        for link in &list.snapshot.book.links {
            let code = link.code.clone();
            let id = format!("friend-link-{}", link.code.to_lowercase());
            let answers: Vec<&&LinkReply> =
                replies.iter().filter(|r| r.code == link.code).collect();
            out = out.child(
                div()
                    .id(SharedString::from(id.clone()))
                    .accessible(Role::ListItem, id.clone(), t(M::LinksTitle))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .pl_8()
                    .pr_4()
                    .py_1p5()
                    .child(Icon::Link.render(px(13.), colors::GRAY_500))
                    .child(
                        note(tr!(M::LinkWaiting, time = expiry_label(link.expires)))
                            .flex_1()
                            .min_w_0(),
                    )
                    .child(
                        Button::new(SharedString::from(format!("{id}-cancel")), t(M::CancelLink))
                            .size(ButtonSize::Slim)
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.cancel_friend_link(&code, cx)
                            })),
                    ),
            );
            for reply in answers {
                out = out.child(self.reply_row(reply, cx));
            }
        }
        out.into_any_element()
    }

    /// An answer to one of this person's links: who, their code, warnings, Accept.
    fn reply_row(&self, reply: &LinkReply, cx: &mut Context<Self>) -> AnyElement {
        let id = format!(
            "friend-reply-{}-{}",
            reply.code.to_lowercase(),
            short(&reply.key)
        );
        let busy = self.friend_list.busy.contains(&reply.key);
        let (accept_code, accept_key) = (reply.code.clone(), reply.key.clone());
        let (ignore_code, ignore_key) = (reply.code.clone(), reply.key.clone());
        div()
            .id(SharedString::from(id.clone()))
            .accessible(
                Role::ListItem,
                id.clone(),
                tr!(M::ReplyFrom, name = reply.name),
            )
            .flex()
            .flex_col()
            .gap_2()
            .mx_4()
            .mb_2()
            .ml_8()
            .px_3()
            .py_2p5()
            .rounded(radius::LG)
            .border_1()
            .border_color(crate::theme::accent::light().opacity(40))
            .bg(crate::theme::accent::light().opacity(6))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .font_weight(weights::BOLD.into())
                                    .text_color(crate::theme::ink::text())
                                    .child(tr!(M::ReplyFrom, name = reply.name)),
                            )
                            .child(
                                note(tr!(M::SafetyCode, code = reply.safety_code))
                                    .font_family(fonts::mono()),
                            ),
                    )
                    .when(busy, |this| this.child(LoadingDots))
                    .child(
                        Button::new(SharedString::from(format!("{id}-ignore")), t(M::Ignore))
                            .size(ButtonSize::Slim)
                            .variant(ButtonVariant::Ghost)
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.ignore_friend_reply(&ignore_code, &ignore_key, cx)
                            })),
                    )
                    .child(
                        Button::new(SharedString::from(format!("{id}-accept")), t(M::Accept))
                            .size(ButtonSize::Slim)
                            .variant(ButtonVariant::Primary)
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.accept_friend_reply(&accept_code, &accept_key, cx)
                            })),
                    ),
            )
            .when(reply.answers > 1, |this| {
                this.child(warning(
                    trn!(P::AnsweredTimes, reply.answers),
                    colors::YELLOW,
                ))
            })
            .when(!reply.same_name_as.is_empty(), |this| {
                this.child(warning(
                    tr!(M::SameNameAs, names = names(&reply.same_name_as)),
                    colors::YELLOW,
                ))
            })
            .into_any_element()
    }

    /// A friend: their name, whether codes were compared, which cores share now; and,
    /// expanded, their servers and what can be done about them.
    fn friend_list_row(&self, friend: &Friend, cx: &mut Context<Self>) -> AnyElement {
        let list = &self.friend_list;
        let key = friend.key.clone();
        let id = format!("friend-list-{}", short(&friend.key));
        let expanded = list.expanded.contains(&friend.key);
        let online = list.online(friend);
        let cores = friend.joinable_cores(launcher_core::friend_list::now());
        let (dot, status_line): (theme::Color, String) = match friend.status {
            Status::Pending => (colors::YELLOW, tr!(M::WaitingFor, name = friend.name)),
            Status::Retired => (colors::GRAY_500, t(M::Retired).into()),
            Status::Active if cores.is_empty() => (colors::GRAY_500, t(M::SharesNothing).into()),
            Status::Active => match online {
                Some(true) => {
                    let names: Vec<String> = cores
                        .iter()
                        .filter(|c| {
                            list.presence
                                .get(&(friend.key.clone(), hex(&c.fingerprint)))
                                == Some(&true)
                        })
                        .map(|c| c.name.clone())
                        .collect();
                    (
                        colors::GREEN_200,
                        tr!(M::OnlineOn, cores = format::list(&names)),
                    )
                }
                _ => (colors::GRAY_500, t(M::Offline).into()),
            },
        };
        let toggle_key = key.clone();
        let verify_key = key.clone();
        let mut row_el = div()
            .id(SharedString::from(id.clone()))
            .accessible(
                Role::ListItem,
                id.clone(),
                friend.display_name().to_string(),
            )
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(crate::theme::ink::border())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_2p5()
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
                                    .child(
                                        div()
                                            .truncate()
                                            .font_weight(weights::BOLD.into())
                                            .text_color(crate::theme::ink::text())
                                            .child(friend.display_name().to_string()),
                                    )
                                    .child(if friend.verified {
                                        badge(t(M::Verified), colors::GREEN_300)
                                    } else {
                                        badge(t(M::NotVerified), colors::YELLOW)
                                    })
                                    .when(friend.hidden, |this| {
                                        this.child(badge(t(M::Hidden), colors::GRAY_400))
                                    }),
                            )
                            .child(note(status_line))
                            .when(friend.nickname.is_some(), |this| {
                                this.child(note(tr!(M::TheirName, name = friend.name)))
                            }),
                    )
                    .when(!friend.verified, |this| {
                        this.child(
                            Button::new(
                                SharedString::from(format!("{id}-verify")),
                                t(M::CompareCodes),
                            )
                            .size(ButtonSize::Slim)
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(
                                move |this, _, _, cx| this.open_friend_verify(&verify_key, cx),
                            )),
                        )
                    })
                    .child(
                        Button::new(SharedString::from(format!("{id}-servers")), t(M::Servers))
                            .size(ButtonSize::Slim)
                            .variant(ButtonVariant::Ghost)
                            .icon_right(if expanded {
                                Icon::ChevronDown
                            } else {
                                Icon::ChevronRight
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_friend(&toggle_key, cx)
                            })),
                    ),
            );
        if expanded {
            row_el = row_el.child(self.friend_panel(friend, &id, cx));
        }
        row_el.into_any_element()
    }

    /// A friend's servers, by core, and the friend's actions.
    fn friend_panel(&self, friend: &Friend, id: &str, cx: &mut Context<Self>) -> AnyElement {
        let list = &self.friend_list;
        let joining = matches!(
            self.friends.join.current().map(|p| &p.stage),
            Some(JoinStage::Connecting { .. } | JoinStage::Preparing { .. })
        );
        let mut panel = div().flex().flex_col().gap_2().pl_8().pr_4().pb_3();
        let cores = friend.joinable_cores(launcher_core::friend_list::now());
        if friend.status == Status::Pending {
            panel = panel.child(note(tr!(M::WaitingFor, name = friend.name)));
        } else if cores.is_empty() {
            panel = panel.child(note(t(M::SharesNothing)));
        }
        for core in cores {
            let fingerprint = hex(&core.fingerprint);
            let slot = (friend.key.clone(), fingerprint.clone());
            let online = list.presence.get(&slot).copied();
            let core_id = format!("{id}-{}", &fingerprint[..8]);
            let mut block = div()
                .id(SharedString::from(core_id.clone()))
                .accessible(Role::Group, core_id.clone(), core.name.clone())
                .flex()
                .flex_col()
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
                        .px_3()
                        .py_2()
                        .child(Icon::Server.render(px(13.), colors::GRAY_400))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .font_weight(weights::BOLD.into())
                                .text_color(crate::theme::ink::text())
                                .child(core.name.clone()),
                        )
                        .child(
                            note(match online {
                                Some(true) => t(M::CoreOnline),
                                Some(false) => t(M::CoreOffline),
                                None => "",
                            })
                            .text_color(if online == Some(true) {
                                colors::GREEN_200
                            } else {
                                crate::theme::ink::muted()
                            }),
                        ),
                );
            match list.servers.get(&slot) {
                Some(CoreServers::Loading) => {
                    block = block.child(div().px_3().pb_2().child(working(t(M::LoadingServers))));
                }
                Some(CoreServers::Failed(error)) => {
                    block = block.child(
                        note(tr!(M::ServersFailed, error = error))
                            .px_3()
                            .pb_2()
                            .text_color(colors::YELLOW),
                    );
                }
                Some(CoreServers::Loaded(servers)) if servers.is_empty() => {
                    block = block.child(note(t(M::NoServersShared)).px_3().pb_2());
                }
                Some(CoreServers::Loaded(servers)) => {
                    for server in servers {
                        block = block.child(self.friend_server_row(
                            &friend.key,
                            &fingerprint,
                            &core_id,
                            server,
                            joining,
                            cx,
                        ));
                    }
                }
                None if online != Some(false) => {
                    let (key, fp) = slot.clone();
                    block = block.child(
                        div().px_3().pb_2().child(
                            Button::new(
                                SharedString::from(format!("{core_id}-load")),
                                t(M::ShowServers),
                            )
                            .size(ButtonSize::Slim)
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.friend_servers(key.clone(), fp.clone(), cx).detach()
                                },
                            )),
                        ),
                    );
                }
                None => {}
            }
            panel = panel.child(block);
        }
        let (verify_key, rename_key, hide_key, remove_key) = (
            friend.key.clone(),
            friend.key.clone(),
            friend.key.clone(),
            friend.key.clone(),
        );
        panel = panel.child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(
                    Button::new(
                        SharedString::from(format!("{id}-codes")),
                        t(M::CompareCodes),
                    )
                    .size(ButtonSize::Slim)
                    .variant(ButtonVariant::Ghost)
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.open_friend_verify(&verify_key, cx)),
                    ),
                )
                .child(
                    Button::new(
                        SharedString::from(format!("{id}-rename")),
                        t(CommonMsg::Rename),
                    )
                    .size(ButtonSize::Slim)
                    .variant(ButtonVariant::Ghost)
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.open_friend_rename(&rename_key, cx)),
                    ),
                )
                .child(
                    Button::new(
                        SharedString::from(format!("{id}-hide")),
                        t(if friend.hidden {
                            M::StopHiding
                        } else {
                            M::HideFromThem
                        }),
                    )
                    .size(ButtonSize::Slim)
                    .variant(ButtonVariant::Ghost)
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.toggle_friend_hidden(&hide_key, cx)),
                    ),
                )
                .child(
                    Button::new(
                        SharedString::from(format!("{id}-remove")),
                        t(CommonMsg::Remove),
                    )
                    .size(ButtonSize::Slim)
                    .variant(ButtonVariant::Danger)
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.open_friend_remove(&remove_key, cx)),
                    ),
                ),
        );
        panel.into_any_element()
    }

    fn friend_server_row(
        &self,
        key: &str,
        fingerprint: &str,
        core_id: &str,
        server: &SharedServer,
        joining: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let as_server = server_instance(server);
        let instance: String = server
            .instance_uuid
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
            .take(36)
            .collect();
        let id = format!("{core_id}-{instance}");
        let running = matches!(
            as_server.state,
            launcher_core::cores::ServerState::Running | launcher_core::cores::ServerState::Unknown
        );
        let (play_key, play_fp, play_server) =
            (key.to_string(), fingerprint.to_string(), server.clone());
        div()
            .id(SharedString::from(id.clone()))
            .accessible(Role::ListItem, id.clone(), server.name.clone())
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_3()
            .py_2()
            .border_t_1()
            .border_color(crate::theme::ink::border())
            .child(
                div()
                    .flex_none()
                    .size(px(6.))
                    .rounded_full()
                    .bg(if running {
                        colors::GREEN_200
                    } else {
                        colors::GRAY_500
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .text_color(crate::theme::ink::text())
                            .child(server.name.clone()),
                    )
                    .child(note(format!(
                        "{} · {}",
                        server_detail(&as_server),
                        status_label(&as_server)
                    ))),
            )
            .child(
                Button::new(
                    SharedString::from(format!("{id}-play")),
                    t(ServersMsg::Play),
                )
                .size(ButtonSize::Slim)
                .variant(ButtonVariant::Primary)
                .disabled(joining)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.play_friend_server(&play_key, &play_fp, &play_server, cx)
                })),
            )
            .into_any_element()
    }
}

/// An input that does `submit` on Enter.
fn on_enter(
    servers: &Entity<Servers>,
    input: AnyElement,
    submit: fn(&mut Servers, &mut Context<Servers>),
) -> gpui::Div {
    let enter = servers.clone();
    div()
        .on_key_down(move |event: &KeyDownEvent, _, cx| {
            if event.keystroke.key == "enter" {
                enter.update(cx, submit);
                cx.stop_propagation();
            }
        })
        .child(input)
}

fn error_note(error: &Option<SharedString>) -> Option<gpui::Div> {
    error.clone().map(|error| {
        note(error)
            .font_weight(weights::BOLD.into())
            .text_color(colors::RED_200)
    })
}

/// The friend list's dialog.
pub fn friend_list_dialog(
    servers: &Entity<Servers>,
    prompt: &FriendPrompt,
    cx: &App,
) -> AnyElement {
    let this = servers.read(cx);
    let close = servers.clone();
    let cancel = servers.clone();
    let cancel_button = move |label: &'static str| {
        let cancel = cancel.clone();
        Button::new("friend-dialog-cancel", label)
            .variant(ButtonVariant::Secondary)
            .on_click(move |_, _, cx| cancel.update(cx, |this, cx| this.close_friend_dialog(cx)))
    };
    let dialog = |id: &'static str, title: String| {
        let close = close.clone();
        Dialog::new(id, title, prompt.focus.clone())
            .width(px(560.))
            .on_close(move |_, cx| close.update(cx, |this, cx| this.close_friend_dialog(cx)))
    };
    let name_of = |key: &str| {
        this.friend_list
            .friend(key)
            .map(|f| f.display_name().to_string())
            .unwrap_or_default()
    };
    match &prompt.dialog {
        FriendDialog::Name {
            input,
            error,
            saving,
        } => {
            let save = servers.clone();
            dialog("friend-name", t(M::NameTitle).into())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(paragraph(t(M::NameBody)))
                        .child(on_enter(
                            servers,
                            input.clone().into_any_element(),
                            |this, cx| this.save_friend_name(cx),
                        ))
                        .children(error_note(error)),
                )
                .footer(
                    Button::new("friend-name-save", t(CommonMsg::Save))
                        .variant(ButtonVariant::Primary)
                        .loading(*saving)
                        .disabled(*saving)
                        .on_click(move |_, _, cx| {
                            save.update(cx, |this, cx| this.save_friend_name(cx))
                        }),
                )
                .footer(cancel_button(t(CommonMsg::Cancel)))
                .into_any_element()
        }
        FriendDialog::Add(add) => add_dialog(
            servers,
            this,
            add,
            dialog("friend-add", t(M::AddTitle).into()),
            cancel_button,
        ),
        FriendDialog::Verify { key } => {
            let friend = this.friend_list.friend(key);
            let name = name_of(key);
            let verified = friend.is_some_and(|f| f.verified);
            let their_code = friend.map(|f| f.safety_code()).unwrap_or_default();
            let toggle = servers.clone();
            let toggle_key = key.clone();
            dialog("friend-verify", tr!(M::VerifyTitle, name = name))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(paragraph(tr!(M::VerifyBody, name = name)))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_3()
                                .child(code_box(
                                    "friend-verify-yours",
                                    t(M::YourCode),
                                    &this.friend_list.snapshot.safety_code,
                                ))
                                .child(code_box(
                                    "friend-verify-theirs",
                                    tr!(M::TheirCode, name = name),
                                    &their_code,
                                )),
                        )
                        .child(
                            Checkbox::new("friend-verify-match", verified, t(M::CodesMatch))
                                .on_change(move |value, _, cx| {
                                    let value = *value;
                                    toggle.update(cx, |this, cx| {
                                        this.set_friend_verified(&toggle_key, value, cx)
                                    })
                                }),
                        ),
                )
                .footer(cancel_button(t(CommonMsg::Done)))
                .into_any_element()
        }
        FriendDialog::Rename { key, input } => {
            let chosen = this
                .friend_list
                .friend(key)
                .map(|f| f.name.clone())
                .unwrap_or_default();
            let save = servers.clone();
            let clear = servers.clone();
            dialog("friend-rename", tr!(M::RenameTitle, name = name_of(key)))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(paragraph(tr!(M::RenameBody, name = chosen)))
                        .child(on_enter(
                            servers,
                            input.clone().into_any_element(),
                            |this, cx| this.save_friend_nickname(false, cx),
                        )),
                )
                .footer(
                    Button::new("friend-rename-save", t(CommonMsg::Save))
                        .variant(ButtonVariant::Primary)
                        .on_click(move |_, _, cx| {
                            save.update(cx, |this, cx| this.save_friend_nickname(false, cx))
                        }),
                )
                .footer(
                    Button::new("friend-rename-clear", t(M::UseTheirName))
                        .variant(ButtonVariant::Ghost)
                        .on_click(move |_, _, cx| {
                            clear.update(cx, |this, cx| this.save_friend_nickname(true, cx))
                        }),
                )
                .footer(cancel_button(t(CommonMsg::Cancel)))
                .into_any_element()
        }
        FriendDialog::Remove {
            key,
            removing,
            error,
        } => {
            let remove = servers.clone();
            let name = name_of(key);
            dialog("friend-remove", tr!(M::RemoveTitle, name = name))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(paragraph(t(M::RemoveBody)))
                        .children(error_note(error)),
                )
                .footer(
                    Button::new("friend-remove-confirm", t(M::RemoveFriend))
                        .variant(ButtonVariant::Danger)
                        .loading(*removing)
                        .disabled(*removing)
                        .on_click(move |_, _, cx| {
                            remove.update(cx, |this, cx| this.confirm_friend_remove(cx))
                        }),
                )
                .footer(cancel_button(t(CommonMsg::Cancel)))
                .into_any_element()
        }
        FriendDialog::Retire {
            understood,
            retiring,
            error,
        } => {
            let retire = servers.clone();
            let tick = servers.clone();
            dialog("friend-retire", t(M::RetireTitle).into())
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(paragraph(t(M::RetireBody)))
                        .child(warning(t(M::RetireWarning), colors::RED_200))
                        .child(
                            Checkbox::new(
                                "friend-retire-understood",
                                *understood,
                                t(M::RetireUnderstood),
                            )
                            .disabled(*retiring)
                            .on_change(move |value, _, cx| {
                                let value = *value;
                                tick.update(cx, |this, cx| this.set_retire_understood(value, cx))
                            }),
                        )
                        .children(error_note(error)),
                )
                .footer(
                    Button::new("friend-retire-confirm", t(M::RetireConfirm))
                        .variant(ButtonVariant::Danger)
                        .loading(*retiring)
                        .disabled(!*understood || *retiring)
                        .on_click(move |_, _, cx| {
                            retire.update(cx, |this, cx| this.confirm_friend_retire(cx))
                        }),
                )
                .footer(cancel_button(t(CommonMsg::Cancel)))
                .into_any_element()
        }
    }
}

/// The Add friend dialog: make a link, or open one (its card first, then Add).
fn add_dialog(
    servers: &Entity<Servers>,
    this: &Servers,
    add: &AddFriend,
    dialog: Dialog,
    cancel_button: impl Fn(&'static str) -> Button,
) -> AnyElement {
    if let Some(name) = &add.added {
        return dialog
            .child(
                div()
                    .id("friend-add-done")
                    .accessible(Role::Status, "friend-add-done", tr!(M::Added, name = name))
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap_3()
                    .child(Icon::CircleCheck.render(px(16.), colors::GREEN_300))
                    .child(paragraph(tr!(M::Added, name = name)).flex_1()),
            )
            .footer(cancel_button(t(CommonMsg::Done)))
            .into_any_element();
    }
    let has_name = add.name_input.is_none();
    let mut body = div().flex().flex_col().gap_4();
    if let Some(input) = &add.name_input {
        let save = servers.clone();
        body = body.child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(paragraph(t(M::NeedNameFirst)))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            on_enter(servers, input.clone().into_any_element(), |this, cx| {
                                this.save_friend_name(cx)
                            })
                            .flex_1(),
                        )
                        .child(
                            Button::new("friend-add-save-name", t(CommonMsg::Save))
                                .variant(ButtonVariant::Primary)
                                .loading(add.saving_name)
                                .disabled(add.saving_name)
                                .on_click(move |_, _, cx| {
                                    save.update(cx, |this, cx| this.save_friend_name(cx))
                                }),
                        ),
                ),
        );
    }

    // a looked-up card: who it's from, and Add
    if let Some((_, preview)) = &add.preview {
        let back = servers.clone();
        let confirm = servers.clone();
        let card = div()
            .id("friend-add-card")
            .accessible(
                Role::Group,
                "friend-add-card",
                tr!(M::PreviewFrom, name = preview.name),
            )
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .font_family(fonts::heading())
                    .text_size(theme::text_size(text::H3))
                    .font_weight(weights::BOLD.into())
                    .text_color(crate::theme::ink::text())
                    .child(tr!(M::PreviewFrom, name = preview.name)),
            )
            .child(code_box(
                "friend-add-code",
                tr!(M::TheirCode, name = preview.name),
                &preview.safety_code,
            ))
            .child(note(tr!(M::PreviewCompare, name = preview.name)))
            .when(!preview.same_name_as.is_empty(), |this| {
                this.child(warning(
                    tr!(M::SameNameAs, names = names(&preview.same_name_as)),
                    colors::YELLOW,
                ))
            })
            .when(preview.already_friends, |this| {
                this.child(warning(
                    tr!(M::AlreadyFriends, name = preview.name),
                    colors::YELLOW,
                ))
            })
            .children(error_note(&add.error));
        return dialog
            .child(body.child(card))
            .footer(
                Button::new("friend-add-confirm", t(CommonMsg::Add))
                    .variant(ButtonVariant::Primary)
                    .loading(add.adding)
                    .disabled(add.adding || !has_name || preview.already_friends)
                    .on_click(move |_, _, cx| {
                        confirm.update(cx, |this, cx| this.add_friend_from_link(cx))
                    }),
            )
            .footer(
                Button::new("friend-add-back", t(CommonMsg::Back))
                    .variant(ButtonVariant::Secondary)
                    .disabled(add.adding)
                    .on_click(move |_, _, cx| {
                        back.update(cx, |this, cx| this.clear_friend_preview(cx))
                    }),
            )
            .into_any_element();
    }

    // send a link
    let make = servers.clone();
    let copy = servers.clone();
    let mut send = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .font_weight(weights::BOLD.into())
                .text_color(crate::theme::ink::text())
                .child(t(M::SendLinkTitle)),
        )
        .child(note(t(M::SendLinkBody)));
    match &this.friend_list.made {
        Some(made) => {
            send = send
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id("friend-add-link")
                                .accessible(Role::Label, "friend-add-link", made.link.clone())
                                .flex_1()
                                .min_w_0()
                                .px_3()
                                .py_2()
                                .rounded(radius::LG)
                                .border_1()
                                .border_color(crate::theme::ink::border())
                                .bg(colors::GRAY_900)
                                .font_family(fonts::mono())
                                .text_size(theme::text_size(text::SMALL))
                                .text_color(crate::theme::ink::text())
                                .truncate()
                                .child(made.link.clone()),
                        )
                        .child(
                            Button::new(
                                "friend-add-copy",
                                t(if add.copied {
                                    CommonMsg::Copied
                                } else {
                                    CommonMsg::Copy
                                }),
                            )
                            .icon(if add.copied { Icon::Check } else { Icon::Copy })
                            .on_click(move |_, _, cx| {
                                copy.update(cx, |this, cx| this.copy_friend_link(cx))
                            }),
                        ),
                )
                .child(note(t(M::LinkWorksOnce)));
        }
        None => {
            send = send.child(
                div().flex().flex_row().child(
                    Button::new("friend-add-make-link", t(M::MakeLink))
                        .icon(Icon::Link)
                        .loading(add.making)
                        .disabled(add.making || !has_name)
                        .on_click(move |_, _, cx| {
                            make.update(cx, |this, cx| this.make_friend_link(cx))
                        }),
                ),
            );
        }
    }

    // open their link
    let look = servers.clone();
    let open = div()
        .flex()
        .flex_col()
        .gap_2()
        .pt_3()
        .border_t_1()
        .border_color(crate::theme::ink::border())
        .child(
            div()
                .font_weight(weights::BOLD.into())
                .text_color(crate::theme::ink::text())
                .child(t(M::OpenLinkTitle)),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    on_enter(servers, add.input.clone().into_any_element(), |this, cx| {
                        this.look_up_friend_link(cx)
                    })
                    .flex_1(),
                )
                .child(
                    Button::new("friend-add-look-up", t(M::LookUp))
                        .loading(add.looking)
                        .disabled(add.looking)
                        .on_click(move |_, _, cx| {
                            look.update(cx, |this, cx| this.look_up_friend_link(cx))
                        }),
                ),
        )
        .children(error_note(&add.error));
    dialog
        .child(body.child(send).child(open))
        .footer(cancel_button(t(CommonMsg::Close)))
        .into_any_element()
}
