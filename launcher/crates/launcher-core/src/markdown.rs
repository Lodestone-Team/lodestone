//! Modrinth's project descriptions (`project.body`, Markdown with some HTML) as a small
//! block model the app draws itself.
//!
//! [`pulldown_cmark`] parses the text. What comes out is only text, styles, links and
//! images, so nothing in a description can run or load on its own:
//!
//! - HTML is shown as its text: tags are removed (`<script>` and `<style>` with their
//!   contents), a few entities are decoded, `<br>` and block tags break lines, and an
//!   `<img>` becomes an image like Markdown's;
//! - links keep only `http`, `https` and `mailto` targets ([`safe_link`]); other links
//!   are plain text;
//! - images keep a URL only when it is on a host the app loads page images from
//!   ([`image_url`](crate::content::page_images::image_url): Modrinth's and CurseForge's
//!   CDNs, GitHub and imgur), with a size cap; others show their alt text, linked to the
//!   image when the URL is a safe link.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use url::Url;

/// Descriptions are cut at this many bytes.
pub const MAX_BODY_BYTES: usize = 128 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inline {
    Text {
        text: String,
        style: Style,
        /// A safe link target.
        link: Option<String>,
    },
    Image {
        /// A URL the app may load, if the image is on an allowed host.
        url: Option<String>,
        alt: String,
        link: Option<String>,
    },
    Break,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Heading {
        level: u8,
        inlines: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    List {
        /// The first number of an ordered list.
        start: Option<u64>,
        items: Vec<Vec<Block>>,
    },
    Quote(Vec<Block>),
    Code(String),
    Rule,
    /// Rows of cells; the first row is the header.
    Table(Vec<Vec<Vec<Inline>>>),
}

/// `url` if it is an absolute `http`, `https` or `mailto` URL.
pub fn safe_link(url: &str) -> Option<String> {
    let parsed = Url::parse(url.trim()).ok()?;
    let ok = match parsed.scheme() {
        "http" | "https" => parsed.host_str().is_some_and(|h| !h.is_empty()),
        "mailto" => true,
        _ => false,
    };
    ok.then(|| parsed.to_string())
}

/// `url` if it is an image on Modrinth's CDN: HTTPS on `cdn.modrinth.com`, no port or
/// user name. (A project page's images may also come from other hosts:
/// [`image_url`](crate::content::page_images::image_url).)
pub fn loadable_image(url: &str) -> Option<String> {
    let parsed = Url::parse(url.trim()).ok()?;
    let ok = parsed.scheme() == "https"
        && parsed.host_str() == Some(lodestone_minecraft::modrinth::CDN_HOST)
        && parsed.port().is_none()
        && parsed.username().is_empty()
        && parsed.password().is_none();
    ok.then(|| parsed.to_string())
}

/// The text of a list of inlines, without styles: for tests and accessibility.
pub fn plain_text(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        match inline {
            Inline::Text { text, .. } => out.push_str(text),
            Inline::Image { alt, .. } => out.push_str(alt),
            Inline::Break => out.push('\n'),
        }
    }
    out
}

enum Container {
    Root(Vec<Block>),
    Quote(Vec<Block>),
    List {
        start: Option<u64>,
        items: Vec<Vec<Block>>,
    },
    Item(Vec<Block>),
}

enum Target {
    Paragraph,
    Heading(u8),
    Cell,
    /// Text outside a paragraph, such as in a tight list item.
    Implicit,
}

struct Builder {
    stack: Vec<Container>,
    target: Option<(Target, Vec<Inline>)>,
    style: Style,
    bold: usize,
    italic: usize,
    strike: usize,
    links: Vec<Option<String>>,
    image: Option<(Option<String>, String, Option<String>)>,
    code: Option<String>,
    html: Option<String>,
    /// Inside an inline `<script>` or `<style>`.
    skipping: bool,
    table: Option<Vec<Vec<Vec<Inline>>>>,
    row: Vec<Vec<Inline>>,
}

