//! One instance's public listing: the "Public" switch and what it shows, after the
//! dashboard's `PublicScopeRow` (`components/Instance/PublicListingSetting.tsx`).
//!
//! Shared by Settings › General (every Minecraft Java instance, for the owner) and the
//! instance's Settings › Sharing (package C). The calls are in
//! `launcher_core::cores::admin::core_settings` (`set_public_scope`, …).

use std::rc::Rc;

use gpui::{App, ParentElement, SharedString, Styled, Window, div, prelude::FluentBuilder as _};
use launcher_core::cores::admin::core_settings::{PublicScope, PublicScopeChange, ScopeFlag};
use lodestone_ui::{
    components::{Checkbox, StatusBadge, Toggle, settings::SettingRow},
    theme::{ink, text},
};

use crate::i18n::{CoreMsg, t};

/// What anyone sees of a listed instance, one field a chip (`shownText`).
pub fn shown_fields(scope: &PublicScope) -> Vec<&'static str> {
    let mut shown = vec![t(CoreMsg::ShownNameVersion), t(CoreMsg::ShownVersion)];
    if scope.status {
        shown.push(t(CoreMsg::ShownStatus));
        shown.push(t(CoreMsg::ShownPlayers));
    }
    if scope.address {
        shown.push(t(CoreMsg::ShownAddress));
    }
    if scope.client_pack {
        shown.push(t(CoreMsg::ShownClientPack));
    }
    shown
}

/// The row's line under its name: what is shown, or why it can't change. None for a
/// server that isn't listed and can be.
pub fn description(scope: &PublicScope, switch_on: bool, can_edit: bool) -> Option<String> {
    if !can_edit {
        Some(t(CoreMsg::ListingOwnerOnly).to_string())
    } else if !switch_on && !scope.listed {
        Some(t(CoreMsg::TurnOnFirst).to_string())
    } else if scope.listed {
        Some(if switch_on {
            t(CoreMsg::ShownText).to_string()
        } else {
            format!("{} {}", t(CoreMsg::ShownText), t(CoreMsg::HiddenWhileOff))
        })
    } else {
        None
    }
}

/// The flag's label.
fn flag_label(flag: ScopeFlag) -> &'static str {
    t(match flag {
        ScopeFlag::Status => CoreMsg::FlagStatus,
        ScopeFlag::Address => CoreMsg::FlagAddress,
        ScopeFlag::ClientPack => CoreMsg::FlagClientPack,
    })
}

/// The row for one instance, named `label`: the "Public" switch, and the Status,
/// Address and Client pack boxes under it. `switch_on` is the core's "List servers
/// publicly"; only the owner (`can_edit`) changes anything; `saving` disables it while
/// a change is sent. `on_change` sends the change (`set_public_scope`).
pub fn scope_row(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    scope: PublicScope,
    switch_on: bool,
    can_edit: bool,
    saving: bool,
    on_change: impl Fn(PublicScopeChange, &mut Window, &mut App) + 'static,
) -> SettingRow {
    let id: SharedString = id.into();
    let label: SharedString = label.into();
    let on_change = Rc::new(on_change);
    let toggle_change = on_change.clone();
    let boxes = ScopeFlag::ALL.into_iter().enumerate().map(|(index, flag)| {
        let on_change = on_change.clone();
        Checkbox::new((id.clone(), index), flag.get(&scope), flag_label(flag))
            .disabled(!can_edit || saving)
            .on_change(move |value, window, cx| on_change(flag.change(*value), window, cx))
    });
    SettingRow::new(label.clone())
        .key(id.clone())
        .hint_opt(description(&scope, switch_on, can_edit))
        .child(
            div()
                .text_size(text::SMALL)
                .text_color(ink::muted())
                .child(t(CoreMsg::ListingPublic)),
        )
        .child(
            Toggle::new((id.clone(), 10usize), scope.listed)
                .label(label)
                // unlisting always works; listing needs the core's switch
                .disabled(!can_edit || saving || (!switch_on && !scope.listed))
                .on_change(move |value, window, cx| {
                    toggle_change(
                        PublicScopeChange {
                            listed: Some(*value),
                            ..Default::default()
                        },
                        window,
                        cx,
                    )
                }),
        )
        .below(
            div()
                .flex()
                .flex_col()
                .gap_2()
                // what is shown, a chip a field, so they can be counted
                .when(scope.listed && can_edit, |this| {
                    this.child(div().flex().flex_row().flex_wrap().gap_1p5().children(
                        shown_fields(&scope).into_iter().map(|field| {
                            StatusBadge::new(field, lodestone_ui::theme::Tone::Neutral)
                        }),
                    ))
                })
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap_x_6()
                        .gap_y_2()
                        .children(boxes),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sentence_says_what_is_shown() {
        lodestone_ui::i18n::force(lodestone_ui::i18n::Locale::En);
        let mut scope = PublicScope::default();
        assert_eq!(
            description(&scope, true, false).as_deref(),
            Some(t(CoreMsg::ListingOwnerOnly))
        );
        assert_eq!(
            description(&scope, false, true).as_deref(),
            Some(t(CoreMsg::TurnOnFirst))
        );
        assert_eq!(description(&scope, true, true), None);
        scope.listed = true;
        scope.address = true;
        assert_eq!(
            description(&scope, true, true).as_deref(),
            Some(t(CoreMsg::ShownText))
        );
        assert_eq!(shown_fields(&scope), ["Name", "Version", "Address"]);
        assert!(
            description(&scope, false, true)
                .unwrap()
                .ends_with("is off.")
        );
    }
}
