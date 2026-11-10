//! How the Lodestone Host page looks: the cores on this computer, each core as a card
//! with its servers, and the dialogs over the window (Add server, the mod update
//! prompt).

use crate::components::RichText;
use crate::skin::Skinned;
use crate::theme::px;
use gpui::{
    AnyElement, App, Context, Entity, IntoElement, Render, SharedString, Subscription, Window, div,
    prelude::*,
};
use launcher_core::cores::{
    CoreError, CoreProduct, LaunchedBy, ServerInstance, ServerLoader, ServerState,
    client::display_address, local_claim::ClaimError, sync::PackChanges, trust::Advisory,
};
use launcher_core::ui_skin::SurfaceId;
use launcher_core::wording;

use super::{
    AdvisoryAnswer, AdvisoryPrompt, CertPrompt, CoreEntry, CoreStatus, NoticeKind, Servers,
    TrustPrompt, UpdatePrompt, core_error, loader_label, local::FoundCore, status_label,
    trust_explanation,
};
use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, Dialog, DialogSlot, Icon, IconButton, ListRow,
        LoadingDots, Toggle,
        a11y::{Accessible, Role},
        settings::{popover, popover_panel},
    },
    i18n::{CommonMsg, ServersMsg as M, ServersPlural as P, format, t, tr, trn},
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
pub(super) fn server_detail(server: &ServerInstance) -> String {
    let loader = match (&server.loader, &server.loader_version) {
        (ServerLoader::Vanilla, _) => return server.game_version.clone(),
        (loader, Some(version)) => format!("{} {version}", loader_label(*loader)),
        (loader, None) => loader_label(*loader).to_string(),
    };
    format!("{} · {loader}", server.game_version)
}

/// What an update changes, for "<server> has different mods": "2 added, 1 updated,
/// 3 other files".
fn changes_summary(changes: &PackChanges) -> String {
    let parts: Vec<String> = [
        (changes.added.len(), P::ModsAdded),
        (changes.updated.len(), P::ModsUpdated),
        (changes.removed.len(), P::ModsRemoved),
        (changes.other.len(), P::OtherFiles),
    ]
    .into_iter()
    .filter(|(count, _)| *count > 0)
    .map(|(count, message)| trn!(message, count))
    .collect();
    if parts.is_empty() {
        t(M::NoChanges).into()
    } else {
        format::list(&parts)
    }
}

/// The advisory's other files: "14 config files, 2 script files, 1 other file".
fn other_files(advisory: &Advisory) -> Option<String> {
    let parts: Vec<String> = [
        (advisory.configs, P::ConfigFiles),
        (advisory.scripts, P::ScriptFiles),
        (advisory.other, P::OtherFiles),
    ]
    .into_iter()
    .filter(|(count, _)| *count > 0)
    .map(|(count, message)| trn!(message, count))
    .collect();
    (!parts.is_empty()).then(|| format::list(&parts))
}

/// A fingerprint in two lines of 16 bytes, in the mono font.
fn fingerprint_block(caption: &'static str, fingerprint: &str, color: theme::Color) -> gpui::Div {
    let parts: Vec<&str> = fingerprint.split(':').collect();
    let lines: Vec<String> = parts.chunks(16).map(|chunk| chunk.join(":")).collect();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .px_3()
        .py_2()
        .rounded(radius::LG)
        .border_1()
        .border_color(color.opacity(30))
        .bg(colors::GRAY_900)
        .child(
            div()
                .text_size(theme::text_size(text::CAPTION))
                .font_weight(weights::BOLD.into())
                .text_color(crate::theme::ink::muted())
                .child(caption),
        )
        .children(lines.into_iter().map(move |line| {
            div()
                .font_family(fonts::mono())
                .text_size(theme::text_size(text::SMALL))
                .text_color(color)
                .child(line)
        }))
}