impl Builder {
    fn new() -> Self {
        Self {
            stack: vec![Container::Root(Vec::new())],
            target: None,
            style: Style::default(),
            bold: 0,
            italic: 0,
            strike: 0,
            links: Vec::new(),
            image: None,
            code: None,
            html: None,
            skipping: false,
            table: None,
            row: Vec::new(),
        }
    }

    fn push_block(&mut self, block: Block) {
        match self.stack.last_mut() {
            Some(Container::Root(blocks) | Container::Quote(blocks) | Container::Item(blocks)) => {
                blocks.push(block)
            }
            // Blocks directly in a list (not in an item) start an item.
            Some(Container::List { items, .. }) => items.push(vec![block]),
            None => {}
        }
    }

    /// Closes a paragraph made of loose text, before a block starts or ends.
    fn flush(&mut self) {
        if matches!(self.target, Some((Target::Implicit, _))) {
            self.finish_target();
        }
        self.flush_html();
    }

    fn flush_html(&mut self) {
        if let Some(html) = self.html.take() {
            for block in html_blocks(&html) {
                self.push_block(block);
            }
        }
    }

    fn finish_target(&mut self) {
        let Some((target, inlines)) = self.target.take() else {
            return;
        };
        let empty = inlines.iter().all(|i| match i {
            Inline::Text { text, .. } => text.trim().is_empty(),
            Inline::Break => true,
            Inline::Image { .. } => false,
        });
        match target {
            Target::Cell => self.row.push(trim(inlines)),
            _ if empty => {}
            Target::Heading(level) => self.push_block(Block::Heading {
                level,
                inlines: trim(inlines),
            }),
            Target::Paragraph | Target::Implicit => {
                self.push_block(Block::Paragraph(trim(inlines)))
            }
        }
    }

    fn inline(&mut self, inline: Inline) {
        if self.target.is_none() {
            self.flush_html();
            self.target = Some((Target::Implicit, Vec::new()));
        }
        let Some((_, inlines)) = &mut self.target else {
            return;
        };
        if let (
            Inline::Text { text, style, link },
            Some(Inline::Text {
                text: last,
                style: last_style,
                link: last_link,
            }),
        ) = (&inline, inlines.last_mut())
            && style == last_style
            && link == last_link
        {
            last.push_str(text);
            return;
        }
        inlines.push(inline);
    }

    fn text(&mut self, text: &str) {
        if self.skipping {
            return;
        }
        if let Some(code) = &mut self.code {
            code.push_str(text);
            return;
        }
        if let Some((_, alt, _)) = &mut self.image {
            alt.push_str(text);
            return;
        }
        let link = self.links.iter().rev().find_map(|l| l.clone());
        self.inline(Inline::Text {
            text: text.to_string(),
            style: self.style,
            link,
        });
    }

    fn sync_style(&mut self) {
        self.style.bold = self.bold > 0;
        self.style.italic = self.italic > 0;
        self.style.strike = self.strike > 0;
    }

