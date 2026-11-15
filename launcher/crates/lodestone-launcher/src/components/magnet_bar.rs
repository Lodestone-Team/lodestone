//! The Play button dissolving into iron filings while a game is prepared, like the
//! website's hero, with the button as the magnet.
//!
//! Pressing Play melts the button: its colour drains as its shape stretches wider
//! and thinner like a drop of liquid, its outline breaks into short strokes that
//! scatter and fade, and the filings peel off those edges and spring out across the
//! bar into line, so no outline is left. Then they trace the field around the
//! step's text, running along its outline and fanning out from its ends, and reach
//! further along the bar as the launch goes on. The pointer is a second magnet that
//! bends them, with a dark spot and a lit ring, as on the website.
//!
//! When the game opens (or the launch is cancelled) the button wells back up from its
//! middle like a drop of liquid, slowing until it comes to rest in its shape, and the
//! swelling sends a wave out across the bar that pushes the filings away, like beads
//! of water off glass: pushed out, slowing, beading up into drops and fading. Then
//! the real button shows over the drop.
//!
//! It takes the skin's colours ([`MagnetLook`]): the button melts in the skin's
//! (or the instance's) Play colour, gradient, shape and glow, and the filings take
//! the skin's progress colours, made to read on the bar the skin paints.
//!
//! Drawn at about 30 frames a second while it shows; with reduced motion, still,
//! without the melting, the springing or the pointer.

use std::cell::Cell;
use std::f32::consts::PI;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, Bounds, ElementId, IntoElement, PathBuilder, Pixels, Point, RenderOnce, Rgba,
    Task, Window, canvas, div, point, prelude::*, size,
};
use launcher_core::ui_skin::{
    Rgb, SurfaceId,
    readable::{self, Backdrop},
};

use crate::theme::{Color, colors, px};

/// The gem's gradient, green to violet, as on the website: the filings' colours
/// without a skin.
pub const GEM: [Rgb; 5] = [
    Rgb(0x4FE0A5),
    Rgb(0x4AC4C8),
    Rgb(0x6AAEEE),
    Rgb(0x7D9EF2),
    Rgb(0x9A8FF5),
];
/// How strongly a filing at full brightness stands out from the bar.
const CONTRAST: f32 = 3.0;
/// Filings are drawn in batches by colour band and brightness.
const LEVELS: usize = 8;
/// Pixels between filings.
const SPACING: f32 = 14.0;
/// How far out from the text the filings show before the launch has come any way.
const HALO: f32 = 26.0;
/// How long the button takes to melt away.
const MELT: f32 = 0.55;
/// How long one stroke of the outline takes to break off and fade.
const SHATTER: f32 = 0.28;
/// How long a filing takes to spring from the button's edge to its place.
const SPRING: f32 = 0.8;
/// After a launch, in seconds since it ended: the wave out from the swelling button
/// leaves its edge at `WAVE_AT` and reaches the furthest filings `WAVE` later, slowing
/// as it spreads.
const WAVE_AT: f32 = 0.1;
const WAVE: f32 = 0.6;
/// How long a filing it hits takes to drift out and fade.
const FLY: f32 = 0.6;
/// How long the button takes to well back up and come to rest.
const GROW: f32 = 0.95;
/// The real button shows over the drop as it comes to rest.
const REVEAL: (f32, f32) = (0.7, 1.05);
/// When it's all over.
const RETURN: f32 = 1.35;
/// No frame for this long and the window was away: minimized while the game ran, say.
const AWAY: Duration = Duration::from_millis(250);
/// Back from being away, the filings wait this long before drawing back in, for the
/// window to finish coming back (Windows' restore, macOS's genie).
const RETURNING: Duration = Duration::from_millis(300);
/// The pointer's pull halves at about this distance; filings closest to it fade, and
/// a ring around it brightens.
const POINTER_REACH: f32 = 80.0;
const FADE_REACH: f32 = 26.0;
const RING_RADIUS: f32 = 48.0;
const RING_WIDTH: f32 = 18.0;
/// Between frames.
const FRAME: Duration = Duration::from_millis(33);

/// Where the Play button is, in the window: [`PlayButton::anchor`] writes it as it's
/// laid out, and [`MagnetBar::anchor`] reads it, so the button melts from its own
/// shape, without the line under it ("Play without joining").
///
/// [`PlayButton::anchor`]: super::PlayButton::anchor
pub type MagnetAnchor = Rc<Cell<Option<Bounds<Pixels>>>>;

/// The Play bar's content (`child`, the Play button) over its filings. Fills the
/// bar; the filings reach `bleed` past it, to the bar's edges.
#[derive(IntoElement)]
pub struct MagnetBar {
    id: ElementId,
    /// How far the launch has come, 0 to 1, while one runs.
    launch: Option<f32>,
    look: MagnetLook,
    bleed: (f32, f32),
    anchor: Option<MagnetAnchor>,
    /// The button is drawn dimmed (disabled) right now.
    dimmed: bool,
    child: AnyElement,
}

impl MagnetBar {
    pub fn new(id: impl Into<ElementId>, child: impl IntoElement) -> Self {
        Self {
            id: id.into(),
            launch: None,
            look: MagnetLook::default(),
            bleed: (0.0, 0.0),
            anchor: None,
            dimmed: false,
            child: child.into_any_element(),
        }
    }

    pub fn launch(mut self, launch: Option<f32>) -> Self {
        self.launch = launch.map(|l| l.clamp(0.0, 1.0));
        self
    }

    /// The skin's colours, for the button to melt from and the filings.
    pub fn look(mut self, look: MagnetLook) -> Self {
        self.look = look;
        self
    }

    /// Where the button is, as the button itself writes it. Without one, the whole
    /// of `child` is taken for the button.
    pub fn anchor(mut self, anchor: MagnetAnchor) -> Self {
        self.anchor = Some(anchor);
        self
    }

    /// Whether the button is drawn dimmed right now (disabled, as while a server's
    /// mods are checked before Play joins it), so it melts from that look.
    pub fn dimmed(mut self, dimmed: bool) -> Self {
        self.dimmed = dimmed;
        self
    }

    /// How far past this element, across and down, the filings go.
    pub fn bleed(mut self, x: f32, y: f32) -> Self {
        self.bleed = (x, y);
        self
    }
}

