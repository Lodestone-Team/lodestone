use crate::theme::px;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, App, ClickEvent, ElementId, Hsla, IntoElement, ParentElement,
    RenderOnce, SharedString, Styled, Window, div, prelude::*,
};

use super::{ClickHandler, Icon, focus::KeyboardFocus};
use crate::theme::{self, Color, colors, leading, radius, text, weights};

/// The dashboard's button `intention`s, plus its `text` variant as [`ButtonVariant::Ghost`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    /// `intention="primary"`: blue, for the main action.
    Primary,
    /// `intention="info"`, the dashboard's default.
    #[default]
    Secondary,
    /// `intention="danger"`: red text that turns into a red button on hover.
    Danger,
    /// `variant="text"`: no background or outline until hovered.
    Ghost,
}

/// The dashboard's button sizes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonSize {
    Slim,
    #[default]
    Medium,
    Large,
}

/// How a button lays out its icon and label when it is wider than its content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    Start,
    #[default]
    Center,
    /// The label at the start and the right icon at the end.
    Between,
}

struct Colors {
    text: Color,
    weight: theme::FontWeightToken,
    bg: Option<Color>,
    outline: Option<Color>,
    hover_text: Color,
    hover_bg: Option<Color>,
    hover_outline: Option<Color>,
    active_bg: Option<Color>,
    disabled_bg: Option<Color>,
}

impl ButtonVariant {
    fn colors(self) -> Colors {
        match self {
            ButtonVariant::Secondary => Colors {
                text: colors::GRAY_300,
                weight: weights::MEDIUM,
                bg: Some(colors::GRAY_800),
                outline: Some(colors::GRAY_FADED.opacity(30)),
                hover_text: colors::GRAY_300,
                hover_bg: Some(colors::GRAY_700),
                hover_outline: Some(colors::WHITE.opacity(50)),
                active_bg: Some(colors::GRAY_800),
                disabled_bg: Some(colors::GRAY_800),
            },
            ButtonVariant::Primary => Colors {
                text: colors::WHITE,
                weight: weights::MEDIUM,
                bg: Some(colors::BLUE),
                outline: Some(colors::BLUE_FADED.opacity(50)),
                hover_text: colors::WHITE,
                hover_bg: Some(colors::BLUE_400),
                hover_outline: Some(colors::BLUE_200.opacity(75)),
                active_bg: Some(colors::BLUE_500),
                disabled_bg: Some(colors::BLUE_FADED.opacity(50)),
            },
            ButtonVariant::Danger => Colors {
                text: colors::RED_200,
                weight: weights::BOLD,
                bg: Some(colors::GRAY_800),
                outline: Some(colors::GRAY_FADED.opacity(30)),
                hover_text: colors::WHITE,
                hover_bg: Some(colors::RED_300),
                hover_outline: Some(colors::WHITE.opacity(50)),
                active_bg: Some(colors::RED_400),
                disabled_bg: Some(colors::GRAY_800),
            },
            ButtonVariant::Ghost => Colors {
                text: colors::WHITE.opacity(50),
                weight: weights::MEDIUM,
                bg: None,
                outline: None,
                hover_text: colors::WHITE.opacity(50),
                hover_bg: Some(colors::GRAY_800),
                hover_outline: None,
                active_bg: Some(colors::GRAY_850),
                disabled_bg: None,
            },
        }
    }
}

