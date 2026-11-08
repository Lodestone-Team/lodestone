//! What the Servers page does beyond listing and joining (milestone 8b): a server's
//! world and resource pack ([`super::tools`]), "Host this modpack" ([`super::host`])
//! and LAN worlds ([`super::lan`]). Their state is one field of [`Servers`], and their
//! dialogs are drawn by the Servers overlay (`ServersOverlay` in [`super::view`]).

use gpui::{App, Entity, IntoElement, SharedString, Subscription, div, prelude::*};

use super::{
    Servers,
    host::HostDialog,
    lan::{LanPicker, LanState},
    tools::ServerTools,
};
use crate::{
    components::DialogSlot,
    theme::{self, colors, leading, radius, text, weights},
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
        .text_color(colors::GRAY_400)
        .child(value.into())
}

pub(super) fn note(value: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_size(theme::text_size(text::SMALL))
        .line_height(theme::line_height(text::SMALL, leading::NORMAL))
        .text_color(colors::GRAY_500)
        .child(value.into())
}

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
    div()
        .id(id)
        .flex()
        .flex_row()
        .flex_wrap()
        .gap_1()
        .p_1()
        .rounded(radius::LG)
        .bg(colors::GRAY_900)
        .border_1()
        .border_color(colors::FADE_700)
        .children(choices.into_iter().enumerate().map(|(i, (value, name))| {
            let is_selected = value == *selected;
            let on_pick = on_pick.clone();
            div()
                .id(SharedString::from(format!("{id}-{i}")))
                .px_3()
                .py_1()
                .rounded(radius::MD)
                .cursor_pointer()
                .text_size(theme::text_size(text::SMALL))
                .font_weight(weights::BOLD.into())
                .map(|this| {
                    if is_selected {
                        this.bg(colors::GRAY_700).text_color(colors::GRAY_300)
                    } else {
                        this.text_color(colors::GRAY_500)
                            .hover(|s| s.text_color(colors::GRAY_300))
                    }
                })
                .child(name)
                .on_click(move |_, _, cx| on_pick(value.clone(), cx))
        }))
}

/// `12.5 MB`.
pub(super) fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}
