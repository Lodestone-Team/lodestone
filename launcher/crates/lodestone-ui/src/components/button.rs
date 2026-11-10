use crate::theme::px;
use std::time::Duration;

use gpui::{
    App, ClickEvent, ElementId, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    Window, div, prelude::*,
};

use super::{
    ClickHandler, Icon,
    a11y::{Accessible, Role},
    focus::KeyboardFocus,
};
use crate::skin::{Paint, Painted};
use crate::theme::{self, Color, colors, leading, radius, text, weights};
use launcher_core::ui_skin::{FontPart, SurfaceId};

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
    /// The skin's surface for buttons of this kind.
    fn surface(self) -> SurfaceId {
        match self {
            ButtonVariant::Primary => SurfaceId::ButtonPrimary,
            ButtonVariant::Secondary => SurfaceId::ButtonSecondary,
            ButtonVariant::Ghost => SurfaceId::ButtonGhost,
            ButtonVariant::Danger => SurfaceId::ButtonDanger,
        }
    }

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
                bg: Some(crate::theme::accent::base()),
                outline: Some(crate::theme::accent::faded().opacity(50)),
                hover_text: colors::WHITE,
                hover_bg: Some(crate::theme::accent::hover()),
                hover_outline: Some(crate::theme::accent::light().opacity(75)),
                active_bg: Some(crate::theme::accent::pressed()),
                disabled_bg: Some(crate::theme::accent::faded().opacity(50)),
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
    /// Text and hover colours in place of the variant's.
    ink: Option<(Color, Color)>,
    /// Its name for assistive technology, when the label is a symbol such as "+".
    accessible_label: Option<SharedString>,
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
            ink: None,
            accessible_label: None,
            on_click: None,
        }
    }

    /// The button's name for assistive technology and automation, when its label is a
    /// symbol ("+") or nothing: "Zoom In".
    pub fn accessible_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessible_label = Some(label.into());
        self
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

    /// Text in `text` and a hover in `hover_bg`, for a button on a skin's image.
    pub fn ink(mut self, text: Color, hover_bg: Color) -> Self {
        self.ink = Some((text, hover_bg));
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
        let mut palette = self.variant.colors();
        // the skin's paint for this kind of button; a button given its own colours
        // keeps them
        let skin = match self.ink {
            Some(_) => None,
            None => crate::skin::painted(self.variant.surface()),
        };
        let mut layered = None;
        if let Some(skin) = &skin {
            if let Some(text) = skin.text {
                palette.text = text;
                palette.hover_text = text;
            }
            if let Some((border, _)) = skin.border {
                palette.outline = Some(border);
                palette.hover_outline = Some(border);
            }
            match &skin.paint {
                Some(Paint::Solid(color)) => {
                    palette.bg = Some(*color);
                    palette.hover_bg = Some(color.mix(colors::WHITE, 0.12));
                    palette.active_bg = Some(color.mix(Color::rgb(0x000000), 0.15));
                    palette.disabled_bg = Some(color.opacity(50));
                }
                Some(paint) => {
                    palette.bg = None;
                    palette.hover_bg = None;
                    palette.active_bg = None;
                    layered = Some(Painted {
                        paint: Some(paint.clone()),
                        radius: skin.radius,
                        ..Painted::default()
                    });
                }
                None => {}
            }
        }
        if let Some((text, hover_bg)) = self.ink {
            palette.text = text;
            palette.hover_text = text;
            palette.hover_bg = Some(hover_bg);
            palette.active_bg = Some(hover_bg);
        }
        // a gradient or image fill lightens on hover instead of changing colour
        let fade_on_hover = layered.is_some();
        let inactive = self.disabled || self.loading;
        let name = match &self.accessible_label {
            Some(label) => label.clone(),
            None if self.label.is_empty() && self.loading => {
                crate::i18n::t(crate::i18n::CommonMsg::Loading).into()
            }
            None => self.label.clone(),
        };
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
        let (px_x, rounding): (_, gpui::AbsoluteLength) = if self.pill {
            (px_x + px(6.), gpui::rems(999.).into())
        } else {
            (
                px_x,
                skin.as_ref()
                    .and_then(|s| s.radius)
                    .map_or(rounding.into(), |r| px(r).into()),
            )
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
            .id(self.id.clone())
            .accessible_id(Role::Button, &self.id, name)
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
            // a gradient or an image under the label
            .when_some(layered, |this, painted| crate::skin::paint(this, &painted))
            .when_some(crate::theme::fonts::own(FontPart::Button), |this, font| {
                this.font_family(font)
            })
            .when(!inactive, |this| {
                this.cursor_pointer()
                    .hover(move |style| {
                        let style = style.text_color(palette.hover_text);
                        let style = if fade_on_hover {
                            style.opacity(0.9)
                        } else {
                            style
                        };
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
                    .when_some(self.on_click, |this, on_click| this.on_press(on_click))
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
        loading_dots(colors::GRAY_500)
    }
}

/// [`LoadingDots`] in `color`, for over an image or a dark scrim.
pub fn loading_dots(color: Color) -> ColoredLoadingDots {
    ColoredLoadingDots(color)
}

/// [`LoadingDots`] in a colour: see [`loading_dots`].
#[derive(IntoElement)]
pub struct ColoredLoadingDots(Color);

/// One pulse of the dots.
const DOTS_CYCLE: Duration = Duration::from_millis(700);
/// How often the dots move. Each move redraws the whole window, so they step 20 times
/// a second instead of animating at the display's rate: that kept the window drawing
/// 60 to 240 frames a second for as long as anything showed them (a sign-in, a
/// download), for three 5-pixel dots.
const DOTS_STEP: Duration = Duration::from_millis(50);

impl RenderOnce for ColoredLoadingDots {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = self.0;
        // With reduced motion the dots stand still, as GPUI's repeating animations do.
        let at = if cx.reduce_motion() {
            0.0
        } else {
            dots_step(window, cx)
        };
        let delta = gpui::pulsating_between(0.2, 1.0)(at);
        div().flex().gap(px(3.)).children((0..3).map(|index| {
            // Offset each dot's phase so the pulse travels across them.
            let phase = (delta + index as f32 / 3.0) % 1.0;
            div().size(px(5.)).rounded_full().bg(color).opacity(phase)
        }))
    }
}

/// The views showing loading dots, all redrawn together at the next step.
struct DotsTicker {
    epoch: std::time::Instant,
    views: std::collections::HashSet<gpui::EntityId>,
    running: bool,
}

impl gpui::Global for DotsTicker {}

/// Where the dots are in their pulse (0 to 1) at the current step, after asking for the
/// view drawing them to be drawn again at the next one. One timer serves every view,
/// and stops once a step finds none that drew dots since the last.
fn dots_step(window: &Window, cx: &mut App) -> f32 {
    let now = std::time::Instant::now();
    if !cx.has_global::<DotsTicker>() {
        cx.set_global(DotsTicker {
            epoch: now,
            views: Default::default(),
            running: false,
        });
    }
    let ticker = cx.global_mut::<DotsTicker>();
    ticker.views.insert(window.current_view());
    let step = now.saturating_duration_since(ticker.epoch).as_nanos() / DOTS_STEP.as_nanos();
    let at = (step * DOTS_STEP.as_nanos()) % DOTS_CYCLE.as_nanos();
    if !std::mem::replace(&mut ticker.running, true) {
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor().timer(DOTS_STEP).await;
                let more = cx.update(|cx| {
                    let ticker = cx.global_mut::<DotsTicker>();
                    let views = std::mem::take(&mut ticker.views);
                    ticker.running = !views.is_empty();
                    for view in &views {
                        cx.notify(*view);
                    }
                    !views.is_empty()
                });
                if !more {
                    break;
                }
            }
        })
        .detach();
    }
    at as f32 / DOTS_CYCLE.as_nanos() as f32
}