    fn inline_html(&mut self, html: &str) {
        let lower = html.trim().to_ascii_lowercase();
        if lower.starts_with("<script") || lower.starts_with("<style") {
            self.skipping = !lower.contains("</script") && !lower.contains("</style");
            return;
        }
        if lower.starts_with("</script") || lower.starts_with("</style") {
            self.skipping = false;
            return;
        }
        if self.skipping {
            return;
        }
        for inline in html_inlines(html) {
            self.inline(inline);
        }
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.text(&text),
            Event::Code(text) => {
                let link = self.links.iter().rev().find_map(|l| l.clone());
                let style = Style {
                    code: true,
                    ..self.style
                };
                self.inline(Inline::Text {
                    text: text.to_string(),
                    style,
                    link,
                });
            }
            Event::InlineMath(text) | Event::DisplayMath(text) => self.text(&text),
            Event::Html(html) => {
                if matches!(self.target, Some((Target::Implicit, _))) {
                    self.finish_target();
                }
                self.html.get_or_insert_with(String::new).push_str(&html);
            }
            Event::InlineHtml(html) => self.inline_html(&html),
            Event::FootnoteReference(name) => self.text(&format!("[{name}]")),
            Event::SoftBreak => self.text(" "),
            Event::HardBreak => {
                if self.code.is_none() && self.image.is_none() {
                    self.inline(Inline::Break);
                }
            }
            Event::Rule => {
                self.flush();
                self.push_block(Block::Rule);
            }
            Event::TaskListMarker(done) => self.text(if done { "☑ " } else { "☐ " }),
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {
                self.flush();
                self.target = Some((Target::Paragraph, Vec::new()));
            }
            Tag::Heading { level, .. } => {
                self.flush();
                self.target = Some((Target::Heading(level as u8), Vec::new()));
            }
            Tag::BlockQuote(_) => {
                self.flush();
                self.stack.push(Container::Quote(Vec::new()));
            }
            Tag::CodeBlock(_) => {
                self.flush();
                self.code = Some(String::new());
            }
            Tag::HtmlBlock => {
                self.flush();
                self.html = Some(String::new());
            }
            Tag::List(start) => {
                self.flush();
                self.stack.push(Container::List {
                    start,
                    items: Vec::new(),
                });
            }
            Tag::Item => {
                self.flush();
                self.stack.push(Container::Item(Vec::new()));
            }
            Tag::Table(_) => {
                self.flush();
                self.table = Some(Vec::new());
            }
            Tag::TableHead | Tag::TableRow => self.row = Vec::new(),
            Tag::TableCell => self.target = Some((Target::Cell, Vec::new())),
            Tag::Emphasis => {
                self.italic += 1;
                self.sync_style();
            }
            Tag::Strong => {
                self.bold += 1;
                self.sync_style();
            }
            Tag::Strikethrough => {
                self.strike += 1;
                self.sync_style();
            }
            Tag::Link { dest_url, .. } => self.links.push(safe_link(&dest_url)),
            Tag::Image { dest_url, .. } => {
                let link = self.links.iter().rev().find_map(|l| l.clone());
                let url = crate::content::page_images::image_url(&dest_url);
                // An image that can't be shown links to itself when it can.
                let link = link.or_else(|| url.is_none().then(|| safe_link(&dest_url)).flatten());
                self.image = Some((url, String::new(), link));
            }
            Tag::FootnoteDefinition(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::Superscript
            | Tag::Subscript
            | Tag::MetadataBlock(_) => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) => self.finish_target(),
            TagEnd::BlockQuote(_) => {
                self.flush();
                if let Some(Container::Quote(blocks)) =
                    self.pop_if(|c| matches!(c, Container::Quote(_)))
                {
                    self.push_block(Block::Quote(blocks));
                }
            }
            TagEnd::CodeBlock => {
                if let Some(code) = self.code.take() {
                    self.push_block(Block::Code(code.trim_end_matches('\n').to_string()));
                }
            }
            TagEnd::HtmlBlock => self.flush_html(),
            TagEnd::List(_) => {
                self.flush();
                if let Some(Container::List { start, items }) =
                    self.pop_if(|c| matches!(c, Container::List { .. }))
                {
                    self.push_block(Block::List { start, items });
                }
            }
            TagEnd::Item => {
                self.flush();
                if let Some(Container::Item(blocks)) =
                    self.pop_if(|c| matches!(c, Container::Item(_)))
                    && let Some(Container::List { items, .. }) = self.stack.last_mut()
                {
                    items.push(blocks);
                }
            }
            TagEnd::TableCell => self.finish_target(),
            TagEnd::TableHead | TagEnd::TableRow => {
                let row = std::mem::take(&mut self.row);
                if let Some(table) = &mut self.table {
                    table.push(row);
                }
            }
            TagEnd::Table => {
                if let Some(table) = self.table.take() {
                    self.push_block(Block::Table(table));
                }
            }
            TagEnd::Emphasis => {
                self.italic = self.italic.saturating_sub(1);
                self.sync_style();
            }
            TagEnd::Strong => {
                self.bold = self.bold.saturating_sub(1);
                self.sync_style();
            }
            TagEnd::Strikethrough => {
                self.strike = self.strike.saturating_sub(1);
                self.sync_style();
            }
            TagEnd::Link => {
                self.links.pop();
            }
            TagEnd::Image => {
                if let Some((url, alt, link)) = self.image.take() {
                    let alt = if alt.trim().is_empty() {
                        "image".to_string()
                    } else {
                        alt.trim().to_string()
                    };
                    self.inline(Inline::Image { url, alt, link });
                }
            }
            TagEnd::FootnoteDefinition
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::Superscript
            | TagEnd::Subscript
            | TagEnd::MetadataBlock(_) => {}
        }
    }

    /// Pops the top container if it is the expected kind (the parser's events nest, so
    /// it always is).
    fn pop_if(&mut self, expected: impl Fn(&Container) -> bool) -> Option<Container> {
        if self.stack.len() > 1 && self.stack.last().is_some_and(expected) {
            self.stack.pop()
        } else {
            None
        }
    }

    fn finish(mut self) -> Vec<Block> {
        self.flush();
        self.finish_target();
        // Close anything a cut-off text left open.
        while self.stack.len() > 1 {
            match self.stack.pop() {
                Some(Container::Quote(blocks)) => self.push_block(Block::Quote(blocks)),
                Some(Container::Item(blocks)) => {
                    if let Some(Container::List { items, .. }) = self.stack.last_mut() {
                        items.push(blocks);
                    }
                }
                Some(Container::List { start, items }) => {
                    self.push_block(Block::List { start, items })
                }
                _ => {}
            }
        }
        match self.stack.pop() {
            Some(Container::Root(blocks)) => blocks,
            _ => Vec::new(),
        }
    }
}