/// How the melting button and the filings look: the skin's colours, or Lodestone's.
#[derive(Clone, Debug, PartialEq)]
pub struct MagnetLook {
    /// The filings' colours, from the bar's left to its right.
    pub palette: Vec<Rgb>,
    /// What the bar shows behind them, so they're made to read on it.
    pub backdrop: Backdrop,
    /// The Play button it melts from: its colour, the one it fades to, and its
    /// outline's.
    pub fill: Color,
    pub gradient: Option<Color>,
    pub edge: Color,
    /// Its corners' radius in pixels; `None` for a pill.
    pub corner: Option<f32>,
    /// A soft glow in its colour.
    pub glow: bool,
}

impl Default for MagnetLook {
    fn default() -> Self {
        Self {
            palette: GEM.to_vec(),
            backdrop: Backdrop::solid(SurfaceId::PlayBar.base()),
            fill: colors::GREEN_400,
            gradient: None,
            edge: colors::GREEN_300,
            corner: Some(8.0),
            glow: false,
        }
    }
}

impl MagnetLook {
    /// A few shades of `color`, for a skin that gives the filings just one.
    pub fn shades(color: Rgb) -> Vec<Rgb> {
        vec![
            color.mix(Rgb::WHITE, 0.3),
            color.mix(Rgb::WHITE, 0.15),
            color,
            color.mix(Rgb::BLACK, 0.12),
        ]
    }

    /// The colours to draw with on this look's bar.
    fn inks(&self) -> Inks {
        let backdrop = self.backdrop;
        let bands = match self.palette.is_empty() {
            true => GEM.to_vec(),
            false => self.palette.clone(),
        };
        Inks {
            bands: bands
                .into_iter()
                .map(|c| channels(readable::suggest(c, backdrop, CONTRAST)))
                .collect(),
            // weak filings: a little of the bar's text colour over the bar
            weak: channels(backdrop.mean.mix(readable::best_ink(backdrop), 0.4)),
        }
    }

    /// The look of the button drawn disabled, as the Play button dims it.
    fn dimmed(&self) -> Self {
        Self {
            fill: self.fill.opacity(25),
            gradient: None,
            edge: self.fill.opacity(30),
            glow: false,
            ..self.clone()
        }
    }

    fn corner_for(&self, half_height: f32) -> f32 {
        self.corner.unwrap_or(half_height)
    }
}

/// The filings' colours as drawn: each band's, and the weak one they brighten from.
struct Inks {
    bands: Vec<(u8, u8, u8)>,
    weak: (u8, u8, u8),
}

fn channels(c: Rgb) -> (u8, u8, u8) {
    ((c.0 >> 16) as u8, (c.0 >> 8) as u8, c.0 as u8)
}

/// A rounded box: its middle, half its size, and its corners' radius.
#[derive(Clone, Copy, Debug, PartialEq)]
struct RoundedBox {
    cx: f32,
    cy: f32,
    hw: f32,
    hh: f32,
    corner: f32,
}

impl RoundedBox {
    fn corner(&self) -> f32 {
        self.corner.min(self.hw).min(self.hh).max(0.0)
    }

    /// How far `(x, y)` is from the outline (negative inside), the outward direction
    /// there, and the angle along the outline.
    fn measure(&self, x: f32, y: f32) -> (f32, (f32, f32), f32) {
        let (x, y) = (x - self.cx, y - self.cy);
        let corner = self.corner();
        let (qx, qy) = (x.abs() - self.hw + corner, y.abs() - self.hh + corner);
        let outside = qx.max(0.0).hypot(qy.max(0.0));
        let d = outside + qx.max(qy).min(0.0) - corner;
        let (gx, gy) = if qx > 0.0 && qy > 0.0 {
            (qx / outside, qy / outside)
        } else if qx > qy {
            (1.0, 0.0)
        } else {
            (0.0, 1.0)
        };
        let (gx, gy) = (gx * sign(x), gy * sign(y));
        (d, (gx, gy), gy.atan2(gx) + PI / 2.0)
    }

    /// The point on the outline nearest `(x, y)`.
    fn nearest(&self, x: f32, y: f32) -> (f32, f32) {
        let (d, (gx, gy), _) = self.measure(x, y);
        (x - gx * d, y - gy * d)
    }

    fn perimeter(&self) -> f32 {
        let r = self.corner();
        4.0 * (self.hw - r + self.hh - r) + 2.0 * PI * r
    }

    /// The point `s` (0 to 1) of the way round the outline, clockwise from the top
    /// left, and the outward direction there.
    fn around(&self, s: f32) -> ((f32, f32), (f32, f32)) {
        let r = self.corner();
        let (w, h) = (2.0 * (self.hw - r), 2.0 * (self.hh - r));
        let arc = PI * r / 2.0;
        let mut at = s.rem_euclid(1.0) * self.perimeter();
        let (left, top) = (self.cx - self.hw, self.cy - self.hh);
        let (right, bottom) = (self.cx + self.hw, self.cy + self.hh);
        // the four sides, each followed by its corner
        let sides = [
            (
                (left + r, top),
                (1.0, 0.0),
                (0.0, -1.0),
                w,
                (right - r, top + r),
                -PI / 2.0,
            ),
            (
                (right, top + r),
                (0.0, 1.0),
                (1.0, 0.0),
                h,
                (right - r, bottom - r),
                0.0,
            ),
            (
                (right - r, bottom),
                (-1.0, 0.0),
                (0.0, 1.0),
                w,
                (left + r, bottom - r),
                PI / 2.0,
            ),
            (
                (left, bottom - r),
                (0.0, -1.0),
                (-1.0, 0.0),
                h,
                (left + r, top + r),
                PI,
            ),
        ];
        for (start, dir, normal, len, centre, from) in sides {
            if at <= len {
                return ((start.0 + dir.0 * at, start.1 + dir.1 * at), normal);
            }
            at -= len;
            if at <= arc {
                let a = from + at / r.max(1e-3);
                let (c, s) = (a.cos(), a.sin());
                return ((centre.0 + c * r, centre.1 + s * r), (c, s));
            }
            at -= arc;
        }
        ((left + r, top), (0.0, -1.0))
    }
}

