//! Liquid Glass, after Apple's design (WWDC25): Settings > General > Transparency >
//! Glass.
//!
//! Glass is the floating layer of navigation and controls, never the content
//! ([`is_glass`]): the sidebar, the top bar, the Play bar, dialogs, popovers and
//! notices. Cards, rows, fields and the hero stay a plain see-through material, and
//! nothing glass sits on other glass (the settings' own sidebar, inside a glass
//! dialog, isn't glass).
//!
//! What it draws, and how close it gets to Apple's material with what GPUI has
//! (two-stop linear gradients, blurred inset and drop shadows, images, opacity; no
//! backdrop blur, no shaders, no reading back what's drawn):
//!
//! - **Lensing.** Real glass bends the light behind it rather than scattering it.
//!   Over a skin's window image, each glass pane gets a lens texture ([`lens`]):
//!   inside, the image frosted; at the rim, the image bent in from just outside the
//!   pane, crisper, with a slight colour fringe and the light gathered in it. It is
//!   built once, in the background, for the pane's size and place, and again only
//!   after a resize settles. Over the desktop (no pixels to bend) only the lit rim
//!   suggests the lens.
//! - **Light.** A lit rim of inset shadows ([`rim`]): the top edge catching the
//!   light, a specular crescent on the side facing it and a shade across. The light
//!   comes from above the screen, so it moves as the window moves ([`set_env`]); on
//!   the Play button it follows the pointer.
//! - **Adapting.** A pane's tint and its shadow follow how bright the image behind
//!   it is ([`tone`]), and the glass dims when the window isn't in front.
//! - **Shapes.** The launcher's own corners are kept: the glass takes the shape of
//!   what it is on.
//! - **Motion.** The Play button flexes with a spring when pressed and lights up
//!   from the press ([`spring`]); dialogs and popovers materialize with a flash of
//!   gathered light that settles ([`materialize`]).
//! - **Accessibility.** The system's Reduce Transparency makes glass nearly opaque
//!   and drops the lens; Increase Contrast gives solid fills with a contrasting
//!   border; Reduce Motion stops the springs and the flash ([`crate::a11y`]).

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    AbsoluteLength, AnyElement, App, BoxShadow, Corners, CornersRefinement, Div, ElementId, Hsla,
    IntoElement, ParentElement, Pixels, RenderImage, Styled, Window, canvas, div, point, px,
};
use image::RgbaImage;
use launcher_core::ui_skin::{Material, SurfaceId};

use crate::a11y::Prefs;

/// The skin's window image, ready to be bent by the glass over it.
pub struct LensSource {
    /// Lightly blurred, at most 1000 pixels wide: what the rim bends.
    sharp: RgbaImage,
    /// Heavily blurred and small: the frosted inside.
    soft: RgbaImage,
    /// How the whole image looks, until a pane measures its own part.
    pub tone: Tone,
}

impl LensSource {
    /// Bytes held, for the performance monitor.
    pub fn bytes(&self) -> usize {
        (self.sharp.len() + self.soft.len()) * std::mem::size_of::<u8>()
    }
}

/// How the image behind a pane looks: its mean colour (0–1 each) and luminance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tone {
    pub rgb: [f32; 3],
    pub luminance: f32,
}

impl Tone {
    /// Behind glass with no image: the launcher's dark grey.
    pub const DARK: Tone = Tone {
        rgb: [0.11, 0.12, 0.13],
        luminance: 0.12,
    };

    fn of(image: &RgbaImage) -> Tone {
        let (mut r, mut g, mut b, mut n) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for pixel in image.pixels().step_by(3) {
            r += f64::from(pixel[0]);
            g += f64::from(pixel[1]);
            b += f64::from(pixel[2]);
            n += 1.0;
        }
        if n == 0.0 {
            return Tone::DARK;
        }
        let rgb = [
            (r / n / 255.0) as f32,
            (g / n / 255.0) as f32,
            (b / n / 255.0) as f32,
        ];
        Tone {
            rgb,
            luminance: luminance(rgb),
        }
    }
}