/// The card's body when the certificate needs the user: not trusted by macOS, or changed
/// since the user trusted it.
fn certificate_warning(
    index: usize,
    err: &CoreError,
    cx: &mut Context<Servers>,
) -> impl IntoElement + use<> {
    let (changed, message, button) = match err {
        CoreError::CertificateChanged { .. } => (
            true,
            t(M::CertChangedWarning).to_string(),
            t(M::ReviewNewCertificate),
        ),
        _ => (
            false,
            tr!(
                M::CertUntrustedWarning,
                computer = t(CommonMsg::ThisComputer)
            ),
            t(M::ReviewCertificate),
        ),
    };
    let fingerprint = match err {
        CoreError::Untrusted { fingerprint, .. } => fingerprint.is_some(),
        _ => true,
    };
    let color = if changed {
        colors::RED_200
    } else {
        colors::YELLOW
    };
    div()
        .flex()
        .flex_row()
        .items_start()
        .gap_3()
        .px_4()
        .py_3()
        .border_t_1()
        .border_color(crate::theme::ink::border())
        .when(changed, |this| this.bg(colors::RED_300.opacity(8)))
        .child(Icon::TriangleExclamation.render(px(14.), color))
        .child(small(message).flex_1().min_w_0().text_color(color))
        .when(fingerprint, |this| {
            this.child(
                Button::new(SharedString::from(format!("review-cert-{index}")), button)
                    .size(ButtonSize::Slim)
                    .variant(if changed {
                        ButtonVariant::Danger
                    } else {
                        ButtonVariant::Secondary
                    })
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.review_certificate(index, cx)),
                    ),
            )
        })
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
        // the same server, saved from its invite link, shows here and not again
        let shared = self.friend_on_core_row(&core_uuid, &server.uuid);
        // an instance made (or last played) for it from its invite link plays it here
        let friend_play = shared
            .filter(|friend| {
                !linked
                    && (self
                        .linked_instance(&friend.saved.id, &server.uuid)
                        .is_some()
                        || friend.saved.instance_id.is_some())
            })
            .map(|friend| friend.saved.id.clone());
        let shared = shared.is_some();
        let busy = self.busy.get(&(core_uuid.clone(), server.uuid.clone()));
        let running = server.state == ServerState::Running;
        let stopped = matches!(server.state, ServerState::Stopped | ServerState::Error);
        let signed_in = core.signed_in();

        let mut actions: Vec<AnyElement> = Vec::new();
        if signed_in {
            let (c, s) = (core_uuid.clone(), server.clone());
            actions.push(
                Button::new(
                    SharedString::from(format!("details-{row_id}")),
                    t(M::Details),
                )
                .size(ButtonSize::Slim)
                .variant(ButtonVariant::Ghost)
                .on_click(cx.listener(move |this, _, _, cx| this.open_detail(&c, s.clone(), cx)))
                .into_any_element(),
            );
        }
        if let Some(busy) = busy {
            actions.push(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .text_color(crate::theme::ink::secondary())
                    .child(LoadingDots)
                    .child(small(busy.label()))
                    .into_any_element(),
            );
        } else {
            if signed_in && stopped {
                let (c, s) = (core_uuid.clone(), server.clone());
                actions.push(
                    Button::new(
                        SharedString::from(format!("start-{row_id}")),
                        t(CommonMsg::Start),
                    )
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
                    Button::new(
                        SharedString::from(format!("stop-{row_id}")),
                        t(CommonMsg::Stop),
                    )
                    .size(ButtonSize::Slim)
                    .variant(ButtonVariant::Danger)
                    .on_click(cx.listener(move |this, _, _, cx| this.stop(&c, s.clone(), cx)))
                    .into_any_element(),
                );
            }
            if let Some(friend) = friend_play.clone() {
                let joinable =
                    server.address_shown() && (running || server.state == ServerState::Hidden);
                actions.push(
                    Button::new(SharedString::from(format!("play-{row_id}")), t(M::Play))
                        .size(ButtonSize::Slim)
                        .variant(ButtonVariant::Primary)
                        .disabled(!joinable)
                        .on_click(cx.listener(move |this, _, _, cx| this.play_friend(&friend, cx)))
                        .into_any_element(),
                );
            } else if !linked {
                let (c, s) = (core_uuid.clone(), server.clone());
                actions.push(
                    Button::new(
                        SharedString::from(format!("create-{row_id}")),
                        t(M::CreateInstance),
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
                        t(M::StartAndJoin),
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
                        small(t(M::AddressHidden))
                            .text_color(crate::theme::ink::muted())
                            .into_any_element(),
                    );
                }
                let (c, s) = (core_uuid.clone(), server.clone());
                actions.push(
                    Button::new(SharedString::from(format!("play-{row_id}")), t(M::Play))
                        .size(ButtonSize::Slim)
                        .variant(ButtonVariant::Primary)
                        .disabled(!joinable)
                        .on_click(cx.listener(move |this, _, _, cx| this.play(&c, s.clone(), cx)))
                        .into_any_element(),
                );
            }
        }

        let status = status_label(server);
        let element_id = SharedString::from(format!("server-{row_id}"));
        let ready = linked || friend_play.is_some();
        let mut description = format!("{status} · {}", server_detail(server));
        if ready {
            description = format!("{description} · {}", t(M::InstanceReady));
        }
        if shared {
            description = format!("{description} · {}", t(M::SharedWithFriends));
        }
        let description: SharedString = description.into();
        div()
            .id(element_id.clone())
            .accessible(Role::ListItem, element_id, server.name.clone())
            .aria_description(description)
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_4()
            .py_2p5()
            .border_t_1()
            .border_color(crate::theme::ink::border())
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
                                    .text_color(crate::theme::ink::text())
                                    .child(RichText::minecraft(server.name.clone())),
                            )
                            .when(ready, |this| {
                                this.child(
                                    div()
                                        .flex_none()
                                        .px_1p5()
                                        .rounded(radius::DEFAULT)
                                        .bg(crate::theme::accent::faded().opacity(15))
                                        .text_color(crate::theme::accent::light())
                                        .text_size(theme::text_size(text::CAPTION))
                                        .font_weight(weights::BOLD.into())
                                        .child(t(M::InstanceReady)),
                                )
                            })
                            .when(shared, |this| {
                                this.child(
                                    div()
                                        .id(SharedString::from(format!("shared-{row_id}")))
                                        .flex_none()
                                        .px_1p5()
                                        .rounded(radius::DEFAULT)
                                        .bg(colors::GREEN_300.opacity(12))
                                        .text_color(colors::GREEN_300)
                                        .text_size(theme::text_size(text::CAPTION))
                                        .font_weight(weights::BOLD.into())
                                        .child(t(M::SharedWithFriends)),
                                )
                            }),
                    )
                    .child(small(server_detail(server)).text_color(crate::theme::ink::muted())),
            )
            .child(
                small(status)
                    .flex_none()
                    .text_color(crate::theme::ink::secondary()),
            )
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

    /// Core `index`'s card, with its servers. In "On this computer" (`place`) it wears
    /// Lodestone Host's green, and takes the place of the scan's row for its core:
    /// "Connect as owner", why claiming failed, and the servers shared from it.
    fn core_card(
        &self,
        index: usize,
        core: &CoreEntry,
        place: Place<'_>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let signed_in = core.signed_in();
        let trusted = core.saved.is_trusted();
        let local = matches!(place, Place::Local { .. });
        let found = match place {
            Place::Local { found } => found,
            Place::Other => None,
        };
        // not the owner here yet (signed out, or signed in some other way)
        let connect = found.map(|(found_index, found)| {
            let uuid = found.info.uuid.clone();
            (
                found_index,
                uuid.clone(),
                !self.is_connected(&uuid, &found.url),
            )
        });
        let claim_error = connect
            .as_ref()
            .and_then(|(found_index, uuid, _)| self.claim_error_row(*found_index, uuid, cx));
        let shared = found.and_then(|(found_index, found)| {
            let rows: Vec<AnyElement> = self
                .friends_shared_from(&found.info.uuid)
                .into_iter()
                .map(|entry| self.friend_row(entry, true, cx))
                .collect();
            (!rows.is_empty()).then(|| shared_from_here(found_index, px(0.), rows))
        });
        let connect_button = connect
            .filter(|(_, _, needed)| *needed)
            .map(|(found_index, uuid, _)| self.connect_button(found_index, uuid, cx));
        let account = match (&core.saved.username, signed_in) {
            (Some(username), true) => tr!(M::SignedInAs, username = username),
            (None, true) => t(M::SignedIn).to_string(),
            _ => t(M::NotSignedIn).to_string(),
        };
        let body: AnyElement = match &core.status {
            CoreStatus::Loading => div()
                .px_4()
                .py_3()
                .border_t_1()
                .border_color(crate::theme::ink::border())
                .child(LoadingDots)
                .into_any_element(),
            CoreStatus::Failed(err) if err.is_certificate() => {
                certificate_warning(index, err, cx).into_any_element()
            }
            CoreStatus::Failed(err) => div()
                .flex()
                .flex_row()
                .items_start()
                .gap_2()
                .px_4()
                .py_3()
                .border_t_1()
                .border_color(crate::theme::ink::border())
                .text_color(if err.is_offline() {
                    colors::YELLOW
                } else {
                    colors::RED_200
                })
                .child(Icon::TriangleExclamation.render(px(14.), colors::YELLOW))
                .child(small(match err {
                    CoreError::Unreachable(_) => t(M::Offline).to_string(),
                    err => core_error(err),
                }))
                .into_any_element(),
            CoreStatus::Online(list) if list.instances.is_empty() => div()
                .px_4()
                .py_3()
                .border_t_1()
                .border_color(crate::theme::ink::border())
                .text_color(crate::theme::ink::muted())
                .child(small(t(if !signed_in && !list.listing_enabled {
                    M::NotListedPublicly
                } else if !signed_in {
                    M::NoPublicServers
                } else {
                    M::NoVisibleServers
                })))
                .into_any_element(),
            CoreStatus::Online(list) => div()
                .flex()
                .flex_col()
                .children(
                    list.instances
                        .iter()
                        .map(|server| self.server_row(core, server, cx)),
                )
                .into_any_element(),
        };
        let card_id = SharedString::from(format!("core-{}", core.saved.uuid));
        let card_description: SharedString = format!(
            "{} · {} · {account}",
            core.saved.product.label(),
            display_address(&core.url)
        )
        .into();
        let sign_in_button = (!signed_in && connect_button.is_none()).then(|| {
            Button::new(
                SharedString::from(format!("sign-in-{index}")),
                t(CommonMsg::SignIn),
            )
            .size(ButtonSize::Slim)
            .icon(Icon::User)
            .on_click(cx.listener(move |this, _, _, cx| this.open_add(Some(index), cx)))
        });
        card_frame(card_id.clone(), local)
            .accessible(Role::Group, card_id, core.saved.name.clone())
            .aria_description(card_description)
            .child(
                card_header(local)
                    .child(core_icon(local, core.saved.product, cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .when(local, |this| this.child(this_computer_label()))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .truncate()
                                            .font_family(fonts::heading())
                                            .text_size(theme::text_size(text::H3))
                                            .font_weight(weights::BOLD.into())
                                            .text_color(crate::theme::ink::text())
                                            .child(RichText::new(core.saved.name.clone())),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(theme::text_size(text::SMALL))
                                    .child(RichText::new(format!(
                                        "{} · {account}",
                                        display_address(&core.url)
                                    )))
                                    .truncate()
                                    .text_color(crate::theme::ink::muted()),
                            )
                            .children(started_by(core.saved.launched_by).map(|started| {
                                small(started).text_color(crate::theme::ink::muted())
                            })),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("trusted-{index}")))
                            .flex_none()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(
                                small(t(M::TrustedForMods))
                                    .text_color(crate::theme::ink::secondary()),
                            )
                            .child(
                                Toggle::new(SharedString::from(format!("trust-{index}")), trusted)
                                    .label(t(M::TrustedForMods))
                                    .on_change(cx.listener(move |this, value: &bool, _, cx| {
                                        this.set_trusted(index, *value, cx)
                                    })),
                            ),
                    )
                    .children(connect_button)
                    .children(sign_in_button)
                    .child(self.core_menu(index, core, cx)),
            )
            .children(claim_error.map(|row| {
                row.px_4()
                    .py_3()
                    .border_t_1()
                    .border_color(crate::theme::ink::border())
            }))
            .when(core.saved.launched_by == LaunchedBy::Direct, |this| {
                this.child(direct_note(index))
            })
            .child(body)
            .children(shared)
            .into_any_element()
    }

    /// "…" on a core's card, and its menu: Copy owner API key (when claiming the core
    /// here gave one), Sign out, Remove, and whether its certificate is pinned.
    fn core_menu(&self, index: usize, core: &CoreEntry, cx: &mut Context<Self>) -> gpui::Div {
        let uuid = core.saved.uuid.clone();
        let open = self.core_menu_open(&uuid);
        let has_key = self
            .local_credentials(&uuid)
            .is_some_and(|credentials| credentials.api_key.is_some());
        let signed_in = core.signed_in();
        let pinned = core.saved.certificate.is_some();
        div()
            .relative()
            .flex_none()
            .child(
                IconButton::new(
                    SharedString::from(format!("core-menu-{index}")),
                    Icon::Ellipsis,
                )
                .label(tr!(M::CoreMoreActions, name = core.saved.name.clone()))
                .on_click(cx.listener(move |this, _, _, cx| this.toggle_core_menu(&uuid, cx))),
            )
            .when(open, |this| {
                let mut panel =
                    popover_panel(SharedString::from(format!("core-menu-list-{index}")))
                        .w(px(220.))
                        .on_mouse_down_out(cx.listener(|this, _, _, cx| this.close_core_menu(cx)));
                if has_key {
                    panel = panel.child(
                        ListRow::new(
                            SharedString::from(format!("owner-copy-key-{index}")),
                            t(M::CopyOwnerApiKey),
                        )
                        .icon(Icon::Copy)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.close_core_menu(cx);
                            this.copy_owner_key(index, cx);
                        })),
                    );
                }
                if signed_in {
                    panel = panel.child(
                        ListRow::new(
                            SharedString::from(format!("sign-out-{index}")),
                            t(CommonMsg::SignOut),
                        )
                        .icon(Icon::RightFromBracket)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.close_core_menu(cx);
                            this.sign_out(index, cx);
                        })),
                    );
                }
                panel = panel.child(
                    ListRow::new(
                        SharedString::from(format!("remove-core-{index}")),
                        t(CommonMsg::Remove),
                    )
                    .icon(Icon::TrashCan)
                    .danger(true)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.close_core_menu(cx);
                        this.remove(index, cx);
                    })),
                );
                // Not a button: says the launcher only accepts this core's own
                // certificate, the one the user trusted.
                if pinned {
                    panel = panel.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(6.))
                            .mt_1()
                            .px_2()
                            .pt_1p5()
                            .pb_1()
                            .border_t_1()
                            .border_color(crate::theme::ink::border())
                            .child(Icon::CircleCheck.render(px(12.), colors::GREEN_300))
                            .child(
                                small(t(M::CertificatePinned))
                                    .text_color(crate::theme::ink::muted()),
                            ),
                    );
                }
                let close = cx.listener(|this, _: &(), _, cx| this.close_core_menu(cx));
                // In a box as wide as the list, under the button and out of the
                // header's layout: the popover starts where its parent does, so the
                // list's right edge lines up with the button's.
                this.child(
                    div()
                        .absolute()
                        .top_full()
                        .right_0()
                        .w(px(220.))
                        .child(popover(
                            SharedString::from(format!("core-menu-popover-{index}")),
                            panel,
                            move |window, cx| close(&(), window, cx),
                        )),
                )
            })
    }

    /// "Connect as owner" for the core the scan found as row `found_index`.
    fn connect_button(&self, found_index: usize, uuid: String, cx: &mut Context<Self>) -> Button {
        let claiming = self.local.claiming.as_deref() == Some(uuid.as_str());
        Button::new(
            SharedString::from(format!("connect-local-{found_index}")),
            t(M::ConnectAsOwner),
        )
        .size(ButtonSize::Slim)
        .variant(ButtonVariant::Primary)
        .loading(claiming)
        .disabled(self.local.claiming.is_some())
        .on_click(cx.listener(move |this, _, _, cx| this.connect_local(&uuid, None, cx)))
    }

    /// Why claiming core `uuid` (the scan's row `found_index`) failed, with "Choose
    /// folder…" when its data folder wasn't found.
    fn claim_error_row(
        &self,
        found_index: usize,
        uuid: &str,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Div> {
        let err = self
            .local
            .error
            .as_ref()
            .filter(|(failed, _)| failed == uuid || failed.is_empty())
            .map(|(_, err)| err.clone())?;
        let busy = self.local.claiming.is_some();
        let folder = matches!(err, ClaimError::FolderNotFound);
        let uuid = uuid.to_string();
        Some(
            div()
                .flex()
                .flex_row()
                .items_start()
                .gap_2()
                .child(Icon::TriangleExclamation.render(px(14.), colors::YELLOW))
                .child(
                    small(super::local::claim_error(&err))
                        .flex_1()
                        .min_w_0()
                        .whitespace_normal()
                        .text_color(colors::RED_200),
                )
                .when(folder, |this| {
                    this.child(
                        div().flex_none().child(
                            Button::new(
                                SharedString::from(format!("choose-folder-{found_index}")),
                                t(M::ChooseDataFolder),
                            )
                            .size(ButtonSize::Slim)
                            .icon(Icon::HardDrive)
                            .disabled(busy)
                            .on_click(cx.listener(
                                move |this, _, _, cx| this.choose_local_folder(uuid.clone(), cx),
                            )),
                        ),
                    )
                }),
        )
    }

    /// A core the scan found here that has no card yet: what it is, and "Connect as
    /// owner".
    fn found_card(
        &self,
        found_index: usize,
        found: &FoundCore,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let uuid = found.info.uuid.clone();
        let shared: Vec<AnyElement> = self
            .friends_shared_from(&uuid)
            .into_iter()
            .map(|entry| self.friend_row(entry, true, cx))
            .collect();
        let error = self.claim_error_row(found_index, &uuid, cx);
        let connect = self.connect_button(found_index, uuid, cx);
        let id = SharedString::from(format!("local-core-{found_index}"));
        card_frame(id.clone(), true)
            .accessible(Role::Group, id, found.info.core_name.clone())
            .child(
                card_header(true)
                    .flex_col()
                    .items_stretch()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .child(core_icon(true, found.info.product, cx))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .child(this_computer_label())
                                    .child(
                                        div().flex().flex_row().items_center().gap_2().child(
                                            div()
                                                .truncate()
                                                .font_family(fonts::heading())
                                                .text_size(theme::text_size(text::H3))
                                                .font_weight(weights::BOLD.into())
                                                .text_color(crate::theme::ink::text())
                                                .child(RichText::new(found.info.core_name.clone())),
                                        ),
                                    )
                                    .child(
                                        small(format!(
                                            "{} · {}",
                                            display_address(&found.url),
                                            started_by(found.info.launched_by).unwrap_or_else(
                                                || t(match found.info.product {
                                                    CoreProduct::Host => M::ProductHostHint,
                                                    CoreProduct::Core => M::ProductCoreHint,
                                                })
                                                .to_string()
                                            )
                                        ))
                                        .text_color(crate::theme::ink::muted()),
                                    ),
                            )
                            .child(connect),
                    )
                    .children(error),
            )
            .children(
                (!shared.is_empty())
                    .then(|| shared_from_here(found_index, px(SHARED_INDENT), shared)),
            )
            .into_any_element()
    }

    /// "On this computer": the cores here, each as a card in Lodestone Host's green.
    /// Those connected show in full (their servers, and the servers shared from them;
    /// the owner key is in the card's "…" menu), the others with "Connect as owner". While nothing runs here,
    /// it says so, and offers Lodestone Host.
    fn render_local(&self, cards: Vec<AnyElement>, cx: &mut Context<Self>) -> AnyElement {
        let local = &self.local;
        let empty = cards.is_empty();
        let offer = (empty && self.offers_host(cx)).then(|| self.render_host_offer(cx));
        let empty_text = if !empty {
            None
        } else if local.scanning || !local.scanned {
            Some(t(M::ScanningLocal))
        } else {
            Some(t(M::NoLocalCores))
        };
        div()
            .id("local-cores")
            .flex()
            .flex_col()
            .gap_2()
            .child(
                section_header(
                    "local-cores-title",
                    t(M::OnThisComputer),
                    t(M::OnThisComputerHint),
                )
                .child(
                    Button::new("rescan-local", t(M::Rescan))
                        .size(ButtonSize::Slim)
                        .variant(ButtonVariant::Ghost)
                        .loading(local.scanning)
                        .disabled(local.scanning)
                        .on_click(cx.listener(|this, _, _, cx| this.scan_local(true, cx))),
                ),
            )
            .children(cards)
            .when(empty, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .w_full()
                        .rounded(radius::XL)
                        .border_1()
                        .border_color(crate::theme::ink::border())
                        .bg(colors::GRAY_850)
                        .surface(SurfaceId::Card)
                        .children(empty_text.map(|text| {
                            div()
                                .px_4()
                                .py_3()
                                .text_color(crate::theme::ink::muted())
                                .child(small(text))
                        }))
                        .children(offer),
                )
            })
            .into_any_element()
    }

    /// "Other cores": those added by address, on other computers, with a neutral look.
    fn render_other(&self, cards: Vec<AnyElement>) -> AnyElement {
        let empty = cards.is_empty();
        div()
            .id("other-cores")
            .flex()
            .flex_col()
            .gap_2()
            .child(section_header(
                "other-cores-title",
                t(M::OtherCoresTitle),
                t(M::OtherCoresHint),
            ))
            .when(empty, |this| {
                this.child(
                    div()
                        .id("other-cores-empty")
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_1()
                        .py_6()
                        .px_6()
                        .rounded(radius::XL)
                        .border_2()
                        .border_dashed()
                        .border_color(colors::GRAY_FADED.opacity(10))
                        .child(
                            div()
                                .font_weight(weights::BOLD.into())
                                .text_color(colors::GRAY_FADED.opacity(50))
                                .child(t(M::NoOtherCores)),
                        )
                        .child(small(t(M::NoServersHint)).text_color(crate::theme::ink::muted())),
                )
            })
            .children(cards)
            .into_any_element()
    }
}

