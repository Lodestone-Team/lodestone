//! The roles a value can have, and the one style each role has in both apps.
//!
//! A reader learns these once: a name is the object (medium weight, the main ink),
//! an exact value is monospace, a number has tabular figures with its unit one ink
//! lighter, a time is muted, a key is faint. The components draw with these
//! styles, and so do the typed sentences of [`tr_spans!`](crate::tr_spans).
//!
//! The colours come from [`ink`](crate::theme::ink) and the families from
//! [`fonts`](crate::theme::fonts), so a launcher skin and the Host's light theme
//! restyle them like everything else.

use gpui::{Hsla, TextRun, TextStyle};

use crate::theme::{self, colors, fonts, ink, weights};

/// What a part of a text is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// The thing a sentence is about: an instance, a server, a world, a mod.
    Object,
    /// A mod's, a plugin's or a modpack's version.
    Version,
    /// A number, with tabular figures so changing values don't jitter.
    Number,
    /// What people copy or compare character by character: a file name, a path, an
    /// address, an id, a key prefix.
    Exact,
    /// When something happened, or how long it took.
    Time,
    /// The label of a value.
    Key,
    /// A number's unit, one ink step lighter than the number.
    Unit,
    /// The sentence around the values.
    Plain,
}

/// `base` as `role` draws it, for `len` bytes of text.
///
/// A run can't change the font size: an exact value inside a sentence keeps the
/// sentence's size (JetBrains Mono reads a little larger than Satoshi at the same
/// size). Long or actionable values belong in a value field instead.
pub fn role_run(base: &TextStyle, role: Role, len: usize) -> TextRun {
    role_style(base, role).to_run(len)
}

/// `base` as `role` draws it.
pub fn role_style(base: &TextStyle, role: Role) -> TextStyle {
    let mut style = base.clone();
    let color = |c: theme::Color| -> Hsla { c.into() };
    match role {
        Role::Object => {
            style.font_weight = weights::MEDIUMBOLD.face();
            style.color = color(ink::text());
        }
        Role::Version | Role::Exact => {
            style.font_family = fonts::mono();
            style.color = color(ink::secondary());
            style.background_color = Some(color(colors::GRAY_900));
        }
        Role::Number => {
            style.font_features = fonts::tabular();
            style.color = color(ink::secondary());
        }
        Role::Unit => {
            style.font_features = fonts::tabular();
            style.color = color(ink::faint());
        }
        Role::Time => style.color = color(ink::muted()),
        Role::Key => {
            style.font_weight = weights::MEDIUM.face();
            style.color = color(ink::faint());
        }
        Role::Plain => {}
    }
    style
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_role_has_its_own_style() {
        let base = TextStyle::default();
        let run = role_run(&base, Role::Exact, 3);
        assert_eq!(run.len, 3);
        assert_eq!(run.font.family, fonts::mono());
        assert!(run.background_color.is_some());
        let number = role_run(&base, Role::Number, 1);
        assert_eq!(number.font.features, fonts::tabular());
        assert_eq!(number.font.family, base.font_family);
        let object = role_run(&base, Role::Object, 1);
        assert_eq!(object.font.weight, weights::MEDIUMBOLD.face());
        let plain = role_run(&base, Role::Plain, 1);
        assert_eq!(plain.color, base.color);
        assert_ne!(
            role_run(&base, Role::Unit, 1).color,
            role_run(&base, Role::Number, 1).color,
            "a unit is lighter than its number"
        );
    }
}
