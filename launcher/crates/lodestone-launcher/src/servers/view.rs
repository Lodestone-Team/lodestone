//! How the Servers page looks: each core as a card with its servers, and the dialogs
//! over the window (Add server, the mod update prompt).

use gpui::{
    AnyElement, Context, Entity, IntoElement, Render, SharedString, Subscription, Window, div,
    prelude::*, px,
};
use launcher_core::cores::{
    CoreError, ServerInstance, ServerLoader, ServerState, client::display_address,
    models::ADDRESS_HIDDEN,
};

use super::{CoreEntry, CoreStatus, Servers};
use crate::{
    components::{Button, ButtonSize, ButtonVariant, Dialog, Icon, LoadingDots},
    theme::{self, colors, fonts, leading, radius, text, weights},
};

fn state_color(state: ServerState) -> theme::Color {
    match state {
        ServerState::Running => colors::GREEN_200,
        ServerState::Starting | ServerState::Stopping => colors::YELLOW,
        ServerState::Error => colors::RED_200,
        ServerState::Stopped | ServerState::Unknown | ServerState::Hidden => colors::GRAY_500,
    }
}

fn small(value: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_size(theme::text_size(text::SMALL))
        .line_height(theme::line_height(text::SMALL, leading::NORMAL))
        .child(value.into())
}

/// "1.20.1 · Fabric 0.16.14", or "1.20.1" for vanilla.
fn detail(server: &ServerInstance) -> String {
    let loader = match (&server.loader, &server.loader_version) {
        (ServerLoader::Vanilla, _) => return server.game_version.clone(),
        (loader, Some(version)) => format!("{} {version}", loader.label()),
        (loader, None) => loader.label().to_string(),
    };
    format!("{} · {loader}", server.game_version)
}