/// How far the servers shared from a core with no card yet are set in: their dots line
/// up with the core's name (16 + 32 + 12 px in, less the rows' own 16).
const SHARED_INDENT: f32 = 44.;

/// Where a core's card is on the page.
#[derive(Clone, Copy)]
enum Place<'a> {
    /// "On this computer". `found`: what the scan found here for it (its row and the
    /// core), unless it didn't answer.
    Local {
        found: Option<(usize, &'a FoundCore)>,
    },
    /// "Other cores".
    Other,
}

/// A section's title, its hint under it, and room on the right for buttons.
fn section_header(id: &'static str, title: &'static str, hint: &'static str) -> gpui::Div {
    div().flex().flex_row().items_center().gap_3().child(
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .child(
                div()
                    .id(id)
                    .role(Role::Heading)
                    .aria_level(2)
                    .aria_label(title)
                    .font_family(fonts::heading())
                    .text_size(theme::text_size(text::H3))
                    .font_weight(weights::BOLD.into())
                    .text_color(crate::theme::ink::text())
                    .child(title),
            )
            .child(small(hint).text_color(crate::theme::ink::muted())),
    )
}

/// A core's card. On this computer (`local`) its edge is Lodestone Host's green.
fn card_frame(id: SharedString, local: bool) -> gpui::Stateful<gpui::Div> {
    let light = theme::palette::is_light();
    div()
        .id(id)
        .flex()
        .flex_col()
        .w_full()
        .rounded(radius::XL)
        .border_1()
        .border_color(if local {
            crate::components::host_green(35, light)
        } else {
            crate::theme::ink::border()
        })
        .bg(colors::GRAY_850)
        .surface(SurfaceId::Card)
}

