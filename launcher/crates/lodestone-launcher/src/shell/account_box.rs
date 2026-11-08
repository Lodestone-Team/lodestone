//! The account box at the top of the sidebar, and its menu: switch, add and
//! remove accounts.

use gpui::{
    AnyElement, Context, IntoElement, Pixels, SharedString, anchored, deferred, div, img,
    prelude::*, px,
};
use launcher_core::accounts::{Account, AccountKind};

use super::LauncherShell;
use crate::{
    accounts::{Accounts, SessionState, offline_dialog::OFFLINE_NOTE},
    components::{Icon, LoadingDots},
    theme::{self, colors, layout, leading, radius, shadow, text, weights},
};

/// The skin head, or a placeholder for offline accounts and while it loads.
pub fn avatar(accounts: &Accounts, account: Option<&Account>, size: Pixels) -> AnyElement {
    match account.and_then(|account| accounts.avatar(&account.id)) {
        Some(image) => img(image.clone())
            .flex_none()
            .size(size)
            .rounded(radius::DEFAULT)
            .into_any_element(),
        None => div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(size)
            .rounded(radius::DEFAULT)
            .bg(colors::GRAY_700)
            .child(Icon::User.render(size * 0.5, colors::GRAY_FADED.opacity(50)))
            .into_any_element(),
    }
}

/// The second line under the account name, and its colour.
fn status(accounts: &Accounts, account: &Account) -> (SharedString, theme::Color) {
    if account.kind == AccountKind::Offline {
        return ("Offline account".into(), colors::YELLOW);
    }
    match accounts.session(&account.id) {
        Some(SessionState::Refreshing) => ("Signing in…".into(), colors::GRAY_500),
        Some(SessionState::NeedsSignIn) => ("Sign in again".into(), colors::RED_200),
        Some(SessionState::Failed(_)) => ("Offline – couldn't connect".into(), colors::YELLOW),
        Some(SessionState::Ready(_)) | None => ("Microsoft account".into(), colors::GRAY_500),
    }
}

/// The status in the menu, where space is short.
fn short_status(accounts: &Accounts, account: &Account) -> SharedString {
    match accounts.session(&account.id) {
        Some(SessionState::NeedsSignIn) => "Sign in".into(),
        _ => "Microsoft".into(),
    }
}

pub fn render(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    let accounts = shell.accounts.read(cx);
    let active = accounts.active().cloned();
    let menu_open = shell.account_menu_open;
    let (name, (detail, detail_color)) = match &active {
        Some(account) => (
            SharedString::from(account.username.clone()),
            status(accounts, account),
        ),
        None => (
            "Sign in".into(),
            ("Add a Minecraft account".into(), colors::GRAY_500),
        ),
    };
    let refreshing = active
        .as_ref()
        .is_some_and(|a| matches!(accounts.session(&a.id), Some(SessionState::Refreshing)));
    let avatar = avatar(accounts, active.as_ref(), px(32.));

    let button = div()
        .id("account-box")
        .flex()
        .flex_row()
        .items_center()
        .gap_2p5()
        .w_full()
        .px_2()
        .py_2()
        .rounded(radius::LG)
        .border_1()
        .cursor_pointer()
        .map(|this| {
            if menu_open {
                this.bg(colors::GRAY_800)
                    .border_color(colors::GRAY_FADED.opacity(30))
            } else {
                this.bg(colors::GRAY_875.opacity(0))
                    .border_color(colors::FADE_700)
                    .hover(|style| style.bg(colors::GRAY_800))
            }
        })
        .on_click(cx.listener(move |this, _, _, cx| this.set_account_menu(!menu_open, cx)))
        .child(avatar)
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .truncate()
                        .font_weight(weights::EXTRABOLD.into())
                        .text_color(colors::GRAY_300)
                        .child(name),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .text_size(theme::text_size(text::SMALL))
                        .line_height(theme::line_height(text::SMALL, leading::SNUG))
                        .font_weight(weights::BOLD.into())
                        .text_color(detail_color)
                        .child(div().truncate().child(detail))
                        .when(refreshing, |this| this.child(LoadingDots)),
                ),
        )
        .child(Icon::CaretDown.render(px(12.), colors::GRAY_FADED.opacity(50)));

    div()
        .w_full()
        .child(button)
        .when(menu_open, |this| this.child(menu(shell, cx)))
}

/// One row of the menu.
fn menu_row(
    id: impl Into<gpui::ElementId>,
    leading: AnyElement,
    label: impl Into<SharedString>,
    color: theme::Color,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .w_full()
        .px_2()
        .py_1p5()
        .rounded(radius::DEFAULT)
        .cursor_pointer()
        .font_weight(weights::BOLD.into())
        .text_color(color)
        .hover(|style| style.bg(colors::GRAY_700))
        .child(leading)
        .child(div().flex_1().min_w_0().truncate().child(label.into()))
}