fn luminance([r, g, b]: [f32; 3]) -> f32 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Prepares the skin's window image for the glass over it, in the background.
pub fn lens_source(image: &RgbaImage) -> LensSource {
    let (w, h) = image.dimensions();
    let sharp = if w == 0 || h == 0 {
        RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 0, 255]))
    } else {
        let scale = (1000.0 / w as f32).min(1.0);
        let small = image::imageops::resize(
            image,
            ((w as f32 * scale).round() as u32).max(1),
            ((h as f32 * scale).round() as u32).max(1),
            image::imageops::FilterType::Triangle,
        );
        image::imageops::fast_blur(&small, 1.5)
    };
    let soft = crate::glass::blurred_copy(image);
    let tone = Tone::of(&soft);
    LensSource { sharp, soft, tone }
}

/// What the glass answers to in this frame: where the light comes from, whether the
/// window is in front, and the system's accessibility settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Env {
    /// Across, from −1 (light from the left) to 1 (from the right).
    pub light: f32,
    pub active: bool,
    pub prefs: Prefs,
}

impl Env {
    const DEFAULT: Env = Env {
        light: -0.35,
        active: true,
        prefs: Prefs {
            reduce_motion: false,
            reduce_transparency: false,
            increase_contrast: false,
        },
    };
}

thread_local! {
    static ENV: Cell<Env> = const { Cell::new(Env::DEFAULT) };
    /// Set by the screenshots: how far into materializing glass is held.
    static FROZEN: Cell<Option<f32>> = const { Cell::new(None) };
    /// Set by the screenshots, so they don't depend on the window or the machine.
    static FORCED: Cell<Option<Env>> = const { Cell::new(None) };
}

/// Reads the light, the window's focus and the accessibility settings for the frame
/// being drawn. Each window's root view calls it, after [`crate::glass::set_frame`].
pub fn set_env(window: &Window, cx: &App) {
    let env = FORCED.with(Cell::get).unwrap_or_else(|| {
        // The light hangs above the middle of the screen: a window right of it is lit
        // from the left, and the light moves across it as it moves.
        let across = window.display(cx).map_or(0.0, |display| {
            let screen = display.bounds();
            let half = f32::from(screen.size.width) / 2.0;
            if half <= 0.0 {
                return 0.0;
            }
            (f32::from(window.bounds().center().x - screen.center().x) / half).clamp(-1.0, 1.0)
        });
        Env {
            light: (-0.2 - 0.8 * across).clamp(-1.0, 1.0),
            active: window.is_window_active(),
            prefs: crate::a11y::prefs(),
        }
    });
    ENV.with(|cell| cell.set(env));
}

/// Uses `env` instead of the window's (`None` to read the window's again).
#[cfg_attr(not(feature = "screenshot"), allow(dead_code))]
pub fn force_env(env: Option<Env>) {
    FORCED.with(|forced| forced.set(env));
}

/// Holds glass that materializes at `delta` (0 to 1) of the way, for the
/// screenshots; `None` lets it animate.
#[cfg_attr(not(feature = "screenshot"), allow(dead_code))]
pub fn freeze_materialize(delta: Option<f32>) {
    FROZEN.with(|frozen| frozen.set(delta));
}

/// The frame's light, focus and accessibility settings.
pub fn env() -> Env {
    ENV.with(Cell::get)
}

/// Whether the window being drawn is in the Glass mode.
pub fn on() -> bool {
    crate::glass::frame().material == Material::Glass
}

/// Whether `surface` is Liquid Glass in this frame: the floating layer of navigation
/// and controls. The settings' sidebar sits inside a glass dialog, so it isn't.
pub fn is_glass(surface: SurfaceId) -> bool {
    on() && matches!(
        surface,
        SurfaceId::Sidebar
            | SurfaceId::TopBar
            | SurfaceId::PlayBar
            | SurfaceId::Dialog
            | SurfaceId::Popover
            | SurfaceId::Toast
    )
}

/// Whether `surface` bends the skin's window image: the panes laid straight on the
/// window. Dialogs and popovers sit over the launcher's own content, which can't be
/// read back, so they only frost.
pub fn bends(surface: SurfaceId) -> bool {
    matches!(
        surface,
        SurfaceId::Sidebar | SurfaceId::TopBar | SurfaceId::PlayBar
    )
}

