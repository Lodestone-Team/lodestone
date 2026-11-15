//! Pages of a long list, as the Modrinth search shows its results: [`PAGE_SIZE`] to a
//! page, and under them a row with Previous, "Page 2 of 8" and Next.
//!
//! The Modrinth search (`create_instance::modrinth_search`, also in the Mods tab's
//! Browse) and the Mods tab's installed list use it. It lives in the Host rather than
//! `lodestone_ui` for now; the launcher's Content tab has a pager of its own.

use std::{ops::Range, rc::Rc};

use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use lodestone_ui::{
    components::{Icon, IconButton},
    theme::{ink, px, text},
};

use crate::i18n::{InstancesMsg as M, t, tr};

/// Items to a page.
pub const PAGE_SIZE: usize = 20;

/// How many pages `len` items make: one at least, for an empty list.
pub fn page_count(len: usize) -> usize {
    len.div_ceil(PAGE_SIZE).max(1)
}

/// `page` (from 0), moved back to the last page when the list is shorter than it was.
pub fn clamp(page: usize, len: usize) -> usize {
    page.min(page_count(len) - 1)
}

/// Which of `len` items are on `page` (clamped).
pub fn range(page: usize, len: usize) -> Range<usize> {
    let start = clamp(page, len) * PAGE_SIZE;
    start..(start + PAGE_SIZE).min(len)
}

/// Whether `len` items need the pager: a list that fits on one page has none.
pub fn paged(len: usize) -> bool {
    len > PAGE_SIZE
}

/// The row under a page of `pages`: Previous, "Page 2 of 8" and Next, which call
/// `on_page` with the page (from 0) to show.
pub fn pager<V: 'static>(
    page: usize,
    pages: usize,
    on_page: impl Fn(&mut V, usize, &mut Context<V>) + 'static,
    cx: &mut Context<V>,
) -> AnyElement {
    let on_page = Rc::new(on_page);
    let previous = on_page.clone();
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(12.))
        .pt(px(4.))
        .text_size(text::MEDIUM)
        .text_color(ink::muted())
        .child(
            IconButton::new("previous-page", Icon::ChevronLeft)
                .label(t(M::PreviousPage))
                .disabled(page == 0)
                .on_click(
                    cx.listener(move |this, _, _, cx| previous(this, page.saturating_sub(1), cx)),
                ),
        )
        .child(tr!(M::SearchPage, page = page + 1, total = pages))
        .child(
            IconButton::new("next-page", Icon::ChevronRight)
                .label(t(M::NextPage))
                .disabled(page + 1 >= pages)
                .on_click(cx.listener(move |this, _, _, cx| on_page(this, page + 1, cx))),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_holds_twenty_and_the_last_the_rest() {
        assert_eq!(page_count(0), 1);
        assert_eq!(page_count(20), 1);
        assert_eq!(page_count(21), 2);
        assert_eq!(page_count(158), 8);
        assert_eq!(range(0, 158), 0..20);
        assert_eq!(range(1, 158), 20..40);
        assert_eq!(range(7, 158), 140..158);
        assert_eq!(range(0, 5), 0..5);
        assert_eq!(range(0, 0), 0..0);
    }

    #[test]
    fn no_pager_for_twenty_or_fewer() {
        assert!(!paged(0));
        assert!(!paged(1));
        assert!(!paged(20));
        assert!(paged(21));
    }

    #[test]
    fn a_page_past_the_end_moves_back_to_the_last() {
        // on the last page of 41, and its one item is removed
        assert_eq!(clamp(2, 41), 2);
        assert_eq!(clamp(2, 40), 1);
        assert_eq!(range(2, 40), 20..40);
        // down to one page, or none
        assert_eq!(clamp(5, 12), 0);
        assert_eq!(range(5, 12), 0..12);
        assert_eq!(clamp(3, 0), 0);
        assert_eq!(range(3, 0), 0..0);
        // pages before the end stay
        assert_eq!(clamp(1, 39), 1);
    }
}
