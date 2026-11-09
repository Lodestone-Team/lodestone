use crate::theme::px;
use gpui::{
    App, ClickEvent, ElementId, IntoElement, RenderOnce, SharedString, Window, div, prelude::*,
    relative,
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
    },
    /// The game is running, with Stop.
    Playing,
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
}

const WIDTH: f32 = 260.;
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
        }
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
fn shell(id: ElementId, shape: Shape, label: SharedString) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.clone())
        .accessible_id(Role::Button, &id, label)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .w(px(WIDTH))
        .h(px(HEIGHT))
        .map(|this| match shape {
            Shape::Rounded => this.rounded(radius::LG),
            Shape::Pill => this.rounded_full(),
            Shape::Pixel => this.rounded_none(),
        })
        .border_1()
        .overflow_hidden()
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
        match self.state {
            PlayButtonState::Disabled | PlayButtonState::Ready => {
                let disabled = self.state == PlayButtonState::Disabled;
                let name: SharedString = match &self.caption {
                    Some(caption) => format!("{} {caption}", t(ShellMsg::Play)).into(),
                    None => t(ShellMsg::Play).into(),
                };
                let liquid =
                    (crate::liquid::on() && !disabled).then(|| Liquid::new(&self.id, window, cx));
                let glow = liquid.as_ref().and_then(Liquid::glow);
                let button = shell(self.id, shape, name)
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
                    .children(glow)
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
                    });
                match liquid {
                    Some(liquid) => liquid.wrap(button, shape, fill),
                    None => button.into_any_element(),
                }
            }
            PlayButtonState::Preparing {
                label,
                detail,
                fraction,
            } => shell(
                self.id,
                shape,
                match &detail {
                    Some(detail) => format!("{label}, {detail}").into(),
                    None => label.clone(),
                },
            )
            .role(Role::ProgressIndicator)
            .when_some(fraction, |this, fraction| {
                this.aria_numeric_value(f64::from(fraction) * 100.)
                    .aria_min_numeric_value(0.)
                    .aria_max_numeric_value(100.)
            })
            .bg(colors::GRAY_800)
            .border_color(progress.opacity(50))
            .child(
                // The progress fill, behind the text.
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .h_full()
                    .w(relative(fraction.unwrap_or(0.0)))
                    .bg(progress.opacity(35)),
            )
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
                            .text_color(colors::WHITE)
                            .child(label),
                    )
                    .when_some(detail, |this, detail| {
                        this.child(
                            div()
                                .truncate()
                                .text_size(theme::text_size(text::SMALL))
                                .font_weight(weights::MEDIUM.into())
                                .text_color(crate::theme::ink::text())
                                .child(detail),
                        )
                    }),
            )
            .child(div().relative().child(side_button(
                "play-cancel",
                t(CommonMsg::Cancel),
                self.on_stop,
            )))
            .into_any_element(),
            PlayButtonState::Playing => shell(self.id, shape, t(ShellMsg::Playing).into())
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

/// The Play button as Liquid Glass ([`crate::liquid`]): a lit rim, a soft light
/// under the pointer, and on a press the glass flexes outward with a spring and
/// lights up from where it was pressed.
struct Liquid {
    touch: gpui::Entity<Touch>,
    /// How far into a press: springs to 1 while held and back, overshooting a little.
    press: f32,
    /// How much the pointer's light shows: 0 to 1.
    hover: f32,
    at: Option<gpui::Point<gpui::Pixels>>,
}

/// Where the pointer is over the button, and whether it is held down.
#[derive(Clone, Default)]
struct Touch {
    down: bool,
    /// Where it was last pressed, or is, in the window.
    at: Option<gpui::Point<gpui::Pixels>>,
    hovered: bool,
}

/// How far a press flexes the glass outward.
const FLEX: f32 = 5.;

