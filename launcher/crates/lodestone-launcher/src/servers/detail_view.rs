//! How a server's page looks: its state and controls, the console with the
//! dashboard's colours, the chat box, the players, and the Minecraft account link.

use crate::theme::px;
use gpui::{
    AnyElement, Context, FontStyle, FontWeight, HighlightStyle, IntoElement, KeyDownEvent, Render,
    ScrollEvent, ScrollStrategy, SharedString, StrikethroughStyle, StyledText, UnderlineStyle,
    Window, div, prelude::*, uniform_list,
};
use launcher_core::cores::{ServerLoader, ServerState};
use lodestone_minecraft::console_format::{
    BADGE_PALETTE, BOLD, ConsoleColor, ConsoleLevel, ConsoleSegment, ConsoleTheme, DIM, INVERSE,
    ITALIC, ParsedConsoleLine, RGB_BASE, STRIKE, UNDERLINE, luminance, readable_color, xterm256,
};

use super::{
    core_error,
    detail::{DetailEvent, LinkState, ServerDetail, StreamStatus},
    loader_label, status_label,
};
use crate::{
    components::{Button, ButtonSize, ButtonVariant, Icon, LoadingDots},
    i18n::{CommonMsg, ServersMsg as M, t, tr},
    theme::{self, Color, colors, fonts, leading, radius, text, weights},
};

/// The dashboard's 16 console colours in its dark theme (`--console-0` … in
/// `globals.css`), which the launcher's console uses.
const CONSOLE_PALETTE: [u32; 16] = [
    0x979CA5, 0xF46F6B, 0x4FC46C, 0xD9B93C, 0x6AA5F2, 0xC985DD, 0x3FC2C9, 0xC9CBD0, 0xA2A7AF,
    0xFF8A85, 0x7AE58F, 0xF5E168, 0x93BEFF, 0xE8A6F6, 0x78E7EC, 0xF5F6F7,
];

const ROW_HEIGHT: f32 = 20.;

fn rgb(value: u32) -> gpui::Hsla {
    Color::rgb(value).into()
}

/// A text colour on the console background, readable in the dark theme.
fn text_color(color: ConsoleColor) -> u32 {
    if color < 16 {
        CONSOLE_PALETTE[color as usize]
    } else {
        let value = if color < RGB_BASE {
            xterm256(color)
        } else {
            color - RGB_BASE
        };
        readable_color(value, ConsoleTheme::Dark)
    }
}

/// The `0xRRGGBB` value of a colour on a badge, the same in both themes.
fn badge_value(color: ConsoleColor) -> u32 {
    if color < 16 {
        BADGE_PALETTE[color as usize]
    } else if color < RGB_BASE {
        xterm256(color)
    } else {
        color - RGB_BASE
    }
}

/// A segment's style, as the dashboard's `ConsoleLine` draws it.
fn segment_style(segment: &ConsoleSegment) -> HighlightStyle {
    // the console's own text and background colour, after an inverse swap
    enum Side {
        Color(ConsoleColor),
        DefaultFg,
        DefaultBg,
        None,
    }
    let as_side = |c: Option<ConsoleColor>| c.map_or(Side::None, Side::Color);
    let (fg, bg) = if segment.flags & INVERSE != 0 {
        (
            segment.bg.map_or(Side::DefaultBg, Side::Color),
            segment.fg.map_or(Side::DefaultFg, Side::Color),
        )
    } else {
        (as_side(segment.fg), as_side(segment.bg))
    };
    let mut style = HighlightStyle::default();
    match bg {
        Side::DefaultFg => {
            // inverse without colours: the console's text and background swapped
            style.background_color = Some(colors::GRAY_300.into());
            style.color = Some(match fg {
                Side::Color(c) => rgb(badge_value(c)),
                _ => colors::GRAY_900.into(),
            });
        }
        Side::Color(bg) => {
            // text on a background colour: a badge, readable on its own
            let background = badge_value(bg);
            style.background_color = Some(rgb(background));
            style.color = Some(match fg {
                Side::Color(c) => rgb(badge_value(c)),
                _ if luminance(background) > 0.179 => rgb(0x000000),
                _ => rgb(0xffffff),
            });
        }
        Side::DefaultBg | Side::None => {
            if let Side::Color(fg) = fg {
                style.color = Some(rgb(text_color(fg)));
            }
        }
    }
    if segment.flags & DIM != 0 {
        style.fade_out = Some(0.35);
    }
    if segment.flags & BOLD != 0 {
        style.font_weight = Some(FontWeight::BOLD);
    }
    if segment.flags & ITALIC != 0 {
        style.font_style = Some(FontStyle::Italic);
    }
    if segment.flags & UNDERLINE != 0 {
        style.underline = Some(UnderlineStyle {
            thickness: px(1.),
            ..Default::default()
        });
    }
    if segment.flags & STRIKE != 0 {
        style.strikethrough = Some(StrikethroughStyle {
            thickness: px(1.),
            ..Default::default()
        });
    }
    style
}

