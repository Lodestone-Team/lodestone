//! The Friends section of an instance's Settings tab: who it's shared with, among the
//! friends added in Lodestone Launcher ([`crate::friends`]). Nothing is shared by
//! default; each friend has a box to tick.

use gpui::{
    AnyElement, App, Entity, IntoElement, ParentElement, SharedString, Styled, div, prelude::*,
};
use lodestone_ui::{
    components::{
        Button, ButtonSize, Checkbox, Icon,
        settings::{SettingRow, card, section, tag},
    },
    theme::{colors, ink, leading, px, text},
};

use crate::{
    friends::{FriendSharing, Phase, Renew, is_connected, is_shared},
    i18n::{InstanceMsg as M, InstancePlural as P, t, tr, trn},
};

/// The section for the instance `instance`.
pub fn section_for(sharing: &Entity<FriendSharing>, instance: &str, cx: &App) -> AnyElement {
    let model = sharing.read(cx);
    let mut rows: Vec<AnyElement> = Vec::new();
    if let Some(error) = &model.error {
        rows.push(error_row(sharing, error.clone(), model.retries_by_itself()));
    }
    match model.phase {
        Phase::Opening => rows.push(
            SettingRow::new(t(M::FriendsLoading))
                .key("friends-loading")
                .into_any_element(),
        ),
        Phase::NeedsLauncher => rows.push(
            SettingRow::new(t(M::FriendsAddInLauncher))
                .key("friends-add-in-launcher")
                .hint(t(M::FriendsAddInLauncherHint))
                .into_any_element(),
        ),
        Phase::Retired => rows.push(
            SettingRow::new(t(M::FriendsRetired))
                .key("friends-retired")
                .hint(t(M::FriendsRetiredHint))
                .into_any_element(),
        ),
        Phase::Off => rows.push(
            SettingRow::new(t(M::FriendsOff))
                .key("friends-off")
                .hint(t(M::FriendsOffHint))
                .into_any_element(),
        ),
        Phase::Failed => {}
        Phase::Ready => {
            let core = model.core.as_ref();
            for friend in &model.rows {
                let pending = model
                    .pending
                    .get(&(friend.key.clone(), instance.to_string()))
                    .copied();
                let checked = pending.unwrap_or_else(|| is_shared(core, &friend.key, instance));
                let connected = is_connected(core, &friend.key);
                let on_change = {
                    let sharing = sharing.clone();
                    let key = friend.key.clone();
                    let instance = instance.to_string();
                    move |on: &bool, _: &mut gpui::Window, cx: &mut App| {
                        sharing.update(cx, |sharing, cx| {
                            sharing.set_shared(&key, &instance, *on, cx)
                        });
                    }
                };
                let tags = div()
                    .flex()
                    .flex_row()
                    .flex_none()
                    .items_center()
                    .ml_auto()
                    .gap(px(8.))
                    // to tell apart friends with the same name
                    .child(
                        div()
                            .text_size(text::SMALL)
                            .text_color(ink::muted())
                            .font_family(lodestone_ui::theme::fonts::mono())
                            .child(friend.safety_code.clone()),
                    )
                    .when(!friend.verified, |this| {
                        this.child(tag(t(M::FriendsNotVerified), colors::YELLOW))
                    })
                    .when(connected, |this| {
                        this.child(tag(t(M::FriendsConnected), colors::GREEN))
                    });
                rows.push(
                    div()
                        .id(SharedString::from(format!("friend-{}", friend.key)))
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap_x(px(16.))
                        .gap_y(px(8.))
                        .px(px(16.))
                        .py(px(12.))
                        .min_h(px(56.))
                        .child(
                            Checkbox::new(
                                SharedString::from(format!("friend-share-{}", friend.key)),
                                checked,
                                tr!(M::FriendsShareWith, name = friend.name),
                            )
                            .disabled(pending.is_some())
                            .on_change(on_change),
                        )
                        .child(tags)
                        .into_any_element(),
                );
                if model
                    .confirm
                    .as_ref()
                    .is_some_and(|(key, at)| key == &friend.key && at == instance)
                {
                    rows.push(confirm_row(sharing, &friend.name));
                }
            }
        }
    }
    if matches!(model.phase, Phase::Ready | Phase::NeedsLauncher) {
        rows.extend(certificate_rows(sharing, model));
    }
    let connected = model
        .rows
        .iter()
        .filter(|f| model.phase == Phase::Ready && is_connected(model.core.as_ref(), &f.key))
        .count();
    let synced = model.synced && model.error.is_none() && model.phase == Phase::Ready;
    section(t(M::FriendsTitle), Some(t(M::FriendsLead).into()))
        .child(card(rows))
        .when(synced || connected > 0, |this| {
            // whether the core has the friend list as it is now (removals reach it
            // only through the Host), and who is playing
            this.child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap_x(px(12.))
                    .text_size(text::SMALL)
                    .text_color(ink::muted())
                    .when(synced, |this| {
                        this.child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(4.))
                                .text_color(colors::GREEN)
                                .child(Icon::Check.render(px(12.), colors::GREEN))
                                .child(t(M::FriendsSynced)),
                        )
                    })
                    .when(connected > 0, |this| {
                        this.child(trn!(P::FriendsConnectedCount, connected))
                    }),
            )
        })
        .into_any_element()
}