/// The top of a core's card: its icon, name and buttons in a row. On this computer
/// (`local`) it has a faint wash of Lodestone Host's gradient.
fn card_header(local: bool) -> gpui::Div {
    let light = theme::palette::is_light();
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .px_4()
        .py_3()
        .when(local, |this| {
            this.rounded_t(radius::XL)
                .bg(crate::components::host_gradient(
                    if light { 8 } else { 5 },
                    light,
                ))
        })
}

/// The icon at the left of a core's card: Lodestone Host's mark for a Host on this
/// computer, a green cube for a Core here, and a plain server for the others.
fn core_icon(local: bool, product: CoreProduct, cx: &App) -> AnyElement {
    let light = theme::palette::is_light();
    let tile = div()
        .flex_none()
        .size(px(32.))
        .flex()
        .items_center()
        .justify_center();
    match (local, product) {
        (true, CoreProduct::Host) => gpui::img(crate::images::mark(true, cx))
            .size(px(32.))
            .flex_none()
            .into_any_element(),
        (true, CoreProduct::Core) => tile
            .child(Icon::Cube.render(px(22.), crate::components::host_green(100, light)))
            .into_any_element(),
        (false, _) => tile
            .rounded(radius::LG)
            .bg(colors::GRAY_FADED.opacity(8))
            .child(Icon::Server.render(px(16.), colors::GRAY_400))
            .into_any_element(),
    }
}