/// A console row: one line, with line breaks inside the event shown as `↵`.
fn console_row(index: usize, line: &ParsedConsoleLine) -> AnyElement {
    let mut content = String::new();
    let mut highlights = Vec::new();
    for segment in &line.segments {
        let start = content.len();
        content.push_str(&segment.text.replace('\n', " ↵ "));
        if !segment.is_plain() && content.len() > start {
            highlights.push((start..content.len(), segment_style(segment)));
        }
    }
    let base = match line.level {
        Some(ConsoleLevel::Warn) => colors::YELLOW,
        Some(ConsoleLevel::Error) => colors::RED_200,
        None => colors::GRAY_300,
    };
    div()
        .id(("console-row", index))
        .h(px(ROW_HEIGHT))
        .px_3()
        .flex()
        .items_center()
        .whitespace_nowrap()
        .overflow_hidden()
        .font_family(fonts::MONO)
        .text_size(theme::text_size(text::SMALL))
        .text_color(base)
        .hover(|style| style.bg(colors::GRAY_800))
        .child(StyledText::new(content).with_highlights(highlights))
        .into_any_element()
}

fn small(value: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_size(theme::text_size(text::SMALL))
        .line_height(theme::line_height(text::SMALL, leading::NORMAL))
        .child(value.into())
}

fn card_title(title: &'static str) -> gpui::Div {
    div()
        .font_weight(weights::BOLD.into())
        .text_color(colors::GRAY_300)
        .child(title)
}

fn card() -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_4()
        .rounded(radius::XL)
        .border_1()
        .border_color(colors::FADE_700)
        .bg(colors::GRAY_850)
}

fn state_color(state: ServerState) -> Color {
    match state {
        ServerState::Running => colors::GREEN_200,
        ServerState::Starting | ServerState::Stopping => colors::YELLOW,
        ServerState::Error => colors::RED_200,
        ServerState::Stopped | ServerState::Unknown | ServerState::Hidden => colors::GRAY_500,
    }
}

fn status_line(status: &StreamStatus) -> gpui::Div {
    let (color, label): (Color, SharedString) = match status {
        StreamStatus::Connecting => (colors::GRAY_500, t(M::Connecting).into()),
        StreamStatus::Live => (colors::GREEN_200, t(M::Live).into()),
        StreamStatus::Reconnecting(reason) => {
            (colors::YELLOW, tr!(M::Reconnecting, reason = reason).into())
        }
        StreamStatus::Off(reason) => (colors::GRAY_500, reason.label().into()),
    };
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_1p5()
        .min_w_0()
        .child(div().flex_none().size(px(6.)).rounded_full().bg(color))
        .child(small(label).truncate().text_color(colors::GRAY_500))
}

impl ServerDetail {
    fn on_scroll(&mut self, event: &ScrollEvent, _: &mut Window, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(px(16.));
        if delta.y > px(0.) {
            self.follow = false;
        } else if self.scroll.is_scrolled_to_end() == Some(true) {
            self.follow = true;
        }
        cx.notify();
    }