/// The melting button at `k` (0 to 1 of [`MELT`]), gliding from where it was to
/// `toward`, where the step's text settles (lower, once "Play without joining" under
/// a server's Play has gone).
fn melted_toward(from: RoundedBox, toward: (f32, f32), k: f32) -> RoundedBox {
    let mut shape = melted(from, k);
    let e = ease_out_cubic(k);
    shape.cx += (toward.0 - from.cx) * e;
    shape.cy += (toward.1 - from.cy) * e;
    shape
}

/// The melting button at `k` (0 to 1 of [`MELT`]): wider, thinner and rounder.
fn melted(from: RoundedBox, k: f32) -> RoundedBox {
    let k = k.clamp(0.0, 1.0);
    let grow = ease_out_back((k * 1.25).min(1.0));
    let hh = from.hh * (1.0 - 0.45 * k);
    RoundedBox {
        cx: from.cx,
        cy: from.cy,
        hw: from.hw * (1.0 + 0.5 * grow),
        hh,
        // rounder as it thins, but a square (pixel) button stays square
        corner: if from.corner <= 0.0 {
            0.0
        } else {
            from.corner + (hh - from.corner) * (k * 2.0).min(1.0)
        },
    }
}

/// The button welling back up `t` seconds after a launch ended: a small round drop in
/// its middle swelling into its shape, rounding out first and then spreading wide,
/// slowing all the way until it comes to rest in place, never past it.
fn welled(to: RoundedBox, t: f32) -> RoundedBox {
    let k = t / GROW;
    let start = to.hh * 0.35;
    let hh = start + (to.hh - start) * come_to_rest(k * 1.25);
    RoundedBox {
        hw: start + (to.hw - start) * come_to_rest(k),
        hh,
        // round while it swells, then its own corners; a square button stays square
        corner: if to.corner <= 0.0 {
            0.0
        } else {
            hh + (to.corner - hh) * smoothstep(0.25, 0.8, k)
        },
        ..to
    }
}

/// 0 to 1, quick at first and slowing ever more gently into 1, like something
/// coming to rest.
fn come_to_rest(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(4)
}

struct Filing {
    x: f32,
    y: f32,
    angle: f32,
    phase: f32,
    /// How far along the bar it is, 0 to 1: its colour.
    along: f32,
    /// Where on the melting button's edge it springs from, and when.
    from: (f32, f32),
    born: f32,
}

/// The button as it was when Play was pressed.
#[derive(Clone)]
struct Pressed {
    at: Instant,
    button: Bounds<Pixels>,
    look: MagnetLook,
}

struct Field {
    filings: Vec<Filing>,
    laid_out: (f32, f32),
    start: Instant,
    last: Option<Instant>,
    active: bool,
    pressed: Option<Pressed>,
    /// The filings still have to find their places on the edge they spring from.
    emerge: bool,
    /// When the launch ended: the filings draw back in, and the button condenses
    /// back in this look.
    released: Option<Instant>,
    returning: Option<MagnetLook>,
    /// Fades in with a launch and out after it, 0 to 1.
    presence: f32,
    /// How far the launch has come, eased, and where it's heading.
    reach: f32,
    reach_to: f32,
    /// The pointer, eased, and how strongly it pulls.
    pointer: (f32, f32),
    pointer_strength: f32,
    /// The button, in the window, from the last layout.
    button: Option<Bounds<Pixels>>,
    /// Whether it has been drawn before: a launch it finds already running shows
    /// settled, without the melting.
    seen: bool,
    /// Draws the next frame settled.
    snap: bool,
    /// The button was drawn dimmed when last seen before a launch.
    dimmed: bool,
    frame: Option<Task<()>>,
}

impl Field {
    fn new() -> Self {
        Self {
            filings: Vec::new(),
            laid_out: (0.0, 0.0),
            start: Instant::now(),
            last: None,
            active: false,
            pressed: None,
            emerge: false,
            released: None,
            returning: None,
            presence: 0.0,
            reach: 0.0,
            reach_to: 0.0,
            pointer: (0.0, 0.0),
            pointer_strength: 0.0,
            button: None,
            seen: false,
            snap: false,
            dimmed: false,
            frame: None,
        }
    }

    /// Starts or ends a launch. Whether there is anything to draw.
    fn set_launch(
        &mut self,
        launch: Option<f32>,
        look: &MagnetLook,
        dimmed: bool,
        still: bool,
    ) -> bool {
        match launch {
            Some(reach) => {
                if !self.active {
                    self.active = true;
                    self.released = None;
                    self.presence = 1.0;
                    match self.button.filter(|_| self.seen && !still) {
                        Some(button) => {
                            self.reach = 0.0;
                            self.pressed = Some(Pressed {
                                at: Instant::now(),
                                button,
                                look: if self.dimmed {
                                    look.dimmed()
                                } else {
                                    look.clone()
                                },
                            });
                            self.emerge = true;
                        }
                        None => {
                            self.reach = reach;
                            self.pressed = None;
                            self.snap = true;
                        }
                    }
                }
                self.reach_to = reach;
            }
            None => {
                if self.active {
                    self.active = false;
                    self.released = (!still).then(Instant::now);
                    // into the button as it is now: Play again, or dimmed
                    self.returning = Some(if dimmed { look.dimmed() } else { look.clone() });
                }
                self.dimmed = dimmed;
            }
        }
        if still {
            self.presence = if self.active { 1.0 } else { 0.0 };
        }
        self.seen = true;
        self.active || self.presence > 0.005
    }

    fn lay_out(&mut self, width: f32, height: f32) {
        if self.laid_out == (width, height) {
            return;
        }
        self.laid_out = (width, height);
        let cols = (width / SPACING).ceil() as usize + 1;
        let rows = (height / SPACING).ceil() as usize + 1;
        let ox = (width - (cols - 1) as f32 * SPACING) / 2.0;
        let oy = (height - (rows - 1) as f32 * SPACING) / 2.0;
        self.filings.clear();
        for j in 0..rows {
            for i in 0..cols {
                let n = (j * cols + i) as u32;
                let (x, y) = (ox + i as f32 * SPACING, oy + j as f32 * SPACING);
                self.filings.push(Filing {
                    x,
                    y,
                    angle: noise(n, 1) * PI,
                    phase: noise(n, 2) * PI * 2.0,
                    // the gem's colours run from the left to the right, tilted
                    along: ((x + (y - height / 2.0) * 0.6) / width).clamp(0.0, 0.999),
                    from: (x, y),
                    born: 0.0,
                });
            }
        }
    }