impl Servers {
    fn server_row(
        &self,
        core: &CoreEntry,
        server: &ServerInstance,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let core_uuid = core.saved.uuid.clone();
        let row_id = format!("{}-{}", core_uuid, server.uuid);
        let linked = self.linked_instance(&core_uuid, &server.uuid).is_some();
        let busy = self.busy.get(&(core_uuid.clone(), server.uuid.clone()));
        let running = server.state == ServerState::Running;
        let stopped = matches!(server.state, ServerState::Stopped | ServerState::Error);
        let signed_in = core.signed_in();

        let mut actions: Vec<AnyElement> = Vec::new();
        if let Some(busy) = busy {
            actions.push(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .text_color(colors::GRAY_400)
                    .child(LoadingDots)
                    .child(small(busy.label()))
                    .into_any_element(),
            );
        } else {
            if signed_in && stopped {
                let (c, s) = (core_uuid.clone(), server.clone());
                actions.push(
                    Button::new(SharedString::from(format!("start-{row_id}")), "Start")
                        .size(ButtonSize::Slim)
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.start(&c, s.clone(), false, cx)),
                        )
                        .into_any_element(),
                );
            }
            if signed_in && !stopped {
                let (c, s) = (core_uuid.clone(), server.clone());
                actions.push(
                    Button::new(SharedString::from(format!("stop-{row_id}")), "Stop")
                        .size(ButtonSize::Slim)
                        .variant(ButtonVariant::Danger)
                        .on_click(cx.listener(move |this, _, _, cx| this.stop(&c, s.clone(), cx)))
                        .into_any_element(),
                );
            }
            if !linked {
                let (c, s) = (core_uuid.clone(), server.clone());
                actions.push(
                    Button::new(
                        SharedString::from(format!("create-{row_id}")),
                        "Create instance",
                    )
                    .size(ButtonSize::Slim)
                    .icon(Icon::SquarePlus)
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.create_instance(&c, s.clone(), cx)),
                    )
                    .into_any_element(),
                );
            } else if signed_in && stopped {
                let (c, s) = (core_uuid.clone(), server.clone());
                actions.push(
                    Button::new(
                        SharedString::from(format!("start-join-{row_id}")),
                        "Start and join",
                    )
                    .size(ButtonSize::Slim)
                    .variant(ButtonVariant::Primary)
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.start(&c, s.clone(), true, cx)),
                    )
                    .into_any_element(),
                );
            } else {
                // without the status, the server may be running
                let joinable =
                    server.address_shown() && (running || server.state == ServerState::Hidden);
                if !server.address_shown() {
                    actions.push(
                        small(ADDRESS_HIDDEN)
                            .text_color(colors::GRAY_500)
                            .into_any_element(),
                    );
                }
                let (c, s) = (core_uuid.clone(), server.clone());
                actions.push(
                    Button::new(SharedString::from(format!("play-{row_id}")), "Play")
                        .size(ButtonSize::Slim)
                        .variant(ButtonVariant::Primary)
                        .disabled(!joinable)
                        .on_click(cx.listener(move |this, _, _, cx| this.play(&c, s.clone(), cx)))
                        .into_any_element(),
                );
            }
        }

        if busy.is_none() {
            let (c, s) = (core_uuid.clone(), server.clone());
            actions.push(
                Button::new(SharedString::from(format!("more-{row_id}")), "More…")
                    .size(ButtonSize::Slim)
                    .variant(ButtonVariant::Ghost)
                    .on_click(cx.listener(move |this, _, _, cx| this.open_tools(&c, s.clone(), cx)))
                    .into_any_element(),
            );
        }

        let status = match server.players_label() {
            Some(players) if running => format!("{} · {players}", server.state.label()),
            _ => server.state.label().to_string(),
        };
        div()
            .id(SharedString::from(format!("server-{row_id}")))
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_4()
            .py_2p5()
            .border_t_1()
            .border_color(colors::FADE_700)
            .when(server.state != ServerState::Hidden, |this| {
                this.child(
                    div()
                        .flex_none()
                        .size(px(8.))
                        .rounded_full()
                        .bg(state_color(server.state)),
                )
            })
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
                                    .text_color(colors::GRAY_300)
                                    .child(server.name.clone()),
                            )
                            .when(linked, |this| {
                                this.child(
                                    div()
                                        .flex_none()
                                        .px_1p5()
                                        .rounded(radius::DEFAULT)
                                        .bg(colors::BLUE_FADED.opacity(15))
                                        .text_color(colors::BLUE_200)
                                        .text_size(theme::text_size(text::CAPTION))
                                        .font_weight(weights::BOLD.into())
                                        .child("Instance ready"),
                                )
                            }),
                    )
                    .child(small(detail(server)).text_color(colors::GRAY_500)),
            )
            .child(small(status).flex_none().text_color(colors::GRAY_400))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_none()
                    .gap_2()
                    .children(actions),
            )
            .into_any_element()
    }

    fn core_card(&self, index: usize, core: &CoreEntry, cx: &mut Context<Self>) -> AnyElement {
        let signed_in = core.signed_in();
        let account = match (&core.saved.username, signed_in) {
            (Some(username), true) => format!("Signed in as {username}"),
            (None, true) => "Signed in".to_string(),
            _ => "Not signed in: public servers only".to_string(),
        };
        let body: AnyElement = match &core.status {
            CoreStatus::Loading => div()
                .px_4()
                .py_3()
                .border_t_1()
                .border_color(colors::FADE_700)
                .child(LoadingDots)
                .into_any_element(),
            CoreStatus::Failed(err) => div()
                .flex()
                .flex_row()
                .items_start()
                .gap_2()
                .px_4()
                .py_3()
                .border_t_1()
                .border_color(colors::FADE_700)
                .text_color(if err.is_offline() { colors::YELLOW } else { colors::RED_200 })
                .child(Icon::TriangleExclamation.render(px(14.), colors::YELLOW))
                .child(small(match err {
                    CoreError::Unreachable(_) => "Offline: couldn't reach this core. Check that it is running and on the same network.".to_string(),
                    err => err.to_string(),
                }))
                .into_any_element(),
            CoreStatus::Online(list) if list.instances.is_empty() => div()
                .px_4()
                .py_3()
                .border_t_1()
                .border_color(colors::FADE_700)
                .text_color(colors::GRAY_500)
                .child(small(if !signed_in && !list.listing_enabled {
                    "The owner hasn't turned on \"List instances publicly\" in Core Settings, so there's nothing to show without signing in."
                } else if !signed_in {
                    "No servers are listed publicly. Sign in to see the ones you have access to."
                } else {
                    "No servers you can see on this core."
                }))
                .into_any_element(),
            CoreStatus::Online(list) => div()
                .flex()
                .flex_col()
                .children(list.instances.iter().map(|server| self.server_row(core, server, cx)))
                .into_any_element(),
        };
        div()
            .id(SharedString::from(format!("core-{}", core.saved.uuid)))
            .flex()
            .flex_col()
            .w_full()
            .rounded(radius::XL)
            .border_1()
            .border_color(colors::FADE_700)
            .bg(colors::GRAY_850)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .font_family(fonts::HEADING)
                                    .text_size(theme::text_size(text::H3))
                                    .font_weight(weights::BOLD.into())
                                    .text_color(colors::GRAY_300)
                                    .child(core.saved.name.clone()),
                            )
                            .child(
                                small(format!("{} · {account}", display_address(&core.url)))
                                    .truncate()
                                    .text_color(colors::GRAY_500),
                            ),
                    )
                    .child(if signed_in {
                        Button::new(SharedString::from(format!("sign-out-{index}")), "Sign out")
                            .size(ButtonSize::Slim)
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(move |this, _, _, cx| this.sign_out(index, cx)))
                    } else {
                        Button::new(SharedString::from(format!("sign-in-{index}")), "Sign in")
                            .size(ButtonSize::Slim)
                            .icon(Icon::User)
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.open_add(Some(index), cx)),
                            )
                    })
                    .child(
                        Button::new(SharedString::from(format!("remove-core-{index}")), "Remove")
                            .size(ButtonSize::Slim)
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(move |this, _, _, cx| this.remove(index, cx))),
                    ),
            )
            .child(body)
            .into_any_element()
    }
}

