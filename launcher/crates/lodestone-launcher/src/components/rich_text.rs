//! Text the pointer can select and copy, optionally with Minecraft's formatting.
//!
//! [`RichText`] draws like a string child (it takes the surrounding font, size, colour
//! and wrapping) and adds:
//!
//! - Minecraft's `§` codes and JSON text components ([`RichText::minecraft`],
//!   [`RichText::formatted`]), drawn with the game's colours, lifted where they would
//!   be too dark to read on the launcher's panels;
//! - selection with the pointer: press and drag (across wrapped lines), double-click
//!   for a word, triple-click for the line; Shift-click extends;
//! - copying the selection as plain text with ⌘C (Ctrl+C elsewhere) or the
//!   right-click menu.
//!
//! One selection exists at a time, kept here rather than in each element, so text
//! nothing has selected does no more work per frame than placing a hitbox. A click
//! that does not drag still reaches the element underneath, so text inside a
//! clickable row or card keeps the row clickable.
//!
//! Use it for text people may want to copy: names, versions, paths, addresses,
//! messages and logs. Not for the labels of buttons, tabs, menus and toggles.

use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::panic::Location;

use gpui::{
    AnyElement, App, Bounds, ClipboardItem, CursorStyle, DispatchPhase, Element, ElementId,
    FontStyle, GlobalElementId, HighlightStyle, Hitbox, HitboxBehavior, InspectorElementId,
    InteractiveElement, IntoElement, KeyBinding, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, SharedString,
    StrikethroughStyle, Styled, StyledText, UnderlineStyle, Window,
    actions, anchored, div, fill, point,
};
use launcher_core::mc_text::{self, Formatted};
use launcher_core::ui_skin::Rgb;

use crate::i18n::{CommonMsg, t};
use crate::theme::{colors, px, weights};

actions!(rich_text, [CopySelection]);

/// Binds ⌘C / Ctrl+C to copying the selected text. Bound without a context, so a
/// focused text field's own Copy wins inside it.
pub(super) fn init(cx: &mut App) {
    let binding = KeyBinding::load(
        "secondary-c",
        Box::new(CopySelection),
        None,
        false,
        None,
        cx,
    )
    .expect("valid copy shortcut");
    cx.bind_keys([binding]);
    cx.on_action(|_: &CopySelection, cx| {
        copy_selection(cx);
    });
}

/// The one selection, wherever it is.
struct Selection {
    /// The element it is in.
    owner: GlobalElementId,
    /// For the trace: the element's own id.
    id: ElementId,
    /// The element's plain text, as copied.
    text: SharedString,
    anchor: usize,
    head: usize,
    /// The left button is down and moving it moves `head`.
    dragging: bool,
    /// The drag has changed the selection, so the release is not a click.
    moved: bool,
    /// The right-click menu is open, at this window position.
    menu: Option<Point<Pixels>>,
}

impl Selection {
    fn range(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }
}

thread_local! {
    static SELECTION: RefCell<Option<Selection>> = const { RefCell::new(None) };
}

fn with_selection<R>(f: impl FnOnce(&mut Option<Selection>) -> R) -> R {
    SELECTION.with(|selection| f(&mut selection.borrow_mut()))
}

/// Copies the selection, if there is one, and says whether it did.
fn copy_selection(cx: &mut App) -> bool {
    let copied = with_selection(|selection| {
        let selection = selection.as_ref()?;
        let range = selection.range();
        let text = selection.text.get(range)?;
        (!text.is_empty()).then(|| (text.to_string(), selection.id.clone()))
    });
    let Some((text, id)) = copied else {
        return false;
    };
    // The length only: what was copied may be anything on screen.
    super::trace_ui(
        "ui.copy",
        "text",
        &id,
        serde_json::json!({ "length": text.chars().count() }),
    );
    cx.write_to_clipboard(ClipboardItem::new_string(text));
    true
}

/// Clears the selection, as a click elsewhere does.
pub fn clear_selection() {
    with_selection(|selection| *selection = None);
}

/// Text that can be selected and copied; see the module docs.
pub struct RichText {
    id: ElementId,
    text: SharedString,
    selectable: bool,
    styled: StyledText,
}