/// "This Mac" (or PC, or computer), in green above the name of a core on this computer.
fn this_computer_label() -> gpui::Div {
    div()
        .text_size(theme::text_size(text::CAPTION))
        .line_height(theme::line_height(text::CAPTION, leading::NORMAL))
        .font_weight(weights::BOLD.into())
        .text_color(crate::components::host_green(
            100,
            theme::palette::is_light(),
        ))
        .child(t(CommonMsg::ThisComputerStart))
}

/// Under a core in "On this computer": the servers it shares that the user joined here
/// with their invite links (`rows`), set in by `indent`.
fn shared_from_here(
    index: usize,
    indent: gpui::Pixels,
    rows: Vec<AnyElement>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(format!("local-shared-{index}")))
        .accessible(
            Role::Group,
            SharedString::from(format!("local-shared-{index}")),
            t(M::SharedWithFriends),
        )
        .flex()
        .flex_col()
        .ml(indent)
        .child(
            div()
                .flex()
                .flex_row()
                .items_baseline()
                .gap_2()
                .px_4()
                .py_2()
                .border_t_1()
                .border_color(crate::theme::ink::border())
                .child(
                    small(t(M::SharedWithFriends))
                        .flex_none()
                        .font_weight(weights::BOLD.into())
                        .text_color(crate::theme::ink::secondary()),
                )
                .child(
                    small(t(M::SharedWithFriendsHint))
                        .min_w_0()
                        .truncate()
                        .text_color(crate::theme::ink::muted()),
                ),
        )
        .children(rows)
}

/// Where lodestone_cli, which runs Lodestone Core and keeps it updated, is downloaded.
const LODESTONE_CLI_RELEASES: &str = "https://github.com/Lodestone-Team/lodestone_cli/releases";

