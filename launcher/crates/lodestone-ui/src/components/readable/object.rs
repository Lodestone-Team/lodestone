//! The object a toast, a notice or a dialog is about, as its own visual unit rather
//! than a name inside a sentence: inline as an [`ObjectChip`], or as an
//! [`ObjectCard`] under a dialog's title.

use std::{panic::Location, rc::Rc};

use gpui::{
    AnyElement, App, ClickEvent, ElementId, IntoElement, Pixels, RenderOnce, SharedString, Window,
    div, prelude::*,
};

use super::{Fact, FactLine, Kind, auto_id, key_value};
use crate::{
    components::{
        Icon,
        a11y::{Accessible, Role},
        focus::KeyboardFocus,
        text_tooltip,
    },
    i18n::format,
    theme::{self, colors, fonts, ink, outline, px, radius, text, weights},
    values::truncate::{MAX_SHOWN, sanitize},
};

type ChipClick = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// A name with its kind's icon, inline: in toasts, notices, headers and lists of
/// names. 14px gray-500 icon, the name at 14px weight 500 in the main ink, at most
/// 220px wide with the whole name in its tooltip. With [`on_click`](Self::on_click)
/// it is a link, focusable from the keyboard.
#[derive(Clone, IntoElement)]
pub struct ObjectChip {
    id: ElementId,
    name: SharedString,
    kind: Option<Kind>,
    icon: Option<Icon>,
    muted: bool,
    mono: bool,
    max_w: Pixels,
    on_click: Option<ChipClick>,
}

impl ObjectChip {
    #[track_caller]
    pub fn new(name: impl Into<SharedString>) -> Self {
        let name: SharedString = sanitize(&name.into(), MAX_SHOWN).into();
        Self {
            id: auto_id(Location::caller(), &name),
            name,
            kind: None,
            icon: None,
            muted: false,
            mono: false,
            max_w: px(220.),
            on_click: None,
        }
    }

    /// The kind's icon, and the kind in its accessible name.
    pub fn kind(mut self, kind: Kind) -> Self {
        self.kind = Some(kind);
        self
    }

    /// An icon of its own, in place of the kind's.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// No icon, as a row's name.
    pub fn no_icon(mut self) -> Self {
        self.icon = None;
        self.kind = None;
        self
    }

    /// In ink-muted: a second, lesser object (the computer a server runs on).
    pub fn muted(mut self) -> Self {
        self.muted = true;
        self
    }

    /// The name in JetBrains Mono, for a file.
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    pub fn max_w(mut self, width: Pixels) -> Self {
        self.max_w = width;
        self
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// Makes it a link to the object.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        let (id, kind) = (self.id.clone(), self.kind.map(|kind| format!("{kind:?}")));
        let handler = Rc::new(handler);
        self.on_click = Some(Rc::new(move |event, window, cx| {
            // the kind, never the name: names are the user's
            crate::components::trace_ui(
                "ui.click",
                "object_chip",
                &id,
                serde_json::json!({ "kind": kind }),
            );
            handler(event, window, cx)
        }));
        self
    }

    pub fn name(&self) -> &SharedString {
        &self.name
    }

    /// "World: Survival", or the name alone.
    fn label(&self) -> SharedString {
        match self.kind {
            Some(kind) => key_value(kind.label(), &self.name),
            None => self.name.clone(),
        }
    }
}

impl RenderOnce for ObjectChip {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let label = self.label();
        let icon = self.icon.or(self.kind.map(Kind::icon));
        let role = if self.on_click.is_some() {
            Role::Link
        } else {
            Role::Label
        };
        div()
            .id(self.id.clone())
            .accessible_id(role, &self.id, label)
            .tooltip(text_tooltip(self.name.clone()))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.))
            .h(px(20.))
            .min_w_0()
            .max_w(self.max_w)
            .text_size(theme::text_size(text::MEDIUM))
            .font_weight(weights::MEDIUMBOLD.into())
            .text_color(if self.muted {
                ink::muted()
            } else {
                ink::text()
            })
            .when(self.mono, |this| {
                this.font_family(fonts::mono())
                    .text_size(theme::text_size(text::SMALL))
                    .font_weight(weights::MEDIUM.into())
            })
            .when_some(icon, |this, icon| {
                this.child(icon.render(px(14.), colors::GRAY_500))
            })
            .child(div().min_w_0().truncate().child(self.name))
            .when_some(self.on_click, |this, on_click| {
                this.keyboard_focus()
                    .rounded(radius::DEFAULT)
                    .cursor_pointer()
                    .hover(|style| style.text_color(ink::strong()))
                    .on_press(move |event, window, cx| on_click(event, window, cx))
            })
    }
}

