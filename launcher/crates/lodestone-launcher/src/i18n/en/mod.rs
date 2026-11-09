//! The messages, in English, by area. Each file defines its area's enums with
//! [`messages!`](crate::i18n::messages); the locales translate every variant.
//!
//! Write messages as the interface shows them, with `{name}` for what is filled in.
//! Product names (Lodestone, Minecraft, Modrinth, Fabric, Forge, NeoForge, Quilt,
//! Microsoft, Mojang) are never translated.

/// Words most screens use: buttons and the platform's names for its parts.
pub mod common;
/// The Content tab: mods, resource packs and shader packs, and Modrinth's pages.
pub mod content;
/// Units, dates and durations, for [`format`](crate::i18n::format).
pub mod format;
/// An instance's Settings tab, the new-instance dialog and their fields.
pub mod instance_settings;
/// Instances: playing, the console, imports and modpacks.
pub mod instances;
/// The Servers page, a server's page and the dialogs about cores.
pub mod servers;
/// Minecraft's own option and key names, in Settings > Shared game settings.
pub mod shared_options;
/// The window's frame: top bar, sidebar, Play tab, the launcher's Settings, accounts
/// and the shared components.
pub mod shell;
/// The Worlds tab: worlds, backups and screenshots.
pub mod worlds;
