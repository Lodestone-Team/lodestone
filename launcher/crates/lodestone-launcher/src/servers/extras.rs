//! What the Servers page does beyond listing and joining (milestone 8b): a server's
//! world and resource pack ([`super::tools`]), "Host this modpack" ([`super::host`])
//! and LAN worlds ([`super::lan`]). Their state is one field of [`Servers`], and their
//! dialogs are drawn by the Servers overlay (`ServersOverlay` in [`super::view`]).

use crate::components::RichText;
use gpui::{App, Entity, IntoElement, SharedString, Subscription, div, prelude::*};

use super::{
    Servers,
    host::HostDialog,
    lan::{LanPicker, LanState},
    tools::ServerTools,
};
use crate::{
    components::DialogSlot,
    theme::{self, colors, leading, text, weights},
};

/// The 8b state of the Servers page.
pub struct Extras {
    pub(super) tools: DialogSlot<Servers, (Entity<ServerTools>, Subscription)>,
    pub(super) host: DialogSlot<Servers, (Entity<HostDialog>, Subscription)>,
    pub(super) lan: LanState,
    pub(super) lan_picker: DialogSlot<Servers, LanPicker>,
}

impl Default for Extras {
    fn default() -> Self {
        Self {
            tools: DialogSlot::new(|this: &mut Servers| &mut this.extras.tools),
            host: DialogSlot::new(|this: &mut Servers| &mut this.extras.host),
            lan: LanState::default(),
            lan_picker: DialogSlot::new(|this: &mut Servers| &mut this.extras.lan_picker),
        }
    }
}

pub(super) fn label(value: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_size(theme::text_size(text::SMALL))
        .font_weight(weights::BOLD.into())
        .text_color(crate::theme::ink::secondary())
        .child(value.into())
}

/// A small line of text, which can be selected and copied.
#[track_caller]
pub(super) fn note(value: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_size(theme::text_size(text::SMALL))
        .line_height(theme::line_height(text::SMALL, leading::NORMAL))
        .text_color(crate::theme::ink::muted())
        .child(RichText::new(value))
}

#[track_caller]
pub(super) fn error_line(value: impl Into<SharedString>) -> gpui::Div {
    note(value)
        .font_weight(weights::BOLD.into())
        .text_color(colors::RED_200)
}

/// A row of choices, the selected one raised, as in "Add server".
pub(super) fn pills<T: Clone + PartialEq + 'static>(
    id: &'static str,
    choices: Vec<(T, SharedString)>,
    selected: &T,
    on_pick: impl Fn(T, &mut App) + Clone + 'static,
) -> impl IntoElement {
    crate::components::segmented(id, choices, Some(selected), false, move |value, _, cx| {
        on_pick(value, cx)
    })
}