/// A button with an optional icon on each side, after the dashboard's `Button` atom.
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    variant: ButtonVariant,
    size: ButtonSize,
    align: Align,
    icon: Option<Icon>,
    icon_right: Option<Icon>,
    disabled: bool,
    loading: bool,
    full_width: bool,
    pill: bool,
    on_click: Option<ClickHandler>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            variant: ButtonVariant::default(),
            size: ButtonSize::default(),
            align: Align::default(),
            icon: None,
            icon_right: None,
            disabled: false,
            loading: false,
            full_width: false,
            pill: false,
            on_click: None,
        }
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn icon_right(mut self, icon: Icon) -> Self {
        self.icon_right = Some(icon);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Shows a loading indicator in place of the label and ignores clicks.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    /// Fully rounded ends, with room for them, for a button inside a pill.
    pub fn pill(mut self) -> Self {
        self.pill = true;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        let (id, label) = (self.id.clone(), self.label.clone());
        self.on_click = Some(Box::new(move |event, window, cx| {
            super::trace_ui(
                "ui.click",
                "button",
                &id,
                serde_json::json!({ "label": label.as_ref() }),
            );
            handler(event, window, cx)
        }));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let palette = self.variant.colors();
        let inactive = self.disabled || self.loading;
        let text_color = if self.disabled {
            colors::WHITE.opacity(50)
        } else {
            palette.text
        };
        let (font_size, gap, px_x, px_y, rounding) = match self.size {
            ButtonSize::Slim => (text::SMALL, px(4.), px(4.), px(4.), radius::XS),
            ButtonSize::Medium => (text::MEDIUM, px(6.), px(8.), px(4.), radius::DEFAULT),
            ButtonSize::Large => (text::H3, px(6.), px(12.), px(6.), radius::DEFAULT),
        };
        let (px_x, rounding) = if self.pill {
            (px_x + px(6.), gpui::rems(999.))
        } else {
            (px_x, rounding)
        };
        let icon_size = px(font_size.0 * 16.);
        let icon_color: Hsla = text_color.opacity(50).into();

        let label = div()
            .relative()
            .flex()
            .items_center()
            .min_w_0()
            .when(self.align == Align::Between, |this| this.flex_1())
            .child(
                div()
                    .truncate()
                    .when(self.loading, |this| this.invisible())
                    .child(self.label),
            )
            .when(self.loading, |this| {
                this.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(LoadingDots),
                )
            });

        div()
            .id(self.id)
            .when(!self.disabled, |this| match palette.bg {
                Some(_) => this.keyboard_focus_outer(),
                None => this.keyboard_focus(),
            })
            .flex()
            .flex_row()
            .flex_none()
            .items_center()
            .gap(gap)
            .px(px_x)
            .py(px_y)
            .rounded(rounding)
            .text_size(theme::text_size(font_size))
            .line_height(theme::line_height(font_size, leading::NORMAL))
            .font_weight(palette.weight.into())
            .text_color(text_color)
            .whitespace_nowrap()
            .map(|this| match self.align {
                Align::Start => this.justify_start(),
                Align::Center => this.justify_center(),
                Align::Between => this.justify_between(),
            })
            .when(self.full_width, |this| this.w_full())
            .when_some(palette.outline, |this, outline| {
                this.border_1().border_color(outline)
            })
            .map(
                |this| match (self.disabled, palette.bg, palette.disabled_bg) {
                    (true, _, Some(bg)) => this.bg(bg),
                    (false, Some(bg), _) => this.bg(bg),
                    _ => this,
                },
            )
            .when(!inactive, |this| {
                this.cursor_pointer()
                    .hover(|style| {
                        let style = style.text_color(palette.hover_text);
                        let style = match palette.hover_bg {
                            Some(bg) => style.bg(bg),
                            None => style,
                        };
                        match palette.hover_outline {
                            Some(outline) => style.border_color(outline),
                            None => style,
                        }
                    })
                    .active(|style| match palette.active_bg {
                        Some(bg) => style.bg(bg),
                        None => style,
                    })
                    .when_some(self.on_click, |this, on_click| this.on_click(on_click))
            })
            .when_some(self.icon, |this, icon| {
                this.child(icon.render(icon_size, icon_color))
            })
            .child(label)
            .when_some(self.icon_right, |this, icon| {
                this.child(icon.render(icon_size, icon_color))
            })
    }
}

/// Three pulsing dots, standing in for the dashboard's `BeatLoader`.
#[derive(IntoElement)]
pub struct LoadingDots;

impl RenderOnce for LoadingDots {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let dot = |index: usize| {
            div()
                .size(px(5.))
                .rounded_full()
                .bg(colors::GRAY_500)
                .with_animation(
                    ("loading-dot", index),
                    Animation::new(Duration::from_millis(700))
                        .repeat()
                        .with_easing(gpui::pulsating_between(0.2, 1.0)),
                    move |dot, delta| {
                        // Offset each dot's phase so the pulse travels across them.
                        let phase = (delta + index as f32 / 3.0) % 1.0;
                        dot.opacity(phase)
                    },
                )
        };
        div().flex().gap(px(3.)).children((0..3).map(dot))
    }
}

/// A square button with only an icon, after the dashboard's `IconButton` atom.
#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon: Icon,
    disabled: bool,
    on_click: Option<ClickHandler>,
}

impl IconButton {
    pub fn new(id: impl Into<ElementId>, icon: Icon) -> Self {
        Self {
            id: id.into(),
            icon,
            disabled: false,
            on_click: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        let (id, icon) = (self.id.clone(), format!("{:?}", self.icon));
        self.on_click = Some(Box::new(move |event, window, cx| {
            super::trace_ui(
                "ui.click",
                "icon_button",
                &id,
                serde_json::json!({ "icon": icon }),
            );
            handler(event, window, cx)
        }));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let color = colors::GRAY_FADED.opacity(30);
        div()
            .id(self.id.clone())
            .when(!self.disabled, |this| this.keyboard_focus_outer())
            .group("icon-button")
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            // p-1.5 around a 1em-high icon, at text-medium.
            .p(px(6.))
            .rounded(radius::DEFAULT)
            .bg(colors::GRAY_800)
            .border_1()
            .border_color(colors::GRAY_FADED.opacity(30))
            .when(self.disabled, |this| this.opacity(0.4))
            .when(!self.disabled, |this| {
                this.cursor_pointer()
                    .hover(|style| {
                        style
                            .bg(colors::GRAY_700)
                            .border_color(colors::WHITE.opacity(50))
                    })
                    .active(|style| style.bg(colors::GRAY_800))
                    .when_some(self.on_click, |this, on_click| this.on_click(on_click))
            })
            .child(
                self.icon
                    .render(px(14.), color)
                    .when(!self.disabled, |icon| {
                        icon.group_hover("icon-button", |style| {
                            style.text_color(colors::WHITE.opacity(50))
                        })
                    }),
            )
    }
}
