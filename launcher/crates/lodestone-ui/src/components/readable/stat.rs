//! Numbers a decision depends on ([`StatTile`], side by side in a [`StatRow`]), how
//! far something has come ([`ProgressBar`]), space needed against space free
//! ([`MeterBar`]), and a task in progress with its object and figures
//! ([`ProgressBlock`]).

use std::panic::Location;

use gpui::{
    App, ElementId, IntoElement, RenderOnce, SharedString, Window, div, prelude::*, relative,
};

use super::{Fact, StatusBadge, VersionTag, auto_id, key_value};
use crate::{
    components::{
        Icon,
        a11y::{Accessible, Role},
    },
    i18n::{
        ValueMsg,
        format::{self, Measure},
        interpolate, t,
    },
    theme::{self, Color, Tone, colors, fonts, ink, pill, px, radius, text, weights},
    values::truncate::{MAX_SHOWN, sanitize},
};

/// The single number a decision depends on, above the explanation: the value at
/// 20px with its unit at 14px ink-muted, and the label under it at 12px ink-faint
/// ("Frees up | 1.4 GB"). A warning or danger tone puts the value on its pill with a
/// warning icon (a shortfall), the only colour it ever has.
#[derive(Clone, Debug, IntoElement)]
pub struct StatTile {
    id: ElementId,
    label: SharedString,
    value: Measure,
    tone: Tone,
}

impl StatTile {
    #[track_caller]
    pub fn new(label: impl Into<SharedString>, value: Measure) -> Self {
        let label = label.into();
        Self {
            id: auto_id(Location::caller(), &label),
            label,
            value,
            tone: Tone::Neutral,
        }
    }

    /// A size: "1.4 GB".
    #[track_caller]
    pub fn bytes(label: impl Into<SharedString>, bytes: u64) -> Self {
        let label = label.into();
        Self {
            id: auto_id(Location::caller(), &label),
            label,
            value: format::bytes_parts(bytes),
            tone: Tone::Neutral,
        }
    }

    /// Only [`Tone::Warning`] (tight) and [`Tone::Danger`] (short) change it.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
}

impl RenderOnce for StatTile {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let marked = matches!(self.tone, Tone::Warning | Tone::Danger);
        let label = key_value(&self.label, &self.value.to_string());
        let value_color = if marked {
            pill::fg(self.tone)
        } else {
            ink::text()
        };
        let unit = |text: &'static str| {
            (!text.is_empty()).then(|| {
                div()
                    .text_size(theme::text_size(text::MEDIUM))
                    .text_color(if marked { value_color } else { ink::muted() })
                    .child(text)
            })
        };
        div()
            .id(self.id.clone())
            .accessible_id(Role::Label, &self.id, label)
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_none()
                    .items_baseline()
                    .self_start()
                    .gap(px(1.))
                    .font_features(fonts::tabular())
                    .font_weight(weights::MEDIUMBOLD.into())
                    .whitespace_nowrap()
                    .when(marked, |this| {
                        this.px(px(6.))
                            .rounded(radius::DEFAULT)
                            .bg(pill::bg(self.tone))
                            .gap(px(4.))
                            .child(
                                div()
                                    .self_center()
                                    .child(Icon::TriangleExclamation.render(px(12.), value_color)),
                            )
                    })
                    .children(unit(self.value.before))
                    .child(
                        div()
                            .text_size(theme::text_size(text::H2))
                            .text_color(value_color)
                            .child(self.value.value.clone()),
                    )
                    .children(unit(self.value.after)),
            )
            .child(
                div()
                    .text_size(theme::text_size(text::SMALL))
                    .font_weight(weights::MEDIUM.into())
                    .text_color(ink::faint())
                    .child(self.label),
            )
    }
}

/// Two or three stat tiles side by side, 24px apart, with an optional meter under
/// them: "Needed 2.4 GB | Free 1.1 GB".
#[derive(IntoElement, Default)]
pub struct StatRow {
    tiles: Vec<StatTile>,
    meter: Option<MeterBar>,
}

impl StatRow {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tile(mut self, tile: StatTile) -> Self {
        self.tiles.push(tile);
        self
    }

    pub fn meter(mut self, meter: MeterBar) -> Self {
        self.meter = Some(meter);
        self
    }
}

impl RenderOnce for StatRow {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .w_full()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap_x(px(24.))
                    .gap_y(px(8.))
                    .children(self.tiles),
            )
            .children(self.meter)
    }
}

/// The fill colour for a tone: blue while it goes, yellow when paused or tight, red
/// when it failed or falls short.
fn fill(tone: Tone) -> Color {
    match tone {
        Tone::Neutral | Tone::Info => colors::BLUE_300,
        Tone::Success => colors::GREEN_300,
        Tone::Warning => colors::YELLOW_300,
        Tone::Danger => colors::RED_300,
    }
}

/// A 4px bar of how far something has come: a gray-700 track with a blue fill
/// (yellow when paused, red when it failed).
#[derive(Clone, Debug, IntoElement)]
pub struct ProgressBar {
    id: ElementId,
    fraction: f32,
    tone: Tone,
    marker: Option<f32>,
    label: Option<SharedString>,
}

impl ProgressBar {
    /// `fraction` from 0 to 1.
    #[track_caller]
    pub fn new(fraction: f32) -> Self {
        Self {
            id: auto_id(Location::caller(), "progress"),
            fraction: clamp01(fraction),
            tone: Tone::Info,
            marker: None,
            label: None,
        }
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }
}

fn clamp01(value: f32) -> f32 {
    if value.is_nan() {
        0.
    } else {
        value.clamp(0., 1.)
    }
}