/// "Started by …", or nothing when the core doesn't say.
fn started_by(launched_by: LaunchedBy) -> Option<String> {
    let who = match launched_by {
        LaunchedBy::Host => "Lodestone Host",
        LaunchedBy::LodestoneCli => "Lodestone CLI",
        LaunchedBy::Docker => "Docker",
        LaunchedBy::Direct => return Some(t(M::StartedDirectly).into()),
        LaunchedBy::Unknown => return None,
    };
    Some(tr!(M::StartedBy, who = who))
}

/// Under a core started directly: Lodestone Core is meant to be run with lodestone_cli,
/// which keeps it updated.
fn direct_note(index: usize) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .px_4()
        .py_2()
        .border_t_1()
        .border_color(crate::theme::ink::border())
        .child(
            small(t(M::DirectNote))
                .flex_1()
                .min_w_0()
                .text_color(crate::theme::ink::secondary()),
        )
        .child(
            Button::new(
                SharedString::from(format!("get-lodestone-cli-{index}")),
                t(M::GetLodestoneCli),
            )
            .size(ButtonSize::Slim)
            .variant(ButtonVariant::Ghost)
            .icon(Icon::ArrowUpRightFromSquare)
            .on_click(|_, _, cx| cx.open_url(LODESTONE_CLI_RELEASES)),
        )
}

impl Render for Servers {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(detail) = self.detail().cloned() {
            return div().size_full().child(detail).into_any_element();
        }
        // the cards borrow `self` while their buttons need `cx`
        let mut local_cards: Vec<AnyElement> = Vec::new();
        // each core found here, as its card when it has one
        for (found_index, found) in self.local.found.iter().enumerate() {
            local_cards.push(match self.core_index(&found.info.uuid) {
                Some(index) => self.core_card(
                    index,
                    &self.cores[index],
                    Place::Local {
                        found: Some((found_index, found)),
                    },
                    cx,
                ),
                None => self.found_card(found_index, found, cx),
            });
        }
        // then those claimed here that didn't answer (stopped, say), or at this
        // computer's address on another port
        let mut other_cards: Vec<AnyElement> = Vec::new();
        for (index, core) in self.cores.iter().enumerate() {
            let found = self
                .local
                .found
                .iter()
                .any(|found| found.info.uuid == core.saved.uuid);
            if found {
                continue;
            }
            if self.is_local_card(index) {
                local_cards.push(self.core_card(index, core, Place::Local { found: None }, cx));
            } else {
                other_cards.push(self.core_card(index, core, Place::Other, cx));
            }
        }
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
                            .min_w_0()
                            .child(
                                div()
                                    .id("servers-title")
                                    .role(Role::Heading)
                                    .aria_level(1)
                                    .aria_label(t(M::ServersTitle))
                                    .font_family(fonts::heading())
                                    .text_size(theme::text_size(text::H1))
                                    .font_weight(weights::BOLD.into())
                                    .text_color(crate::theme::ink::text())
                                    .child(t(M::ServersTitle)),
                            )
                            .child(small(t(M::PageHint)).text_color(crate::theme::ink::muted())),
                    )
                    .child(
                        Button::new("add-server", t(M::AddServer))
                            .variant(ButtonVariant::Primary)
                            .icon(Icon::Plus)
                            .on_click(cx.listener(|this, _, _, cx| this.open_add(None, cx))),
                    ),
            )
            .children(self.notice.as_ref().map(|notice| {
                let (title, message) = notice.text();
                let (color, icon) = match notice.kind {
                    NoticeKind::Warning => (colors::YELLOW, Some(Icon::TriangleExclamation)),
                    NoticeKind::Success => (colors::GREEN_300, Some(Icon::Check)),
                    NoticeKind::Info => (crate::theme::accent::light(), None),
                };
                let notice_label: SharedString = format!("{title}: {message}").into();
                div()
                    .id("servers-notice")
                    .accessible(Role::Status, "servers-notice", notice_label)
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .rounded(radius::LG)
                    .border_1()
                    .border_color(color.opacity(30))
                    .bg(color.opacity(8))
                    .children(icon.map(|icon| div().pt_0p5().child(icon.render(px(14.), color))))
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
                                    .child(title),
                            )
                            .child(small(message).text_color(crate::theme::ink::secondary())),
                    )
                    .child(
                        Button::new("dismiss-notice", t(M::Dismiss))
                            .size(ButtonSize::Slim)
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(|this, _, _, cx| this.dismiss_notice(cx))),
                    )
            }))
            .child(self.render_local(local_cards, cx))
            .child(self.render_friend_list(cx))
            .child(self.render_friends(cx))
            .child(self.render_other(other_cards))
            .child(self.render_lan(cx))
            .into_any_element()
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app: &App = cx;
        let entity = &self.servers;
        let servers = entity.read(app);
        let extras = &servers.extras;
        // One open dialog shows at a time, the first of: hosting, the LAN picker, the
        // certificate, trust, the advisory, "Add server", the update prompt. Those
        // closing fade out under it.
        let mut shown = false;
        let mut pick = |slot_open: bool| {
            let show = slot_open && !shown;
            shown |= show;
            show
        };
        let host = pick(extras.host.is_open());
        let lan = pick(extras.lan_picker.is_open());
        let cert = pick(servers.cert_prompt.is_open());
        let trust = pick(servers.trust_prompt.is_open());
        let advisory = pick(servers.advisory.is_open());
        let add = pick(servers.dialog.is_open());
        let update = pick(servers.prompt.is_open());
        // the join dialog waits under the questions its Play asks
        let join = pick(servers.friends.join.is_open());
        let friend_list = pick(servers.friend_list.dialog.is_open());
        let window = &*window;
        let mut children = Vec::new();
        children.extend(slot_layers(
            &servers.friend_list.dialog,
            friend_list,
            window,
            |prompt| {
                Some(super::friend_list_view::friend_list_dialog(
                    entity, prompt, app,
                ))
            },
        ));
        children.extend(slot_layers(&servers.friends.join, join, window, |prompt| {
            Some(super::friends_view::join_dialog(entity, prompt, app))
        }));
        children.extend(slot_layers(&servers.prompt, update, window, |prompt| {
            Some(update_dialog(entity, prompt))
        }));
        children.extend(slot_layers(&servers.dialog, add, window, |(dialog, _)| {
            Some(dialog.clone().into_any_element())
        }));
        children.extend(slot_layers(&servers.advisory, advisory, window, |prompt| {
            Some(advisory_dialog(entity, prompt).into_any_element())
        }));
        children.extend(slot_layers(
            &servers.trust_prompt,
            trust,
            window,
            |prompt| Some(trust_dialog(entity, prompt).into_any_element()),
        ));
        children.extend(slot_layers(&servers.cert_prompt, cert, window, |prompt| {
            Some(certificate_dialog(entity, prompt).into_any_element())
        }));
        children.extend(slot_layers(&extras.lan_picker, lan, window, |picker| {
            Some(super::lan::render_picker(entity, picker, app))
        }));
        children.extend(slot_layers(&extras.host, host, window, |(dialog, _)| {
            Some(dialog.clone().into_any_element())
        }));
        div().absolute().inset_0().children(children)
    }
}