    /// A minimized window draws no frames, so the launcher, minimized while the game
    /// ran, misses the filings drawing back in when it ends: the launch ends as the
    /// window is brought back, and by the time it's on screen they nearly have. Back
    /// from being away, they draw back in from the start, once the window is back.
    fn hold_while_away(&mut self, now: Instant) {
        let (Some(last), Some(released)) = (self.last, self.released) else {
            return;
        };
        let away = now.saturating_duration_since(last) > AWAY;
        // not once it's over: nothing is drawn then, so every frame looks like a return
        let playing = last.saturating_duration_since(released).as_secs_f32() < RETURN;
        if away && playing {
            self.released = Some(now + RETURNING);
        }
    }

    /// Seconds since the launch ended, while it plays back (negative while it waits
    /// for the window to come back).
    fn returned(&self, now: Instant) -> Option<f32> {
        let released = self.released.filter(|_| !self.active)?;
        Some(match now.checked_duration_since(released) {
            Some(since) => since.as_secs_f32(),
            None => -released.duration_since(now).as_secs_f32(),
        })
    }

    /// How much of the real button shows: none while it condenses back after a
    /// launch, until it's back in shape.
    fn button_shown(&self, now: Instant) -> f32 {
        self.returned(now)
            .map_or(1.0, |since| smoothstep(REVEAL.0, REVEAL.1, since))
    }