impl RichText {
    /// Plain text. A `§` in it is shown as it is.
    #[track_caller]
    pub fn new(text: impl Into<SharedString>) -> Self {
        let text = text.into();
        Self::build(text, Vec::new(), Location::caller())
    }

    /// Text with Minecraft's `§` codes, as pack, world and server names have them.
    #[track_caller]
    pub fn minecraft(text: impl Into<SharedString>) -> Self {
        let text = text.into();
        let caller = Location::caller();
        if !text.contains('§') {
            return Self::build(text, Vec::new(), caller);
        }
        let formatted = mc_text::parse(&text);
        Self::build_formatted(formatted, caller)
    }

    /// Text already parsed, from a JSON text component.
    #[track_caller]
    pub fn formatted(formatted: Formatted) -> Self {
        Self::build_formatted(formatted, Location::caller())
    }

    fn build_formatted(formatted: Formatted, caller: &'static Location<'static>) -> Self {
        let spans = formatted
            .spans
            .iter()
            .map(|(range, style)| (range.clone(), highlight(style)))
            .collect();
        Self::build(formatted.text.into(), spans, caller)
    }

    fn build(
        text: SharedString,
        highlights: Vec<(Range<usize>, HighlightStyle)>,
        caller: &'static Location<'static>,
    ) -> Self {
        // An id from where the text is made and what it says, so callers needn't give
        // one: two texts at the same place under the same parent differ in text.
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        caller.hash(&mut hasher);
        text.hash(&mut hasher);
        let styled = if highlights.is_empty() {
            StyledText::new(text.clone())
        } else {
            StyledText::new(text.clone()).with_highlights(highlights)
        };
        Self {
            id: ElementId::Integer(hasher.finish()),
            text,
            selectable: true,
            styled,
        }
    }

    /// Gives the text an id of its own, for text that changes while it is selected,
    /// such as a log.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// Whether the pointer selects it (yes by default).
    pub fn selectable(mut self, selectable: bool) -> Self {
        self.selectable = selectable;
        self
    }
}

/// How a Minecraft style draws in the launcher.
fn highlight(style: &mc_text::Style) -> HighlightStyle {
    HighlightStyle {
        color: style.color.map(|rgb| crate::theme::Color::rgb(readable(rgb)).into()),
        font_weight: style.bold.then(|| weights::BOLD.into()),
        font_style: style.italic.then_some(FontStyle::Italic),
        underline: style.underlined.then(|| UnderlineStyle {
            thickness: px(1.),
            color: None,
            wavy: false,
        }),
        strikethrough: style.strikethrough.then(|| StrikethroughStyle {
            thickness: px(1.),
            color: None,
        }),
        // The game scrambles obfuscated text every frame; dimming it says the same
        // without the flicker.
        fade_out: style.obfuscated.then_some(0.55),
        ..HighlightStyle::default()
    }
}

/// A Minecraft colour, lightened (towards white, so it keeps its hue) until it reads
/// on the launcher's panels: black, dark blue and dark grey are made for the game's
/// lighter backgrounds.
fn readable(rgb: u32) -> u32 {
    Rgb(rgb).readable_against(crate::theme::accent::PANEL, 3.0).0
}

/// The word around `index`: letters, digits and `_`, or a run of spaces, or one
/// other character.
fn word_at(text: &str, index: usize) -> Range<usize> {
    #[derive(PartialEq)]
    enum Class {
        Word,
        Space,
        Other,
    }
    fn class(c: char) -> Class {
        if c.is_alphanumeric() || c == '_' {
            Class::Word
        } else if c.is_whitespace() {
            Class::Space
        } else {
            Class::Other
        }
    }
    let index = floor_char_boundary(text, index);
    let Some(c) = text[index..]
        .chars()
        .next()
        .or_else(|| text[..index].chars().next_back())
    else {
        return index..index;
    };
    let target = class(c);
    if target == Class::Other {
        let start = if index == text.len() {
            index - c.len_utf8()
        } else {
            index
        };
        return start..start + c.len_utf8();
    }
    let start = text[..index]
        .char_indices()
        .rev()
        .take_while(|(_, c)| class(*c) == target)
        .last()
        .map_or(index, |(i, _)| i);
    let end = text[index..]
        .char_indices()
        .find(|(_, c)| class(*c) != target)
        .map_or(text.len(), |(i, _)| index + i);
    start..end
}