impl Render for Servers {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let cards: Vec<AnyElement> = (0..self.cores.len())
            .map(|index| {
                // the cards borrow `self` while their buttons need `cx`
                let core = &self.cores[index];
                self.core_card(index, core, cx)
            })
            .collect();
        let empty = self.cores.is_empty();
        div()
            .id("servers-page")
            .flex()
            .flex_col()
            .size_full()
            .overflow_y_scroll()
            .px_8()
            .py_6()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .child(
                                div()
                                    .font_family(fonts::HEADING)
                                    .text_size(theme::text_size(text::H1))
                                    .font_weight(weights::BOLD.into())
                                    .text_color(colors::GRAY_300)
                                    .child("Servers"),
                            )
                            .child(
                                small("Servers on your Lodestone cores. Create an instance that matches one, then play on it.")
                                    .text_color(colors::GRAY_500),
                            ),
                    )
                    .child(
                        Button::new("add-server", "Add server")
                            .variant(ButtonVariant::Primary)
                            .icon(Icon::Plus)
                            .on_click(cx.listener(|this, _, _, cx| this.open_add(None, cx))),
                    ),
            )
            .children(self.notice.clone().map(|(title, message)| {
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .rounded(radius::LG)
                    .border_1()
                    .border_color(colors::YELLOW.opacity(30))
                    .bg(colors::YELLOW.opacity(8))
                    .child(Icon::TriangleExclamation.render(px(14.), colors::YELLOW))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .child(
                                div()
                                    .font_weight(weights::BOLD.into())
                                    .text_color(colors::GRAY_300)
                                    .child(title),
                            )
                            .child(small(message).text_color(colors::GRAY_400)),
                    )
                    .child(
                        Button::new("dismiss-notice", "Dismiss")
                            .size(ButtonSize::Slim)
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(|this, _, _, cx| this.dismiss_notice(cx))),
                    )
            }))
            .when(empty, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_3()
                        .py_10()
                        .px_6()
                        .rounded(radius::XL)
                        .border_2()
                        .border_dashed()
                        .border_color(colors::GRAY_FADED.opacity(10))
                        .child(Icon::Cube.render(px(28.), colors::GRAY_FADED.opacity(30)))
                        .child(
                            div()
                                .font_weight(weights::BOLD.into())
                                .text_color(colors::GRAY_FADED.opacity(50))
                                .child("No servers yet"),
                        )
                        .child(
                            small("Add a Lodestone core by its address. Without signing in you see the servers its owner lists publicly.")
                                .text_color(colors::GRAY_500),
                        ),
                )
            })
            .children(cards)
            .child(self.render_lan(cx))
    }
}

/// The Servers page's dialogs, drawn over the whole window by the shell.
pub struct ServersOverlay {
    servers: Entity<Servers>,
    _observe: Subscription,
}

impl ServersOverlay {
    pub fn new(servers: Entity<Servers>, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&servers, |_, _, cx| cx.notify());
        Self {
            servers,
            _observe: observe,
        }
    }
}

impl Render for ServersOverlay {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(dialog) = super::extras::overlay(&self.servers, cx) {
            return dialog;
        }
        let servers = self.servers.read(cx);
        if let Some(dialog) = servers.dialog() {
            return dialog.clone().into_any_element();
        }
        let Some(prompt) = &servers.prompt else {
            return div().into_any_element();
        };
        let entity = self.servers.clone();
        let changes = &prompt.changes;
        let mut lines: Vec<String> = Vec::new();
        for (names, verb) in [
            (&changes.added, "New"),
            (&changes.updated, "Changed"),
            (&changes.removed, "Removed"),
        ] {
            for name in names.iter().take(6) {
                lines.push(format!("{verb}: {name}"));
            }
        }
        let update = self.servers.clone();
        let play = self.servers.clone();
        let cancel = self.servers.clone();
        Dialog::new(
            "update-server-mods",
            format!("{} has different mods", prompt.server.name),
            prompt.focus.clone(),
        )
        .on_close(move |_, cx| entity.update(cx, |this, cx| this.cancel_prompt(cx)))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .text_color(colors::GRAY_400)
                .child(format!(
                    "The server's mods changed since this instance got them ({}). Update first so you can join; your own mods stay as they are.",
                    changes.summary()
                ))
                .children(lines.into_iter().map(|line| {
                    small(line)
                        .font_family(fonts::MONO)
                        .text_color(colors::GRAY_300)
                })),
        )
        .footer(
            Button::new("update-and-play", "Update and play")
                .variant(ButtonVariant::Primary)
                .on_click(move |_, _, cx| update.update(cx, |this, cx| this.answer_prompt(true, cx))),
        )
        .footer(
            Button::new("play-anyway", "Play anyway")
                .on_click(move |_, _, cx| play.update(cx, |this, cx| this.answer_prompt(false, cx))),
        )
        .footer(
            Button::new("cancel-update", "Cancel")
                .variant(ButtonVariant::Ghost)
                .on_click(move |_, _, cx| cancel.update(cx, |this, cx| this.cancel_prompt(cx))),
        )
        .into_any_element()
    }
}