    /// Moves everything on and paints it. `bounds` is the filings' area in the
    /// window; `mouse` the pointer, in the window.
    fn paint(
        &mut self,
        bounds: Bounds<Pixels>,
        mouse: Point<Pixels>,
        look: &MagnetLook,
        inks: &Inks,
        still: bool,
        window: &mut Window,
    ) {
        let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        if width < 1.0 || height < 1.0 {
            return;
        }
        self.lay_out(width, height);
        let now = Instant::now();
        self.hold_while_away(now);
        let dt = self
            .last
            .map_or(0.0, |last| now.duration_since(last).as_secs_f32())
            .min(0.1);
        self.last = Some(now);
        let settle = still || std::mem::take(&mut self.snap);
        let ease = |rate: f32| {
            if settle {
                1.0
            } else {
                1.0 - (-dt * rate).exp()
            }
        };
        let t = if still {
            0.0
        } else {
            now.duration_since(self.start).as_secs_f32()
        };
        let origin = bounds.origin;
        let local = |b: Bounds<Pixels>, corner: f32| RoundedBox {
            cx: f32::from(b.origin.x - origin.x) + f32::from(b.size.width) / 2.0,
            cy: f32::from(b.origin.y - origin.y) + f32::from(b.size.height) / 2.0,
            hw: f32::from(b.size.width) / 2.0,
            hh: f32::from(b.size.height) / 2.0,
            corner,
        };

        // Playing back after the launch, until the button is whole again.
        let returned = self.returned(now);
        match returned {
            Some(since) => self.presence = if since < RETURN { 1.0 } else { 0.0 },
            None if !self.active => self.presence = 0.0,
            None => {}
        }
        self.reach += (self.reach_to - self.reach) * ease(1.6);
        if self.presence <= 0.005 {
            self.pressed = None;
            return;
        }

        // The text in the middle: the magnet, a rounded box with nothing under it.
        let magnet = match self.button {
            Some(b) => local(b, look.corner_for(f32::from(b.size.height) / 2.0)),
            None => RoundedBox {
                cx: width / 2.0,
                cy: height / 2.0,
                hw: 190.0,
                hh: 30.0,
                corner: look.corner_for(30.0),
            },
        };
        // how far out the filings show: a halo at first, the bar's ends at the end
        let furthest = (magnet.cx - magnet.hw)
            .max(width - magnet.cx - magnet.hw)
            .max(HALO);
        let reach = HALO + self.reach * (furthest - HALO);

        // The melting button, and where each filing springs from on its edge.
        let pressed = self.pressed.clone().filter(|_| self.active);
        let since = pressed
            .as_ref()
            .map_or(f32::MAX, |p| now.duration_since(p.at).as_secs_f32());
        let from = pressed.as_ref().map(|p| {
            let corner = p.look.corner_for(f32::from(p.button.size.height) / 2.0);
            local(p.button, corner)
        });
        if let Some(from) = from.filter(|_| std::mem::take(&mut self.emerge)) {
            for (n, f) in self.filings.iter_mut().enumerate() {
                let (d, _, tangent) = from.measure(f.x, f.y);
                // nearer ones first, as the button stretches out to meet them
                f.born = 0.03 + d.max(0.0) * 0.0011 + noise(n as u32, 3) * 0.1;
                let edge = melted_toward(from, (magnet.cx, magnet.cy), f.born / MELT);
                f.from = edge.nearest(f.x, f.y);
                f.angle = tangent;
            }
        }
        if let (Some(p), Some(from)) = (pressed, from)
            && since < MELT + SHATTER
        {
            paint_melting(window, origin, from, (magnet.cx, magnet.cy), &p.look, since);
        }
        if let (Some(back), Some(returning)) = (returned, &self.returning)
            && back > 0.0
        {
            paint_welling(window, origin, magnet, returning, back, reach);
        }

        // The pointer: a second magnet pointing at the text, so the two pull together.
        let (mx, my) = (f32::from(mouse.x - origin.x), f32::from(mouse.y - origin.y));
        let inside = mx >= 0.0 && my >= 0.0 && mx <= width && my <= height;
        let pull = if inside && !still && self.active {
            1.0
        } else {
            0.0
        };
        if self.pointer_strength < 0.01 {
            self.pointer = (mx, my);
        } else {
            self.pointer.0 += (mx - self.pointer.0) * ease(14.0);
            self.pointer.1 += (my - self.pointer.1) * ease(14.0);
        }
        self.pointer_strength += (pull - self.pointer_strength) * ease(5.0);
        let (ptr_x, ptr_y) = self.pointer;
        let to_magnet = {
            let (x, y) = (magnet.cx - ptr_x, magnet.cy - ptr_y);
            let l = x.hypot(y).max(1.0);
            (x / l, y / l)
        };
        let pointer_spread = 2.0 * (POINTER_REACH / 1.18).powi(2);
        let fade_spread = 2.0 * (FADE_REACH / 1.18).powi(2);
        // the field breathes a little
        let sway = 0.25 * (t * 0.35).sin();

        let bands = inks.bands.len();
        let mut batches: Vec<Vec<f32>> = vec![Vec::new(); bands * LEVELS];
        for (n, f) in self.filings.iter_mut().enumerate() {
            let (d, normal, tangent) = magnet.measure(f.x, f.y);
            // Close to the text the lines run along its outline; further out they
            // fan out from its ends and arc between them, a bar magnet's field.
            let curve = bar_magnet(
                f.x - magnet.cx,
                f.y - magnet.cy,
                magnet.hw * 0.8,
                magnet.hh,
                sway,
            );
            let hug = (-d.max(0.0) / 26.0).exp();
            let mut target = curve + half_turn(tangent - curve) * hug;
            target += 0.05 * (t * 1.4 + f.phase).sin();

            let mut fade = 0.0;
            let mut ring = 0.0;
            if self.pointer_strength > 0.01 {
                let (qx, qy) = (f.x - ptr_x, f.y - ptr_y);
                let d2 = qx * qx + qy * qy;
                let near = self.pointer_strength * (-d2 / pointer_spread).exp();
                fade = self.pointer_strength * (-d2 / fade_spread).exp();
                let from_ring = d2.sqrt() - RING_RADIUS;
                ring = self.pointer_strength
                    * (-(from_ring * from_ring) / (2.0 * RING_WIDTH * RING_WIDTH)).exp();
                if near > 0.003 {
                    let field = dipole(qx / 60.0, qy / 60.0, to_magnet.0, to_magnet.1);
                    target += half_turn(field - target) * (near * 1.15).min(1.0);
                }
            }

            // Springing out from the melting button's edge, turning from along the
            // edge to the field as it goes; or drawing back in.
            let (mut x, mut y, mut born) = (f.x, f.y, 1.0);
            let springing = !settle && since < f.born + SPRING;
            if springing {
                let e = ((since - f.born) / SPRING).clamp(0.0, 1.0);
                if e <= 0.0 {
                    continue;
                }
                let s = ease_out_back(e);
                x = f.from.0 + (f.x - f.from.0) * s;
                y = f.from.1 + (f.y - f.from.1) * s;
                born = smoothstep(0.0, 0.2, e);
                f.angle += half_turn(target - f.angle) * ease_out_cubic(e) * 0.2;
            } else {
                f.angle += half_turn(target - f.angle) * ease(7.0);
            }
            // After the launch, pushed away by the button welling back up, like beads
            // of water flicked off glass.
            let mut angle = f.angle;
            let (mut stretch, mut charge, mut sink) = (1.0, 0.0, 1.0);
            if let Some(back) = returned {
                let n = n as u32;
                // the wave reaches it, slowing as it spreads
                let out = (d / reach.max(1.0)).clamp(0.0, 1.0);
                let hit = WAVE_AT + WAVE * out.powf(1.6) + 0.05 * noise(n, 5);
                let e = ((back - hit) / FLY).clamp(0.0, 1.0);
                if e >= 1.0 {
                    continue;
                }
                if e > 0.0 {
                    // flung out from the button, a little either way, and further the
                    // closer it was, then slowing as if through water
                    let spread = (noise(n, 6) - 0.5) * 0.7;
                    let (c, s) = (spread.cos(), spread.sin());
                    let (nx, ny) = (normal.0 * c - normal.1 * s, normal.0 * s + normal.1 * c);
                    let push = (22.0 + 38.0 * noise(n, 4)) * (1.4 - 0.7 * out);
                    let go = come_to_rest(e) * push;
                    x += nx * go;
                    y += ny * go;
                    angle += half_turn(ny.atan2(nx) - angle) * smoothstep(0.0, 0.3, e);
                    // drawn out by the shove, then beading up into a drop
                    stretch =
                        (1.0 + 0.8 * (1.0 - e).powi(3)) * (1.0 - 0.75 * smoothstep(0.15, 0.7, e));
                    // a flash as it's hit, then fading
                    charge = (1.0 - e).powi(4);
                    sink = (1.0 - e).powf(1.4);
                }
            }

            let lit = smoothstep(reach, reach - 30.0, d) * smoothstep(1.0, 14.0, d);
            let strength = (-d.max(0.0) / 70.0).exp();
            // just born, they are as bright as the edge they came from
            // ones springing past the reach show only on the way, fainter further out
            let flash = if springing {
                (1.0 - born * 0.6) * (-(d - reach).max(0.0) / 90.0).exp()
            } else {
                0.0
            };
            // after the launch, flashing as they're hit, then fading
            let shown = ((self.presence
                * born
                * ((lit * (0.22 + 0.7 * strength)).max(flash * 0.8)).min(1.0)
                * (1.0 - 0.95 * fade)
                + self.presence * 0.4 * ring * lit.max(0.25))
                * (1.0 + 1.3 * charge)
                * sink)
                .min(1.0);
            if shown < 0.04 {
                continue;
            }
            let level = ((shown * LEVELS as f32) as usize).min(LEVELS - 1);
            let band = ((f.along * bands as f32) as usize).min(bands - 1);
            let half = SPACING * (0.22 + 0.14 * strength) * stretch;
            let (ux, uy) = (angle.cos() * half, angle.sin() * half);
            batches[band * LEVELS + level].extend([x - ux, y - uy, x + ux, y + uy]);
        }

        let at = |x: f32, y: f32| point(origin.x + px(x), origin.y + px(y));
        for (index, batch) in batches.iter().enumerate() {
            if batch.is_empty() {
                continue;
            }
            let (band, level) = (index / LEVELS, index % LEVELS);
            let s = (level as f32 + 0.5) / LEVELS as f32;
            let to = inks.bands[band];
            let mut path = PathBuilder::stroke(px(1.5));
            for seg in batch.as_chunks::<4>().0 {
                path.move_to(at(seg[0], seg[1]));
                path.line_to(at(seg[2], seg[3]));
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, mix(inks.weak, to, s, 0.08 + 0.72 * s));
            }
        }
    }
}