    fn header(&self, cx: &mut Context<Self>) -> AnyElement {
        let server = &self.server;
        let loader = match (&server.loader, &server.loader_version) {
            (ServerLoader::Vanilla, _) => String::new(),
            (loader, Some(version)) => format!(" · {} {version}", loader_label(*loader)),
            (loader, None) => format!(" · {}", loader_label(*loader)),
        };
        let access = self.access.unwrap_or_default();
        let stopped = matches!(server.state, ServerState::Stopped | ServerState::Error);
        let status = status_label(server);
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_4()
            .child(
                Button::new("detail-back", t(M::ServersTitle))
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Slim)
                    .icon(Icon::ArrowLeft)
                    .on_click(cx.listener(|this, _, _, cx| this.close(cx))),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .font_family(fonts::HEADING)
                            .text_size(theme::text_size(text::H1))
                            .font_weight(weights::BOLD.into())
                            .text_color(colors::GRAY_300)
                            .child(server.name.clone()),
                    )
                    .child(
                        small(format!(
                            "{} · {}{loader}",
                            self.core_name, server.game_version
                        ))
                        .truncate()
                        .text_color(colors::GRAY_500),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .flex_none()
                    .child(
                        div()
                            .size(px(8.))
                            .rounded_full()
                            .bg(state_color(server.state)),
                    )
                    .child(small(status).text_color(colors::GRAY_400)),
            )
            .when(access.start && stopped, |this| {
                this.child(
                    Button::new("detail-start", t(CommonMsg::Start))
                        .size(ButtonSize::Slim)
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(DetailEvent::Start))),
                )
            })
            .when(access.stop && !stopped, |this| {
                this.child(
                    Button::new("detail-stop", t(CommonMsg::Stop))
                        .size(ButtonSize::Slim)
                        .variant(ButtonVariant::Danger)
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(DetailEvent::Stop))),
                )
            })
            .into_any_element()
    }

    fn console_card(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let access = self.access;
        let can_read = access.is_some_and(|a| a.console_read);
        let can_write = access.is_some_and(|a| a.console_write);
        let count = self.console.len();
        if self.seen_rows != count {
            self.seen_rows = count;
            if self.follow && count > 0 {
                self.scroll
                    .scroll_to_item(count - 1, ScrollStrategy::Bottom);
            }
        }
        let body: AnyElement = if access.is_none() {
            div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .child(LoadingDots)
                .into_any_element()
        } else if !can_read {
            div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_2()
                .size_full()
                .px_8()
                .child(
                    div()
                        .font_weight(weights::BOLD.into())
                        .text_color(colors::GRAY_FADED.opacity(50))
                        .child(t(M::ConsoleNotShown)),
                )
                .child(
                    small(match &self.console_status {
                        StreamStatus::Off(reason) => reason.label(),
                        _ => tr!(
                            M::ConsoleKeyCantReadShort,
                            computer = t(CommonMsg::ThisComputer)
                        ),
                    })
                    .text_center()
                    .text_color(colors::GRAY_500),
                )
                .into_any_element()
        } else if count == 0 {
            div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .text_color(colors::GRAY_FADED.opacity(30))
                .font_weight(weights::BOLD.into())
                .child(t(M::OutputAppearsHere))
                .into_any_element()
        } else {
            uniform_list(
                "server-console-rows",
                count,
                cx.processor(|this, range: std::ops::Range<usize>, _, _| {
                    range
                        .filter_map(|index| {
                            let row = this.console.get(index)?;
                            Some(console_row(index, &row.line))
                        })
                        .collect()
                }),
            )
            .track_scroll(&self.scroll)
            .size_full()
            .py_2()
            .into_any_element()
        };

        let chat: AnyElement = if can_write {
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .id("server-chat")
                                .flex_1()
                                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                                    if event.keystroke.key == "enter" {
                                        this.send_chat(cx);
                                        cx.stop_propagation();
                                    }
                                }))
                                .child(self.chat.clone()),
                        )
                        .child(
                            Button::new("server-chat-send", t(M::Send))
                                .variant(ButtonVariant::Primary)
                                .loading(self.sending)
                                .on_click(cx.listener(|this, _, _, cx| this.send_chat(cx))),
                        ),
                )
                .child(
                    small(match &self.chat_error {
                        Some(error) => error.clone(),
                        None => t(M::ChatHint).into(),
                    })
                    .text_color(if self.chat_error.is_some() {
                        colors::RED_200
                    } else {
                        colors::GRAY_550
                    }),
                )
                .into_any_element()
        } else {
            small(if access.is_some() {
                tr!(
                    M::ConsoleKeyCantWrite,
                    computer = t(CommonMsg::ThisComputer),
                    option = t(M::SendChat)
                )
            } else {
                String::new()
            })
            .text_color(colors::GRAY_550)
            .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(card_title(t(M::Console)))
                    .when(can_read, |this| {
                        this.child(status_line(&self.console_status))
                    })
                    .child(div().flex_1())
                    .when(!self.follow && can_read && count > 0, |this| {
                        this.child(
                            Button::new("console-follow", t(M::JumpToLatest))
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Slim)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.follow = true;
                                    this.scroll.scroll_to_bottom();
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .child(
                div()
                    .id("server-console")
                    .flex_1()
                    .min_h_0()
                    .rounded(radius::LG)
                    .border_1()
                    .border_color(colors::FADE_700)
                    .bg(colors::GRAY_900)
                    .overflow_hidden()
                    .on_scroll(cx.listener(Self::on_scroll))
                    .child(body),
            )
            .child(chat)
            .into_any_element()
    }

    fn players_card(&self) -> AnyElement {
        let mut card = card().child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(card_title(t(M::Players)))
                .child(status_line(&self.events_status)),
        );
        card = match &self.players {
            None => card.child(LoadingDots),
            Some(players) if players.is_empty() => card.child(
                small(t(if self.server.state == ServerState::Running {
                    M::NoOneOnline
                } else {
                    M::ServerNotRunning
                }))
                .text_color(colors::GRAY_500),
            ),
            Some(players) => card.children(players.iter().map(|player| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(div().size(px(6.)).rounded_full().bg(colors::GREEN_200))
                    .child(
                        div()
                            .truncate()
                            .text_color(colors::GRAY_300)
                            .child(player.name.clone()),
                    )
            })),
        };
        card.into_any_element()
    }

    fn account_card(&self, cx: &mut Context<Self>) -> AnyElement {
        let user = self.user_label();
        let mut card = card().child(card_title(t(M::MinecraftAccount)));
        match &self.links {
            LinkState::Loading => return card.child(LoadingDots).into_any_element(),
            LinkState::Unsupported => {
                return card
                    .child(small(t(M::CoreTooOldToLink)).text_color(colors::GRAY_500))
                    .into_any_element();
            }
            LinkState::Failed(err) => {
                card = card.child(small(core_error(err)).text_color(colors::RED_200));
            }
            LinkState::Known(links) => {
                card = card.children(links.iter().map(|link| {
                    let uuid = link.minecraft_uuid.clone();
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(Icon::Check.render(px(12.), colors::GREEN_200))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_color(colors::GRAY_300)
                                .child(format!("{} → {user}", link.minecraft_name)),
                        )
                        .child(
                            Button::new(SharedString::from(format!("unlink-{uuid}")), t(M::Unlink))
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Slim)
                                .on_click(
                                    cx.listener(move |this, _, _, cx| {
                                        this.unlink(uuid.clone(), cx)
                                    }),
                                ),
                        )
                }));
            }
        }
        let selected = self.selected_microsoft_account(cx);
        card = match selected {
            Some((_, name)) if !self.selected_is_linked(cx) => card
                .child(
                    Button::new(
                        "link-account",
                        tr!(M::LinkAccount, account = name, user = user),
                    )
                    .variant(ButtonVariant::Primary)
                    .loading(self.linking)
                    .full_width()
                    .on_click(cx.listener(|this, _, _, cx| this.link_selected(cx))),
                )
                .child(small(t(M::LinkCheckHint)).text_color(colors::GRAY_550)),
            Some(_) => card,
            None if self.selected_is_offline(cx) => {
                card.child(small(t(M::OfflineCantLink)).text_color(colors::GRAY_500))
            }
            None => card.child(small(t(M::SignInMicrosoftToLink)).text_color(colors::GRAY_500)),
        };
        if let Some((error, message)) = &self.link_notice {
            card = card.child(small(message.clone()).text_color(if *error {
                colors::RED_200
            } else {
                colors::GREEN_200
            }));
        }
        card.into_any_element()
    }
}

impl Render for ServerDetail {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let header = self.header(cx);
        let console = self.console_card(cx);
        let players = self.players_card();
        let account = self.account_card(cx);
        div()
            .id("server-detail")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .size_full()
            .px_8()
            .py_6()
            .gap_5()
            .child(header)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_5()
                    .flex_1()
                    .min_h_0()
                    .child(console)
                    .child(
                        div()
                            .id("server-detail-side")
                            .flex()
                            .flex_col()
                            .gap_4()
                            .w(px(280.))
                            .flex_none()
                            .overflow_y_scroll()
                            .child(players)
                            .child(account),
                    ),
            )
    }
}
