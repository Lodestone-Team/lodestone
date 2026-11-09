use crate::theme::px;
use std::rc::Rc;

use gpui::{
    App, Bounds, ElementId, IntoElement, MouseMoveEvent, PathBuilder, Pixels, Point, RenderOnce,
    SharedString, Window, canvas, div, point, prelude::*,
};

use crate::theme::{Color, colors, fonts, ink, radius, text};

type Label = Rc<dyn Fn(usize, f32) -> SharedString + 'static>;

/// What a chart remembers between frames: where it was drawn, and the sample under
/// the pointer.
#[derive(Default)]
struct Hover {
    bounds: Option<Bounds<Pixels>>,
    index: Option<usize>,
}

/// A line chart of evenly spaced samples, oldest first, such as the last minute of a
/// server's CPU load, after the dashboard's `PerformanceGraph`.
///
/// Missing samples (`None`) leave a gap that a dashed grey line bridges, as Chart.js
/// draws `spanGaps`. The area under the line is filled. The grid has a line every
/// [`LineChart::grid_every`] samples and four bands up to `max`; the edges are
/// brighter. Hovering shows a sample's [`LineChart::hover_label`].
#[derive(IntoElement)]
pub struct LineChart {
    id: ElementId,
    samples: Vec<Option<f32>>,
    max: f32,
    color: Color,
    fill: Color,
    height: Pixels,
    grid_every: usize,
    /// Under the left and right ends of the x axis.
    x_labels: Option<(SharedString, SharedString)>,
    /// Beside the top and bottom of the y axis.
    y_labels: Option<(SharedString, SharedString)>,
    hover_label: Option<Label>,
    accessible_label: SharedString,
}

impl LineChart {
    pub fn new(id: impl Into<ElementId>, samples: Vec<Option<f32>>, max: f32) -> Self {
        Self {
            id: id.into(),
            samples,
            max: if max > 0.0 { max } else { 1.0 },
            color: colors::GREEN_200,
            fill: colors::GREEN_FADED.opacity(25),
            height: px(160.),
            grid_every: 5,
            x_labels: None,
            y_labels: None,
            hover_label: None,
            accessible_label: SharedString::default(),
        }
    }

    /// The line's colour and the fill under it.
    pub fn colors(mut self, line: Color, fill: Color) -> Self {
        self.color = line;
        self.fill = fill;
        self
    }

    pub fn height(mut self, height: Pixels) -> Self {
        self.height = height;
        self
    }

    /// A vertical grid line every `samples` samples.
    pub fn grid_every(mut self, samples: usize) -> Self {
        self.grid_every = samples.max(1);
        self
    }

    pub fn x_labels(
        mut self,
        left: impl Into<SharedString>,
        right: impl Into<SharedString>,
    ) -> Self {
        self.x_labels = Some((left.into(), right.into()));
        self
    }

    pub fn y_labels(
        mut self,
        top: impl Into<SharedString>,
        bottom: impl Into<SharedString>,
    ) -> Self {
        self.y_labels = Some((top.into(), bottom.into()));
        self
    }

    /// The text shown for the sample under the pointer, from how many samples before
    /// the newest it is and its value.
    pub fn hover_label(mut self, label: impl Fn(usize, f32) -> SharedString + 'static) -> Self {
        self.hover_label = Some(Rc::new(label));
        self
    }

    /// What the chart shows, for assistive technology.
    pub fn accessible_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessible_label = label.into();
        self
    }
}

/// The x of sample `i` of `count` across `bounds`.
fn x_of(bounds: &Bounds<Pixels>, i: usize, count: usize) -> Pixels {
    let step = if count > 1 {
        bounds.size.width / (count - 1) as f32
    } else {
        px(0.)
    };
    bounds.origin.x + step * i as f32
}

/// The y of `value` (0 to `max`) in `bounds`, clamped inside.
fn y_of(bounds: &Bounds<Pixels>, value: f32, max: f32) -> Pixels {
    let fraction = (value / max).clamp(0.0, 1.0);
    bounds.origin.y + bounds.size.height * (1.0 - fraction)
}

/// The sample nearest to `x` of `count` across `bounds`.
pub fn sample_at(x: f32, left: f32, width: f32, count: usize) -> Option<usize> {
    if count == 0 || width <= 0.0 {
        return None;
    }
    let fraction = ((x - left) / width).clamp(0.0, 1.0);
    Some(((fraction * (count - 1) as f32).round() as usize).min(count - 1))
}