/// The line (between newlines) around `index`.
fn line_at(text: &str, index: usize) -> Range<usize> {
    let index = floor_char_boundary(text, index);
    let start = text[..index].rfind('\n').map_or(0, |i| i + 1);
    let end = text[index..].find('\n').map_or(text.len(), |i| index + i);
    start..end
}

fn floor_char_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// The byte index under `position`, or the nearest one.
fn index_at(layout: &gpui::TextLayout, text: &str, position: Point<Pixels>) -> usize {
    let index = layout.index_for_position(position).unwrap_or_else(|i| i);
    floor_char_boundary(text, index)
}

impl IntoElement for RichText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for RichText {
    type RequestLayoutState = ();
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.styled.request_layout(None, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.styled
            .prepaint(None, inspector_id, bounds, state, window, cx);
        if !self.selectable {
            return None;
        }
        let menu_at = global_id.and_then(|global_id| {
            with_selection(|selection| {
                selection
                    .as_ref()
                    .filter(|s| &s.owner == global_id)
                    .and_then(|s| s.menu)
            })
        });
        if let Some(position) = menu_at {
            // Over everything else, as popovers are. Laid out on its own here: the
            // text's layout is a measured leaf with no room for a child.
            let mut menu = copy_menu(position);
            menu.layout_as_root(window.viewport_size().into(), window, cx);
            window.defer_draw(menu, Point::default(), 2, None);
        }
        Some(window.insert_hitbox(bounds, HitboxBehavior::Normal))
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let layout = self.styled.layout().clone();
        let (Some(hitbox), Some(global_id)) = (hitbox.as_ref(), global_id) else {
            self.styled
                .paint(None, inspector_id, bounds, state, &mut (), window, cx);
            return;
        };
        window.set_cursor_style(CursorStyle::IBeam, hitbox);

        // What the selection is doing here, if it is here.
        let mine = with_selection(|selection| {
            selection
                .as_ref()
                .filter(|s| &s.owner == global_id)
                .map(|s| (s.range(), s.dragging, s.menu.is_some()))
        });
        if let Some((range, _, _)) = &mine
            && !range.is_empty()
        {
            paint_selection(&layout, range.clone(), bounds, window);
        }
        self.styled
            .paint(None, inspector_id, bounds, state, &mut (), window, cx);

        // A press here starts a selection.
        {
            let hitbox = hitbox.clone();
            let layout = layout.clone();
            let text = self.text.clone();
            let owner = global_id.clone();
            let id = self.id.clone();
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble || !hitbox.is_hovered(window) {
                    return;
                }
                let index = index_at(&layout, &text, event.position);
                match event.button {
                    MouseButton::Left => {
                        with_selection(|selection| {
                            let extending = event.modifiers.shift
                                && selection.as_ref().is_some_and(|s| s.owner == owner);
                            if let (true, Some(s)) = (extending, selection.as_mut()) {
                                s.head = index;
                                s.dragging = true;
                                s.menu = None;
                                return;
                            }
                            let range = match event.click_count {
                                0 | 1 => index..index,
                                2 => word_at(&text, index),
                                _ => line_at(&text, index),
                            };
                            *selection = Some(Selection {
                                owner: owner.clone(),
                                id: id.clone(),
                                text: text.clone(),
                                anchor: range.start,
                                head: range.end,
                                dragging: true,
                                moved: event.click_count > 1,
                                menu: None,
                            });
                        });
                        window.refresh();
                    }
                    MouseButton::Right => {
                        with_selection(|selection| {
                            let inside = selection.as_ref().is_some_and(|s| {
                                s.owner == owner && {
                                    let range = s.range();
                                    !range.is_empty() && range.start <= index && index <= range.end
                                }
                            });
                            if !inside {
                                let range = word_at(&text, index);
                                *selection = Some(Selection {
                                    owner: owner.clone(),
                                    id: id.clone(),
                                    text: text.clone(),
                                    anchor: range.start,
                                    head: range.end,
                                    dragging: false,
                                    moved: false,
                                    menu: None,
                                });
                            }
                            if let Some(s) = selection.as_mut() {
                                s.menu = Some(event.position);
                            }
                        });
                        cx.stop_propagation();
                        window.refresh();
                    }
                    _ => {}
                }
            });
        }

        let Some((_, dragging, menu_open)) = mine else {
            return;
        };

        // A press anywhere else clears it. With the menu open, the menu's Copy takes
        // the press first (in the bubble phase), so clear after it instead.
        {
            let hitbox = hitbox.clone();
            let owner = global_id.clone();
            window.on_mouse_event(move |_: &MouseDownEvent, phase, window, _| {
                if hitbox.is_hovered(window) {
                    return;
                }
                let now = if menu_open {
                    DispatchPhase::Bubble
                } else {
                    DispatchPhase::Capture
                };
                // Unless another text took the selection first.
                let still_mine = with_selection(|s| s.as_ref().is_some_and(|s| s.owner == owner));
                if phase == now && still_mine {
                    clear_selection();
                    window.refresh();
                }
            });
        }

        if !dragging {
            return;
        }
        {
            let layout = layout.clone();
            let text = self.text.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                match phase {
                    DispatchPhase::Capture => {
                        if event.pressed_button != Some(MouseButton::Left) {
                            // Released outside the window.
                            with_selection(|selection| {
                                if let Some(s) = selection.as_mut() {
                                    s.dragging = false;
                                }
                            });
                            return;
                        }
                        let index = index_at(&layout, &text, event.position);
                        let changed = with_selection(|selection| {
                            let s = selection.as_mut()?;
                            if s.head == index {
                                return Some(false);
                            }
                            s.head = index;
                            s.moved = true;
                            Some(true)
                        });
                        if changed == Some(true) {
                            window.refresh();
                        }
                    }
                    DispatchPhase::Bubble => {
                        // A drag that selects doesn't also drag the row it is in.
                        if with_selection(|s| s.as_ref().is_some_and(|s| s.moved)) {
                            cx.stop_propagation();
                        }
                    }
                }
            });
        }
        window.on_mouse_event(move |_: &MouseUpEvent, phase, window, cx| match phase {
            DispatchPhase::Capture => with_selection(|selection| {
                if let Some(s) = selection.as_mut() {
                    s.dragging = false;
                }
            }),
            DispatchPhase::Bubble => {
                // A release that ends a selecting drag is not a click on what is under
                // the text.
                let moved = with_selection(|selection| {
                    selection
                        .as_mut()
                        .map(|s| std::mem::take(&mut s.moved))
                        .unwrap_or(false)
                });
                if moved {
                    cx.stop_propagation();
                }
                window.refresh();
            }
        });
    }
}