/// Whether `surface` floats over what's under it, with a shadow beneath.
pub fn lifts(surface: SurfaceId) -> bool {
    matches!(
        surface,
        SurfaceId::Sidebar
            | SurfaceId::PlayBar
            | SurfaceId::Dialog
            | SurfaceId::Popover
            | SurfaceId::Toast
    )
}

/// How the image behind `surface` looks, as its lens last measured it.
pub fn tone(surface: SurfaceId, source: Option<&LensSource>) -> Tone {
    TONES
        .with(|tones| tones.borrow().get(&surface).copied())
        .or(source.map(|source| source.tone))
        .unwrap_or(Tone::DARK)
}

/// How opaque a glass pane's tint is: darker over a brighter image, so light text
/// stays readable; nearly solid with Reduce Transparency, solid with Increase
/// Contrast, and a little more solid while the window isn't in front.
pub fn tint_alpha(alpha: f32, tone: Tone) -> f32 {
    let env = env();
    if env.prefs.increase_contrast {
        return 0.97;
    }
    let mut alpha = (alpha + 0.50 * tone.luminance).min(0.95);
    if env.prefs.reduce_transparency {
        alpha = alpha.max(0.92);
    }
    if !env.active {
        alpha = (alpha + 0.08).min(0.97);
    }
    alpha
}

fn inset(x: f32, y: f32, blur: f32, spread: f32, color: Hsla) -> BoxShadow {
    BoxShadow {
        color,
        offset: point(px(x), px(y)),
        blur_radius: px(blur),
        spread_radius: px(spread),
        inset: true,
    }
}

fn drop(x: f32, y: f32, blur: f32, spread: f32, color: Hsla) -> BoxShadow {
    BoxShadow {
        inset: false,
        ..inset(x, y, blur, spread, color)
    }
}

fn white(alpha: f32) -> Hsla {
    gpui::hsla(0., 0., 1., alpha.clamp(0.0, 1.0))
}

fn black(alpha: f32) -> Hsla {
    gpui::hsla(0., 0., 0., alpha.clamp(0.0, 1.0))
}

/// The light on glass, as inset shadows: the top edge catching it, a faint
/// reflected bottom edge, a specular crescent on the side facing the light and a
/// shade across, and light gathered all round the rim. `strength` scales it (1 for
/// a pane), and it dims while the window isn't in front.
pub fn rim_shadows(strength: f32) -> Vec<BoxShadow> {
    let env = env();
    if env.prefs.increase_contrast {
        return Vec::new();
    }
    let s = strength * if env.active { 1.0 } else { 0.5 };
    let l = env.light;
    vec![
        inset(0., 1., 0., 0., white(0.40 * s)),
        inset(0., -1., 0., 0., white(0.10 * s)),
        inset(-l * 4., 4., 9., -3., white(0.20 * s)),
        inset(l * 4., -4., 10., -3., black(0.16 * s)),
        inset(0., 0., 12., 0., white(0.07 * s)),
    ]
}

/// A layer lighting the glass under it, rounded as `radii` (children are clipped to
/// a rectangle, not to rounded corners).
pub fn rim(radii: CornersRefinement<AbsoluteLength>, strength: f32) -> Div {
    let mut layer = div().absolute().inset_0();
    layer.style().corner_radii = radii;
    layer.style().box_shadow = Some(rim_shadows(strength));
    layer
}

/// The shadow under floating glass: deeper under larger pieces, and stronger over
/// a darker image so the edge still reads (weaker over a light one).
pub fn depth(surface: SurfaceId, tone: Tone) -> Vec<BoxShadow> {
    let env = env();
    let big = matches!(surface, SurfaceId::Sidebar | SurfaceId::Dialog);
    let key = (0.20 + 0.20 * (1.0 - tone.luminance)).clamp(0.15, 0.42)
        * if env.active { 1.0 } else { 0.6 };
    vec![
        drop(0., 1., 3., 0., black(0.10)),
        if big {
            drop(0., 12., 32., 0., black(key))
        } else {
            drop(0., 6., 18., 0., black(key))
        },
    ]
}

