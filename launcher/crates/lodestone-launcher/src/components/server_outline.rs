//! The outline of an instance linked to a server: a thin line in Lodestone Host's
//! green, the gradient of its logo (`assets/images/lodestone_mark.svg`).

use gpui::{
    Background, Bounds, IntoElement, PathBuilder, Pixels, Rems, Styled, canvas, linear_color_stop,
    linear_gradient, point, px,
};

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

/// A `width` outline in the logo's gradient around the element it is put in, at
/// `opacity` percent: over its edge, where a 1px border is, with the element's
/// corner `radius`. Put it last in a `.relative()` element, over the rest.
pub fn server_outline(radius: Rems, width: f32, opacity: u32, on_light: bool) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let radius = radius.to_pixels(window.rem_size());
            if let Some(path) = rounded_stroke(bounds, radius, px(width)) {
                window.paint_path(path, host_gradient(opacity, on_light));
            }
        },
    )
    .absolute()
    // over the element's 1px border, not inside it
    .top(px(-1.))
    .left(px(-1.))
    .right(px(-1.))
    .bottom(px(-1.))
}

/// A rounded rectangle's outline, `width` wide, inside `bounds`.
fn rounded_stroke(
    bounds: Bounds<Pixels>,
    radius: Pixels,
    width: Pixels,
) -> Option<gpui::Path<Pixels>> {
    let half = width / 2.;
    let (left, top) = (bounds.left() + half, bounds.top() + half);
    let (right, bottom) = (bounds.right() - half, bounds.bottom() - half);
    if right <= left || bottom <= top {
        return None;
    }
    let r = (radius - half)
        .max(px(0.))
        .min((right - left) / 2.)
        .min((bottom - top) / 2.);
    let corner = point(r, r);
    let mut path = PathBuilder::stroke(width);
    path.move_to(point(left + r, top));
    path.line_to(point(right - r, top));
    path.arc_to(corner, px(0.), false, true, point(right, top + r));
    path.line_to(point(right, bottom - r));
    path.arc_to(corner, px(0.), false, true, point(right - r, bottom));
    path.line_to(point(left + r, bottom));
    path.arc_to(corner, px(0.), false, true, point(left, bottom - r));
    path.line_to(point(left, top + r));
    path.arc_to(corner, px(0.), false, true, point(left + r, top));
    path.close();
    path.build().ok()
}
