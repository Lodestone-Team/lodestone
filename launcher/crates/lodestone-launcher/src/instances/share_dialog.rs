//! "Share code": an instance's Modrinth content as a code to paste into another
//! Lodestone Launcher, and what the code leaves out.

use std::time::Duration;

use gpui::{
    App, AppContext as _, ClipboardItem, Context, Entity, EventEmitter, FocusHandle, Focusable,
    IntoElement, Render, SharedString, Task, Window, div, prelude::*,
};
use launcher_core::content::{
    ContentKind,
    share::{self, LeftOutReason, SharePlan},
};

use super::Instances;
use crate::{
    components::{Button, ButtonVariant, Dialog, Icon, LoadingDots, RichText},
    i18n::{
        CommonMsg, ContentMsg, InstanceSettingsMsg as M, InstanceSettingsPlural as P, format, t,
        tr, trn,
    },
    theme::{self, colors, fonts, px, radius, text, weights},
};

/// The names shown in the "not shared" list before "and N more".
const SHOWN_LEFT_OUT: usize = 6;

pub struct ShareDismissed;

enum State {
    Reading,
    Ready { plan: SharePlan, code: String },
    Failed(SharedString),
}

pub struct ShareDialog {
    name: String,
    state: State,
    copied: bool,
    focus: FocusHandle,
    _task: Option<Task<()>>,
}

impl EventEmitter<ShareDismissed> for ShareDialog {}

impl Focusable for ShareDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ShareDialog {
    /// Reads the instance `id`'s content and makes its code, in the background.
    pub fn new(instances: Entity<Instances>, id: &str, cx: &mut Context<Self>) -> Self {
        let (instance, api, dirs, target) = {
            let instances = instances.read(cx);
            (
                instances.list().iter().find(|i| i.id == id).cloned(),
                instances.modrinth(),
                instances.content_dirs(id),
                instances.content_target(id),
            )
        };
        let name = instance
            .as_ref()
            .map(|i| i.name.clone())
            .unwrap_or_default();
        let task = match (instance, target) {
            (Some(instance), Some(target)) => {
                let work = cx.background_spawn(async move {
                    share::collect(
                        api.as_deref(),
                        &dirs,
                        &target,
                        Some(instance.name.clone()),
                        instance.loader,
                        instance.loader_version.clone(),
                    )
                    .await
                });
                Some(cx.spawn(async move |this, cx| {
                    let result = work.await;
                    this.update(cx, |this, cx| {
                        this.state = match result {
                            Ok(plan) => {
                                let code = plan.code.encode();
                                State::Ready { plan, code }
                            }
                            Err(err) => State::Failed(err.to_string().into()),
                        };
                        cx.notify();
                    })
                    .ok();
                }))
            }
            _ => None,
        };
        Self {
            name,
            state: State::Reading,
            copied: false,
            focus: cx.focus_handle(),
            _task: task,
        }
    }

    /// A finished dialog, for the offscreen screenshots.
    pub fn preview(name: &str, plan: SharePlan, cx: &mut Context<Self>) -> Self {
        let code = plan.code.encode();
        Self {
            name: name.to_string(),
            state: State::Ready { plan, code },
            copied: false,
            focus: cx.focus_handle(),
            _task: None,
        }
    }

    fn copy(&mut self, cx: &mut Context<Self>) {
        let State::Ready { code, .. } = &self.state else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(code.clone()));
        self.copied = true;
        cx.notify();
        // "Copied" goes back to "Copy code" after a moment.
        self._task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            this.update(cx, |this, cx| {
                this.copied = false;
                cx.notify();
            })
            .ok();
        }));
    }
}

fn count_tile(label: &'static str, count: usize) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .gap_0p5()
        .px_3()
        .py_2()
        .rounded(radius::LG)
        .border_1()
        .border_color(colors::FADE_700)
        .bg(colors::GRAY_800)
        .child(
            div()
                .text_size(theme::text_size(text::SMALL))
                .text_color(colors::GRAY_500)
                .child(label),
        )
        .child(
            div()
                .text_size(theme::text_size(text::H3))
                .font_weight(weights::BOLD.into())
                .text_color(colors::GRAY_300)
                .child(format::number(count as u64)),
        )
}