/// Trims the whitespace at the start and end of a run of inlines.
fn trim(mut inlines: Vec<Inline>) -> Vec<Inline> {
    while matches!(inlines.last(), Some(Inline::Break)) {
        inlines.pop();
    }
    if let Some(Inline::Text { text, .. }) = inlines.first_mut() {
        *text = text.trim_start().to_string();
    }
    if let Some(Inline::Text { text, .. }) = inlines.last_mut() {
        *text = text.trim_end().to_string();
    }
    inlines.retain(|i| !matches!(i, Inline::Text { text, .. } if text.is_empty()));
    inlines
}

/// Parses a description.
pub fn parse(body: &str) -> Vec<Block> {
    let mut end = body.len().min(MAX_BODY_BYTES);
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let mut builder = Builder::new();
    for event in Parser::new_ext(&body[..end], options) {
        builder.event(event);
    }
    builder.finish()
}

/// Decodes the HTML entities descriptions use.
fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest[..rest.len().min(12)].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..end];
        let decoded = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => name
                .strip_prefix("#x")
                .or_else(|| name.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| name.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32)
                .filter(|c| !c.is_control() || *c == '\n'),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// An attribute's value in a tag such as `<img src="x" alt='y'>`.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut search = 0;
    while let Some(found) = lower[search..].find(name) {
        let at = search + found;
        search = at + name.len();
        let before = lower[..at].chars().last();
        if !before.is_some_and(char::is_whitespace) {
            continue;
        }
        let rest = tag[at + name.len()..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start();
        let value = match rest.chars().next() {
            Some(quote @ ('"' | '\'')) => rest[1..].split(quote).next().unwrap_or_default(),
            _ => rest
                .split(|c: char| c.is_whitespace() || c == '>')
                .next()
                .unwrap_or_default(),
        };
        return Some(decode_entities(value));
    }
    None
}

