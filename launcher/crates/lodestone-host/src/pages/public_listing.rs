//! One instance's public listing: the "Public" switch and what it shows, after the
//! dashboard's `PublicScopeRow` (`components/Instance/PublicListingSetting.tsx`).
//!
//! Shared by Settings › General (every Minecraft Java instance, for the owner) and the
//! instance's Settings › Sharing (package C). The calls are in
//! `launcher_core::cores::admin::core_settings` (`set_public_scope`, …).

use std::rc::Rc;

use gpui::{App, ParentElement, SharedString, Styled, Window, div};
use launcher_core::cores::admin::core_settings::{PublicScope, PublicScopeChange, ScopeFlag};
use lodestone_ui::{
    components::{Checkbox, Toggle, settings::SettingRow},
    theme::{ink, text},
};

use crate::i18n::{CoreMsg, t, tr};

/// What anyone sees of a listed instance, as a sentence (`shownText`).
fn shown_text(scope: &PublicScope) -> String {
    let mut shown = vec![t(CoreMsg::ShownNameVersion)];
    if scope.status {
        shown.push(t(CoreMsg::ShownStatus));
    }
    if scope.address {
        shown.push(t(CoreMsg::ShownAddress));
    }
    if scope.client_pack {
        shown.push(t(CoreMsg::ShownClientPack));
    }
    tr!(
        CoreMsg::ShownText,
        list = lodestone_ui::i18n::format::list(&shown)
    )
}

/// The row's line under its name: what is shown, or why it can't change.
pub fn description(scope: &PublicScope, switch_on: bool, can_edit: bool) -> String {
    if !can_edit {
        t(CoreMsg::ListingOwnerOnly).to_string()
    } else if !switch_on && !scope.listed {
        t(CoreMsg::TurnOnFirst).to_string()
    } else if scope.listed {
        if switch_on {
            shown_text(scope)
        } else {
            tr!(CoreMsg::HiddenWhileOff, shown = shown_text(scope))
        }
    } else {
        t(CoreMsg::NotShown).to_string()
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
        Checkbox::new((id.clone(), index), flag_label(flag), flag.get(&scope))
            .disabled(!can_edit || saving)
            .on_change(move |value, window, cx| on_change(flag.change(*value), window, cx))
    });
    SettingRow::new(label.clone())
        .key(id.clone())
        .hint(description(&scope, switch_on, can_edit))
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
                .flex_row()
                .flex_wrap()
                .gap_x_6()
                .gap_y_2()
                .children(boxes),
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
            description(&scope, true, false),
            t(CoreMsg::ListingOwnerOnly)
        );
        assert_eq!(description(&scope, false, true), t(CoreMsg::TurnOnFirst));
        assert_eq!(description(&scope, true, true), t(CoreMsg::NotShown));
        scope.listed = true;
        scope.address = true;
        let shown = description(&scope, true, true);
        assert!(shown.contains("name and version") && shown.contains("address"));
        assert!(description(&scope, false, true).ends_with("is off."));
    }
}