impl RenderOnce for ProgressBar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let percent = (self.fraction * 100.).round() as u64;
        let (role, label) = match (self.marker, self.label) {
            (Some(_), Some(label)) => (Role::Meter, label),
            _ => (Role::ProgressIndicator, format::percent(percent).into()),
        };
        div()
            .id(self.id.clone())
            .accessible_id(role, &self.id, label)
            .relative()
            .w_full()
            .h(px(4.))
            .rounded_full()
            .bg(colors::GRAY_700)
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .h_full()
                    .w(relative(self.fraction))
                    .rounded_full()
                    .bg(fill(self.tone)),
            )
            .when_some(self.marker, |this, at| {
                this.child(
                    div()
                        .absolute()
                        .top(px(-3.))
                        .left(relative(at))
                        .ml(px(-1.))
                        .w(px(2.))
                        .h(px(10.))
                        .rounded_full()
                        .bg(ink::text()),
                )
            })
    }
}

/// Space needed against space free: a bar of the free space with a 2px mark where
/// the needed amount falls. Red when the need is more than what is free, yellow when
/// it leaves less than a tenth.
#[derive(Clone, Debug, IntoElement)]
pub struct MeterBar {
    bar: ProgressBar,
}

impl MeterBar {
    #[track_caller]
    pub fn needed_free(needed: u64, free: u64) -> Self {
        let scale = needed.max(free).max(1) as f32;
        let tone = meter_tone(needed, free);
        let label = interpolate(
            t(ValueMsg::NeededOfFree),
            &[
                ("needed", &format::bytes(needed) as &dyn std::fmt::Display),
                ("free", &format::bytes(free) as &dyn std::fmt::Display),
            ],
        );
        let mut bar = ProgressBar::new(free as f32 / scale).tone(tone);
        bar.id = auto_id(Location::caller(), "meter");
        bar.marker = Some(clamp01(needed as f32 / scale));
        bar.label = Some(label.into());
        Self { bar }
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.bar.id = id.into();
        self
    }
}

/// How a need sits against what is free.
fn meter_tone(needed: u64, free: u64) -> Tone {
    if needed > free {
        Tone::Danger
    } else if needed.saturating_mul(10) > free.saturating_mul(9) {
        Tone::Warning
    } else {
        Tone::Info
    }
}

impl RenderOnce for MeterBar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        self.bar
    }
}

/// A task in progress: its status badge, the object's name and version, and the
/// percentage right-aligned in a fixed cell; the bar under them; then its figures as
/// keyed facts 16px apart ("Files 3 / 5", "Size 1.2 / 3.4 MB").
#[derive(IntoElement)]
pub struct ProgressBlock {
    id: ElementId,
    status: StatusBadge,
    object: SharedString,
    version: Option<VersionTag>,
    percent: Option<u8>,
    stats: Vec<Fact>,
}

impl ProgressBlock {
    pub fn new(
        id: impl Into<ElementId>,
        status: StatusBadge,
        object: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            status,
            object: sanitize(&object.into(), MAX_SHOWN).into(),
            version: None,
            percent: None,
            stats: Vec::new(),
        }
    }

    pub fn version(mut self, version: VersionTag) -> Self {
        self.version = Some(version);
        self
    }

    pub fn percent(mut self, percent: u8) -> Self {
        self.percent = Some(percent.min(100));
        self
    }

    pub fn stat(mut self, fact: Fact) -> Self {
        self.stats.push(fact);
        self
    }
}

impl RenderOnce for ProgressBlock {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let tone = match self.status.tone() {
            Tone::Warning => Tone::Warning,
            Tone::Danger => Tone::Danger,
            _ => Tone::Info,
        };
        let fraction = self.percent.map_or(0., |p| p as f32 / 100.);
        div()
            .id(self.id.clone())
            .accessible_id(Role::Group, &self.id, self.object.clone())
            .flex()
            .flex_col()
            .gap(px(6.))
            .w_full()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .min_w_0()
                    .child(self.status)
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(theme::text_size(text::MEDIUM))
                            .font_weight(weights::MEDIUMBOLD.into())
                            .text_color(ink::text())
                            .child(self.object),
                    )
                    .children(self.version)
                    .child(div().flex_1())
                    .when_some(self.percent, |this, percent| {
                        this.child(
                            div()
                                .flex_none()
                                .w(px(40.))
                                .text_right()
                                .text_size(theme::text_size(text::SMALL))
                                .font_weight(weights::MEDIUMBOLD.into())
                                .font_features(fonts::tabular())
                                .text_color(ink::secondary())
                                .child(format::percent(percent as u64)),
                        )
                    }),
            )
            .child(
                ProgressBar::new(fraction)
                    .tone(tone)
                    .id(("progress", self.percent.unwrap_or_default() as usize)),
            )
            .when(!self.stats.is_empty(), |this| {
                this.child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap_x(px(16.))
                        .gap_y(px(4.))
                        .text_size(theme::text_size(text::SMALL))
                        .font_weight(weights::MEDIUM.into())
                        .text_color(ink::secondary())
                        .children(
                            self.stats
                                .into_iter()
                                .enumerate()
                                .map(|(index, fact)| fact.render_in_row(index)),
                        ),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_meter_turns_red_when_short_and_yellow_when_tight() {
        assert_eq!(meter_tone(2_400, 1_100), Tone::Danger);
        assert_eq!(meter_tone(950, 1_000), Tone::Warning);
        assert_eq!(meter_tone(500, 1_000), Tone::Info);
        assert_eq!(meter_tone(0, 0), Tone::Info);
        assert_eq!(clamp01(f32::NAN), 0.);
        assert_eq!(clamp01(3.), 1.);
    }
}