/// The object a dialog is about, under its title: a gray-800 card with a 32px
/// thumbnail or kind icon, the name, and up to three facts under it. The compact
/// form (a 24px icon, one line) is for confirmations that change nothing for good.
/// The dialog's text then refers to it generically ("Its folder…").
#[derive(IntoElement)]
pub struct ObjectCard {
    id: ElementId,
    name: SharedString,
    kind: Option<Kind>,
    thumbnail: Option<AnyElement>,
    facts: Vec<Fact>,
    compact: bool,
    untitled: bool,
}

impl ObjectCard {
    #[track_caller]
    pub fn new(name: impl Into<SharedString>) -> Self {
        let name: SharedString = sanitize(&name.into(), MAX_SHOWN).into();
        Self {
            id: auto_id(Location::caller(), &name),
            name,
            kind: None,
            thumbnail: None,
            facts: Vec::new(),
            compact: false,
            untitled: false,
        }
    }

    pub fn kind(mut self, kind: Kind) -> Self {
        self.kind = Some(kind);
        self
    }

    /// A picture in place of the kind's icon (an instance's icon), drawn at 32px (or
    /// 24px when compact).
    pub fn thumbnail(mut self, thumbnail: impl IntoElement) -> Self {
        self.thumbnail = Some(thumbnail.into_any_element());
        self
    }

    /// A fact under the name; the first three are shown.
    pub fn fact(mut self, fact: Fact) -> Self {
        self.facts.push(fact);
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    /// The name is a placeholder ("Untitled backup"), drawn as one.
    pub fn untitled(mut self, untitled: bool) -> Self {
        self.untitled = untitled;
        self
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }
}

impl RenderOnce for ObjectCard {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let size = if self.compact { 24. } else { 32. };
        let mut facts = self.facts;
        facts.truncate(3);
        let description: Vec<SharedString> = facts.iter().map(Fact::label).collect();
        let label = match self.kind {
            Some(kind) => key_value(kind.label(), &self.name),
            None => self.name.clone(),
        };
        let picture = match self.thumbnail {
            Some(thumbnail) => div()
                .flex_none()
                .size(px(size))
                .rounded(radius::MD)
                .overflow_hidden()
                .child(thumbnail),
            None => div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(size))
                .rounded(radius::MD)
                .bg(colors::GRAY_750)
                .when_some(self.kind, |this, kind| {
                    this.child(
                        kind.icon()
                            .render(px(if self.compact { 12. } else { 16. }), colors::GRAY_500),
                    )
                }),
        };
        let name = div()
            .min_w_0()
            .truncate()
            .text_size(theme::text_size(text::MEDIUM))
            .when(self.untitled, |this| {
                this.font_weight(weights::MEDIUM.into())
                    .text_color(ink::muted())
            })
            .when(!self.untitled, |this| {
                this.font_weight(weights::MEDIUMBOLD.into())
                    .text_color(ink::text())
            })
            .child(self.name.clone());
        let line = (!facts.is_empty()).then(|| FactLine::row("facts").facts(facts));
        div()
            .id(self.id.clone())
            .accessible_id(Role::Group, &self.id, label)
            .aria_description(format::list(&description))
            .tooltip(text_tooltip(self.name))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(if self.compact { 8. } else { 10. }))
            .w_full()
            .min_w_0()
            .bg(colors::GRAY_800)
            .border_1()
            .border_color(outline())
            .rounded(radius::LG)
            .when(self.compact, |this| this.p(px(8.)))
            .when(!self.compact, |this| this.px(px(12.)).py(px(10.)))
            .child(picture)
            .child(if self.compact {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.))
                    .flex_1()
                    .min_w_0()
                    .child(div().flex_shrink(1.).min_w_0().child(name))
                    .children(line.map(|line| div().flex_1().min_w_0().child(line)))
            } else {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .flex_1()
                    .min_w_0()
                    .child(name)
                    .children(line)
            })
    }
}
