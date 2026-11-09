//! The messages, in English, by area. Each file defines its area's enums with
//! [`messages!`](crate::i18n::messages); the locales translate every variant.
//!
//! Write messages as the interface shows them, with `{name}` for what is filled in.
//! Product names (Lodestone, Minecraft, Modrinth, Fabric, Forge, NeoForge, Quilt,
//! Microsoft, Mojang) are never translated.

/// Words most screens use: buttons and the platform's names for its parts.
pub mod common;
/// Units, dates and durations, for [`format`](crate::i18n::format).
pub mod format;