/// The button melting away `since` seconds after Play was pressed: its colour
/// draining as it stretches, and its outline breaking into short strokes, end to end
/// at first so it still looks whole, that each tip outward a little, scatter and fade.
fn paint_melting(
    window: &mut Window,
    origin: Point<Pixels>,
    from: RoundedBox,
    toward: (f32, f32),
    look: &MagnetLook,
    since: f32,
) {
    let k = (since / MELT).clamp(0.0, 1.0);
    let shape = melted_toward(from, toward, k);
    let at = |x: f32, y: f32| point(origin.x + px(x), origin.y + px(y));
    let faded = |color: Color, by: f32| {
        let color = Rgba::from(color);
        Rgba {
            a: color.a * by,
            ..color
        }
    };
    let rect = |grow: f32| {
        Bounds::new(
            at(shape.cx - shape.hw - grow, shape.cy - shape.hh - grow),
            size(px((shape.hw + grow) * 2.0), px((shape.hh + grow) * 2.0)),
        )
    };
    let body = (1.0 - k).powf(2.0);
    // the skin's glow, spreading and fading with it
    if look.glow {
        for (grow, alpha) in [(14.0, 0.08), (8.0, 0.14), (3.0, 0.22)] {
            let grow = grow * (1.0 + k);
            window.paint_quad(
                gpui::fill(rect(grow), faded(look.fill, alpha * (1.0 - k)))
                    .corner_radii(px(shape.corner() + grow)),
            );
        }
    }
    if body > 0.01 {
        // its colour, or its gradient from top to bottom
        let paint: gpui::Background = match look.gradient {
            Some(to) => gpui::linear_gradient(
                180.,
                gpui::linear_color_stop(faded(look.fill, body), 0.),
                gpui::linear_color_stop(faded(to, body), 1.),
            ),
            None => faded(look.fill, body).into(),
        };
        window.paint_quad(gpui::fill(rect(0.0), paint).corner_radii(px(shape.corner())));
    }

    // The outline, in strokes about as long as the gaps between filings are wide.
    let edge = Rgba::from(look.edge);
    let count = (shape.perimeter() / 5.0).round().max(8.0) as usize;
    let step = 1.0 / count as f32;
    let mut batches: [Vec<f32>; 4] = Default::default();
    for i in 0..count {
        let n = i as u32;
        // each stroke breaks off at its own moment, the first ones at once
        let breaks = noise(n, 7) * MELT * 0.8;
        let b = ((since - breaks) / SHATTER).clamp(0.0, 1.0);
        if b >= 1.0 {
            continue;
        }
        let ((x, y), (nx, ny)) = shape.around((i as f32 + 0.5) * step);
        let drift = 2.0 + 10.0 * noise(n, 8);
        let (x, y) = (x + nx * drift * b, y + ny * drift * b);
        let tangent = ny.atan2(nx) + PI / 2.0;
        let twist = (noise(n, 9) - 0.5) * 2.4 * ease_out_cubic(b);
        let half = 2.6 * (1.0 - 0.5 * b);
        let (ux, uy) = (
            (tangent + twist).cos() * half,
            (tangent + twist).sin() * half,
        );
        let level = (((1.0 - b) * 4.0) as usize).min(3);
        batches[level].extend([x - ux, y - uy, x + ux, y + uy]);
    }
    paint_outline(window, origin, &batches, edge);
}

/// The button welling back up `since` seconds after the launch ended, into its shape
/// (`to`, where it is now): a drop swelling from its middle with a bright rim, like
/// liquid glass, a faint wave going out from it across the bar as `reach` far as the
/// filings were, and the real button showing over it once it settles.
fn paint_welling(
    window: &mut Window,
    origin: Point<Pixels>,
    to: RoundedBox,
    look: &MagnetLook,
    since: f32,
    reach: f32,
) {
    let shape = welled(to, since);
    // The real button shows over it: under one that's `shown`, the drop keeps just
    // enough that the two together cover as the button alone does (all of it, under
    // an opaque button), so it neither dims nor doubles as they cross.
    let shown = smoothstep(REVEAL.0, REVEAL.1, since);
    if shown >= 1.0 {
        return;
    }
    let opacity = Rgba::from(look.fill).a;
    let keep = (1.0 - shown) / (1.0 - opacity * shown);
    let at = |x: f32, y: f32| point(origin.x + px(x), origin.y + px(y));
    let faded = |color: Color, by: f32| {
        let color = Rgba::from(color);
        Rgba {
            a: color.a * by,
            ..color
        }
    };
    let rect = |shape: RoundedBox, grow: f32| {
        Bounds::new(
            at(shape.cx - shape.hw - grow, shape.cy - shape.hh - grow),
            size(px((shape.hw + grow) * 2.0), px((shape.hh + grow) * 2.0)),
        )
    };

    // the wave pushing the filings away, slowing as it spreads
    let w = (since - WAVE_AT) / WAVE;
    if (0.0..1.0).contains(&w) {
        let grow = 2.0 + (reach - 2.0).max(0.0) * w.powf(1.0 / 1.6);
        window.paint_quad(
            gpui::fill(rect(to, grow), gpui::transparent_black())
                .corner_radii(px(to.corner() + grow))
                .border_widths(px(1.5))
                .border_color(faded(look.edge, 0.35 * (1.0 - w).powi(2))),
        );
    }

    let body = smoothstep(0.0, 0.12, since) * keep;
    if look.glow {
        for (grow, alpha) in [(14.0, 0.08), (8.0, 0.14), (3.0, 0.22)] {
            window.paint_quad(
                gpui::fill(rect(shape, grow), faded(look.fill, alpha * body))
                    .corner_radii(px(shape.corner() + grow)),
            );
        }
    }
    let paint: gpui::Background = match look.gradient {
        Some(to) => gpui::linear_gradient(
            180.,
            gpui::linear_color_stop(faded(look.fill, body), 0.),
            gpui::linear_color_stop(faded(to, body), 1.),
        ),
        None => faded(look.fill, body).into(),
    };
    // its rim catches the light, fading into the button's own outline
    window.paint_quad(
        gpui::fill(rect(shape, 0.0), paint)
            .corner_radii(px(shape.corner()))
            .border_widths(px(1.5))
            .border_color(faded(look.edge, body * (1.0 - 0.6 * shown))),
    );
}