/// What one HTML tag adds to the text: a line break, an image, or nothing.
fn tag_inline(tag: &str) -> Option<Inline> {
    let lower = tag.to_ascii_lowercase();
    let name: String = lower
        .trim_start_matches('<')
        .trim_start_matches('/')
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    match name.as_str() {
        "br" => Some(Inline::Break),
        "img" => {
            let src = attribute(tag, "src").unwrap_or_default();
            let alt = attribute(tag, "alt")
                .filter(|a| !a.trim().is_empty())
                .unwrap_or_else(|| "image".into());
            let url = crate::content::page_images::image_url(&src);
            let link = url.is_none().then(|| safe_link(&src)).flatten();
            Some(Inline::Image { url, alt, link })
        }
        _ => None,
    }
}

/// The inlines of a piece of inline HTML (usually one tag).
fn html_inlines(html: &str) -> Vec<Inline> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        let text = decode_entities(&rest[..start]);
        if !text.is_empty() {
            out.push(Inline::Text {
                text,
                style: Style::default(),
                link: None,
            });
        }
        let Some(end) = rest[start..].find('>') else {
            rest = "";
            break;
        };
        out.extend(tag_inline(&rest[start..start + end + 1]));
        rest = &rest[start + end + 1..];
    }
    let text = decode_entities(rest);
    if !text.is_empty() {
        out.push(Inline::Text {
            text,
            style: Style::default(),
            link: None,
        });
    }
    out
}

/// A block of HTML as paragraphs and headings of its text (and its images), keeping
/// bold, italic and safe links. CurseForge's descriptions are all HTML.
fn html_blocks(html: &str) -> Vec<Block> {
    const BLOCK_TAGS: &[&str] = &[
        "p",
        "div",
        "li",
        "ul",
        "ol",
        "tr",
        "table",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "center",
        "blockquote",
        "details",
        "summary",
        "hr",
        "section",
    ];
    let mut blocks = Vec::new();
    let mut current: Vec<Inline> = Vec::new();
    // The heading being read, if any: what `close` makes of the text.
    let mut heading: Option<u8> = None;
    let mut bold = 0usize;
    let mut italic = 0usize;
    let mut link: Option<String> = None;
    let close = |current: &mut Vec<Inline>, blocks: &mut Vec<Block>, heading: Option<u8>| {
        let inlines = trim(collapse(std::mem::take(current)));
        if !inlines.is_empty() {
            blocks.push(match heading {
                Some(level) => Block::Heading { level, inlines },
                None => Block::Paragraph(inlines),
            });
        }
    };
    let mut rest = html;
    loop {
        let style = Style {
            bold: bold > 0 || heading.is_some(),
            italic: italic > 0,
            ..Style::default()
        };
        let Some(start) = rest.find('<') else {
            push_text(&mut current, &decode_entities(rest), style, link.clone());
            break;
        };
        push_text(
            &mut current,
            &decode_entities(&rest[..start]),
            style,
            link.clone(),
        );
        rest = &rest[start..];
        if let Some(comment) = rest.strip_prefix("<!--") {
            rest = comment.find("-->").map_or("", |end| &comment[end + 3..]);
            continue;
        }
        let Some(end) = rest.find('>') else {
            break;
        };
        let tag = &rest[..=end];
        rest = &rest[end + 1..];
        let lower = tag.to_ascii_lowercase();
        let closing = lower.starts_with("</");
        let name: String = lower
            .trim_start_matches('<')
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        if (name == "script" || name == "style") && !closing {
            let closing = format!("</{name}");
            let lower_rest = rest.to_ascii_lowercase();
            rest = match lower_rest.find(&closing) {
                Some(at) => rest[at..].find('>').map_or("", |e| &rest[at + e + 1..]),
                None => "",
            };
            continue;
        }
        if BLOCK_TAGS.contains(&name.as_str()) {
            close(&mut current, &mut blocks, heading);
            heading = match (name.as_bytes(), closing) {
                ([b'h', level @ b'1'..=b'6'], false) => Some(level - b'0'),
                _ => None,
            };
            if name == "li" && !closing {
                push_text(&mut current, "• ", Style::default(), None);
            }
            continue;
        }
        match (name.as_str(), closing) {
            ("b" | "strong", false) => bold += 1,
            ("b" | "strong", true) => bold = bold.saturating_sub(1),
            ("i" | "em", false) => italic += 1,
            ("i" | "em", true) => italic = italic.saturating_sub(1),
            ("a", false) => link = attribute(tag, "href").and_then(|h| safe_link(&h)),
            ("a", true) => link = None,
            _ => match tag_inline(tag) {
                // An image inside a link opens the link.
                Some(Inline::Image {
                    url,
                    alt,
                    link: own,
                }) => current.push(Inline::Image {
                    link: if url.is_some() { link.clone() } else { own },
                    url,
                    alt,
                }),
                Some(other) => current.push(other),
                None => {}
            },
        }
    }
    close(&mut current, &mut blocks, heading);
    blocks
}

