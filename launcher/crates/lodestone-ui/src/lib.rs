//! What Lodestone Launcher and Lodestone Host share on screen: the theme carried over
//! from the dashboard, the components, the bundled fonts and icons, the translations'
//! plumbing and common words, and the part of the launcher's skins the components
//! paint with.
//!
//! Each app opens its own windows and owns its pages; this crate has no app state.
//! Run `cargo run -p lodestone-launcher --example components` to see the components.

pub mod assets;
pub mod components;
pub mod file_dialog;
pub mod i18n;
pub mod images;
pub mod markdown;
pub mod skin;
pub mod theme;
pub mod undo;

// Undo and Redo, which a focused text field takes first for its text, and an app's
// window then for its settings (see `undo`). In the `launcher` namespace, as the
// launcher has always named them.
gpui::actions!(launcher, [Undo, Redo]);

/// Sets up the bundled fonts and the components' key bindings. Call once, before
/// opening windows.
pub fn init(cx: &mut gpui::App) {
    if let Err(err) = assets::load_fonts(cx) {
        // GPUI falls back to the system font for families that did not load.
        eprintln!("lodestone-ui: could not load the bundled fonts: {err:#}");
    }
    components::init(cx);
}
