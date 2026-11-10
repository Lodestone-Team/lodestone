use crate::theme::px;
use gpui::{
    AnimationExt as _, App, ClickEvent, ElementId, IntoElement, RenderOnce, SharedString, Window,
    div, prelude::*, relative,
};
use launcher_core::ui_skin::SurfaceId;

use super::ClickHandler;
use super::a11y::{Accessible, Role};
use super::focus::KeyboardFocus;
use crate::{
    i18n::{CommonMsg, ShellMsg, t},
    theme::{self, Color, colors, fonts, leading, radius, text, weights},
};
use launcher_core::ui_skin::{PlaySpec, Rgb, Shape};

/// What the Play button shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum PlayButtonState {
    /// Nothing to play.
    #[default]
    Disabled,
    Ready,
    /// Downloading or starting, with Cancel.
    Preparing {
        label: SharedString,
        detail: Option<SharedString>,
        /// 0 to 1; `None` while the size is unknown.
        fraction: Option<f32>,
        step: LaunchStep,
    },
    /// The game is running, with Stop.
    Playing,
}

/// How far a launch has come, for the steps along the bottom of the Play button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LaunchStep {
    /// Signing in and checking the game's files.
    #[default]
    Checking,
    /// Downloading and installing what's missing.
    Downloading,
    /// Unpacking the natives and starting Java.
    Starting,
    /// Java runs; the game's window hasn't opened yet.
    Launching,
}

impl LaunchStep {
    pub const ALL: [LaunchStep; 4] = [
        LaunchStep::Checking,
        LaunchStep::Downloading,
        LaunchStep::Starting,
        LaunchStep::Launching,
    ];

    fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    /// How far the whole launch has come, 0 to 1: the steps done, and `fraction` of
    /// this one (some of it while its size is unknown).
    pub fn overall(self, fraction: Option<f32>) -> f32 {
        let within = fraction.unwrap_or(0.4).clamp(0.0, 1.0);
        (self.index() as f32 + within) / Self::ALL.len() as f32
    }
}

/// The large Play button in the bottom bar, in the dashboard's green.
#[derive(IntoElement)]
pub struct PlayButton {
    id: ElementId,
    state: PlayButtonState,
    on_click: Option<ClickHandler>,
    on_stop: Option<ClickHandler>,
    caption: Option<SharedString>,
    /// The skin's look (with the instance's colour), and its progress colour.
    look: PlaySpec,
    progress: Option<Rgb>,
    /// Whether to show the skin's prepared paint (an image or a rich gradient).
    layer: bool,
    /// Where it writes its place as it's laid out, for the filings it melts into.
    anchor: Option<super::MagnetAnchor>,
}

const WIDTH: f32 = 260.;
/// Wider while preparing, so the step and its progress fit beside Cancel.
const PREPARING_WIDTH: f32 = 380.;
const HEIGHT: f32 = 60.;

impl PlayButton {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            state: PlayButtonState::Ready,
            on_click: None,
            on_stop: None,
            caption: None,
            look: PlaySpec::default(),
            progress: None,
            layer: false,
            anchor: None,
        }
    }

    /// Writes where the button is into `anchor` as it's laid out, so the filings
    /// melt from it alone, without the line under it.
    pub fn anchor(mut self, anchor: super::MagnetAnchor) -> Self {
        self.anchor = Some(anchor);
        self
    }

    /// Draws it as a skin says: colour, gradient, shape, label and glow.
    pub fn look(mut self, look: PlaySpec, progress: Option<Rgb>) -> Self {
        self.look = look;
        self.progress = progress;
        self
    }

    /// Shows the skin's Play button paint (an image or a rich gradient) under the
    /// label.
    pub fn layer(mut self, layer: bool) -> Self {
        self.layer = layer;
        self
    }

    /// A line under "PLAY" (ready or disabled): "and join Survival SMP".
    pub fn caption(mut self, caption: impl Into<SharedString>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        if disabled {
            self.state = PlayButtonState::Disabled;
        }
        self
    }

    pub fn state(mut self, state: PlayButtonState) -> Self {
        self.state = state;
        self
    }

    /// Called when Play is pressed.
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    /// Called by Cancel (while preparing) and Stop (while playing).
    pub fn on_stop(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_stop = Some(Box::new(handler));
        self
    }
}