/// The runs of consecutive samples that have values, as index ranges.
pub fn runs(samples: &[Option<f32>]) -> Vec<std::ops::Range<usize>> {
    let mut runs = Vec::new();
    let mut start = None;
    for (i, sample) in samples.iter().enumerate() {
        match (sample.is_some(), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                runs.push(s..i);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        runs.push(s..samples.len());
    }
    runs
}

fn paint_line(
    window: &mut Window,
    from: Point<Pixels>,
    to: Point<Pixels>,
    width: f32,
    color: Color,
    dashed: bool,
) {
    let mut builder = PathBuilder::stroke(px(width));
    if dashed {
        builder = builder.dash_array(&[px(6.), px(6.)]);
    }
    builder.move_to(from);
    builder.line_to(to);
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

impl RenderOnce for LineChart {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let hover = window.use_keyed_state((self.id.clone(), "hover"), cx, |_, _| Hover::default());
        let count = self.samples.len();
        let hovered = hover
            .read(cx)
            .index
            .filter(|i| *i < count)
            .and_then(|i| self.samples[i].map(|value| (i, value)));
        let tooltip = hovered.and_then(|(i, value)| {
            let label = self.hover_label.as_ref()?(count - 1 - i, value);
            Some((i, label))
        });
        let samples = Rc::new(self.samples);
        let (max, line, fill, grid_every) = (self.max, self.color, self.fill, self.grid_every);
        let label_color = ink::muted();

        let plot = {
            let samples = samples.clone();
            let hover = hover.clone();
            canvas(
                |bounds, _, _| bounds,
                move |_, bounds, window, cx| {
                    hover.update(cx, |hover, _| hover.bounds = Some(bounds));
                    let edge = colors::GRAY_500;
                    let grid = colors::GRAY_600;
                    // the grid: vertical lines, then four bands
                    for i in (0..count).filter(|i| i % grid_every == 0 || *i == count - 1) {
                        let x = x_of(&bounds, i, count);
                        let is_edge = i == 0 || i == count - 1;
                        paint_line(
                            window,
                            point(x, bounds.top()),
                            point(x, bounds.bottom()),
                            if is_edge { 2.0 } else { 1.0 },
                            if is_edge { edge } else { grid },
                            false,
                        );
                    }
                    for band in 0..=4 {
                        let y = bounds.top() + bounds.size.height * (band as f32 / 4.0);
                        let is_edge = band == 0 || band == 4;
                        paint_line(
                            window,
                            point(bounds.left(), y),
                            point(bounds.right(), y),
                            if is_edge { 2.0 } else { 1.0 },
                            if is_edge { edge } else { grid },
                            false,
                        );
                    }
                    let point_of = |i: usize, value: f32| {
                        point(x_of(&bounds, i, count), y_of(&bounds, value, max))
                    };
                    let runs = runs(&samples);
                    // gaps: dashed, from the end of one run to the start of the next
                    for pair in runs.windows(2) {
                        let (a, b) = (pair[0].end - 1, pair[1].start);
                        let (Some(va), Some(vb)) = (samples[a], samples[b]) else {
                            continue;
                        };
                        let (pa, pb) = (point_of(a, va), point_of(b, vb));
                        let mut area = PathBuilder::fill();
                        area.move_to(point(pa.x, bounds.bottom()));
                        area.line_to(pa);
                        area.line_to(pb);
                        area.line_to(point(pb.x, bounds.bottom()));
                        area.close();
                        if let Ok(path) = area.build() {
                            window.paint_path(path, colors::GRAY_FADED.opacity(15));
                        }
                        paint_line(window, pa, pb, 2.0, colors::GRAY_500, true);
                    }
                    for run in runs {
                        let points: Vec<_> = run
                            .clone()
                            .filter_map(|i| samples[i].map(|v| point_of(i, v)))
                            .collect();
                        let (Some(first), Some(last)) = (points.first(), points.last()) else {
                            continue;
                        };
                        if points.len() > 1 {
                            let mut area = PathBuilder::fill();
                            area.move_to(point(first.x, bounds.bottom()));
                            for p in &points {
                                area.line_to(*p);
                            }
                            area.line_to(point(last.x, bounds.bottom()));
                            area.close();
                            if let Ok(path) = area.build() {
                                window.paint_path(path, fill);
                            }
                            let mut stroke = PathBuilder::stroke(px(2.));
                            stroke.move_to(*first);
                            for p in &points[1..] {
                                stroke.line_to(*p);
                            }
                            if let Ok(path) = stroke.build() {
                                window.paint_path(path, line);
                            }
                        } else {
                            // one sample alone: a short dash so it shows
                            paint_line(
                                window,
                                point(first.x - px(2.), first.y),
                                point(first.x + px(2.), first.y),
                                2.0,
                                line,
                                false,
                            );
                        }
                    }
                },
            )
            .size_full()
        };

        let marker = tooltip.as_ref().and_then(|(i, _)| {
            let bounds = hover.read(cx).bounds?;
            let x = x_of(&bounds, *i, count) - bounds.left();
            Some(x)
        });
        let width = hover.read(cx).bounds.map(|b| b.size.width);
        let on_move = {
            let hover = hover.clone();
            move |event: &MouseMoveEvent, _: &mut Window, cx: &mut App| {
                hover.update(cx, |hover, cx| {
                    let index = hover.bounds.and_then(|bounds| {
                        bounds.contains(&event.position).then(|| {
                            sample_at(
                                f32::from(event.position.x),
                                f32::from(bounds.left()),
                                f32::from(bounds.size.width),
                                count,
                            )
                        })?
                    });
                    if hover.index != index {
                        hover.index = index;
                        cx.notify();
                    }
                });
            }
        };
        let leave = hover.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .w_full()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.))
                    .when_some(self.y_labels.clone(), |this, (top, bottom)| {
                        this.child(
                            div()
                                .flex()
                                .flex_col()
                                .justify_between()
                                .h(self.height)
                                .font_family(fonts::mono())
                                .text_size(text::SMALL)
                                .text_color(label_color)
                                .child(top)
                                .child(bottom),
                        )
                    })
                    .child(
                        div()
                            .id(self.id.clone())
                            .relative()
                            .flex_1()
                            .min_w_0()
                            .h(self.height)
                            .on_mouse_move(on_move)
                            .on_hover(move |hovered, _, cx| {
                                if !hovered {
                                    leave.update(cx, |hover, cx| {
                                        hover.index = None;
                                        cx.notify();
                                    });
                                }
                            })
                            .child(plot)
                            .when_some(marker.zip(tooltip), |this, (x, (_, label))| {
                                // above the pointer's sample, kept inside the chart
                                let box_width = px(150.);
                                let left = width
                                    .map(|w| {
                                        (x - box_width / 2.)
                                            .clamp(px(0.), (w - box_width).max(px(0.)))
                                    })
                                    .unwrap_or(px(0.));
                                this.child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .bottom_0()
                                        .left(x)
                                        .w(px(1.))
                                        .bg(colors::GRAY_400),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .top(px(4.))
                                        .left(left)
                                        .w(box_width)
                                        .flex()
                                        .justify_center()
                                        .child(
                                            div()
                                                .px(px(8.))
                                                .py(px(4.))
                                                .rounded(radius::MD)
                                                .bg(colors::GRAY_850)
                                                .border_1()
                                                .border_color(ink::border())
                                                .font_family(fonts::mono())
                                                .text_size(text::SMALL)
                                                .text_color(ink::text())
                                                .child(label),
                                        ),
                                )
                            }),
                    ),
            )
            .when_some(self.x_labels, |this, (left, right)| {
                this.child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .when(self.y_labels.is_some(), |this| this.pl(px(40.)))
                        .font_family(fonts::mono())
                        .text_size(text::SMALL)
                        .text_color(label_color)
                        .child(left)
                        .child(right),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaps_split_the_runs() {
        let samples = [None, Some(1.0), Some(2.0), None, None, Some(3.0)];
        assert_eq!(runs(&samples), vec![1..3, 5..6]);
        assert!(runs(&[None, None]).is_empty());
        assert_eq!(runs(&[Some(1.0)]), vec![0..1]);
    }

    #[test]
    fn the_pointer_picks_the_nearest_sample() {
        assert_eq!(sample_at(0.0, 0.0, 100.0, 11), Some(0));
        assert_eq!(sample_at(54.0, 0.0, 100.0, 11), Some(5));
        assert_eq!(sample_at(500.0, 0.0, 100.0, 11), Some(10));
        assert_eq!(sample_at(10.0, 0.0, 100.0, 0), None);
    }
}
