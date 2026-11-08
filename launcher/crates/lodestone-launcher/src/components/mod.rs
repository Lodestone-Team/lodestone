//! A small set of in-house components styled after the dashboard's atoms.
//!
//! Run `cargo run --example components` to see each one.

mod button;
mod dialog;
mod dialog_slot;
mod icon;
mod list_row;
mod play_button;
pub mod settings;
mod tabs;
mod text_input;
mod toggle;

pub use button::{Align, Button, ButtonSize, ButtonVariant, IconButton, LoadingDots};
pub use dialog::Dialog;
pub use dialog_slot::{DialogSlot, EXIT, EXIT_REDUCED, Presence};
pub use icon::Icon;
pub use list_row::ListRow;
pub use play_button::{PlayButton, PlayButtonState};
pub use tabs::Tabs;
pub use text_input::TextInput;
pub use toggle::Toggle;

use gpui::{App, ClickEvent, Window};

/// A click handler, as components store them.
type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// Registers the components' key bindings. Call once at startup.
pub fn init(cx: &mut App) {
    dialog::init(cx);
    text_input::init(cx);
}