fn icon_slot(icon: Icon, color: theme::Color) -> AnyElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(20.))
        .child(icon.render(px(12.), color))
        .into_any_element()
}

fn menu(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    let accounts = shell.accounts.read(cx);
    let active_id = accounts.active().map(|a| a.id.clone());
    let offline_enabled = accounts.offline_enabled();
    let active = accounts.active().cloned();
    let needs_sign_in = active
        .as_ref()
        .is_some_and(|a| matches!(accounts.session(&a.id), Some(SessionState::NeedsSignIn)));

    let mut rows: Vec<AnyElement> = Vec::new();
    for (index, account) in accounts.accounts().iter().enumerate() {
        let is_active = active_id.as_deref() == Some(account.id.as_str());
        let id = account.id.clone();
        rows.push(
            menu_row(
                ("account", index),
                avatar(accounts, Some(account), px(20.)),
                account.username.clone(),
                colors::GRAY_300,
            )
            .child(
                div()
                    .flex_none()
                    .text_size(theme::text_size(text::CAPTION))
                    .text_color(colors::GRAY_500)
                    .child(match account.kind {
                        AccountKind::Offline => SharedString::from("Offline"),
                        AccountKind::Microsoft => short_status(accounts, account),
                    }),
            )
            .child(div().flex_none().w(px(12.)).when(is_active, |this| {
                this.child(Icon::Check.render(px(12.), colors::BLUE_200))
            }))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.set_account_menu(false, cx);
                this.accounts
                    .update(cx, |accounts, cx| accounts.switch(&id, cx));
            }))
            .into_any_element(),
        );
    }

    let divider = || div().my_1p5().h(px(1.)).bg(colors::GRAY_FADED.opacity(30));
    let has_accounts = !rows.is_empty();
    let active_offline = active.as_ref().is_some_and(Account::is_offline);

    let panel = div()
        .id("account-menu")
        .occlude()
        .flex()
        .flex_col()
        .w(layout::SIDEBAR_WIDTH - px(16.))
        .mt_1p5()
        .py_2()
        .px_1p5()
        .rounded(radius::DEFAULT)
        .border_1()
        .border_color(colors::GRAY_FADED.opacity(30))
        .bg(colors::GRAY_800)
        .shadow(vec![gpui::BoxShadow {
            color: shadow::COLOR.into(),
            offset: gpui::point(px(0.), px(4.)),
            blur_radius: px(12.),
            spread_radius: px(0.),
            inset: false,
        }])
        .on_mouse_down_out(cx.listener(|this, _, _, cx| this.set_account_menu(false, cx)))
        .children(rows)
        .when(active_offline, |this| {
            this.child(
                div()
                    .mx_2()
                    .mt_1()
                    .px_2()
                    .py_1()
                    .rounded(radius::DEFAULT)
                    .bg(colors::YELLOW_FADED.opacity(10))
                    .text_size(theme::text_size(text::CAPTION))
                    .line_height(theme::line_height(text::CAPTION, leading::NORMAL))
                    .font_weight(weights::BOLD.into())
                    .text_color(colors::YELLOW)
                    .child(OFFLINE_NOTE),
            )
        })
        .when(has_accounts, |this| this.child(divider()))
        .when(needs_sign_in, |this| {
            this.child(
                menu_row(
                    "sign-in-again",
                    icon_slot(Icon::TriangleExclamation, colors::RED_200),
                    "Sign in again",
                    colors::RED_200,
                )
                .on_click(cx.listener(|this, _, window, cx| this.open_sign_in(Some(window), cx))),
            )
        })
        .child(
            menu_row(
                "add-microsoft",
                icon_slot(Icon::Plus, colors::GRAY_400),
                "Add Microsoft account",
                colors::GRAY_300,
            )
            .on_click(cx.listener(|this, _, window, cx| this.open_sign_in(Some(window), cx))),
        )
        .when(offline_enabled, |this| {
            this.child(
                menu_row(
                    "add-offline",
                    icon_slot(Icon::Plus, colors::GRAY_400),
                    "Add offline account",
                    colors::GRAY_300,
                )
                .on_click(
                    cx.listener(|this, _, window, cx| this.open_offline(None, Some(window), cx)),
                ),
            )
        })
        .when_some(active, |this, account| {
            let id = account.id.clone();
            this.child(divider()).child(
                menu_row(
                    "remove-account",
                    icon_slot(Icon::RightFromBracket, colors::RED_200),
                    format!("Remove {}", account.username),
                    colors::RED_200,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.confirm_remove(&id, Some(window), cx)
                })),
            )
        });

    // Drawn above the rest of the window, under the account box.
    deferred(anchored().child(panel)).with_priority(1)
}
