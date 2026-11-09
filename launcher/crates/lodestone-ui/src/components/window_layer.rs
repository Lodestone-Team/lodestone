//! A layer over the whole window, for a [`Dialog`](super::Dialog) opened by a view that
//! fills only part of it (a page beside a sidebar).

use gpui::{AnyElement, IntoElement, ParentElement, Styled, Window, anchored, deferred, div, point};

use crate::theme::px;

/// `children` drawn over the whole window, above the page, in a relatively positioned
/// layer the size of the window: a [`Dialog`](super::Dialog) in it covers the window,
/// not only the view that renders it. Nothing is drawn when `children` is empty.
pub fn window_layer(
    window: &Window,
    children: Vec<AnyElement>,
) -> Option<impl IntoElement + use<>> {
    if children.is_empty() {
        return None;
    }
    let size = window.viewport_size();
    Some(
        deferred(
            anchored().position(point(px(0.), px(0.))).child(
                div()
                    .relative()
                    .w(size.width)
                    .h(size.height)
                    .children(children),
            ),
        )
        .with_priority(2),
    )
}
