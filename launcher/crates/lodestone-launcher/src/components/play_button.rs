use gpui::{
    App, ClickEvent, ElementId, IntoElement, RenderOnce, SharedString, Window, div, prelude::*, px,
    relative,
};

use super::ClickHandler;
use crate::theme::{self, colors, fonts, leading, radius, text, weights};

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
        }
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

fn shell(id: ElementId) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .w(px(WIDTH))
        .h(px(HEIGHT))
        .rounded(radius::LG)
        .border_1()
        .overflow_hidden()
}

/// The small Cancel or Stop button at the right of the Play button.
fn side_button(
    id: &'static str,
    label: &'static str,
    on_click: Option<ClickHandler>,
) -> impl IntoElement {
    div()
        .id(id)
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
        .text_color(colors::GRAY_300)
        .cursor_pointer()
        .hover(|style| style.bg(colors::RED_300).text_color(colors::WHITE))
        .when_some(on_click, |this, on_click| this.on_click(on_click))
        .child(label)
}

impl RenderOnce for PlayButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        match self.state {
            PlayButtonState::Disabled | PlayButtonState::Ready => {
                let disabled = self.state == PlayButtonState::Disabled;
                shell(self.id)
                    .justify_center()
                    .font_family(fonts::TITLE)
                    .text_size(theme::text_size(text::H1))
                    .line_height(theme::line_height(text::H1, leading::TIGHT))
                    .font_weight(weights::EXTRABOLD.into())
                    .map(|this| {
                        if disabled {
                            this.bg(colors::GREEN_400.opacity(25))
                                .border_color(colors::GREEN_400.opacity(30))
                                .text_color(colors::WHITE.opacity(40))
                        } else {
                            this.bg(colors::GREEN_400)
                                .border_color(colors::GREEN_300)
                                .text_color(colors::WHITE)
                                .cursor_pointer()
                                .hover(|style| style.bg(colors::GREEN_300))
                                .when_some(self.on_click, |this, on_click| this.on_click(on_click))
                        }
                    })
                    .map(|this| match self.caption {
                        None => this.child("PLAY"),
                        Some(caption) => this.child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .min_w_0()
                                .px_3()
                                .child("PLAY")
                                .child(
                                    div()
                                        .max_w_full()
                                        .truncate()
                                        .font_family(fonts::SANS)
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
            } => shell(self.id)
                .bg(colors::GRAY_800)
                .border_color(colors::GREEN_400.opacity(50))
                .child(
                    // The progress fill, behind the text.
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .h_full()
                        .w(relative(fraction.unwrap_or(0.0)))
                        .bg(colors::GREEN_400.opacity(35)),
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
                                    .text_color(colors::GRAY_300)
                                    .child(detail),
                            )
                        }),
                )
                .child(
                    div()
                        .relative()
                        .child(side_button("play-cancel", "Cancel", self.on_stop)),
                )
                .into_any_element(),
            PlayButtonState::Playing => shell(self.id)
                .bg(colors::GRAY_800)
                .border_color(colors::GREEN_400)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .flex_1()
                        .pl_5()
                        .child(div().size(px(10.)).rounded_full().bg(colors::GREEN_200))
                        .child(
                            div()
                                .font_family(fonts::TITLE)
                                .text_size(theme::text_size(text::H2))
                                .font_weight(weights::EXTRABOLD.into())
                                .text_color(colors::GREEN_300)
                                .child("PLAYING"),
                        ),
                )
                .child(side_button("play-stop", "Stop", self.on_stop))
                .into_any_element(),
        }
    }
}
