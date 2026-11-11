//! The account box at the top of the sidebar, and its menu: switch, add and
//! remove accounts.

use crate::skin::Skinned;
use crate::theme::px;
use gpui::{AnyElement, Context, IntoElement, Pixels, SharedString, div, img, prelude::*};
use launcher_core::accounts::{Account, AccountKind};
use launcher_core::ui_skin::SurfaceId;

use super::LauncherShell;
use crate::{
    accounts::{Accounts, SessionState},
    components::{
        Dropdown, Icon, KeyboardFocus, LoadingDots,
        a11y::{Accessible, Role, Toggled, automation_id},
    },
    i18n::{CommonMsg, ShellMsg as M, t, tr},
    theme::{self, colors, layout, leading, radius, shadow, text, weights},
};

/// Opens or closes the account menu, for assistive technology's Expand and Collapse.
fn menu_toggle(
    open: bool,
    cx: &Context<LauncherShell>,
) -> impl Fn(&mut gpui::Window, &mut gpui::App) + Clone + 'static {
    let shell = cx.entity().downgrade();
    move |_, cx| {
        shell
            .update(cx, |this, cx| this.set_account_menu(!open, cx))
            .ok();
    }
}

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
        return (t(M::OfflineAccount).into(), colors::YELLOW);
    }
    match accounts.session(&account.id) {
        Some(SessionState::Refreshing) => (t(M::SigningIn).into(), colors::GRAY_500),
        Some(SessionState::NeedsSignIn) => (t(M::SignInAgain).into(), colors::RED_200),
        Some(SessionState::Failed(_)) => (t(M::OfflineCouldntConnect).into(), colors::YELLOW),
        Some(SessionState::Ready(_)) | None if account.uses_launcher_identity() => {
            (t(M::LauncherIdentity).into(), colors::YELLOW)
        }
        Some(SessionState::Ready(_)) | None => (t(M::MicrosoftAccount).into(), colors::GRAY_500),
    }
}

/// The small badge on accounts signed in as the Minecraft Launcher.
pub(super) fn launcher_identity_badge() -> impl IntoElement {
    div()
        .flex_none()
        // The account box above shows it in full; here the name comes first.
        .max_w(px(72.))
        .truncate()
        .px_1()
        .rounded(radius::DEFAULT)
        .bg(colors::YELLOW_FADED.opacity(10))
        .border_1()
        .border_color(colors::YELLOW_FADED.opacity(50))
        .text_size(theme::text_size(text::CAPTION))
        .line_height(theme::line_height(text::CAPTION, leading::SNUG))
        .font_weight(weights::BOLD.into())
        .text_color(colors::YELLOW)
        .child(t(M::LauncherIdentity))
}

/// The status in the menu, where space is short.
fn short_status(accounts: &Accounts, account: &Account) -> SharedString {
    match accounts.session(&account.id) {
        Some(SessionState::NeedsSignIn) => t(CommonMsg::SignIn).into(),
        _ => "Microsoft".into(),
    }
}

/// The account box of the folded sidebar: the avatar alone, opening the same menu.
/// The name shows as a tooltip.
pub fn render_compact(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    let accounts = shell.accounts.read(cx);
    let active = accounts.active().cloned();
    let menu_open = shell.account_menu_open;
    let name: SharedString = match &active {
        Some(account) => account.username.clone().into(),
        None => t(CommonMsg::SignIn).into(),
    };
    let toggle = menu_toggle(menu_open, cx);
    let button = div()
        .id("account-box")
        .accessible(Role::Button, "account-box", name.clone())
        .expandable(menu_open, toggle)
        .keyboard_focus()
        .p_1()
        .rounded(radius::LG)
        .border_1()
        .cursor_pointer()
        .map(|this| {
            if menu_open {
                this.bg(colors::GRAY_800)
                    .border_color(colors::GRAY_FADED.opacity(30))
            } else {
                this.border_color(crate::theme::ink::border())
                    .hover(|style| style.bg(colors::GRAY_800))
            }
        })
        .tooltip(crate::components::text_tooltip(name))
        .on_press(cx.listener(move |this, _, _, cx| this.set_account_menu(!menu_open, cx)))
        .child(avatar(accounts, active.as_ref(), px(32.)));
    div().child(with_menu(button, menu_open, shell, cx))
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
            t(CommonMsg::SignIn).into(),
            (t(M::AddMinecraftAccount).into(), colors::GRAY_500),
        ),
    };
    let refreshing = active
        .as_ref()
        .is_some_and(|a| matches!(accounts.session(&a.id), Some(SessionState::Refreshing)));
    let avatar = avatar(accounts, active.as_ref(), px(32.));

    let toggle = menu_toggle(menu_open, cx);
    let button = div()
        .id("account-box")
        .accessible(Role::Button, "account-box", name.clone())
        .aria_description(detail.clone())
        .expandable(menu_open, toggle)
        .keyboard_focus()
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
                this.bg(crate::skin::sidebar_inks().fill)
                    .border_color(colors::GRAY_FADED.opacity(30))
            } else {
                this.bg(colors::GRAY_875.opacity(0))
                    .border_color(crate::theme::ink::border())
                    .hover(|style| style.bg(crate::skin::sidebar_inks().fill))
            }
        })
        .on_press(cx.listener(move |this, _, _, cx| this.set_account_menu(!menu_open, cx)))
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
                        .text_color(crate::skin::sidebar_inks().primary)
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
                        .text_color(if detail_color == colors::GRAY_500 {
                            crate::skin::sidebar_inks().muted
                        } else {
                            detail_color
                        })
                        .child(div().truncate().child(detail))
                        .when(refreshing, |this| this.child(LoadingDots)),
                ),
        )
        .child(Icon::CaretDown.render(px(12.), crate::skin::sidebar_inks().icon));

    div()
        .w_full()
        .child(with_menu(button, menu_open, shell, cx))
}

