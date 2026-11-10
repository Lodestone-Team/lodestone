//! The components: the shared ones from `lodestone_ui::components`, and the launcher's
//! own Play button.
//!
//! Run `cargo run --example components` to see each one.

pub use lodestone_ui::components::*;

mod magnet_bar;
mod play_button;

pub use magnet_bar::{GEM as MAGNET_GEM, MagnetBar, MagnetLook};
pub use play_button::{LaunchStep, PlayButton, PlayButtonState};

mod server_outline;

pub use server_outline::{host_gradient, host_green};
