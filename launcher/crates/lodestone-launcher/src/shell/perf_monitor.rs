//! The performance monitor: a small window, opened from Settings → Developer, with
//! the launcher's memory, CPU, threads and open files, how the main window draws, a
//! running game's memory and CPU, and what the launcher holds. One at a time;
//! opening it again brings it to the front. It floats above the launcher's windows.
//!
//! It samples once a second while open ([`crate::perf::Sampler`]) and redraws only
//! then; closing it stops everything. Like the skin editor it draws in the launcher's
//! own look, whatever the skin.

use std::collections::VecDeque;

use gpui::{
    AnyElement, App, ClipboardItem, Context, Global, IntoElement, MouseButton, Render,
    SharedString, Task, Window, WindowHandle, WindowKind, div, prelude::*, size,
};

use crate::{
    components::{Button, ButtonSize, ButtonVariant, Icon},
    i18n::{ShellMsg as M, ShellPlural as P, format, t, tr, trn},
    perf::{ProcessReading, Sample, Sampler, Sources},
    theme::{self, Color, colors, fonts, px, text, weights},
};

/// Readings kept for the small charts: a minute.
const HISTORY: usize = 60;

/// The open monitor window, if there is one.
#[derive(Default)]
struct MonitorWindow(Option<WindowHandle<PerfMonitor>>);

impl Global for MonitorWindow {}

/// Opens the monitor, or brings it to the front if it is open.
pub fn open(sources: Sources, cx: &mut App) -> Option<WindowHandle<PerfMonitor>> {
    if let Some(handle) = cx.try_global::<MonitorWindow>().and_then(|w| w.0)
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        return Some(handle);
    }
    let mut options = crate::window_options(t(M::PerfMonitor), 420., 640., cx);
    options.window_min_size = Some(size(px(360.), px(320.)));
    // above the launcher's other windows
    options.kind = WindowKind::Floating;
    let Ok(handle) = cx.open_window(options, |_, cx| cx.new(|cx| PerfMonitor::new(sources, cx)))
    else {
        return None;
    };
    cx.set_global(MonitorWindow(Some(handle)));
    #[cfg(feature = "screenshot")]
    if crate::preview::requested_scene().is_some_and(|scene| scene == "perf-monitor")
        && let Some(path) = crate::screenshot_path()
    {
        crate::capture_window(handle.into(), path, cx);
    }
    Some(handle)
}

pub struct PerfMonitor {
    sources: Sources,
    sampler: Sampler,
    history: VecDeque<Sample>,
    copied: bool,
    /// Showing made-up readings (the screenshot): no sampling.
    frozen: bool,
    _sampling: Task<()>,
}

impl PerfMonitor {
    fn new(sources: Sources, cx: &mut Context<Self>) -> Self {
        let mut sampler = Sampler::new();
        let mut history = VecDeque::with_capacity(HISTORY);
        history.push_back(sampler.sample(&sources, cx));
        let sampling = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(1))
                    .await;
                let sampled = this.update(cx, |this, cx| {
                    if this.frozen {
                        return;
                    }
                    let sample = this.sampler.sample(&this.sources, cx);
                    this.push(sample);
                    cx.notify();
                });
                if sampled.is_err() {
                    break;
                }
            }
        });
        Self {
            sources,
            sampler,
            history,
            copied: false,
            frozen: false,
            _sampling: sampling,
        }
    }

    fn push(&mut self, sample: Sample) {
        if self.history.len() == HISTORY {
            self.history.pop_front();
        }
        self.history.push_back(sample);
    }

    /// Fills the history with made-up readings, for the offscreen screenshot.
    #[cfg(feature = "screenshot")]
    pub fn preview_history(&mut self, history: Vec<Sample>) {
        self.history = history.into_iter().collect();
        self.frozen = true;
    }

    fn series(&self, value: impl Fn(&Sample) -> Option<f64>) -> Vec<Option<f64>> {
        self.history.iter().map(value).collect()
    }
}

