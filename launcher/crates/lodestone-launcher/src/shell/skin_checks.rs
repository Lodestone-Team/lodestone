//! The skin editor's readability suggestions: each surface's text that falls short
//! of its contrast, by how much, and a one-click fix. Readability is the skin's
//! responsibility; the launcher only suggests.

use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, prelude::FluentBuilder};
use launcher_core::ui_skin::{
    SurfaceId,
    readable::{Check, Fix, TextRole},
};

use crate::{
    components::{Button, ButtonSize, ButtonVariant, Icon},
    i18n::{ShellMsg as M, t, tr},
    theme::{colors, px},
};

/// The name of `surface`, for people.
pub fn surface_name(surface: SurfaceId) -> &'static str {
    t(match surface {
        SurfaceId::Window => M::SkinSurfaceWindow,
        SurfaceId::TopBar => M::SkinSurfaceTopBar,
        SurfaceId::Sidebar => M::SkinSurfaceSidebar,
        SurfaceId::Main => M::SkinSurfaceMain,
        SurfaceId::Hero => M::SkinSurfaceHero,
        SurfaceId::PlayBar => M::SkinSurfacePlayBar,
        SurfaceId::Card => M::SkinSurfaceCard,
        SurfaceId::SettingCard => M::SkinSurfaceSettingCard,
        SurfaceId::Row => M::SkinSurfaceRow,
        SurfaceId::Dialog => M::SkinSurfaceDialog,
        SurfaceId::SettingsNav => M::SkinSurfaceSettingsNav,
        SurfaceId::Popover => M::SkinSurfacePopover,
        SurfaceId::Input => M::SkinSurfaceInput,
        SurfaceId::ButtonPrimary => M::SkinSurfaceButtonPrimary,
        SurfaceId::ButtonSecondary => M::SkinSurfaceButtonSecondary,
        SurfaceId::ButtonGhost => M::SkinSurfaceButtonGhost,
        SurfaceId::ButtonDanger => M::SkinSurfaceButtonDanger,
        SurfaceId::Toggle => M::SkinSurfaceToggle,
        SurfaceId::Tab => M::SkinSurfaceTab,
        SurfaceId::Pill => M::SkinSurfacePill,
        SurfaceId::PlayButton => M::SkinSurfacePlayButton,
        SurfaceId::Progress => M::SkinSurfaceProgress,
        SurfaceId::Toast => M::SkinSurfaceToast,
    })
}

/// The name of a text role, for people.
pub fn role_name(role: TextRole) -> &'static str {
    t(match role {
        TextRole::Text => M::SkinRoleText,
        TextRole::Muted => M::SkinRoleMuted,
        TextRole::Faint => M::SkinRoleFaint,
        TextRole::Label => M::SkinRoleLabel,
    })
}

fn ratio(value: f32) -> String {
    crate::i18n::format::decimal(f64::from(value), 1)
}

/// The line for a check that falls short: "Sidebar, muted text: 3.1:1, needs 4.5:1".
pub fn describe(check: &Check) -> String {
    tr!(
        M::SkinCheckShort,
        surface = surface_name(check.surface),
        role = role_name(check.role),
        ratio = ratio(check.ratio),
        target = ratio(check.target)
    )
}

/// The suggestions: a line for each check that falls short with its fix, and Fix all;
/// or that everything passes. `apply` gets a fix, or `None` for all of them. Fixes
/// are offered only when `editable`.
pub fn render<V: 'static>(
    checks: &[Check],
    editable: bool,
    id_prefix: &'static str,
    apply: impl Fn(&mut V, Option<Fix>, &mut Context<V>) + Clone + 'static,
    cx: &mut Context<V>,
) -> AnyElement {
    let short: Vec<&Check> = checks.iter().filter(|c| !c.passes()).collect();
    if short.is_empty() {
        return div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1p5()
            .text_color(colors::GREEN_300)
            .child(Icon::Check.render(px(14.), colors::GREEN_300))
            .child(tr!(M::SkinChecksPass, count = checks.len().to_string()))
            .into_any_element();
    }
    let fixable = short.iter().filter(|c| c.fix.is_some()).count();
    let rows = short.iter().enumerate().map(|(i, check)| {
        let fix = check.fix.clone();
        let apply = apply.clone();
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_x_3()
            .gap_y_1()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .text_color(colors::YELLOW)
                    .child(Icon::TriangleExclamation.render(px(14.), colors::YELLOW))
                    .child(describe(check)),
            )
            .map(|row| match fix {
                Some(fix) if editable => row.child(
                    Button::new((id_prefix, i), t(M::SkinApplyFix))
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Slim)
                        .on_click(
                            cx.listener(move |this, _, _, cx| apply(this, Some(fix.clone()), cx)),
                        ),
                ),
                Some(_) => row,
                None => row.child(
                    div()
                        .text_color(crate::theme::ink::muted())
                        .child(t(M::SkinCheckNoFix)),
                ),
            })
    });
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_color(crate::theme::ink::muted())
                .child(t(M::SkinChecksNote)),
        )
        .children(rows)
        .when(editable && fixable > 1, |this| {
            let apply = apply.clone();
            this.child(
                div().flex().child(
                    Button::new((id_prefix, usize::MAX), t(M::SkinFixAll))
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Slim)
                        .on_click(cx.listener(move |this, _, _, cx| apply(this, None, cx))),
                ),
            )
        })
        .into_any_element()
}
