//! Lodestone Host's green, the gradient of its logo
//! (`assets/images/lodestone_mark.svg`), for things that stand for a server.

use gpui::{Background, linear_color_stop, linear_gradient};

use crate::theme::Color;

/// The Host logo's gradient: its light green, then its teal.
const HOST_GREEN: Color = Color::rgb(0x2EF29A);
const HOST_TEAL: Color = Color::rgb(0x1FBFA8);
/// The same greens, darker, for a line on a light background, where the logo's
/// greens would hardly show.
const HOST_GREEN_DEEP: Color = Color::rgb(0x0FAE6A);
const HOST_TEAL_DEEP: Color = Color::rgb(0x0E8C7C);

/// The logo's gradient across a shape, at `opacity` percent. `on_light`: the deeper
/// greens, for a light background.
pub fn host_gradient(opacity: u32, on_light: bool) -> Background {
    let (from, to) = if on_light {
        (HOST_GREEN_DEEP, HOST_TEAL_DEEP)
    } else {
        (HOST_GREEN, HOST_TEAL)
    };
    linear_gradient(
        120.,
        linear_color_stop(from.opacity(opacity), 0.),
        linear_color_stop(to.opacity(opacity), 1.),
    )
}