/// A flash of gathered light that settles as glass appears: Apple's glass
/// materializes by building up its lensing rather than fading in. Empty with Reduce
/// Motion.
pub fn materialize(id: impl Into<ElementId>, radii: CornersRefinement<AbsoluteLength>) -> AnyElement {
    use gpui::{Animation, AnimationExt};
    let env = env();
    if env.prefs.reduce_motion || env.prefs.increase_contrast {
        return div().into_any_element();
    }
    let mut layer = div().absolute().inset_0();
    layer.style().corner_radii = radii;
    layer.style().box_shadow = Some(vec![
        inset(0., 0., 22., 2., white(0.42)),
        inset(0., 1., 0., 0., white(0.55)),
    ]);
    if let Some(delta) = FROZEN.with(Cell::get) {
        return layer.opacity(1.0 - delta).into_any_element();
    }
    layer
        .with_animation(
            id,
            Animation::new(Duration::from_millis(460))
                .with_easing(crate::components::motion::ease_out_cubic),
            |layer, delta| layer.opacity(1.0 - delta),
        )
        .into_any_element()
}

/// A damped spring toward `target`: overshoots a little and settles, so glass
/// flexes like a gel. Per element `key`; asks for frames only while it moves, and
/// jumps with Reduce Motion.
pub fn spring(key: impl Into<ElementId>, target: f32, window: &mut Window, cx: &mut App) -> f32 {
    struct Spring {
        from: f32,
        to: f32,
        start: Instant,
    }
    if env().prefs.reduce_motion {
        return target;
    }
    let now = Instant::now();
    let state = window.use_keyed_state(key, cx, |_, _| Spring {
        from: target,
        to: target,
        start: now,
    });
    let (value, done) = state.update(cx, |spring, _| {
        if spring.to != target {
            let current = spring_at(spring.from, spring.to, now - spring.start);
            *spring = Spring {
                from: current,
                to: target,
                start: now,
            };
        }
        let elapsed = now - spring.start;
        (
            spring_at(spring.from, spring.to, elapsed),
            elapsed >= SPRING_SETTLED,
        )
    });
    if !done {
        window.request_animation_frame();
    }
    value
}

/// When the spring counts as settled.
const SPRING_SETTLED: Duration = Duration::from_millis(650);

/// An underdamped spring from `from` to `to` after `elapsed`.
fn spring_at(from: f32, to: f32, elapsed: Duration) -> f32 {
    if elapsed >= SPRING_SETTLED {
        return to;
    }
    // about 20% overshoot, settled in roughly half a second
    const DAMPING: f32 = 0.45;
    const OMEGA: f32 = 24.0;
    let t = elapsed.as_secs_f32();
    let damped = OMEGA * (1.0 - DAMPING * DAMPING).sqrt();
    to + (from - to) * (-DAMPING * OMEGA * t).exp() * (damped * t).cos()
}