/// One row of the menu. An account's name is cut short beside its status; an
/// action's label wraps when it is long.
fn menu_row(
    id: impl Into<gpui::ElementId>,
    leading: AnyElement,
    label: impl Into<SharedString>,
    color: theme::Color,
) -> gpui::Stateful<gpui::Div> {
    menu_row_with(id, leading, label, color, false)
}

fn action_row(
    id: impl Into<gpui::ElementId>,
    leading: AnyElement,
    label: impl Into<SharedString>,
    color: theme::Color,
) -> gpui::Stateful<gpui::Div> {
    menu_row_with(id, leading, label, color, true)
}

fn menu_row_with(
    id: impl Into<gpui::ElementId>,
    leading: AnyElement,
    label: impl Into<SharedString>,
    color: theme::Color,
    wrap: bool,
) -> gpui::Stateful<gpui::Div> {
    let id = id.into();
    let label: SharedString = label.into();
    div()
        .id(id.clone())
        .accessible(Role::MenuItem, automation_id(&id), label.clone())
        .keyboard_focus()
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
        .child(
            div()
                .flex_1()
                .map(|this| {
                    if wrap {
                        this.min_w_0()
                    } else {
                        this.min_w_0().truncate()
                    }
                })
                .child(label),
        )
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

/// The account box `button`, and its menu under it while `open`.
fn with_menu(
    button: impl IntoElement,
    open: bool,
    shell: &LauncherShell,
    cx: &mut Context<LauncherShell>,
) -> impl IntoElement {
    let close = cx.listener(|this, _: &(), _, cx| this.set_account_menu(false, cx));
    Dropdown::new(button).when(open, |this| {
        this.popover("account-menu-scope", menu(shell, cx), move |window, cx| {
            close(&(), window, cx)
        })
    })
}

fn menu(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement + use<> {
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
            .role(Role::MenuItemRadio)
            .aria_toggled(if is_active {
                Toggled::True
            } else {
                Toggled::False
            })
            .map(|row| {
                let needs_sign_in = matches!(
                    accounts.session(&account.id),
                    Some(SessionState::NeedsSignIn)
                );
                if account.uses_launcher_identity() && !needs_sign_in {
                    row.child(launcher_identity_badge())
                } else {
                    row.child(
                        div()
                            .flex_none()
                            .max_w(px(96.))
                            .truncate()
                            .text_size(theme::text_size(text::CAPTION))
                            .text_color(crate::theme::ink::muted())
                            .child(match account.kind {
                                AccountKind::Offline => SharedString::from(t(M::Offline)),
                                AccountKind::Microsoft => short_status(accounts, account),
                            }),
                    )
                }
            })
            .child(div().flex_none().w(px(12.)).when(is_active, |this| {
                this.child(Icon::Check.render(px(12.), crate::theme::accent::light()))
            }))
            .on_press(cx.listener(move |this, _, _, cx| {
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

    div()
        .id("account-menu")
        .role(Role::Menu)
        .accessibility_id("account-menu")
        .occlude()
        .flex()
        .flex_col()
        .w(layout::sidebar_width() - px(16.))
        .py_2()
        .px_1p5()
        .rounded(radius::DEFAULT)
        .border_1()
        .border_color(colors::GRAY_FADED.opacity(30))
        .bg(colors::GRAY_800)
        .surface(SurfaceId::Popover)
        .shadow(vec![gpui::BoxShadow {
            color: shadow::COLOR.into(),
            offset: gpui::point(px(0.), px(4.)),
            blur_radius: px(12.),
            spread_radius: px(0.),
            inset: false,
        }])
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
                    .child(t(M::OfflineNote)),
            )
        })
        .when(has_accounts, |this| this.child(divider()))
        .when(needs_sign_in, |this| {
            this.child(
                action_row(
                    "sign-in-again",
                    icon_slot(Icon::TriangleExclamation, colors::RED_200),
                    t(M::SignInAgain),
                    colors::RED_200,
                )
                .on_press(cx.listener(|this, _, window, cx| this.open_sign_in(Some(window), cx))),
            )
        })
        .child(
            action_row(
                "add-microsoft",
                icon_slot(Icon::Plus, colors::GRAY_400),
                t(M::AddMicrosoftAccount),
                colors::GRAY_300,
            )
            .on_press(cx.listener(|this, _, window, cx| this.open_sign_in(Some(window), cx))),
        )
        .when(offline_enabled, |this| {
            this.child(
                action_row(
                    "add-offline",
                    icon_slot(Icon::Plus, colors::GRAY_400),
                    t(M::AddOfflineAccount),
                    colors::GRAY_300,
                )
                .on_press(
                    cx.listener(|this, _, window, cx| this.open_offline(None, Some(window), cx)),
                ),
            )
        })
        .when_some(active, |this, account| {
            let id = account.id.clone();
            this.child(divider()).child(
                action_row(
                    "remove-account",
                    icon_slot(Icon::RightFromBracket, colors::RED_200),
                    tr!(M::RemoveNamed, name = account.username),
                    colors::RED_200,
                )
                .on_press(cx.listener(move |this, _, window, cx| {
                    this.confirm_remove(&id, Some(window), cx)
                })),
            )
        })
}