/// Fills the selected part of each line the text takes.
fn paint_selection(
    layout: &gpui::TextLayout,
    range: Range<usize>,
    bounds: Bounds<Pixels>,
    window: &mut Window,
) {
    let (Some(start), Some(end)) = (
        layout.position_for_index(range.start),
        layout.position_for_index(range.end),
    ) else {
        return;
    };
    let line_height = layout.line_height();
    let color = crate::theme::accent::faded().opacity(50);
    let rect = |left: Pixels, top: Pixels, right: Pixels| {
        fill(
            Bounds::from_corners(point(left, top), point(right, top + line_height)),
            color,
        )
    };
    if start.y == end.y {
        window.paint_quad(rect(start.x, start.y, end.x));
        return;
    }
    // The first line from the start, whole lines between, the last up to the end.
    window.paint_quad(rect(start.x, start.y, bounds.right()));
    let mut top = start.y + line_height;
    while top < end.y {
        window.paint_quad(rect(bounds.left(), top, bounds.right()));
        top += line_height;
    }
    window.paint_quad(rect(bounds.left(), end.y, end.x));
}

/// The right-click menu: Copy.
fn copy_menu(position: Point<Pixels>) -> AnyElement {
    anchored()
        .position(position)
        .snap_to_window_with_margin(px(8.))
        .child(
            super::settings::popover_panel("rich-text-menu").child(
                div()
                    .id("rich-text-copy")
                    .px_3()
                    .py_1()
                    .min_w(px(120.))
                    .rounded(crate::theme::radius::DEFAULT)
                    .cursor_pointer()
                    .text_color(colors::GRAY_300)
                    .hover(|style| style.bg(colors::GRAY_700))
                    .child(t(CommonMsg::Copy))
                    .on_mouse_down(MouseButton::Left, |_, window, cx| {
                        copy_selection(cx);
                        with_selection(|selection| {
                            if let Some(s) = selection.as_mut() {
                                s.menu = None;
                            }
                        });
                        cx.stop_propagation();
                        window.refresh();
                    }),
            ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_and_lines() {
        let text = "Fresh Animations v1.9, by FreshLX\nsecond line";
        assert_eq!(&text[word_at(text, 2)], "Fresh");
        assert_eq!(&text[word_at(text, 5)], " ");
        assert_eq!(&text[word_at(text, 21)], ",");
        assert_eq!(&text[word_at(text, text.len())], "line");
        assert_eq!(&text[line_at(text, 3)], "Fresh Animations v1.9, by FreshLX");
        assert_eq!(&text[line_at(text, text.len() - 1)], "second line");
        assert_eq!(word_at("", 0), 0..0);
        // Never inside a character.
        let text = "é_x";
        assert_eq!(&text[word_at(text, 1)], "é_x");
    }

    #[test]
    fn dark_minecraft_colours_are_lifted_keeping_their_hue() {
        // Black, dark blue and dark grey become readable on the panels.
        for dark in [0x000000, 0x0000AA, 0x555555] {
            let lifted = Rgb(readable(dark));
            assert!(lifted.contrast(crate::theme::accent::PANEL) >= 3.0, "{dark:06x}");
        }
        let blue = readable(0x0000AA);
        assert!(blue & 0xFF > (blue >> 16) & 0xFF, "still blue: {blue:06x}");
        // Light colours are as the game draws them.
        assert_eq!(readable(0xFFAA00), 0xFFAA00);
        assert_eq!(readable(0x55FF55), 0x55FF55);
    }
}

#[cfg(all(test, feature = "screenshot"))]
mod ui_tests {
    use std::{cell::Cell, rc::Rc};

    use gpui::{
        Context, InteractiveElement, IntoElement, Modifiers, MouseButton, ParentElement, Render,
        StatefulInteractiveElement, Styled, TestAppContext, Window, div, point, px,
    };

    use super::RichText;

    /// A clickable row with selectable text in it.
    struct Row {
        clicks: Rc<Cell<usize>>,
    }

    impl Render for Row {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let clicks = self.clicks.clone();
            div().size_full().child(
                div()
                    .id("row")
                    .w(px(400.))
                    .on_click(move |_, _, _| clicks.set(clicks.get() + 1))
                    .child(RichText::minecraft("§6Hello §lworld")),
            )
        }
    }

    #[test]
    fn a_drag_selects_and_copies_while_a_click_reaches_the_row() {
        let mut cx = TestAppContext::single();
        cx.update(super::init);
        let clicks = Rc::new(Cell::new(0));
        let (_, cx) = cx.add_window_view({
            let clicks = clicks.clone();
            |_, _| Row { clicks }
        });
        let none = Modifiers::default();
        let start = point(px(1.), px(5.));
        // A plain click is the row's.
        cx.simulate_click(start, none);
        assert_eq!(clicks.get(), 1);
        // Nothing selected: copying copies nothing.
        cx.simulate_keystrokes("secondary-c");
        assert!(cx.read_from_clipboard().is_none());

        // A drag past the end selects the whole text and isn't a click.
        let end = point(px(390.), px(5.));
        cx.simulate_mouse_down(start, MouseButton::Left, none);
        cx.simulate_mouse_move(point(px(100.), px(5.)), MouseButton::Left, none);
        cx.simulate_mouse_move(end, MouseButton::Left, none);
        cx.simulate_mouse_up(end, MouseButton::Left, none);
        assert_eq!(clicks.get(), 1, "a selecting drag doesn't click the row");
        cx.simulate_keystrokes("secondary-c");
        let copied = cx.read_from_clipboard().and_then(|item| item.text());
        assert_eq!(copied.as_deref(), Some("Hello world"), "copied without codes");

        // A click outside clears it.
        cx.simulate_click(point(px(450.), px(200.)), none);
        assert!(super::SELECTION.with(|s| s.borrow().is_none()));
    }
}