/// The core's friend certificate: once retired (here or in the Launcher), "Get a new
/// certificate", or how to by hand on a core too old for it; otherwise "Retire this
/// core's certificate", confirmed first (for a computer whose data may have been
/// copied).
fn certificate_rows(sharing: &Entity<FriendSharing>, model: &FriendSharing) -> Vec<AnyElement> {
    let working = model.renew == Renew::Working;
    if model.certificate_retired {
        let row = if model.renew == Renew::Unsupported {
            SettingRow::new(t(M::FriendsCertificateRetired))
                .key("friends-certificate-retired")
                .hint(tr!(
                    M::FriendsCertificateManual,
                    path = model.certificate_file().display().to_string()
                ))
                .into_any_element()
        } else {
            let renew = sharing.clone();
            SettingRow::new(t(M::FriendsCertificateRetired))
                .key("friends-certificate-retired")
                .hint(t(M::FriendsCertificateRetiredHint))
                .child(
                    Button::new("friends-certificate-new", t(M::FriendsCertificateNew))
                        .size(ButtonSize::Slim)
                        .loading(working)
                        .disabled(working)
                        .on_click(move |_, _, cx| {
                            renew.update(cx, |sharing, cx| sharing.renew_certificate(cx));
                        }),
                )
                .into_any_element()
        };
        return vec![row];
    }
    if model.renew == Renew::Confirming || working {
        let (renew, cancel) = (sharing.clone(), sharing.clone());
        return vec![
            div()
                .id("friends-certificate-confirm")
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap(px(12.))
                .px(px(16.))
                .py(px(12.))
                .bg(colors::YELLOW.opacity(8))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(240.))
                        .text_size(text::SMALL)
                        .line_height(lodestone_ui::theme::line_height(
                            text::SMALL,
                            leading::NORMAL,
                        ))
                        .text_color(ink::text())
                        .child(t(M::FriendsCertificateRetireBody)),
                )
                .child(
                    Button::new(
                        "friends-certificate-retire",
                        t(M::FriendsCertificateRetireConfirm),
                    )
                    .size(ButtonSize::Slim)
                    .loading(working)
                    .disabled(working)
                    .on_click(move |_, _, cx| {
                        renew.update(cx, |sharing, cx| sharing.renew_certificate(cx));
                    }),
                )
                .child(
                    Button::new("friends-certificate-cancel", t(M::FriendsCancel))
                        .size(ButtonSize::Slim)
                        .disabled(working)
                        .on_click(move |_, _, cx| {
                            cancel.update(cx, |sharing, cx| sharing.cancel_renew(cx));
                        }),
                )
                .into_any_element(),
        ];
    }
    if model.phase != Phase::Ready || model.core.is_none() {
        return Vec::new();
    }
    let ask = sharing.clone();
    vec![
        SettingRow::new(t(M::FriendsCertificate))
            .key("friends-certificate")
            .hint(t(M::FriendsCertificateHint))
            .child(
                Button::new("friends-certificate-ask", t(M::FriendsCertificateRetire))
                    .size(ButtonSize::Slim)
                    .on_click(move |_, _, cx| {
                        ask.update(cx, |sharing, cx| sharing.ask_renew(cx));
                    }),
            )
            .into_any_element(),
    ]
}

/// Before the first share with a friend whose safety code wasn't compared: a warning,
/// "Share anyway" and "Cancel".
fn confirm_row(sharing: &Entity<FriendSharing>, name: &str) -> AnyElement {
    let (share, cancel) = (sharing.clone(), sharing.clone());
    div()
        .id("friends-confirm")
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(12.))
        .px(px(16.))
        .py(px(12.))
        .bg(colors::YELLOW.opacity(8))
        .child(
            div()
                .flex_1()
                .min_w(px(240.))
                .text_size(text::SMALL)
                .line_height(lodestone_ui::theme::line_height(
                    text::SMALL,
                    leading::NORMAL,
                ))
                .text_color(ink::text())
                .child(tr!(M::FriendsConfirmUnverified, name = name)),
        )
        .child(
            Button::new("friends-confirm-share", t(M::FriendsShareAnyway))
                .size(ButtonSize::Slim)
                .on_click(move |_, _, cx| {
                    share.update(cx, |sharing, cx| sharing.confirm_share(cx));
                }),
        )
        .child(
            Button::new("friends-confirm-cancel", t(M::FriendsCancel))
                .size(ButtonSize::Slim)
                .on_click(move |_, _, cx| {
                    cancel.update(cx, |sharing, cx| sharing.cancel_share(cx));
                }),
        )
        .into_any_element()
}

/// A failure, in red, with Try again (and, when it does, that the Host tries again by
/// itself).
fn error_row(sharing: &Entity<FriendSharing>, error: String, retrying: bool) -> AnyElement {
    let sharing = sharing.clone();
    div()
        .id("friends-error")
        .flex()
        .flex_row()
        .items_center()
        .gap(px(16.))
        .px(px(16.))
        .py(px(12.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(text::SMALL)
                .line_height(lodestone_ui::theme::line_height(
                    text::SMALL,
                    leading::NORMAL,
                ))
                .text_color(colors::RED_200)
                .child(error)
                .when(retrying, |this| {
                    this.child(div().text_color(ink::muted()).child(t(M::FriendsRetrying)))
                }),
        )
        .child(
            Button::new("friends-retry", t(M::FriendsRetry))
                .size(ButtonSize::Slim)
                .on_click(move |_, _, cx| {
                    sharing.update(cx, |sharing, cx| sharing.retry(cx));
                }),
        )
        .into_any_element()
}
