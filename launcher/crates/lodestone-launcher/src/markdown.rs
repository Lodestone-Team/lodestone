//! Draws a project description (`launcher_core::markdown`'s blocks): headings,
//! paragraphs with styles and links, lists, quotes, code, rules, tables and images.
//!
//! Links open in the browser. Images are drawn only when the caller has them (it loads
//! them from Modrinth's CDN, with a size cap); otherwise their alt text is shown.

use crate::theme::px;
use std::{collections::HashMap, ops::Range, sync::Arc};

use crate::components::RichText;
use gpui::{
    AnyElement, FontStyle, HighlightStyle, IntoElement, ObjectFit, RenderImage, SharedString,
    StrikethroughStyle, StyledText, UnderlineStyle, div, img, prelude::*,
};
use launcher_core::markdown::{Block, Inline};

use crate::{
    components::KeyboardFocus,
    theme::{self, colors, fonts, leading, radius, text, weights},
};

/// At most this many blocks are drawn; long descriptions end with a note.
pub const MAX_BLOCKS: usize = 400;
/// Images are drawn at most this wide and high.
const MAX_IMAGE_WIDTH: f32 = 640.;
const MAX_IMAGE_HEIGHT: f32 = 320.;

/// The image URLs a description would draw, in order.
pub fn image_urls(blocks: &[Block]) -> Vec<String> {
    fn inlines(list: &[Inline], out: &mut Vec<String>) {
        for inline in list {
            if let Inline::Image { url: Some(url), .. } = inline {
                out.push(url.clone());
            }
        }
    }
    fn walk(blocks: &[Block], out: &mut Vec<String>) {
        for block in blocks {
            match block {
                Block::Heading { inlines: list, .. } | Block::Paragraph(list) => inlines(list, out),
                Block::List { items, .. } => items.iter().for_each(|item| walk(item, out)),
                Block::Quote(inner) => walk(inner, out),
                Block::Table(rows) => rows.iter().flatten().for_each(|cell| inlines(cell, out)),
                Block::Code(_) | Block::Rule => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(blocks, &mut out);
    out.dedup();
    out
}

struct Renderer<'a> {
    images: &'a HashMap<String, Arc<RenderImage>>,
    next_id: usize,
    prefix: SharedString,
}

impl Renderer<'_> {
    fn id(&mut self) -> (SharedString, usize) {
        self.next_id += 1;
        (self.prefix.clone(), self.next_id)
    }

    /// A run of text inlines as one wrapping text, with clickable links.
    fn text(&mut self, list: &[&Inline]) -> Option<AnyElement> {
        let mut content = String::new();
        let mut highlights: Vec<(Range<usize>, HighlightStyle)> = Vec::new();
        let mut code: Vec<(Range<usize>, SharedString)> = Vec::new();
        let mut links: Vec<(Range<usize>, String)> = Vec::new();
        for inline in list {
            let start = content.len();
            match inline {
                Inline::Text { text, style, link } => {
                    content.push_str(text);
                    let range = start..content.len();
                    if range.is_empty() {
                        continue;
                    }
                    let mut highlight = HighlightStyle::default();
                    if style.bold {
                        highlight.font_weight = Some(weights::EXTRABOLD.into());
                    }
                    if style.italic {
                        highlight.font_style = Some(FontStyle::Italic);
                    }
                    if style.strike {
                        highlight.strikethrough = Some(StrikethroughStyle {
                            thickness: px(1.),
                            color: None,
                        });
                    }
                    if style.code {
                        highlight.background_color = Some(colors::GRAY_700.into());
                        highlight.color = Some(colors::GRAY_300.into());
                        code.push((range.clone(), fonts::mono()));
                    }
                    if let Some(link) = link {
                        highlight.color = Some(colors::BLUE_150.into());
                        highlight.underline = Some(UnderlineStyle {
                            thickness: px(1.),
                            color: Some(colors::BLUE_150.opacity(50).into()),
                            wavy: false,
                        });
                        links.push((range.clone(), link.clone()));
                    }
                    if highlight != HighlightStyle::default() {
                        highlights.push((range, highlight));
                    }
                }
                Inline::Break => content.push('\n'),
                Inline::Image { alt, link, .. } => {
                    // Images that aren't drawn are shown as their alt text.
                    content.push_str(&format!("[{alt}]"));
                    let range = start..content.len();
                    let mut highlight = HighlightStyle {
                        color: Some(colors::GRAY_500.into()),
                        ..HighlightStyle::default()
                    };
                    if let Some(link) = link {
                        highlight.color = Some(colors::BLUE_150.into());
                        links.push((range.clone(), link.clone()));
                    }
                    highlights.push((range, highlight));
                }
            }
        }
        if content.trim().is_empty() {
            return None;
        }
        let content = SharedString::from(content);
        let styled = StyledText::new(content.clone())
            .with_highlights(highlights)
            .with_font_family_overrides(code);
        // Selectable, so a description's text can be copied; links open on a click.
        let (ranges, targets): (Vec<_>, Vec<_>) = links.into_iter().unzip();
        Some(
            RichText::styled(self.id(), content, styled)
                .links(ranges, move |index, _, cx| {
                    if let Some(url) = targets.get(index) {
                        cx.open_url(url);
                    }
                })
                .into_any_element(),
        )
    }

    fn image(&mut self, image: &Arc<RenderImage>, link: Option<&String>) -> AnyElement {
        let size = image.size(0);
        let (w, h) = (size.width.0.max(1) as f32, size.height.0.max(1) as f32);
        let scale = (MAX_IMAGE_WIDTH / w).min(MAX_IMAGE_HEIGHT / h).min(1.0);
        let element = img(image.clone())
            .w(px(w * scale))
            .h(px(h * scale))
            .rounded(radius::MD)
            .object_fit(ObjectFit::Contain);
        match link {
            Some(link) => {
                let link = link.clone();
                div()
                    .id(self.id())
                    .keyboard_focus()
                    .cursor_pointer()
                    .on_click(move |_, _, cx| cx.open_url(&link))
                    .child(element)
                    .into_any_element()
            }
            None => element.into_any_element(),
        }
    }

    /// Inlines: one text when there are no drawn images, else text runs and images in a
    /// wrapping row.
    fn inlines(&mut self, list: &[Inline]) -> AnyElement {
        let drawn = |inline: &Inline| match inline {
            Inline::Image { url: Some(url), .. } => self.images.get(url).cloned(),
            _ => None,
        };
        let has_images = list.iter().any(|i| drawn(i).is_some());
        if !has_images {
            let all: Vec<&Inline> = list.iter().collect();
            return self.text(&all).unwrap_or_else(|| div().into_any_element());
        }
        let mut row = div().flex().flex_row().flex_wrap().items_center().gap_2();
        let mut run: Vec<&Inline> = Vec::new();
        for inline in list {
            match (inline, drawn(inline)) {
                (Inline::Image { link, .. }, Some(image)) => {
                    if let Some(text) = self.text(&std::mem::take(&mut run)) {
                        row = row.child(text);
                    }
                    row = row.child(self.image(&image, link.as_ref()));
                }
                _ => run.push(inline),
            }
        }
        if let Some(text) = self.text(&run) {
            row = row.child(text);
        }
        row.into_any_element()
    }

    fn block(&mut self, block: &Block) -> AnyElement {
        match block {
            Block::Heading { level, inlines } => {
                let size = match level {
                    1 => text::H1,
                    2 => text::H2,
                    3 => text::H3,
                    _ => text::MEDIUM,
                };
                div()
                    .pt_2()
                    .font_family(fonts::heading())
                    .text_size(theme::text_size(size))
                    .line_height(theme::line_height(size, leading::TIGHT))
                    .font_weight(weights::BOLD.into())
                    .text_color(crate::theme::ink::text())
                    .child(self.inlines(inlines))
                    .into_any_element()
            }
            Block::Paragraph(inlines) => div()
                .text_color(crate::theme::ink::secondary())
                .child(self.inlines(inlines))
                .into_any_element(),
            Block::List { start, items } => {
                let mut list = div().flex().flex_col().gap_1();
                for (index, item) in items.iter().enumerate() {
                    let marker = match start {
                        Some(first) => format!("{}.", first + index as u64),
                        None => "•".to_string(),
                    };
                    list = list.child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(
                                div()
                                    .flex_none()
                                    .min_w(px(16.))
                                    .text_color(crate::theme::ink::muted())
                                    .child(marker),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_1()
                                    .children(item.iter().map(|b| self.block(b))),
                            ),
                    );
                }
                list.into_any_element()
            }
            Block::Quote(inner) => div()
                .flex()
                .flex_col()
                .gap_2()
                .pl_3()
                .border_l_2()
                .border_color(colors::GRAY_600)
                .children(inner.iter().map(|b| self.block(b)))
                .into_any_element(),
            Block::Code(code) => div()
                .px_3()
                .py_2()
                .rounded(radius::MD)
                .bg(colors::GRAY_900)
                .border_1()
                .border_color(crate::theme::ink::border())
                .font_family(fonts::mono())
                .text_size(theme::text_size(text::SMALL))
                .text_color(crate::theme::ink::text())
                .child(code.clone())
                .into_any_element(),
            Block::Rule => div()
                .h(px(1.))
                .w_full()
                .bg(colors::FADE_700)
                .into_any_element(),
            Block::Table(rows) => {
                let mut table = div()
                    .flex()
                    .flex_col()
                    .rounded(radius::MD)
                    .border_1()
                    .border_color(crate::theme::ink::border())
                    .overflow_hidden();
                for (index, row) in rows.iter().enumerate() {
                    let mut line = div()
                        .flex()
                        .flex_row()
                        .when(index == 0, |this| {
                            this.bg(colors::GRAY_800).font_weight(weights::BOLD.into())
                        })
                        .when(index > 0, |this| {
                            this.border_t_1().border_color(crate::theme::ink::border())
                        });
                    for cell in row {
                        line = line.child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .px_2()
                                .py_1()
                                .text_size(theme::text_size(text::SMALL))
                                .text_color(crate::theme::ink::secondary())
                                .child(self.inlines(cell)),
                        );
                    }
                    table = table.child(line);
                }
                table.into_any_element()
            }
        }
    }
}

/// The description as an element. `images` are the loaded images by URL.
pub fn render(
    id: impl Into<SharedString>,
    blocks: &[Block],
    images: &HashMap<String, Arc<RenderImage>>,
) -> AnyElement {
    let mut renderer = Renderer {
        images,
        next_id: 0,
        prefix: id.into(),
    };
    let shown = blocks.len().min(MAX_BLOCKS);
    div()
        .flex()
        .flex_col()
        .gap_3()
        .line_height(theme::line_height(text::MEDIUM, leading::NORMAL))
        .whitespace_normal()
        .children(blocks[..shown].iter().map(|b| renderer.block(b)))
        .when(blocks.len() > shown, |this| {
            this.child(
                div()
                    .text_color(crate::theme::ink::muted())
                    .child(crate::i18n::t(crate::i18n::ShellMsg::RestOnModrinth)),
            )
        })
        .into_any_element()
}