/// A soft round light, white fading out, for the glow under a press or the pointer.
pub fn glow_image() -> Arc<RenderImage> {
    thread_local! {
        static GLOW: Arc<RenderImage> = {
            const SIZE: u32 = 96;
            let image = RgbaImage::from_fn(SIZE, SIZE, |x, y| {
                let c = (SIZE as f32 - 1.0) / 2.0;
                let d = ((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt() / c;
                let a = (1.0 - d).clamp(0.0, 1.0).powf(1.8);
                image::Rgba([255, 255, 255, (a * 255.0).round() as u8])
            });
            crate::images::render_image(image)
        };
    }
    GLOW.with(Arc::clone)
}

/// Where a lens was built, to find it again: the pane, its place and size, its
/// corners and the window's size (in quarter pixels), the scale it was built at and
/// the image it bends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Key {
    source: usize,
    surface: SurfaceId,
    area: [i32; 4],
    content: [i32; 4],
    radii: [i32; 4],
    scale: i32,
}

struct Built {
    image: Arc<RenderImage>,
    used: u64,
}

thread_local! {
    static CACHE: RefCell<HashMap<Key, Built>> = RefCell::new(HashMap::new());
    /// The lens each pane wants now: a build that waited for a resize to settle
    /// goes ahead only if it is still the one wanted.
    static WANTED: RefCell<HashMap<SurfaceId, Key>> = RefCell::new(HashMap::new());
    static PENDING: RefCell<HashSet<Key>> = RefCell::new(HashSet::new());
    static TONES: RefCell<HashMap<SurfaceId, Tone>> = RefCell::new(HashMap::new());
    static TICK: Cell<u64> = const { Cell::new(0) };
}

/// Lenses kept at once: one for each pane, and a few for sizes just left behind.
const KEPT: usize = 8;
/// How long a pane's size has to hold still before its lens is built.
const SETTLE: Duration = Duration::from_millis(140);
/// The most pixels a lens is built with: past it, it is built at a lower scale.
const MAX_PIXELS: f32 = 900_000.0;

/// Whether a lens is waiting to be built (the screenshots wait for it).
#[cfg_attr(not(feature = "screenshot"), allow(dead_code))]
pub fn building() -> bool {
    PENDING.with(|pending| !pending.borrow().is_empty())
}

/// The lens textures held, and their bytes, for the performance monitor.
pub fn cache_bytes() -> (usize, usize) {
    CACHE.with(|cache| {
        let cache = cache.borrow();
        let bytes = cache
            .values()
            .map(|built| {
                let size = built.image.size(0);
                size.width.0 as usize * size.height.0 as usize * 4
            })
            .sum();
        (cache.len(), bytes)
    })
}

/// The skin's window image as glass `surface` shows it: bent at the rim and frosted
/// inside ([`build`]), or, until that is ready, `fallback` (the plain blurred copy).
/// `opacity` is how much of it covers the desktop behind; `radii` are the pane's
/// corners. Laid as the pane's first child.
pub fn lens(
    surface: SurfaceId,
    source: Arc<LensSource>,
    fallback: Option<Arc<RenderImage>>,
    opacity: f32,
    radii: CornersRefinement<AbsoluteLength>,
) -> impl IntoElement {
    let paint = move |bounds: gpui::Bounds<Pixels>, _: (), window: &mut Window, cx: &mut App| {
        let rem = window.rem_size();
        let corner = |r: Option<AbsoluteLength>| r.map_or(Pixels::ZERO, |r| r.to_pixels(rem));
        let corners = Corners {
            top_left: corner(radii.top_left),
            top_right: corner(radii.top_right),
            bottom_right: corner(radii.bottom_right),
            bottom_left: corner(radii.bottom_left),
        }
        .clamp_radii_for_quad_size(bounds.size);
        let content = crate::components::window_frame::content_bounds(window);
        let mut scale = window.scale_factor().min(1.5);
        let pixels = f32::from(bounds.size.width) * f32::from(bounds.size.height) * scale * scale;
        if pixels > MAX_PIXELS {
            scale *= (MAX_PIXELS / pixels).sqrt();
        }
        let quarter = |p: Pixels| (f32::from(p) * 4.0).round() as i32;
        let key = Key {
            source: Arc::as_ptr(&source) as usize,
            surface,
            area: [
                quarter(bounds.origin.x),
                quarter(bounds.origin.y),
                quarter(bounds.size.width),
                quarter(bounds.size.height),
            ],
            content: [
                quarter(content.origin.x),
                quarter(content.origin.y),
                quarter(content.size.width),
                quarter(content.size.height),
            ],
            radii: [
                quarter(corners.top_left),
                quarter(corners.top_right),
                quarter(corners.bottom_right),
                quarter(corners.bottom_left),
            ],
            scale: (scale * 100.0).round() as i32,
        };
        let tick = TICK.with(|tick| {
            tick.set(tick.get() + 1);
            tick.get()
        });
        let built = CACHE.with(|cache| {
            cache.borrow_mut().get_mut(&key).map(|built| {
                built.used = tick;
                built.image.clone()
            })
        });
        match built {
            Some(image) => {
                window
                    .paint_image(bounds, bounds, corners, image, 0, false)
                    .ok();
            }
            None => {
                if let Some(fallback) = &fallback {
                    let size = fallback.size(0);
                    let laid = crate::glass::cover(
                        content,
                        gpui::size(px(size.width.0 as f32), px(size.height.0 as f32)),
                    );
                    window
                        .paint_image(bounds, laid, corners, fallback.clone(), 0, false)
                        .ok();
                }
                request(key, &source, bounds, content, corners, scale, window, cx);
            }
        }
    };
    div()
        .absolute()
        .inset_0()
        .opacity(opacity)
        .child(canvas(|_, _, _| (), paint).size_full())
}

/// Builds the lens for `key` in the background once the pane has held still for
/// [`SETTLE`], keeps it, and draws the window again.
#[allow(clippy::too_many_arguments)]
fn request(
    key: Key,
    source: &Arc<LensSource>,
    area: gpui::Bounds<Pixels>,
    content: gpui::Bounds<Pixels>,
    corners: Corners<Pixels>,
    scale: f32,
    window: &mut Window,
    cx: &mut App,
) {
    WANTED.with(|wanted| wanted.borrow_mut().insert(key.surface, key));
    let fresh = PENDING.with(|pending| pending.borrow_mut().insert(key));
    if !fresh {
        return;
    }
    let source = source.clone();
    let rect = |b: gpui::Bounds<Pixels>| {
        [
            f32::from(b.origin.x),
            f32::from(b.origin.y),
            f32::from(b.size.width),
            f32::from(b.size.height),
        ]
    };
    let area = rect(area);
    let content = rect(content);
    let radii = [
        f32::from(corners.top_left),
        f32::from(corners.top_right),
        f32::from(corners.bottom_right),
        f32::from(corners.bottom_left),
    ];
    window
        .spawn(cx, async move |cx| {
            cx.background_executor().timer(SETTLE).await;
            let still = WANTED.with(|wanted| wanted.borrow().get(&key.surface) == Some(&key));
            if !still {
                PENDING.with(|pending| pending.borrow_mut().remove(&key));
                return;
            }
            let (image, tone) = cx
                .background_executor()
                .spawn(async move { build(&source, content, area, radii, scale) })
                .await;
            let image = crate::images::render_image(image);
            PENDING.with(|pending| pending.borrow_mut().remove(&key));
            TONES.with(|tones| tones.borrow_mut().insert(key.surface, tone));
            cx.update(|window, _| {
                let tick = TICK.with(Cell::get);
                let evicted = CACHE.with(|cache| {
                    let mut cache = cache.borrow_mut();
                    cache.insert(key, Built { image, used: tick });
                    let mut evicted = Vec::new();
                    while cache.len() > KEPT {
                        let Some(oldest) = cache
                            .iter()
                            .min_by_key(|(_, built)| built.used)
                            .map(|(key, _)| *key)
                        else {
                            break;
                        };
                        if let Some(built) = cache.remove(&oldest) {
                            evicted.push(built.image);
                        }
                    }
                    evicted
                });
                for image in evicted {
                    window.drop_image(image).ok();
                }
                window.refresh();
            })
            .ok();
        })
        .detach();
}

/// How wide the rim that bends the light is, for a pane `w` × `h`: thicker glass
/// for larger pieces, as Apple's is.
fn band(w: f32, h: f32) -> f32 {
    (w.min(h) * 0.22).clamp(6.0, 22.0)
}

/// The lens for a pane at `area` (x, y, w, h, in the window's pixels) with corners
/// `radii` (top left, top right, bottom right, bottom left), over the source image
/// covering `content`, at `scale` pixels per window pixel; and how the image under
/// it looks.
fn build(
    source: &LensSource,
    content: [f32; 4],
    area: [f32; 4],
    radii: [f32; 4],
    scale: f32,
) -> (RgbaImage, Tone) {
    let [ax, ay, aw, ah] = area;
    let out_w = ((aw * scale).round() as u32).max(1);
    let out_h = ((ah * scale).round() as u32).max(1);
    // where the image lies over the window: covering the content, centred
    let place = |image: &RgbaImage| {
        let (w, h) = (image.width() as f32, image.height() as f32);
        let [cx, cy, cw, ch] = content;
        let s = (cw / w).max(ch / h);
        let (pw, ph) = (w * s, h * s);
        [cx + (cw - pw) / 2.0, cy + (ch - ph) / 2.0, pw, ph]
    };
    let sharp_at = place(&source.sharp);
    let soft_at = place(&source.soft);
    let band = band(aw, ah);
    let (half_w, half_h) = (aw / 2.0, ah / 2.0);
    let mut out = RgbaImage::new(out_w, out_h);
    let (mut sum, mut n) = ([0.0f32; 3], 0.0f32);
    for oy in 0..out_h {
        for ox in 0..out_w {
            // the point, from the pane's centre, in window pixels
            let px = (ox as f32 + 0.5) / scale;
            let py = (oy as f32 + 0.5) / scale;
            let (qx, qy) = (px - half_w, py - half_h);
            let r = match (qx < 0.0, qy < 0.0) {
                (true, true) => radii[0],
                (false, true) => radii[1],
                (false, false) => radii[2],
                (true, false) => radii[3],
            }
            .min(half_w)
            .min(half_h);
            let (inside, normal) = edge(qx, qy, half_w, half_h, r);
            let (wx, wy) = (ax + px, ay + py);
            let frost = sample(&source.soft, soft_at, wx, wy);
            let color = if inside < band {
                // at the rim: the image from just outside the pane, bent in
                let t = 1.0 - inside / band;
                let shift = band * 0.95 * t.powf(2.2);
                let bent = |k: f32| {
                    sample(
                        &source.sharp,
                        sharp_at,
                        wx + normal.0 * shift * k,
                        wy + normal.1 * shift * k,
                    )
                };
                // a slight colour fringe: red bends a little more than blue
                let red = bent(1.08)[0];
                let green = bent(1.0)[1];
                let blue = bent(0.92)[2];
                let rim = [red, green, blue];
                let mix = smoothstep(0.0, 0.85, t);
                let mut c = [0.0; 3];
                for i in 0..3 {
                    c[i] = frost[i] + (rim[i] - frost[i]) * mix;
                    // light gathers in the rim
                    c[i] += (1.0 - c[i]) * 0.12 * t * t * t;
                }
                c
            } else {
                frost
            };
            // a touch more vivid, as light through glass is
            let l = luminance(color);
            let color = color.map(|c| (l + (c - l) * 1.12).clamp(0.0, 1.0));
            for i in 0..3 {
                sum[i] += color[i];
            }
            n += 1.0;
            out.put_pixel(
                ox,
                oy,
                image::Rgba([
                    (color[0] * 255.0).round() as u8,
                    (color[1] * 255.0).round() as u8,
                    (color[2] * 255.0).round() as u8,
                    255,
                ]),
            );
        }
    }
    let rgb = if n > 0.0 {
        sum.map(|s| s / n)
    } else {
        Tone::DARK.rgb
    };
    (
        out,
        Tone {
            rgb,
            luminance: luminance(rgb),
        },
    )
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How far a point (`qx`, `qy` from the centre) is inside a rounded rectangle
/// (`hw` × `hh` half size, corner radius `r`), and the outward direction of the
/// nearest edge.
fn edge(qx: f32, qy: f32, hw: f32, hh: f32, r: f32) -> (f32, (f32, f32)) {
    let (sx, sy) = (qx.signum(), qy.signum());
    let (dx, dy) = (qx.abs() - (hw - r), qy.abs() - (hh - r));
    if dx > 0.0 && dy > 0.0 {
        // in a corner's quarter circle
        let len = (dx * dx + dy * dy).sqrt().max(1e-4);
        (r - len, (sx * dx / len, sy * dy / len))
    } else {
        let to_side = hw - qx.abs();
        let to_end = hh - qy.abs();
        if to_side < to_end {
            (to_side, (sx, 0.0))
        } else {
            (to_end, (0.0, sy))
        }
    }
}

/// The colour of `image` (laid at `at`: x, y, w, h in window pixels) at window
/// point (`x`, `y`), bilinear, clamped at its edges; 0–1 each.
fn sample(image: &RgbaImage, at: [f32; 4], x: f32, y: f32) -> [f32; 3] {
    let (w, h) = (image.width(), image.height());
    let u = ((x - at[0]) / at[2] * w as f32 - 0.5).clamp(0.0, (w - 1) as f32);
    let v = ((y - at[1]) / at[3] * h as f32 - 0.5).clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (u.floor() as u32, v.floor() as u32);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (fx, fy) = (u - x0 as f32, v - y0 as f32);
    let p = |x: u32, y: u32| image.get_pixel(x, y).0;
    let (a, b, c, d) = (p(x0, y0), p(x1, y0), p(x0, y1), p(x1, y1));
    let mut out = [0.0; 3];
    for i in 0..3 {
        let top = f32::from(a[i]) + (f32::from(b[i]) - f32::from(a[i])) * fx;
        let bottom = f32::from(c[i]) + (f32::from(d[i]) - f32::from(c[i])) * fx;
        out[i] = (top + (bottom - top) * fy) / 255.0;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stripes() -> RgbaImage {
        // vertical stripes, 10 pixels wide: anything bent shows
        RgbaImage::from_fn(400, 200, |x, _| {
            if (x / 10) % 2 == 0 {
                image::Rgba([255, 255, 255, 255])
            } else {
                image::Rgba([0, 0, 0, 255])
            }
        })
    }

    #[test]
    fn the_rim_bends_and_the_inside_frosts() {
        let source = lens_source(&stripes());
        let (lens, tone) = build(
            &source,
            [0., 0., 400., 200.],
            [100., 50., 200., 100.],
            [14.; 4],
            1.0,
        );
        assert_eq!(lens.dimensions(), (200, 100));
        // inside, the stripes are frosted to grey
        let middle = lens.get_pixel(100, 50).0;
        assert!((60..200).contains(&middle[0]), "frosted: {middle:?}");
        // at the rim, crisper: further from grey than the middle
        let row = 50;
        let contrast = |x: u32| (i32::from(lens.get_pixel(x, row).0[1]) - 128).abs();
        let rim = (0..4).map(contrast).max().unwrap();
        assert!(rim > contrast(100), "rim {rim} vs middle {}", contrast(100));
        assert!(tone.luminance > 0.2 && tone.luminance < 0.8);
    }

    #[test]
    fn edges_point_outward() {
        let (inside, normal) = edge(-95., 0., 100., 50., 10.);
        assert!((inside - 5.).abs() < 1e-4);
        assert_eq!(normal, (-1.0, 0.0));
        let (inside, normal) = edge(0., 48., 100., 50., 10.);
        assert!((inside - 2.).abs() < 1e-4);
        assert_eq!(normal, (0.0, 1.0));
        // in a corner, diagonally out
        let (_, normal) = edge(95., 45., 100., 50., 10.);
        assert!(normal.0 > 0.5 && normal.1 > 0.5);
    }

    #[test]
    fn the_spring_overshoots_then_settles() {
        let peak = (1..60)
            .map(|ms| spring_at(0.0, 1.0, Duration::from_millis(ms * 10)))
            .fold(0.0f32, f32::max);
        assert!(peak > 1.05 && peak < 1.4, "overshoot {peak}");
        assert_eq!(spring_at(0.0, 1.0, SPRING_SETTLED), 1.0);
        assert!((spring_at(0.0, 1.0, Duration::from_millis(600)) - 1.0).abs() < 0.03);
    }

    #[test]
    fn a_brighter_image_takes_a_darker_tint() {
        let dark = tint_alpha(0.4, Tone::DARK);
        let bright = tint_alpha(
            0.4,
            Tone {
                rgb: [0.9; 3],
                luminance: 0.9,
            },
        );
        assert!(bright > dark);
        assert!(bright <= 0.95);
    }

    #[test]
    fn large_lenses_are_built_smaller() {
        // the cap the paint applies before building
        let (w, h, mut scale) = (2000.0f32, 1200.0f32, 1.5f32);
        let pixels = w * h * scale * scale;
        if pixels > MAX_PIXELS {
            scale *= (MAX_PIXELS / pixels).sqrt();
        }
        assert!(w * h * scale * scale <= MAX_PIXELS * 1.001);
    }
}
