//! A small set of in-house components styled after the dashboard's atoms, shared by
//! Lodestone Launcher and Lodestone Host.
//!
//! Run `cargo run --example components` to see each one.

pub mod a11y;
mod button;
pub mod console_line;
mod dialog;
mod dialog_slot;
pub mod focus;
mod icon;
mod list_row;
pub mod log_view;
pub mod motion;
mod rich_text;
mod segmented;
pub mod settings;
mod slider;
mod tabs;
mod text_editor;
mod text_input;
mod toast;
mod toggle;
mod tooltip;
pub mod window_frame;
mod window_layer;

pub use a11y::Accessible;
pub use button::{Align, Button, ButtonSize, ButtonVariant, IconButton, LoadingDots, loading_dots};
pub use dialog::prefers_reduced_motion;
pub use dialog::{Dialog, UnderDialogs};
pub use dialog_slot::{DialogSlot, EXIT, EXIT_REDUCED, Presence};
pub use focus::{ArrowGroup, FocusScope, KeyboardFocus};
pub use icon::Icon;
pub use list_row::ListRow;
pub use rich_text::{CopySelection, RichText, clear_selection};
pub use segmented::segmented;
pub use slider::Slider;
pub use tabs::Tabs;
pub use text_editor::{Language, Pos, TextEditor, TextEditorEvent, Token};
pub use text_input::TextInput;
pub use toast::{Toast, ToastAction, ToastLevel, Toasts};
pub use toggle::Toggle;
pub use tooltip::{TextTooltip, text_tooltip};
pub use window_layer::window_layer;

use gpui::{App, ClickEvent, Window};

/// A click handler, as components store them.
pub type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// Registers the components' key bindings. Call once at startup.
pub fn init(cx: &mut App) {
    focus::init(cx);
    // First: its ⌘C (Ctrl+C) has no context, and a binding added later wins, so the
    // text fields' own Copy must come after it.
    rich_text::init(cx);
    log_view::init(cx);
    text_input::init(cx);
    text_editor::init(cx);
}

/// Emits `ui.click` (or another `ui.*` topic) for a shared control, with its element
/// id and label, for the debug traces. Labels are the control's own text, never what
/// was typed.
pub fn trace_ui(topic: &str, control: &str, id: &gpui::ElementId, detail: serde_json::Value) {
    launcher_core::trace::emit(topic, || {
        let mut event = serde_json::json!({ "control": control, "id": id.to_string() });
        if let (serde_json::Value::Object(event), serde_json::Value::Object(detail)) =
            (&mut event, detail)
        {
            event.extend(detail);
        }
        event
    });
}

mod checkbox;
mod line_chart;
pub use checkbox::{Checkbox, check_box};
pub use line_chart::LineChart;
