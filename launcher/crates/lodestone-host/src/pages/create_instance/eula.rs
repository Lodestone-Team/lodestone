//! The required "I agree to the Minecraft EULA" box (`EulaCheckbox.tsx`). It starts
//! unticked, and only the user ticks it: the Host never agrees for them.

use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, Window, div, prelude::*};
use lodestone_ui::{
    components::Checkbox,
    theme::{colors, ink, px, text},
};

use crate::i18n::{InstancesMsg as M, t};

/// Where users read Mojang's Minecraft EULA.
pub const MINECRAFT_EULA_URL: &str = "https://aka.ms/MinecraftEULA";

/// The box, with the link to the EULA in its label and, while unticked, why it's needed.
pub fn eula_box(
    checked: bool,
    disabled: bool,
    on_change: impl Fn(&bool, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let sentence = t(M::EulaAgree);
    let (before, after) = sentence.split_once("{link}").unwrap_or((sentence, ""));
    let link = div()
        .id("eula-link")
        .text_color(colors::BLUE_200)
        .underline()
        .cursor_pointer()
        .hover(|style| style.text_color(colors::BLUE_150))
        .child(t(M::EulaLink))
        // the link sits in the box's label: opening it must not tick the box
        .on_click(|_, _, cx| {
            cx.stop_propagation();
            cx.open_url(MINECRAFT_EULA_URL);
        });
    let mut checkbox = Checkbox::new("eula", checked, before.trim_end().to_string())
        .disabled(disabled)
        .child(link);
    if !after.trim().is_empty() {
        checkbox = checkbox.child(div().child(after.trim().to_string()));
    }
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(checkbox.on_change(on_change))
        .when(!checked, |this| {
            this.child(
                div()
                    .pl(px(24.))
                    .text_size(text::SMALL)
                    .text_color(ink::muted())
                    .child(t(M::EulaRequired)),
            )
        })
        .into_any_element()
}