/// A square button with only an icon, after the dashboard's `IconButton` atom.
#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon: Icon,
    disabled: bool,
    /// Its tooltip and accessible name; without one, it is named after its icon.
    label: Option<SharedString>,
    /// The icon's color, when it says something (a server's power icon shows its
    /// state); faded grey otherwise.
    color: Option<Color>,
    on_click: Option<ClickHandler>,
}

impl IconButton {
    pub fn new(id: impl Into<ElementId>, icon: Icon) -> Self {
        Self {
            id: id.into(),
            icon,
            disabled: false,
            label: None,
            color: None,
            on_click: None,
        }
    }

    /// Colors the icon, in place of the faded grey.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Names the button: a tooltip, and its name for screen readers and automation.
    /// Without one, it is named after its icon.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
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
        let tinted = self.color.is_some();
        let color = self.color.unwrap_or(colors::GRAY_FADED.opacity(30));
        let name = self.label.clone().unwrap_or_else(|| self.icon.label());
        div()
            .id(self.id.clone())
            .accessible_id(Role::Button, &self.id, name)
            .when(!self.disabled, |this| this.keyboard_focus_outer())
            .when_some(self.label, |this, label| {
                this.tooltip(super::text_tooltip(label))
            })
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
                    .when_some(self.on_click, |this, on_click| this.on_press(on_click))
            })
            .child(
                self.icon
                    .render(px(14.), color)
                    .when(!self.disabled && !tinted, |icon| {
                        icon.group_hover("icon-button", |style| {
                            style.text_color(colors::WHITE.opacity(50))
                        })
                    }),
            )
    }
}
