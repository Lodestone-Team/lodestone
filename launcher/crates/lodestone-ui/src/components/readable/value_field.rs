//! A long or actionable value on its own line, out of the sentence that explains it:
//! a folder, an address, a code, a key.

use std::{rc::Rc, time::Duration};

use gpui::{
    App, ClipboardItem, ElementId, IntoElement, RenderOnce, SharedString, Window, div, prelude::*,
};

use super::{PathText, key_text, key_value};
use crate::{
    components::{
        Icon, IconButton,
        a11y::{Accessible, Role},
    },
    i18n::{CommonMsg, ValueMsg, t},
    theme::{self, colors, fonts, ink, outline, px, radius, text},
    values::truncate::{MAX_SHOWN, sanitize},
};

/// What a [`ValueField`] holds, which picks its actions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ValueKind {
    /// A folder or a file: Copy, and Show in Finder/Explorer when the app can.
    #[default]
    Path,
    /// A web address: Copy and Open.
    Url,
    /// An exit code, an id, a safety code: Copy.
    Code,
    /// A key or a token: hidden until Reveal; Copy copies it all the same.
    Secret,
}

/// How long the Copy button says "Copied".
const COPIED_FOR: Duration = Duration::from_secs(2);

type RevealHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A value on a 28px gray-900 line in JetBrains Mono, shortened in the middle (a
/// path keeps its last two parts and shows home as `~`), with small Copy and Reveal
/// or Open buttons at its end and an optional key above it ("Folder", "Kept aside
/// in"). A secret is masked until Reveal; while it is, screen readers hear "Hidden"
/// and neither its tooltip nor the debug traces carry it.
#[derive(IntoElement)]
pub struct ValueField {
    id: ElementId,
    value: SharedString,
    label: Option<SharedString>,
    kind: ValueKind,
    /// What a masked secret shows before its dots (a key's prefix).
    visible_prefix: Option<SharedString>,
    on_reveal: Option<RevealHandler>,
    home: Option<Option<String>>,
}

impl ValueField {
    pub fn new(id: impl Into<ElementId>, value: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            value: sanitize(&value.into(), MAX_SHOWN).into(),
            label: None,
            kind: ValueKind::Path,
            visible_prefix: None,
            on_reveal: None,
            home: None,
        }
    }

    /// The key above the value, which also names it for screen readers.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn kind(mut self, kind: ValueKind) -> Self {
        self.kind = kind;
        self
    }

    /// For a secret: the part it shows while masked, such as a key's prefix.
    pub fn visible_prefix(mut self, prefix: impl Into<SharedString>) -> Self {
        self.visible_prefix = Some(prefix.into());
        self
    }

    /// For a path: shows it in the file manager. Without it there is no such button.
    pub fn on_reveal(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_reveal = Some(Rc::new(handler));
        self
    }

    /// The home folder a path shows as `~` (the user's by default).
    pub fn home(mut self, home: Option<String>) -> Self {
        self.home = Some(home);
        self
    }
}

impl RenderOnce for ValueField {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let copied =
            window.use_keyed_state(ElementId::from((self.id.clone(), "copied")), cx, |_, _| {
                false
            });
        let shown =
            window.use_keyed_state(ElementId::from((self.id.clone(), "shown")), cx, |_, _| {
                false
            });
        let masked = self.kind == ValueKind::Secret && !*shown.read(cx);
        let spoken: SharedString = if masked {
            t(ValueMsg::Hidden).into()
        } else {
            self.value.clone()
        };
        let label = match &self.label {
            Some(key) => key_value(key, &spoken),
            None => spoken,
        };

        let value: gpui::AnyElement = if masked {
            let prefix = self.visible_prefix.clone().unwrap_or_default();
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(format!("{prefix}••••••••"))
                .into_any_element()
        } else {
            let path = match self.kind {
                ValueKind::Path => PathText::path(self.value.clone()),
                ValueKind::Url | ValueKind::Code | ValueKind::Secret => {
                    PathText::exact(self.value.clone())
                }
            };
            let path = match self.home.clone() {
                Some(home) => path.home(home),
                None => path,
            };
            div()
                .flex()
                .flex_1()
                .min_w_0()
                .child(path.id((self.id.clone(), "value")).label(label.clone()))
                .into_any_element()
        };

        let copy = {
            let value = self.value.clone();
            let id = self.id.clone();
            let copied_state = copied.clone();
            IconButton::new((self.id.clone(), "copy"), Icon::Copy)
                .small()
                .label(t(if *copied.read(cx) {
                    CommonMsg::Copied
                } else {
                    CommonMsg::Copy
                }))
                .on_click(move |_, _, cx| {
                    // the length only: the value may be a secret
                    crate::components::trace_ui(
                        "ui.copy",
                        "value_field",
                        &id,
                        serde_json::json!({ "length": value.chars().count() }),
                    );
                    cx.write_to_clipboard(ClipboardItem::new_string(value.to_string()));
                    copied_state.update(cx, |copied, cx| {
                        *copied = true;
                        cx.notify();
                    });
                    let copied_state = copied_state.clone();
                    cx.spawn(async move |cx| {
                        cx.background_executor().timer(COPIED_FOR).await;
                        copied_state.update(cx, |copied, cx| {
                            *copied = false;
                            cx.notify();
                        });
                    })
                    .detach();
                })
        };
        let other = match self.kind {
            ValueKind::Path => self.on_reveal.clone().map(|reveal| {
                IconButton::new((self.id.clone(), "reveal"), Icon::Folder)
                    .small()
                    .label(t(CommonMsg::ShowInFileManager))
                    .on_click(move |_, window, cx| reveal(window, cx))
            }),
            ValueKind::Url => {
                let url = self.value.clone();
                Some(
                    IconButton::new((self.id.clone(), "open"), Icon::ArrowUpRightFromSquare)
                        .small()
                        .label(t(ValueMsg::OpenLink))
                        .on_click(move |_, _, cx| cx.open_url(&url)),
                )
            }
            ValueKind::Code => None,
            ValueKind::Secret => {
                let shown = shown.clone();
                Some(
                    IconButton::new(
                        (self.id.clone(), "reveal"),
                        if masked { Icon::Eye } else { Icon::EyeSlash },
                    )
                    .small()
                    .label(t(if masked {
                        ValueMsg::Reveal
                    } else {
                        ValueMsg::Hide
                    }))
                    .on_click(move |_, _, cx| {
                        shown.update(cx, |shown, cx| {
                            *shown = !*shown;
                            cx.notify();
                        })
                    }),
                )
            }
        };

        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .w_full()
            .min_w_0()
            .when_some(self.label.clone(), |this, key| this.child(key_text(key)))
            .child(
                div()
                    .id(self.id.clone())
                    .accessible_id(Role::Group, &self.id, label)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.))
                    .h(px(28.))
                    .px(px(8.))
                    .rounded(radius::DEFAULT)
                    .bg(colors::GRAY_900)
                    .border_1()
                    .border_color(outline())
                    .font_family(fonts::mono())
                    .text_size(theme::text_size(text::SMALL))
                    .text_color(ink::text())
                    .child(value)
                    .child(copy)
                    .children(other),
            )
    }
}