fn push_text(inlines: &mut Vec<Inline>, text: &str, style: Style, link: Option<String>) {
    if text.is_empty() {
        return;
    }
    match inlines.last_mut() {
        Some(Inline::Text {
            text: last,
            style: last_style,
            link: last_link,
        }) if *last_style == style && *last_link == link => last.push_str(text),
        _ => inlines.push(Inline::Text {
            text: text.to_string(),
            style,
            link,
        }),
    }
}

/// Collapses runs of whitespace in HTML text, as a browser would.
fn collapse(inlines: Vec<Inline>) -> Vec<Inline> {
    inlines
        .into_iter()
        .map(|inline| match inline {
            Inline::Text { text, style, link } => {
                let mut out = String::with_capacity(text.len());
                let mut space = false;
                for c in text.chars() {
                    if c.is_whitespace() {
                        space = true;
                    } else {
                        if space {
                            out.push(' ');
                        }
                        space = false;
                        out.push(c);
                    }
                }
                if space {
                    out.push(' ');
                }
                Inline::Text {
                    text: out,
                    style,
                    link,
                }
            }
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(blocks: &[Block]) -> Vec<String> {
        blocks
            .iter()
            .map(|b| match b {
                Block::Heading { inlines, .. } | Block::Paragraph(inlines) => plain_text(inlines),
                Block::Code(code) => code.clone(),
                other => format!("{other:?}"),
            })
            .collect()
    }

    #[test]
    fn structure() {
        let blocks = parse(
            "# Sodium\n\nA *fast* **renderer** with `code` and ~~old~~.\n\n- one\n- two\n  1. nested\n\n> quoted\n\n```\nlet x = 1;\n```\n\n---\n\n| a | b |\n|---|---|\n| 1 | 2 |\n",
        );
        assert_eq!(
            blocks[0],
            Block::Heading {
                level: 1,
                inlines: vec![Inline::Text {
                    text: "Sodium".into(),
                    style: Style::default(),
                    link: None
                }]
            }
        );
        let Block::Paragraph(inlines) = &blocks[1] else {
            panic!("{:?}", blocks[1]);
        };
        assert_eq!(plain_text(inlines), "A fast renderer with code and old.");
        assert!(inlines.iter().any(
            |i| matches!(i, Inline::Text { text, style, .. } if text == "fast" && style.italic)
        ));
        assert!(inlines.iter().any(
            |i| matches!(i, Inline::Text { text, style, .. } if text == "renderer" && style.bold)
        ));
        assert!(inlines.iter().any(
            |i| matches!(i, Inline::Text { text, style, .. } if text == "code" && style.code)
        ));
        assert!(inlines.iter().any(
            |i| matches!(i, Inline::Text { text, style, .. } if text == "old" && style.strike)
        ));
        let Block::List { start: None, items } = &blocks[2] else {
            panic!("{:?}", blocks[2]);
        };
        assert_eq!(items.len(), 2);
        assert_eq!(texts(&items[0]), ["one"]);
        assert_eq!(texts(&items[1][..1]), ["two"]);
        assert!(
            matches!(&items[1][1], Block::List { start: Some(1), items } if texts(&items[0]) == ["nested"])
        );
        assert!(matches!(&blocks[3], Block::Quote(inner) if texts(inner) == ["quoted"]));
        assert_eq!(blocks[4], Block::Code("let x = 1;".into()));
        assert_eq!(blocks[5], Block::Rule);
        let Block::Table(rows) = &blocks[6] else {
            panic!("{:?}", blocks[6]);
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(plain_text(&rows[1][1]), "2");
    }

    #[test]
    fn html_is_text() {
        let blocks = parse(
            "<div align=\"center\">\n<h1>Big &amp; <b>bold</b></h1>\n<script>alert('x')</script>\n<style>p{}</style><p>a&lt;b&gt; &#x41;&#66;</p>\n<!-- hidden -->\n</div>\n\nInline <span onclick=\"x()\">span</span> and <script>evil()</script> done.<br>next",
        );
        let all = texts(&blocks).join("|");
        assert_eq!(all, "Big & bold|a<b> AB|Inline span and  done.\nnext");
        assert!(!all.contains("alert") && !all.contains("evil") && !all.contains("hidden"));
        assert!(!all.contains('<') || all.contains("a<b>"));
        // markup in text is never markup
        let blocks = parse("1 < 2 and <notatag");
        assert_eq!(texts(&blocks), ["1 < 2 and <notatag"]);
    }

    #[test]
    fn html_keeps_headings_styles_and_links() {
        let blocks = parse(
            "<h2>Features</h2>
<ul>
<li><strong>Fast</strong> and <em>small</em></li>
<li>See <a href=\"https://example.com/wiki\">the wiki</a> or <a href=\"javascript:x\">this</a></li>
</ul>
<p><a href=\"https://example.com/big\"><img src=\"https://media.forgecdn.net/attachments/1/2/a.png\" alt=\"shot\"></a></p>",
        );
        assert!(
            matches!(&blocks[0], Block::Heading { level: 2, inlines } if plain_text(inlines) == "Features")
        );
        let Block::Paragraph(first) = &blocks[1] else {
            panic!("{:?}", blocks[1])
        };
        assert_eq!(plain_text(first), "• Fast and small");
        assert!(first.iter().any(
            |i| matches!(i, Inline::Text { text, style, .. } if text == "Fast" && style.bold)
        ));
        assert!(first.iter().any(
            |i| matches!(i, Inline::Text { text, style, .. } if text == "small" && style.italic)
        ));
        let Block::Paragraph(second) = &blocks[2] else {
            panic!("{:?}", blocks[2])
        };
        assert!(second.iter().any(|i| matches!(i, Inline::Text { text, link: Some(l), .. } if text == "the wiki" && l == "https://example.com/wiki")));
        assert!(
            second.iter().any(
                |i| matches!(i, Inline::Text { text, link: None, .. } if text.ends_with("this"))
            )
        );
        let Block::Paragraph(image) = &blocks[3] else {
            panic!("{:?}", blocks[3])
        };
        assert!(
            matches!(&image[0], Inline::Image { url: Some(u), link: Some(l), .. }
            if u == "https://media.forgecdn.net/attachments/1/2/a.png" && l == "https://example.com/big")
        );
    }

    #[test]
    fn links_and_images() {
        let blocks = parse(
            "[ok](https://modrinth.com/mod/sodium) [mail](mailto:a@b.c) [js](javascript:alert(1)) [rel](/mod/x) [file](file:///etc/passwd) [data](data:text/html,x)\n\n![shot](https://cdn.modrinth.com/data/AANobbMI/images/a.png) ![shot2](https://i.imgur.com/x.png) ![other](https://example.com/x.png) ![bad](javascript:x) [![badge](https://img.shields.io/x.svg)](https://github.com/x)\n\n<img src=\"https://cdn.modrinth.com/data/x/b.webp\" alt=\"html image\"> <img src=\"http://evil.example/track.gif\">",
        );
        let Block::Paragraph(links) = &blocks[0] else {
            panic!()
        };
        let link_of = |name: &str| {
            links.iter().find_map(|i| match i {
                Inline::Text { text, link, .. } if text == name => Some(link.clone()),
                _ => None,
            })
        };
        assert_eq!(
            link_of("ok"),
            Some(Some("https://modrinth.com/mod/sodium".into()))
        );
        assert_eq!(link_of("mail"), Some(Some("mailto:a@b.c".into())));
        for unsafe_link in ["js", "rel", "file", "data"] {
            // shown as text without a link (merged into the text around it)
            assert!(
                links.iter().all(|i| !matches!(i, Inline::Text { text, link: Some(_), .. } if text == unsafe_link)),
                "{unsafe_link}"
            );
        }
        let Block::Paragraph(images) = &blocks[1] else {
            panic!()
        };
        let images: Vec<_> = images
            .iter()
            .filter_map(|i| match i {
                Inline::Image { url, alt, link } => Some((url.clone(), alt.clone(), link.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(
            images,
            [
                (
                    Some("https://cdn.modrinth.com/data/AANobbMI/images/a.png".into()),
                    "shot".into(),
                    None
                ),
                (
                    Some("https://i.imgur.com/x.png".into()),
                    "shot2".into(),
                    None
                ),
                (
                    None,
                    "other".into(),
                    Some("https://example.com/x.png".into())
                ),
                (None, "bad".into(), None),
                (None, "badge".into(), Some("https://github.com/x".into())),
            ]
        );
        let Block::Paragraph(html) = &blocks[2] else {
            panic!("{:?}", blocks[2])
        };
        assert!(
            matches!(&html[0], Inline::Image { url: Some(u), alt, .. } if u == "https://cdn.modrinth.com/data/x/b.webp" && alt == "html image")
        );
        assert!(html.iter().any(|i| matches!(i, Inline::Image { url: None, link: Some(l), .. } if l == "http://evil.example/track.gif")));
    }

    #[test]
    fn safe_urls() {
        assert_eq!(
            safe_link(" https://example.com/a?b#c ").as_deref(),
            Some("https://example.com/a?b#c")
        );
        for bad in [
            "javascript:alert(1)",
            "JAVASCRIPT:alert(1)",
            "vbscript:x",
            "file:///x",
            "/relative",
            "#anchor",
            "data:text/html,<script>",
            "http://",
            "",
        ] {
            assert_eq!(safe_link(bad), None, "{bad}");
        }
        assert!(loadable_image("https://cdn.modrinth.com/x.png").is_some());
        for bad in [
            "http://cdn.modrinth.com/x.png",
            "https://cdn.modrinth.com:8443/x.png",
            "https://user@cdn.modrinth.com/x.png",
            "https://cdn.modrinth.com.evil.com/x.png",
            "https://evil.com/cdn.modrinth.com/x.png",
        ] {
            assert_eq!(loadable_image(bad), None, "{bad}");
        }
    }

    #[test]
    fn long_and_odd_text() {
        let long = "word ".repeat(100_000);
        let blocks = parse(&long);
        let text = texts(&blocks).join("");
        assert!(text.len() <= MAX_BODY_BYTES);
        // a cut in the middle of a character
        let mut multibyte = "a".repeat(MAX_BODY_BYTES - 1);
        multibyte.push('é');
        assert_eq!(parse(&multibyte).len(), 1);
        assert!(parse("").is_empty());
        assert!(parse("<div>\n\n</div>").is_empty());
        assert_eq!(
            decode_entities("&bogus; &#0; &#x110000; &amp"),
            "&bogus; &#0; &#x110000; &amp"
        );
    }
}