/// The outline's strokes, in four batches from faintest to full `edge`.
fn paint_outline(window: &mut Window, origin: Point<Pixels>, batches: &[Vec<f32>; 4], edge: Rgba) {
    let at = |x: f32, y: f32| point(origin.x + px(x), origin.y + px(y));
    for (level, batch) in batches.iter().enumerate() {
        if batch.is_empty() {
            continue;
        }
        let mut path = PathBuilder::stroke(px(1.5));
        for seg in batch.as_chunks::<4>().0 {
            path.move_to(at(seg[0], seg[1]));
            path.line_to(at(seg[2], seg[3]));
        }
        if let Ok(path) = path.build() {
            let a = (level as f32 + 0.5) / 4.0;
            window.paint_path(
                path,
                Rgba {
                    a: edge.a * a,
                    ..edge
                },
            );
        }
    }
}

/// The angle of a bar magnet's field at `(x, y)` from its middle: a pole at each
/// end, `pole` from the middle, softened by `soft` so it stays smooth close up.
/// `sway` tips the poles a little up and down.
fn bar_magnet(x: f32, y: f32, pole: f32, soft: f32, sway: f32) -> f32 {
    let lift = sway * soft * 0.6;
    let mut field = (0.0, 0.0);
    for (px, py, sign) in [(-pole, lift, 1.0), (pole, -lift, -1.0)] {
        let (rx, ry) = (x - px, y - py);
        let r2 = rx * rx + ry * ry + soft * soft * 0.25;
        let s = sign / (r2 * r2.sqrt());
        field.0 += s * rx;
        field.1 += s * ry;
    }
    field.1.atan2(field.0)
}

/// The angle of a dipole's field at `(x, y)` from it, for the axis `(mx, my)`.
fn dipole(x: f32, y: f32, mx: f32, my: f32) -> f32 {
    let r2 = x * x + y * y + 0.05;
    let r = r2.sqrt();
    let (ux, uy) = (x / r, y / r);
    let dot = mx * ux + my * uy;
    (3.0 * dot * uy - my).atan2(3.0 * dot * ux - mx)
}

/// Filings have no head or tail, so angles wrap at half a turn.
fn half_turn(d: f32) -> f32 {
    ((d + PI / 2.0) % PI + PI) % PI - PI / 2.0
}

fn sign(v: f32) -> f32 {
    if v < 0.0 { -1.0 } else { 1.0 }
}

/// 0 at `edge0`, 1 at `edge1`, smoothly.
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Past 1 a little and back, like a spring.
fn ease_out_back(t: f32) -> f32 {
    let (c1, c3) = (1.4, 2.4);
    let u = t.clamp(0.0, 1.0) - 1.0;
    1.0 + c3 * u * u * u + c1 * u * u
}

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// A steady pseudo-random number in 0..1 for filing `n`.
fn noise(n: u32, salt: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    (x & 0xFFFF) as f32 / 65536.0
}

/// `t` of the way from `from` to `to`, with `alpha`.
fn mix(from: (u8, u8, u8), to: (u8, u8, u8), t: f32, alpha: f32) -> Rgba {
    let channel = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) / 255.0;
    Rgba {
        r: channel(from.0, to.0),
        g: channel(from.1, to.1),
        b: channel(from.2, to.2),
        a: alpha.clamp(0.0, 1.0),
    }
}