impl Render for ShareDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let body = match &self.state {
            State::Reading => div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .py_6()
                .text_color(colors::GRAY_500)
                .child(LoadingDots)
                .child(t(M::ShareReading))
                .into_any_element(),
            State::Failed(error) => div()
                .text_color(colors::RED_200)
                .child(RichText::new(error.clone()))
                .into_any_element(),
            State::Ready { plan, code } => {
                let counts = div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(count_tile(
                        t(ContentMsg::KindMods),
                        plan.code.count(ContentKind::Mod),
                    ))
                    .child(count_tile(
                        t(ContentMsg::KindResourcePacks),
                        plan.code.count(ContentKind::ResourcePack),
                    ))
                    .child(count_tile(
                        t(ContentMsg::KindShaders),
                        plan.code.count(ContentKind::ShaderPack),
                    ));
                let left_out = (!plan.left_out.is_empty()).then(|| {
                    let shown: Vec<_> = plan.left_out.iter().take(SHOWN_LEFT_OUT).collect();
                    let more = plan.left_out.len().saturating_sub(SHOWN_LEFT_OUT);
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .px_4()
                        .py_3()
                        .rounded(radius::LG)
                        .border_1()
                        .border_color(colors::YELLOW.opacity(30))
                        .bg(colors::YELLOW.opacity(8))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .child(Icon::TriangleExclamation.render(px(14.), colors::YELLOW))
                                .child(
                                    div()
                                        .font_weight(weights::BOLD.into())
                                        .text_color(colors::GRAY_300)
                                        .child(trn!(P::ShareNotShared, plan.left_out.len())),
                                ),
                        )
                        .child(
                            div()
                                .text_size(theme::text_size(text::SMALL))
                                .text_color(colors::GRAY_400)
                                .child(t(M::ShareNotSharedBody)),
                        )
                        .children(shown.into_iter().map(|item| {
                            let reason = match item.reason {
                                LeftOutReason::CurseForge => t(M::ShareFromCurseForge),
                                LeftOutReason::NotOnModrinth => t(M::ShareAddedByHand),
                            };
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .text_size(theme::text_size(text::SMALL))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_color(colors::GRAY_300)
                                        .child(RichText::minecraft(item.file_name.clone())),
                                )
                                .child(div().flex_none().text_color(colors::GRAY_500).child(reason))
                        }))
                        .when(more > 0, |this| {
                            this.child(
                                div()
                                    .text_size(theme::text_size(text::SMALL))
                                    .text_color(colors::GRAY_500)
                                    .child(tr!(M::ShareMore, count = format::number(more as u64))),
                            )
                        })
                });
                let preview: String = code.chars().take(72).collect();
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .text_size(theme::text_size(text::MEDIUM))
                    .child(div().text_color(colors::GRAY_400).child(t(M::ShareIntro)))
                    .child(counts)
                    .children(left_out)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .px_3()
                            .py_2()
                            .rounded(radius::LG)
                            .border_1()
                            .border_color(colors::FADE_700)
                            .bg(colors::GRAY_900)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .font_family(fonts::MONO)
                                    .text_size(theme::text_size(text::SMALL))
                                    .text_color(colors::GRAY_400)
                                    .child(format!("{preview}…")),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(theme::text_size(text::SMALL))
                                    .text_color(colors::GRAY_500)
                                    .child(format::bytes(code.len() as u64)),
                            ),
                    )
                    .into_any_element()
            }
        };
        let ready = matches!(self.state, State::Ready { .. });
        Dialog::new(
            "share-code",
            tr!(M::ShareTitle, name = self.name),
            self.focus.clone(),
        )
        .width(px(600.))
        .on_close({
            let entity = entity.clone();
            move |_, cx| entity.update(cx, |_, cx| cx.emit(ShareDismissed))
        })
        .child(body)
        .footer(
            Button::new("close-share", t(CommonMsg::Close))
                .variant(ButtonVariant::Ghost)
                .on_click(cx.listener(|_, _, _, cx| cx.emit(ShareDismissed))),
        )
        .footer(
            Button::new(
                "copy-share-code",
                if self.copied {
                    t(M::CodeCopied)
                } else {
                    t(M::CopyCode)
                },
            )
            .variant(ButtonVariant::Primary)
            .icon(if self.copied { Icon::Check } else { Icon::Copy })
            .disabled(!ready)
            .on_click(cx.listener(|this, _, _, cx| this.copy(cx))),
        )
        .footer_start(div())
    }
}