/// A small bar chart of the last minute, newest on the right. Bars start from zero,
/// or with `from_zero` false from a little under the lowest value, so a slow change
/// (memory) shows.
fn sparkline(values: &[Option<f64>], color: Color, from_zero: bool) -> AnyElement {
    let max = values.iter().flatten().copied().fold(0.0_f64, f64::max);
    let min = if from_zero {
        0.0
    } else {
        let min = values.iter().flatten().copied().fold(f64::MAX, f64::min);
        // a tenth of the range below the lowest, so the lowest bar still shows
        (min - (max - min) * 0.1).max(0.0)
    };
    let range = (max - min).max(f64::EPSILON);
    let height = 22.0;
    let empty = HISTORY.saturating_sub(values.len());
    div()
        .flex()
        .flex_row()
        .items_end()
        .gap(px(1.))
        .h(px(height))
        .w_full()
        .children((0..empty).map(|_| div().flex_1().into_any_element()))
        .children(values.iter().map(|value| {
            let bar = value.map_or(0.0, |v| ((v - min) / range * height as f64).max(1.0) as f32);
            div()
                .flex_1()
                .h(px(bar))
                .rounded_t(px(1.))
                .bg(color)
                .into_any_element()
        }))
        .into_any_element()
}

fn heading(label: &'static str) -> impl IntoElement {
    div()
        .pt_3()
        .pb_1()
        .text_size(theme::text_size(text::SMALL))
        .font_weight(weights::BOLD.into())
        .text_color(colors::GRAY_500)
        .child(label.to_uppercase())
}

/// A label with its value on the right, and a chart under them when given.
fn row(label: impl Into<SharedString>, value: impl Into<SharedString>) -> gpui::Div {
    div().flex().flex_col().gap_1().py_1().child(
        div()
            .flex()
            .flex_row()
            .justify_between()
            .gap_3()
            .child(div().text_color(colors::GRAY_400).child(label.into()))
            .child(
                div()
                    .text_color(colors::GRAY_300)
                    .font_weight(weights::BOLD.into())
                    .child(value.into()),
            ),
    )
}

fn unknown() -> String {
    t(M::PerfUnknown).to_string()
}

fn memory(bytes: Option<u64>) -> String {
    bytes.map_or_else(unknown, format::bytes_binary)
}

fn cpu(percent: Option<f64>) -> String {
    percent.map_or_else(unknown, |p| format!("{}%", format::decimal(p, 1)))
}

fn count(value: Option<u32>) -> String {
    value.map_or_else(unknown, |n| format::number(u64::from(n)))
}

fn ms(value: Option<f64>) -> String {
    value.map_or_else(unknown, |v| format!("{} ms", format::decimal(v, 1)))
}