impl RenderOnce for MagnetBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let still = crate::components::prefers_reduced_motion();
        let field = window.use_keyed_state(self.id, cx, |_, _| Field::new());
        let look = self.look;
        let dimmed = self.dimmed;
        let (drawing, shown) = field.update(cx, |field, _| {
            let now = Instant::now();
            field.hold_while_away(now);
            let drawing = field.set_launch(self.launch, &look, dimmed, still);
            (drawing, field.button_shown(now))
        });
        if drawing && !still {
            // the next frame, about 30 a second; drawing again sooner starts it over
            let task = window.spawn(cx, async move |cx| {
                cx.background_executor().timer(FRAME).await;
                cx.update(|window, _| window.refresh()).ok();
            });
            field.update(cx, |field, _| field.frame = Some(task));
        }
        let inks = drawing.then(|| look.inks());
        let (bleed_x, bleed_y) = self.bleed;
        let painter = field.clone();
        let reader = field.clone();
        let anchor = self.anchor;
        let measure_child = anchor.is_none();
        div()
            .relative()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .flex_1()
            .self_stretch()
            // the button's place, as it wrote it while being laid out this frame,
            // before the filings are drawn
            .when_some(anchor, |this, anchor| {
                this.child(
                    canvas(
                        |_, _, _| {},
                        move |_, _, _, cx| {
                            if let Some(button) = anchor.get() {
                                reader.update(cx, |field, _| field.button = Some(button));
                            }
                        },
                    )
                    .absolute()
                    .size_0(),
                )
            })
            .when(drawing, |this| {
                this.child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, cx| {
                            let mouse = window.mouse_position();
                            painter.update(cx, |field, _| {
                                if let Some(inks) = &inks {
                                    field.paint(bounds, mouse, &look, inks, still, window)
                                }
                            });
                        },
                    )
                    .absolute()
                    .top(px(-bleed_y))
                    .bottom(px(-bleed_y))
                    .left(px(-bleed_x))
                    .right(px(-bleed_x)),
                )
            })
            .child(
                div()
                    .relative()
                    // hidden while it condenses back after a launch
                    .when(shown < 1.0, |this| this.opacity(shown))
                    .child(self.child)
                    // without an anchor, the whole child is the button
                    .when(measure_child, |this| {
                        this.child(
                            canvas(
                                move |bounds, _, cx| {
                                    field.update(cx, |field, _| field.button = Some(bounds))
                                },
                                |_, _, _, _| {},
                            )
                            .absolute()
                            .inset_0(),
                        )
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOX: RoundedBox = RoundedBox {
        cx: 0.0,
        cy: 0.0,
        hw: 100.0,
        hh: 30.0,
        corner: 12.0,
    };

    #[test]
    fn half_turns_wrap() {
        assert!(half_turn(PI).abs() < 1e-5);
        assert!((half_turn(PI / 4.0) - PI / 4.0).abs() < 1e-5);
        assert!((half_turn(-3.0 * PI / 4.0) - PI / 4.0).abs() < 1e-5);
    }

    #[test]
    fn runs_along_the_outline() {
        // above the middle of the top edge: horizontal, 10 px out
        let (d, _, angle) = BOX.measure(0.0, -40.0);
        assert!((d - 10.0).abs() < 1e-4);
        assert!(half_turn(angle).abs() < 1e-4);
        assert_eq!(BOX.nearest(0.0, -40.0), (0.0, -30.0));
        // right of the right edge: vertical
        let (d, _, angle) = BOX.measure(120.0, 0.0);
        assert!((d - 20.0).abs() < 1e-4);
        assert!((half_turn(angle).abs() - PI / 2.0).abs() < 1e-4);
        assert_eq!(BOX.nearest(120.0, 0.0), (100.0, 0.0));
        // inside
        assert!(BOX.measure(0.0, 0.0).0 < 0.0);
    }

    #[test]
    fn goes_round_the_outline() {
        for i in 0..200 {
            let ((x, y), (nx, ny)) = BOX.around(i as f32 / 200.0);
            let (d, _, _) = BOX.measure(x, y);
            assert!(d.abs() < 1e-3, "{i}: {d}");
            assert!((nx.hypot(ny) - 1.0).abs() < 1e-4);
            // pointing out of the box
            assert!(BOX.measure(x + nx, y + ny).0 > 0.5);
        }
    }

    #[test]
    fn filings_read_on_any_bar() {
        // a pale skin colour on a light bar is darkened until it stands out
        let look = MagnetLook {
            palette: vec![Rgb(0xD8F5E4)],
            backdrop: Backdrop::solid(Rgb(0xF4F6FB)),
            ..MagnetLook::default()
        };
        let inks = look.inks();
        let rgb = |(r, g, b): (u8, u8, u8)| {
            Rgb((u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b))
        };
        assert!(readable::ratio_on(rgb(inks.bands[0]), look.backdrop) >= CONTRAST - 0.05);
        // weak filings lean toward the bar's text: dark on a light bar
        assert!(rgb(inks.weak).luminance() < Rgb(0xF4F6FB).luminance() * 0.8);
        // a square button stays square as it melts
        let square = RoundedBox { corner: 0.0, ..BOX };
        assert_eq!(melted(square, 0.7).corner, 0.0);
    }

    #[test]
    fn melts_wider_and_thinner() {
        let end = melted(BOX, 1.0);
        assert!(end.hw > BOX.hw * 1.4 && end.hh < BOX.hh * 0.6);
        assert_eq!(end.corner, end.hh);
        let start = melted(BOX, 0.0);
        assert!((start.hw - BOX.hw).abs() < 1e-3 && start.hh == BOX.hh);
        assert_eq!(start.corner, BOX.corner);
    }

    #[test]
    fn wells_back_up_into_the_button() {
        // a small round drop in the middle
        let start = welled(BOX, 0.0);
        assert_eq!((start.cx, start.cy), (BOX.cx, BOX.cy));
        assert!(start.hw < BOX.hh && start.hw == start.hh && start.corner() == start.hh);
        // swelling steadily into the button, never past it, slowing to a stop
        let frames: Vec<_> = (0..=60)
            .map(|i| welled(BOX, GROW * i as f32 / 60.0))
            .collect();
        for pair in frames.windows(2) {
            assert!(pair[1].hw >= pair[0].hw && pair[1].hh >= pair[0].hh);
            assert!(pair[1].hw <= BOX.hw + 1e-3 && pair[1].hh <= BOX.hh + 1e-3);
        }
        let last_step = frames[60].hw - frames[59].hw;
        assert!(last_step < (frames[1].hw - frames[0].hw) * 0.01);
        // at rest by the time the real button has shown
        let settled = welled(BOX, REVEAL.1);
        assert!((settled.hw - BOX.hw).abs() < 1e-3 && (settled.hh - BOX.hh).abs() < 1e-3);
        assert!((settled.corner() - BOX.corner()).abs() < 1e-3);
        // a square button stays square
        let square = RoundedBox { corner: 0.0, ..BOX };
        assert_eq!(welled(square, 0.2).corner, 0.0);
    }

    #[test]
    fn hides_the_button_until_it_has_welled_back() {
        let look = MagnetLook::default();
        let mut field = Field::new();
        field.set_launch(Some(0.5), &look, false, false);
        assert_eq!(field.button_shown(Instant::now()), 1.0);
        // ends into a dimmed button: condenses back in its dimmed look
        field.set_launch(None, &look, true, false);
        assert_eq!(field.returning, Some(look.dimmed()));
        let ended = field.released.unwrap();
        assert_eq!(field.button_shown(ended), 0.0);
        assert_eq!(field.button_shown(ended + Duration::from_secs(2)), 1.0);
        // still waiting for the window to come back
        field.released = Some(ended + RETURNING);
        assert!(field.returned(ended).unwrap() < 0.0);
        assert_eq!(field.button_shown(ended), 0.0);
        // with reduced motion it's just there
        let mut still = Field::new();
        still.set_launch(Some(0.5), &look, false, true);
        still.set_launch(None, &look, false, true);
        assert_eq!(still.button_shown(Instant::now()), 1.0);
    }

    #[test]
    fn shows_while_launching_and_fades_after() {
        let look = MagnetLook::default();
        let mut field = Field::new();
        assert!(!field.set_launch(None, &look, false, true));
        assert!(field.set_launch(Some(0.3), &look, false, true));
        assert_eq!(field.reach_to, 0.3);
        assert!(!field.set_launch(None, &look, false, true));
        // a dimmed button melts from its dimmed look
        let dim = look.dimmed();
        assert_eq!(dim.fill, look.fill.opacity(25));
        assert!(dim.gradient.is_none() && !dim.glow);
    }

    #[test]
    fn draws_back_in_once_the_window_is_back() {
        let look = MagnetLook::default();
        let mut field = Field::new();
        field.set_launch(Some(0.5), &look, false, false);
        let shown = Instant::now();
        field.last = Some(shown);
        // the game ends while the window is still minimized
        field.set_launch(None, &look, false, false);
        let back = shown + Duration::from_secs(5);
        field.hold_while_away(back);
        assert_eq!(field.released, Some(back + RETURNING));
        // frames drawn one after another leave it alone
        field.last = Some(back);
        field.hold_while_away(back + FRAME);
        assert_eq!(field.released, Some(back + RETURNING));
        // once it's over, frames stop and it stays over: the button shows
        let over = back + RETURNING + Duration::from_secs(2);
        field.last = Some(over);
        field.hold_while_away(over + Duration::from_secs(10));
        assert_eq!(field.released, Some(back + RETURNING));
        assert_eq!(field.button_shown(over + Duration::from_secs(10)), 1.0);
    }
}