impl Liquid {
    fn new(id: &ElementId, window: &mut Window, cx: &mut App) -> Self {
        let child = |name: &'static str| {
            ElementId::NamedChild(std::sync::Arc::new(id.clone()), name.into())
        };
        let touch = window.use_keyed_state(child("liquid-touch"), cx, |_, _| Touch::default());
        let state = touch.read(cx).clone();
        let press = crate::liquid::spring(
            child("liquid-press"),
            if state.down { 1. } else { 0. },
            window,
            cx,
        );
        let hover = super::motion::switch(child("liquid-hover"), state.hovered, window, cx);
        Self {
            touch,
            press,
            hover,
            at: state.at,
        }
    }

    /// The light: under the press while it lasts, else under the pointer. Laid under
    /// the label.
    fn glow(&self) -> Option<gpui::Div> {
        use gpui::canvas;
        let press = self.press;
        let lit = (0.16 * self.hover + 0.34 * press.clamp(0., 1.2)).min(0.6);
        let at = self.at.filter(|_| lit > 0.005)?;
        let size = HEIGHT * (1.0 + 0.9 * press.max(0.));
        Some(
            div().absolute().inset_0().opacity(lit).child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        let r = gpui::px(size / 2.);
                        let center = gpui::point(
                            at.x.clamp(bounds.left(), bounds.right()),
                            at.y.clamp(bounds.top(), bounds.bottom()),
                        );
                        let spot = gpui::Bounds::new(
                            gpui::point(center.x - r, center.y - r),
                            gpui::size(r * 2., r * 2.),
                        );
                        window
                            .paint_image(
                                spot,
                                spot,
                                gpui::Corners::all(r),
                                crate::liquid::glow_image(),
                                0,
                                false,
                            )
                            .ok();
                    },
                )
                .size_full(),
            ),
        )
    }

    /// `button` with the pointer and press followed, its rim lit, and the glass
    /// swelling past it as it flexes.
    fn wrap(
        self,
        button: gpui::Stateful<gpui::Div>,
        shape: Shape,
        fill: Color,
    ) -> gpui::AnyElement {
        use gpui::{Hsla, MouseButton};
        let Liquid { touch, press, .. } = self;
        let (down, up, up_out, moved, hovered) = (
            touch.clone(),
            touch.clone(),
            touch.clone(),
            touch.clone(),
            touch,
        );
        let mut button = button
            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                down.update(cx, |touch, _| {
                    touch.down = true;
                    touch.at = Some(event.position);
                });
                window.refresh();
            })
            .on_mouse_up(MouseButton::Left, move |_, window, cx| {
                up.update(cx, |touch, _| touch.down = false);
                window.refresh();
            })
            .on_mouse_up_out(MouseButton::Left, move |_, window, cx| {
                up_out.update(cx, |touch, _| touch.down = false);
                window.refresh();
            })
            .on_mouse_move(move |event, window, cx| {
                // the light follows the pointer, a few pixels at a time
                let touch = moved.read(cx);
                let far = touch.at.is_none_or(|at| {
                    let d = at - event.position;
                    f32::from(d.x).abs() + f32::from(d.y).abs() > 3.
                });
                if far && !touch.down {
                    moved.update(cx, |touch, _| touch.at = Some(event.position));
                    window.refresh();
                }
            })
            .on_hover(move |on, window, cx| {
                hovered.update(cx, |touch, _| {
                    touch.hovered = *on;
                    if !on {
                        touch.down = false;
                    }
                });
                window.refresh();
            });
        let rim = crate::liquid::rim_shadows(0.9);
        let style = button.style();
        let mut shadows = style.box_shadow.take().unwrap_or_default();
        shadows.extend(rim.iter().cloned());
        style.box_shadow = Some(shadows);
        let grow = FLEX * press.max(0.);
        let swell = (grow > 0.05).then(|| {
            let swell = div()
                .absolute()
                .top(gpui::px(-grow))
                .bottom(gpui::px(-grow))
                .left(gpui::px(-grow))
                .right(gpui::px(-grow));
            let mut swell = match shape {
                Shape::Pill => swell.rounded_full(),
                Shape::Rounded => swell.rounded(radius::LG),
                Shape::Pixel => swell.rounded_none(),
            }
            .bg(Hsla::from(fill).opacity(0.35 * press.clamp(0., 1.)));
            swell.style().box_shadow = Some(rim);
            swell
        });
        div()
            .relative()
            .flex_none()
            .children(swell)
            .child(button)
            .into_any_element()
    }
}