/// The dialogs of `slot`: all of them when its open one is the one shown, else only
/// those closing.
fn slot_layers<T>(
    slot: &DialogSlot<Servers, T>,
    show_open: bool,
    window: &Window,
    render: impl FnMut(&T) -> Option<AnyElement>,
) -> Vec<AnyElement> {
    if show_open {
        slot.render(window, render)
    } else {
        slot.render_closing(window, render)
    }
}

/// "<server> has different mods": update before joining, or play as it is.
fn update_dialog(servers: &Entity<Servers>, prompt: &UpdatePrompt) -> AnyElement {
    let entity = servers.clone();
    let changes = &prompt.changes;
    let mut lines: Vec<String> = Vec::new();
    for (names, message) in [
        (&changes.added, M::ChangeNew),
        (&changes.updated, M::ChangeChanged),
        (&changes.removed, M::ChangeRemoved),
        (&changes.other, M::ChangeConfig),
    ] {
        for name in names.iter().take(6) {
            lines.push(tr!(message, name = name));
        }
    }
    let update = servers.clone();
    let play = servers.clone();
    let cancel = servers.clone();
    Dialog::new(
        "update-server-mods",
        tr!(M::DifferentMods, name = prompt.server.name),
        prompt.focus.clone(),
    )
    .on_close(move |_, cx| entity.update(cx, |this, cx| this.cancel_prompt(cx)))
    .child(
        div()
            .flex()
            .flex_col()
            .gap_2()
            .text_color(crate::theme::ink::secondary())
            .child(tr!(
                M::DifferentModsBody,
                changes = changes_summary(changes)
            ))
            .children(lines.into_iter().map(|line| {
                small(line)
                    .font_family(fonts::mono())
                    .text_color(crate::theme::ink::text())
            })),
    )
    .footer(
        Button::new("update-and-play", t(M::UpdateAndPlay))
            .variant(ButtonVariant::Primary)
            .on_click(move |_, _, cx| update.update(cx, |this, cx| this.answer_prompt(true, cx))),
    )
    .footer(
        Button::new("play-anyway", t(M::PlayAnyway))
            .on_click(move |_, _, cx| play.update(cx, |this, cx| this.answer_prompt(false, cx))),
    )
    .footer(
        Button::new("cancel-update", t(CommonMsg::Cancel))
            .variant(ButtonVariant::Ghost)
            .on_click(move |_, _, cx| cancel.update(cx, |this, cx| this.cancel_prompt(cx))),
    )
    .into_any_element()
}

fn paragraph(value: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_size(theme::text_size(text::MEDIUM))
        .line_height(theme::line_height(text::MEDIUM, leading::NORMAL))
        .text_color(crate::theme::ink::secondary())
        .child(value.into())
}

/// The certificate prompt: trust a certificate macOS doesn't, or a changed one.
fn certificate_dialog(servers: &Entity<Servers>, prompt: &CertPrompt) -> Dialog {
    let changed = prompt.previous.is_some();
    let name = prompt.name.clone().unwrap_or_else(|| prompt.host.clone());
    let close = servers.clone();
    let trust = servers.clone();
    let cancel = servers.clone();
    let title = if changed {
        tr!(M::CertChangedTitle, name = name)
    } else {
        t(M::CertTrustTitle).to_string()
    };
    let intro = if changed {
        tr!(M::CertChangedIntro, address = prompt.address)
    } else {
        tr!(
            M::CertUntrustedIntro,
            os = wording::OS,
            name = name,
            address = prompt.address
        )
    };
    Dialog::new("core-certificate", title, prompt.focus.clone())
        .width(px(560.))
        .on_close(move |_, cx| close.update(cx, |this, cx| this.answer_certificate(false, cx)))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(paragraph(intro))
                .children(prompt.previous.as_ref().map(|previous| {
                    fingerprint_block(t(M::FingerprintTrustedBefore), previous, colors::GRAY_400)
                }))
                .child(fingerprint_block(
                    t(if changed {
                        M::FingerprintPresented
                    } else {
                        M::FingerprintCaption
                    }),
                    &prompt.presented,
                    if changed {
                        colors::RED_200
                    } else {
                        colors::GRAY_300
                    },
                ))
                .child(small(t(M::FingerprintHelp)).text_color(crate::theme::ink::muted()))
                .child(
                    small(tr!(
                        if changed {
                            M::CertIfMatchChanged
                        } else {
                            M::CertIfMatch
                        },
                        host = prompt.host
                    ))
                    .text_color(crate::theme::ink::muted()),
                ),
        )
        .footer(
            Button::new(
                "trust-certificate",
                t(if changed {
                    M::TrustNewCertificate
                } else {
                    M::TrustCertificate
                }),
            )
            .variant(if changed {
                ButtonVariant::Danger
            } else {
                ButtonVariant::Primary
            })
            .on_click(move |_, _, cx| {
                trust.update(cx, |this, cx| this.answer_certificate(true, cx))
            }),
        )
        .footer(
            Button::new("cancel-certificate", t(CommonMsg::Cancel))
                .variant(ButtonVariant::Secondary)
                .on_click(move |_, _, cx| {
                    cancel.update(cx, |this, cx| this.answer_certificate(false, cx))
                }),
        )
}