/// The button's frame, named `label` for assistive technology and automation.
fn shell(
    id: ElementId,
    width: f32,
    shape: Shape,
    label: SharedString,
    anchor: Option<super::MagnetAnchor>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.clone())
        .accessible_id(Role::Button, &id, label)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .w(px(width))
        .h(px(HEIGHT))
        .map(|this| match shape {
            Shape::Rounded => this.rounded(radius::LG),
            Shape::Pill => this.rounded_full(),
            Shape::Pixel => this.rounded_none(),
        })
        .border_1()
        .overflow_hidden()
        .when_some(anchor, |this, anchor| {
            this.child(
                gpui::canvas(
                    move |bounds, _, _| anchor.set(Some(bounds)),
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
        })
}

/// The light top and dark bottom edges of a block-like button.
fn bevel() -> [gpui::Div; 2] {
    [
        div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .h(px(4.))
            .bg(colors::WHITE.opacity(30)),
        div()
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .h(px(5.))
            .bg(colors::GRAY_900.opacity(45)),
    ]
}

fn color(rgb: Rgb) -> Color {
    Color::rgb(rgb.0)
}

/// The small Cancel or Stop button at the right of the Play button.
fn side_button(
    id: &'static str,
    label: impl Into<SharedString>,
    on_click: Option<ClickHandler>,
) -> impl IntoElement {
    let label: SharedString = label.into();
    div()
        .id(id)
        .accessible(Role::Button, id, label.clone())
        .keyboard_focus_outer()
        .flex_none()
        .px_3()
        .py_1p5()
        .mr_3()
        .rounded(radius::MD)
        .border_1()
        .border_color(colors::GRAY_FADED.opacity(30))
        .bg(colors::GRAY_800)
        .text_size(theme::text_size(text::SMALL))
        .font_weight(weights::BOLD.into())
        .text_color(crate::theme::ink::text())
        .cursor_pointer()
        .hover(|style| style.bg(colors::RED_300).text_color(colors::WHITE))
        .when_some(on_click, |this, on_click| this.on_press(on_click))
        .child(label)
}

impl RenderOnce for PlayButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // It widens as a launch starts and narrows again as the game opens.
        let width = super::motion::tween(
            (self.id.clone(), "width"),
            match self.state {
                PlayButtonState::Preparing { .. } => PREPARING_WIDTH,
                _ => WIDTH,
            },
            std::time::Duration::from_millis(280),
            window,
            cx,
        );
        let look = self.look;
        let label = look
            .label
            .clone()
            .filter(|l| !l.trim().is_empty())
            .unwrap_or_else(|| t(ShellMsg::Play).to_uppercase());
        let play = || label.clone();
        let shape = look.shape;
        // the skin's colours, or the dashboard's green
        let fill = look.color.map_or(colors::GREEN_400, color);
        let edge = look
            .color
            .map_or(colors::GREEN_300, |c| color(c.mix(Rgb::WHITE, 0.2)));
        let hover = look
            .color
            .map_or(colors::GREEN_300, |c| color(c.mix(Rgb::WHITE, 0.12)));
        // white or near-black, whichever reads best over the whole fill
        let ink = look.color.map_or(colors::WHITE, |from| {
            let colors = [Some(from), look.gradient].into_iter().flatten();
            color(launcher_core::ui_skin::readable::best_ink(
                launcher_core::ui_skin::readable::Backdrop::measure(colors)
                    .unwrap_or(launcher_core::ui_skin::readable::Backdrop::solid(from)),
            ))
        });
        let gradient = look.gradient.map(color);
        let progress = self.progress.map_or(colors::GREEN_400, color);
        // while preparing the text sits on the Play bar: in the colours the skin
        // gives it there
        let bar = crate::skin::painted(SurfaceId::PlayBar);
        let bar_text = bar.as_ref().and_then(|p| p.text);
        let bar_muted = bar
            .as_ref()
            .and_then(|p| p.muted.or(p.text.map(|t| t.opacity(70))));
        match self.state {
            PlayButtonState::Disabled | PlayButtonState::Ready => {
                let disabled = self.state == PlayButtonState::Disabled;
                let name: SharedString = match &self.caption {
                    Some(caption) => format!("{} {caption}", t(ShellMsg::Play)).into(),
                    None => t(ShellMsg::Play).into(),
                };
                shell(self.id, width, shape, name, self.anchor)
                    .justify_center()
                    .font_family(fonts::title())
                    .text_size(theme::text_size(text::H1))
                    .line_height(theme::line_height(text::H1, leading::TIGHT))
                    .font_weight(weights::EXTRABOLD.into())
                    .map(|this| {
                        if disabled {
                            this.bg(fill.opacity(25))
                                .border_color(fill.opacity(30))
                                .text_color(ink.opacity(40))
                        } else {
                            this.keyboard_focus_outer()
                                .map(|this| match gradient {
                                    Some(to) => this.bg(gpui::linear_gradient(
                                        180.,
                                        gpui::linear_color_stop(fill, 0.),
                                        gpui::linear_color_stop(to, 1.),
                                    )),
                                    None => this.bg(fill),
                                })
                                .border_color(edge)
                                .text_color(ink)
                                .cursor_pointer()
                                .hover(move |style| style.bg(hover))
                                .when(look.glow, |this| {
                                    this.shadow(vec![gpui::BoxShadow {
                                        color: fill.opacity(60).into(),
                                        offset: gpui::point(px(0.), px(0.)),
                                        blur_radius: px(20.),
                                        spread_radius: px(1.),
                                        inset: false,
                                    }])
                                })
                                .when_some(self.on_click, |this, on_click| this.on_press(on_click))
                        }
                    })
                    .when(self.layer && !disabled, |this| {
                        match crate::skin::painted(SurfaceId::PlayButton) {
                            Some(painted) if painted.paint.is_some() => crate::skin::paint(
                                this,
                                &crate::skin::Painted {
                                    paint: painted.paint,
                                    radius: painted.radius,
                                    ..Default::default()
                                },
                            ),
                            _ => this,
                        }
                    })
                    .when(shape == Shape::Pixel && !disabled, |this| {
                        this.children(bevel())
                    })
                    .map(|this| match self.caption {
                        None => this.child(div().max_w_full().px_3().truncate().child(play())),
                        Some(caption) => this.child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .min_w_0()
                                .px_3()
                                .child(div().max_w_full().truncate().child(play()))
                                .child(
                                    div()
                                        .max_w_full()
                                        .truncate()
                                        .font_family(fonts::body())
                                        .text_size(theme::text_size(text::SMALL))
                                        .line_height(theme::line_height(
                                            text::SMALL,
                                            leading::TIGHT,
                                        ))
                                        .font_weight(weights::BOLD.into())
                                        .child(caption),
                                ),
                        ),
                    })
                    .into_any_element()
            }
            PlayButtonState::Preparing {
                label,
                detail,
                fraction,
                step: _,
            } => shell(
                self.id.clone(),
                width,
                shape,
                match &detail {
                    Some(detail) => format!("{label}, {detail}").into(),
                    None => label.clone(),
                },
                self.anchor,
            )
            .role(Role::ProgressIndicator)
            .when_some(fraction, |this, fraction| {
                this.aria_numeric_value(f64::from(fraction) * 100.)
                    .aria_min_numeric_value(0.)
                    .aria_max_numeric_value(100.)
            })
            // The button has dissolved into the filings around it (see
            // `MagnetBar`): what's left is the step, its progress and Cancel.
            .border_color(gpui::transparent_black())
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .pl_4()
                    .child(
                        div()
                            .truncate()
                            .font_weight(weights::BOLD.into())
                            .text_color(bar_text.unwrap_or_else(crate::theme::ink::text))
                            .child(label),
                    )
                    .when_some(detail, |this, detail| {
                        this.child(
                            div()
                                .truncate()
                                .text_size(theme::text_size(text::SMALL))
                                .font_weight(weights::MEDIUM.into())
                                .text_color(bar_muted.unwrap_or_else(crate::theme::ink::muted))
                                .child(detail),
                        )
                    })
                    // a thin line under the text while the size is known
                    .when_some(fraction, |this, fraction| {
                        this.child(
                            div()
                                .mt_1()
                                .h(px(2.))
                                .w_full()
                                .rounded_full()
                                .bg(crate::theme::ink::border())
                                .child(
                                    div()
                                        .h_full()
                                        .w(relative(fraction))
                                        .rounded_full()
                                        .bg(progress),
                                ),
                        )
                    })
                    .with_animation(
                        "play-preparing-in",
                        gpui::Animation::new(std::time::Duration::from_millis(450))
                            .with_easing(gpui::ease_out_quint()),
                        |this, t| this.opacity(((t - 0.25) / 0.75).clamp(0., 1.)),
                    ),
            )
            .child(div().relative().child(side_button(
                "play-cancel",
                t(CommonMsg::Cancel),
                self.on_stop,
            )))
            .into_any_element(),
            PlayButtonState::Playing => shell(
                self.id,
                width,
                shape,
                t(ShellMsg::Playing).into(),
                self.anchor,
            )
            .role(Role::Status)
            .bg(colors::GRAY_800)
            .border_color(colors::GREEN_400)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .flex_1()
                    .min_w_0()
                    .pl_5()
                    .child(
                        div()
                            .flex_none()
                            .size(px(10.))
                            .rounded_full()
                            .bg(colors::GREEN_200),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_family(fonts::title())
                            .text_size(theme::text_size(text::H2))
                            .font_weight(weights::EXTRABOLD.into())
                            .text_color(colors::GREEN_300)
                            .child(t(ShellMsg::Playing).to_uppercase()),
                    ),
            )
            .child(side_button("play-stop", t(CommonMsg::Stop), self.on_stop))
            .into_any_element(),
        }
    }
}
