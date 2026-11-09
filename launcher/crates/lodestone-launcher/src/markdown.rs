//! Draws a project description: the renderer is `lodestone_ui::markdown`, shared with
//! Lodestone Host; this adds the launcher's note for a description too long to show.

use std::{collections::HashMap, sync::Arc};

use gpui::{AnyElement, RenderImage, SharedString};
use launcher_core::markdown::Block;

pub use lodestone_ui::markdown::{MAX_BLOCKS, image_urls};

/// The description as an element. `images` are the loaded images by URL.
pub fn render(
    id: impl Into<SharedString>,
    blocks: &[Block],
    images: &HashMap<String, Arc<RenderImage>>,
) -> AnyElement {
    lodestone_ui::markdown::render(
        id,
        blocks,
        images,
        crate::i18n::t(crate::i18n::ShellMsg::RestOnModrinth),
    )
}