/// "Do you trust <core>?"
fn trust_dialog(servers: &Entity<Servers>, prompt: &TrustPrompt) -> Dialog {
    let close = servers.clone();
    let not_now = servers.clone();
    let trust = servers.clone();
    Dialog::new(
        "trust-core",
        tr!(M::TrustTitle, name = prompt.name),
        prompt.focus.clone(),
    )
    .on_close(move |_, cx| close.update(cx, |this, cx| this.answer_trust(false, cx)))
    .child(
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(paragraph(tr!(
                M::CoreAt,
                name = prompt.name,
                address = prompt.address
            )))
            .child(paragraph(trust_explanation()))
            .child(
                small(if prompt.after_add {
                    tr!(M::TrustAfterAdd, setting = t(M::TrustedForMods))
                } else {
                    t(M::TrustLater).to_string()
                })
                .text_color(crate::theme::ink::muted()),
            ),
    )
    .footer(
        Button::new("trust-core-yes", t(M::TrustThisCore))
            .variant(ButtonVariant::Secondary)
            .on_click(move |_, _, cx| trust.update(cx, |this, cx| this.answer_trust(true, cx))),
    )
    // the safe answer, so the one that stands out, though last like any Cancel
    .footer(
        Button::new("trust-not-now", t(CommonMsg::NotNow))
            .variant(ButtonVariant::Primary)
            .on_click(move |_, _, cx| not_now.update(cx, |this, cx| this.answer_trust(false, cx))),
    )
}

/// A file's name, for the lists: `mods/sodium.jar` is `sodium.jar`.
fn file_label(path: &str) -> String {
    path.strip_prefix("mods/").unwrap_or(path).to_string()
}

/// "This server's mods will run on your Mac".
fn advisory_dialog(servers: &Entity<Servers>, prompt: &AdvisoryPrompt) -> Dialog {
    const SHOWN: usize = 8;
    let advisory = &prompt.advisory;
    let close = servers.clone();
    let cancel = servers.clone();
    let once = servers.clone();
    let trust = servers.clone();
    let more = |count: usize| {
        (count > SHOWN)
            .then(|| small(trn!(P::AndMore, count - SHOWN)).text_color(crate::theme::ink::muted()))
    };
    let uploaded = (!advisory.uploaded.is_empty()).then(|| {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .px_3()
            .py_2()
            .rounded(radius::LG)
            .border_1()
            .border_color(colors::YELLOW.opacity(40))
            .bg(colors::YELLOW.opacity(8))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(Icon::TriangleExclamation.render(px(14.), colors::YELLOW))
                    .child(
                        small(tr!(
                            M::UploadedNotChecked,
                            count = format::number(advisory.uploaded.len() as u64)
                        ))
                        .font_weight(weights::BOLD.into())
                        .text_color(colors::YELLOW),
                    ),
            )
            .children(advisory.uploaded.iter().take(SHOWN).map(|path| {
                small(file_label(path))
                    .font_family(fonts::mono())
                    .text_color(crate::theme::ink::text())
            }))
            .children(more(advisory.uploaded.len()))
    });
    let on_modrinth = (!advisory.on_modrinth.is_empty()).then(|| {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .px_3()
            .py_2()
            .rounded(radius::LG)
            .border_1()
            .border_color(crate::theme::ink::border())
            .bg(colors::GRAY_900)
            .child(
                small(tr!(
                    M::AlsoOnModrinth,
                    count = format::number(advisory.on_modrinth.len() as u64)
                ))
                .font_weight(weights::BOLD.into())
                .text_color(crate::theme::ink::secondary()),
            )
            .children(advisory.on_modrinth.iter().take(SHOWN).map(|found| {
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(
                        small(format!("{} {}", found.project, found.version))
                            .text_color(crate::theme::ink::text()),
                    )
                    .child(
                        small(file_label(&found.path))
                            .font_family(fonts::mono())
                            .truncate()
                            .text_color(crate::theme::ink::muted()),
                    )
            }))
            .children(more(advisory.on_modrinth.len()))
    });
    let intro = tr!(
        if prompt.update {
            M::AdvisoryUpdate
        } else {
            M::AdvisoryNew
        },
        server = prompt.server_name,
        core = prompt.core_name
    );
    let trust_label = tr!(M::TrustAndInstall, core = prompt.core_name);
    Dialog::new(
        "mod-advisory",
        tr!(M::AdvisoryTitle, computer = t(CommonMsg::ThisComputer)),
        prompt.focus.clone(),
    )
    .width(px(600.))
    .on_close(move |_, cx| {
        close.update(cx, |this, cx| {
            this.answer_advisory(AdvisoryAnswer::Cancel, cx)
        })
    })
    .child(
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(paragraph(intro))
            .when(advisory.unchecked, |this| {
                this.child(small(t(M::ModrinthUnreachable)).text_color(colors::YELLOW))
            })
            .children(uploaded)
            .children(on_modrinth)
            .children(other_files(advisory).map(|files| {
                small(tr!(
                    if advisory.scripts > 0 {
                        M::AlsoFilesScripts
                    } else {
                        M::AlsoFiles
                    },
                    files = files
                ))
                .text_color(crate::theme::ink::secondary())
            }))
            .child(
                paragraph(tr!(M::OnlyIfYouTrust, core = prompt.core_name))
                    .font_weight(weights::BOLD.into())
                    .text_color(crate::theme::ink::text()),
            )
            .child(
                small(tr!(
                    M::AdvisoryTrustNote,
                    button = trust_label,
                    explanation = trust_explanation()
                ))
                .text_color(crate::theme::ink::muted()),
            ),
    )
    // one footer item that wraps: the trust button holds the core's name
    .footer(
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap_2()
            .child(
                Button::new("advisory-once", t(M::InstallThisTime))
                    .variant(ButtonVariant::Secondary)
                    .on_click(move |_, _, cx| {
                        once.update(cx, |this, cx| {
                            this.answer_advisory(AdvisoryAnswer::InstallOnce, cx)
                        })
                    }),
            )
            .child(
                Button::new("advisory-trust", trust_label)
                    .variant(ButtonVariant::Ghost)
                    .on_click(move |_, _, cx| {
                        trust.update(cx, |this, cx| {
                            this.answer_advisory(AdvisoryAnswer::TrustAndInstall, cx)
                        })
                    }),
            )
            // the safe answer, so the one that stands out, though last like any Cancel
            .child(
                Button::new("advisory-cancel", t(CommonMsg::Cancel))
                    .variant(ButtonVariant::Primary)
                    .on_click(move |_, _, cx| {
                        cancel.update(cx, |this, cx| {
                            this.answer_advisory(AdvisoryAnswer::Cancel, cx)
                        })
                    }),
            ),
    )
}
