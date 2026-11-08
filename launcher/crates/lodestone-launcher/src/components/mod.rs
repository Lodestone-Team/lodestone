//! A small set of in-house components styled after the dashboard's atoms.
//!
//! Run `cargo run --example components` to see each one.

mod button;
mod dialog;
mod dialog_slot;
pub mod focus;
mod icon;
mod list_row;
mod play_button;
mod segmented;
pub mod settings;
mod slider;
mod tabs;
mod text_input;
mod toggle;
mod tooltip;

pub use button::{Align, Button, ButtonSize, ButtonVariant, IconButton, LoadingDots};
pub use dialog::Dialog;
pub(crate) use dialog::prefers_reduced_motion;
pub use dialog_slot::{DialogSlot, EXIT, EXIT_REDUCED, Presence};
pub use focus::{ArrowGroup, FocusScope, KeyboardFocus};
pub use icon::Icon;
pub use list_row::ListRow;
pub use play_button::{PlayButton, PlayButtonState};
pub use segmented::segmented;
pub use slider::Slider;
pub use tabs::Tabs;
pub use text_input::TextInput;
pub use toggle::Toggle;
pub use tooltip::{TextTooltip, text_tooltip};

use gpui::{App, ClickEvent, Window};

/// A click handler, as components store them.
type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// Registers the components' key bindings. Call once at startup.
pub fn init(cx: &mut App) {
    focus::init(cx);
    text_input::init(cx);
}