impl Render for PerfMonitor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // the monitor in the launcher's own look
        crate::skin::set_frame(&crate::skin::launcher_look());
        window.set_rem_size(theme::zoom::rem_size());
        let Some(now) = self.history.back().cloned() else {
            return div().into_any_element();
        };
        let mac = cfg!(target_os = "macos");
        let launcher = &now.launcher;
        let memory_series = self.series(|s| s.launcher.memory.map(|m| m as f64));
        let cpu_series = self.series(|s| s.launcher.cpu_percent);
        let frame_series = self.series(|s| s.frames.p95_ms);

        let process_rows = |process: &ProcessReading, full: bool| {
            let mut rows = vec![
                row(t(M::PerfMemory), memory(process.memory)).into_any_element(),
                row(
                    t(M::PerfCpu),
                    process.cpu_percent.map_or_else(unknown, |p| {
                        tr!(M::PerfCpuValue, percent = cpu(Some(p)), cores = now.cores)
                    }),
                )
                .into_any_element(),
            ];
            if full {
                rows.push(row(t(M::PerfThreads), count(process.threads)).into_any_element());
                rows.push(row(t(M::PerfOpenFiles), count(process.open_files)).into_any_element());
            }
            rows
        };

        let mut launcher_rows = process_rows(launcher, true);
        launcher_rows.insert(1, sparkline(&memory_series, colors::BLUE_200, false));
        launcher_rows.insert(3, sparkline(&cpu_series, colors::GREEN_300, true));

        let frames = &now.frames;
        let rendering = vec![
            row(t(M::PerfFps), format::decimal(frames.fps, 1)).into_any_element(),
            row(
                t(M::PerfFrameTime),
                tr!(
                    M::PerfFrameStats,
                    avg = ms(frames.avg_ms),
                    p95 = ms(frames.p95_ms),
                    max = ms(frames.max_ms)
                ),
            )
            .into_any_element(),
            sparkline(&frame_series, colors::YELLOW_300, true),
        ];

        let games: Vec<AnyElement> = if now.games.is_empty() {
            vec![
                div()
                    .py_1()
                    .text_color(colors::GRAY_500)
                    .child(t(M::PerfNoGame))
                    .into_any_element(),
            ]
        } else {
            now.games
                .iter()
                .flat_map(|game| {
                    let mut rows = vec![
                        div()
                            .pt_1()
                            .text_color(colors::GRAY_300)
                            .child(format!("{} · pid {}", game.name, game.process.pid))
                            .into_any_element(),
                    ];
                    rows.extend(process_rows(&game.process, false));
                    rows
                })
                .collect()
        };

        let mut internals = vec![
            row(
                t(M::PerfImages),
                format!(
                    "{} · {}",
                    format::number(now.images.count as u64),
                    format::bytes_binary(now.images.bytes)
                ),
            )
            .into_any_element(),
        ];
        if let Some((entries, bytes)) = now.content_cache {
            internals.push(
                row(
                    t(M::PerfContentCache),
                    format!(
                        "{} · {}",
                        trn!(P::PerfEntries, entries),
                        format::bytes_binary(bytes)
                    ),
                )
                .into_any_element(),
            );
        }
        internals.push(row(t(M::PerfJobs), format::number(now.jobs as u64)).into_any_element());
        internals.push(
            row(
                t(M::PerfDownloads),
                match now.download_rate {
                    Some(rate) => format!(
                        "{} · {}",
                        format::number(now.downloads as u64),
                        tr!(M::PerfRate, rate = format::bytes_binary(rate as u64))
                    ),
                    None => format::number(now.downloads as u64),
                },
            )
            .into_any_element(),
        );
        if let Some(clients) = now.clients {
            internals
                .push(row(t(M::PerfCliClients), format::number(clients as u64)).into_any_element());
        }
        internals.push(
            row(
                t(M::PerfTraceBus),
                match now.trace {
                    Some(trace) => tr!(
                        M::PerfTraceValue,
                        subscribers = trace.subscribers,
                        rate = trace
                            .events_per_sec
                            .map_or_else(unknown, |r| format::decimal(r, 1))
                    ),
                    None => t(M::PerfOff).to_string(),
                },
            )
            .into_any_element(),
        );

        let copied = self.copied;
        div()
            .id("perf-monitor")
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors::GRAY_875)
            .font_family(fonts::SANS)
            .map(crate::i18n::locale_fonts)
            .text_color(colors::GRAY_300)
            .text_size(theme::text_size(text::SMALL))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_none()
                    .items_center()
                    .gap_2()
                    .h(gpui::px(super::top_bar::TOP_BAR_HEIGHT + 1.0))
                    .pl(px(if mac { 84. } else { 16. }))
                    .pr_3()
                    .bg(colors::GRAY_800)
                    .border_b_1()
                    .border_color(colors::GRAY_FADED.opacity(30))
                    .child(Icon::Gear.render(px(14.), colors::GRAY_400))
                    .child(
                        div()
                            .font_weight(weights::BOLD.into())
                            .text_size(theme::text_size(text::MEDIUM))
                            .child(t(M::PerfMonitor)),
                    )
                    // the empty part of the bar moves the window
                    .child(
                        div()
                            .id("perf-monitor-drag")
                            .flex_1()
                            .h_full()
                            .on_mouse_down(MouseButton::Left, |_, window, _| {
                                window.start_window_move()
                            }),
                    )
                    .child(
                        Button::new(
                            "perf-copy",
                            t(if copied { M::PerfCopied } else { M::PerfCopy }),
                        )
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Slim)
                        .icon(Icon::Copy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(sample) = this.history.back() {
                                cx.write_to_clipboard(ClipboardItem::new_string(sample.to_text()));
                                this.copied = true;
                                cx.notify();
                            }
                        })),
                    ),
            )
            .child(
                div()
                    .id("perf-monitor-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_4()
                    .pb_4()
                    .child(heading(t(M::PerfLauncher)))
                    .children(launcher_rows)
                    .child(heading(t(M::PerfRendering)))
                    .children(rendering)
                    .when(frames.idle, |this| {
                        this.child(
                            div()
                                .py_1()
                                .text_color(colors::GRAY_500)
                                .child(t(M::PerfIdle)),
                        )
                    })
                    .child(heading(t(M::PerfGame)))
                    .children(games)
                    .child(heading(t(M::PerfInternals)))
                    .children(internals)
                    .child(
                        div()
                            .mt_3()
                            .pt_2()
                            .border_t_1()
                            .border_color(colors::FADE_700)
                            .flex()
                            .flex_col()
                            .gap_1()
                            .text_color(colors::GRAY_500)
                            .child(t(M::PerfLastMinute))
                            .child(t(M::PerfSelfNote)),
                    ),
            )
            .children(super::top_bar::window_controls(window, cx))
            .into_any_element()
    }
}
