//! The launcher's skin as the window shows it: the chosen pack's colours right away,
//! and its images and font files once they are prepared in the background (see
//! [`launcher_core::ui_skin`] for packs).
//!
//! Each surface the skin paints ([`SurfaceId`]) is resolved once into a [`Painted`]:
//! a colour, a gradient GPUI draws, or an image prepared at the surface's shape
//! (tiled, blurred and tinted, or a rich gradient drawn once). Elements paint
//! themselves with [`Skinned::surface`]. Nothing is redrawn or animated while the
//! window is idle.
//!
//! The launcher doesn't adjust a skin's colours for readability unless the pack
//! opts in (`readability: auto`); [`checks`] says what falls short, for the editor.
//!
//! An instance can override the skin's accent and Play button colour; the window
//! uses the selected instance's ([`resolve`]).
//!
//! Safe mode skips the skin: `--no-skin`, `LODESTONE_LAUNCHER_NO_SKIN=1`, or a skin
//! that fails to load falls back to the default look.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use gpui::{App, Global, RenderImage, SharedString, Task};
use image::RgbaImage;
use launcher_core::{
    instances::Instance,
    ui_skin::{
        Fill, Fit, FontPart, GradientKind, ImageFill, PlaySpec, Readability, Rgb, SkinEntry,
        SkinSpec, SkinStore, SurfaceId,
        readable::{self, Backdrop, Palette},
    },
};

use crate::theme::{Color, colors};

use crate::instances::appearance::{Background, Scrim, scrim_for};

use lodestone_ui::skin::glass;
pub use lodestone_ui::skin::{Glass, Paint, Painted, SkinFrame, Skinned, paint, painted};

/// The skin the window shows.
pub struct Look {
    pub id: String,
    pub spec: SkinSpec,
    /// The surfaces the skin paints.
    pub surfaces: HashMap<SurfaceId, Painted>,
    /// How each image fill looks behind text, before its tint.
    pub backdrops: HashMap<SurfaceId, Backdrop>,
    /// The Play tab's image for an instance without its own, with its shade.
    pub hero: Option<Background>,
    /// The font families the skin uses, by part, once loaded.
    pub fonts: HashMap<FontPart, SharedString>,
    /// Layers' pictures, blurred for the materials that show them, by layer and
    /// blur.
    pub blurred: HashMap<(SurfaceId, u8), Arc<RenderImage>>,
}

impl Look {
    /// The skin's colours and bundled fonts, before its images and font files are
    /// ready.
    fn plain(id: &str, spec: SkinSpec) -> Self {
        let surfaces = SurfaceId::ALL
            .into_iter()
            .filter_map(|surface| {
                let own = spec.surface(surface)?;
                // a material shows the fill over what is behind at its opacity
                let material = own.material.filter(|_| surface.takes_material());
                let alpha = material.map_or(100, |m| u32::from(m.opacity(own.fill.as_ref())));
                let paint = match &own.fill {
                    Some(Fill::Solid { color, opacity }) => {
                        Some(Paint::Solid(Color::rgb(color.0).opacity(match material {
                            Some(_) => alpha,
                            None => u32::from(*opacity),
                        })))
                    }
                    Some(Fill::Gradient(gradient)) if gradient.is_simple() => {
                        let stop = |i: usize| Color::rgb(gradient.stops[i].color.0).opacity(alpha);
                        Some(Paint::Gradient(gpui::linear_gradient(
                            f32::from(gradient.angle),
                            gpui::linear_color_stop(stop(0), 0.),
                            gpui::linear_color_stop(stop(1), 1.),
                        )))
                    }
                    // a see-through surface without a fill: its own colour, faintly
                    None if material.is_some() => {
                        Some(Paint::Solid(Color::rgb(surface.base().0).opacity(alpha)))
                    }
                    _ => None,
                };
                Some((
                    surface,
                    Painted {
                        glass: material.and_then(|_| Glass::of(&spec, surface)),
                        see_through: material.map(|_| alpha as f32 / 100.0),
                        material: material.is_some(),
                        records: matches!(
                            surface,
                            SurfaceId::Window | SurfaceId::Main | SurfaceId::Dialog
                        )
                        .then_some(surface),
                        paint,
                        border: own
                            .border
                            .map(|c| (Color::rgb(c.0), f32::from(own.border_width.unwrap_or(1)))),
                        radius: own.radius.map(f32::from),
                        // a control's label: as given, else white or near-black,
                        // whichever its fill takes
                        text: own
                            .text
                            .or_else(|| {
                                let fill = own.fill.as_ref()?;
                                surface.is_control().then(|| {
                                    readable::best_ink(readable::backdrop_of(
                                        Some(fill),
                                        surface.base(),
                                        None,
                                    ))
                                })
                            })
                            .map(|c| Color::rgb(c.0)),
                        muted: own.muted.map(|c| Color::rgb(c.0)),
                    },
                ))
            })
            .collect();
        let fonts = FontPart::ALL
            .into_iter()
            .filter_map(|part| {
                let name = spec.fonts.get(part)?;
                (!launcher_core::ui_skin::surface::is_font_file(name))
                    .then(|| (part, SharedString::from(name.to_string())))
            })
            .collect();
        Self {
            id: id.into(),
            spec,
            surfaces,
            backdrops: HashMap::new(),
            hero: None,
            fonts,
            blurred: HashMap::new(),
        }
    }

    /// Takes the blurred layer pictures, and gives each blurred surface the one it
    /// shows.
    fn set_blurred(&mut self, blurred: HashMap<(SurfaceId, u8), Arc<RenderImage>>) {
        for painted in self.surfaces.values_mut() {
            if let Some(glass) = &mut painted.glass
                && let Some(image) = glass.source().and_then(|key| blurred.get(&key))
            {
                glass.with_image(image.clone());
            }
        }
        self.blurred = blurred;
    }

    fn default_look() -> Self {
        Self::plain("default", SkinSpec::default())
    }

    /// Whether it needs anything prepared in the background.
    fn needs_preparing(&self) -> bool {
        !self.spec.images().is_empty()
            || !self.spec.fonts.files().is_empty()
            || self
                .spec
                .surfaces
                .values()
                .any(|s| matches!(&s.fill, Some(Fill::Gradient(g)) if !g.is_simple()))
            || !glass::sources(&self.spec).is_empty()
    }
}

impl SkinFrame for Look {
    fn painted(&self, surface: SurfaceId) -> Option<Painted> {
        self.surfaces.get(&surface).cloned()
    }
}

/// The skin now, and the decoding of its images.
pub struct SkinState {
    look: Arc<Look>,
    /// Bumped by each apply, so a slower decode of an older skin is dropped.
    generation: u64,
    _decode: Option<Task<()>>,
    /// Whether the skin's images are still being prepared.
    decoding: bool,
    /// The font families already handed to the text system.
    loaded_fonts: HashSet<String>,
    /// Why the chosen skin couldn't be used, if it couldn't.
    pub error: Option<String>,
}

impl Global for SkinState {}

impl Default for SkinState {
    fn default() -> Self {
        Self {
            look: Arc::new(Look::default_look()),
            generation: 0,
            _decode: None,
            decoding: false,
            loaded_fonts: HashSet::new(),
            error: None,
        }
    }
}

/// Goes up each time a skin is applied, or the chosen one again after its pack
/// changed.
pub fn generation(cx: &App) -> u64 {
    cx.try_global::<SkinState>()
        .map_or(0, |state| state.generation)
}

/// The skin the window shows.
pub fn look(cx: &App) -> Arc<Look> {
    cx.try_global::<SkinState>()
        .map(|state| state.look.clone())
        .unwrap_or_else(|| Arc::new(Look::default_look()))
}

/// Adds the skin's prepared images, for the performance monitor.
pub fn tally_images(tally: &mut crate::perf::ImageTally, cx: &App) {
    let look = look(cx);
    for painted in look.surfaces.values() {
        if let Some(Paint::Image(image)) = &painted.paint {
            tally.add(image);
        }
    }
    if let Some(hero) = &look.hero {
        tally.add(&hero.image);
    }
    for image in look.blurred.values() {
        tally.add(image);
    }
}

/// Whether the skin's images are still being prepared (the screenshots wait).
pub fn decoding(cx: &App) -> bool {
    cx.try_global::<SkinState>()
        .is_some_and(|state| state.decoding)
}

/// Why the chosen skin isn't shown, if it isn't.
pub fn error(cx: &App) -> Option<String> {
    cx.try_global::<SkinState>().and_then(|s| s.error.clone())
}

/// Whether the launcher was started without its skin: `--no-skin` or
/// `LODESTONE_LAUNCHER_NO_SKIN=1`.
pub fn safe_mode() -> bool {
    std::env::args().any(|arg| arg == "--no-skin")
        || std::env::var("LODESTONE_LAUNCHER_NO_SKIN").is_ok_and(|v| !v.is_empty() && v != "0")
}

/// Shows skin `id` from `store` (the default look for `None`, in safe mode, or if it
/// can't be read). Its colours apply now; its images when they are decoded.
pub fn apply(id: Option<&str>, store: &SkinStore, cx: &mut App) {
    let wanted = if safe_mode() { None } else { id };
    let (entry, error) = match wanted.map(|id| store.get(id)) {
        None => (None, None),
        Some(Ok(entry)) => (Some(entry), None),
        Some(Err(err)) => (None, Some(err.to_string())),
    };
    apply_entry(entry, store.clone(), error, cx);
}

/// Shows `entry` (the default look for `None`).
pub fn apply_entry(
    entry: Option<SkinEntry>,
    store: SkinStore,
    error: Option<String>,
    cx: &mut App,
) {
    let (id, spec) = entry.map_or_else(
        || ("default".to_string(), SkinSpec::default()),
        |e| (e.id, e.spec),
    );
    let look = Look::plain(&id, spec.clone());
    let state = cx.default_global::<SkinState>();
    state.generation += 1;
    let generation = state.generation;
    state.error = error;
    if !look.needs_preparing() {
        state.look = Arc::new(look);
        state._decode = None;
        state.decoding = false;
        cx.refresh_windows();
        return;
    }
    state.look = Arc::new(look);
    let work = cx.background_executor().spawn({
        let (id, spec) = (id.clone(), spec.clone());
        async move { prepare(&id, &spec, &store) }
    });
    let task = cx.spawn(async move |cx| {
        let prepared = work.await;
        cx.update(|cx| {
            if cx.default_global::<SkinState>().generation != generation {
                return;
            }
            let fonts = load_fonts(&prepared.fonts, cx);
            let mut look = Look::plain(&id, spec);
            finish(&mut look, prepared, fonts);
            let state = cx.default_global::<SkinState>();
            state.decoding = false;
            state.look = Arc::new(look);
            cx.refresh_windows();
        });
    });
    let state = cx.default_global::<SkinState>();
    state._decode = Some(task);
    state.decoding = true;
    cx.refresh_windows();
}

/// Hands the skin's font files to the text system (each family once) and returns
/// the families that can be used, by file name. A font the system can't use is left
/// out, so its parts keep the launcher's font.
fn load_fonts(files: &[(String, String, Vec<u8>)], cx: &mut App) -> HashMap<String, SharedString> {
    let mut usable = HashMap::new();
    for (file, family, bytes) in files {
        let state = cx.default_global::<SkinState>();
        let new = state.loaded_fonts.insert(family.clone());
        if new
            && let Err(err) = cx
                .text_system()
                .add_fonts(vec![std::borrow::Cow::Owned(bytes.clone())])
        {
            log::warn!("skin font {file} couldn't be loaded: {err}");
            continue;
        }
        let text = cx.text_system();
        let id = text.resolve_font(&gpui::font(family.clone()));
        let direct = text
            .get_font_for_id(id)
            .is_some_and(|font| font.family.as_ref() == family);
        if direct {
            usable.insert(file.clone(), SharedString::from(family.clone()));
        } else {
            log::warn!("skin font {file} ({family}) isn't usable; keeping the launcher's");
        }
    }
    usable
}

/// Puts what was prepared in the background into `look`.
fn finish(look: &mut Look, prepared: Prepared, fonts: HashMap<String, SharedString>) {
    for (surface, image, backdrop) in prepared.images {
        if let Some(backdrop) = backdrop {
            look.backdrops.insert(surface, backdrop);
        }
        let painted = look.surfaces.entry(surface).or_default();
        painted.paint = Some(Paint::Image(crate::images::render_image(image)));
    }
    if let Some((image, scrim, backdrop)) = prepared.hero {
        look.backdrops.insert(SurfaceId::Hero, backdrop);
        look.hero = Some(Background {
            image: crate::images::render_image(image),
            scrim,
        });
    }
    for part in FontPart::ALL {
        if let Some(family) = look.spec.fonts.get(part).and_then(|file| fonts.get(file)) {
            look.fonts.insert(part, family.clone());
        }
    }
    look.set_blurred(
        prepared
            .blurred
            .into_iter()
            .map(|(key, image)| (key, crate::images::render_image(image)))
            .collect(),
    );
}

/// Shows a changed description of the skin on screen: its images are prepared again
/// only when what they are made from changed, so a colour or a slider doesn't
/// decode anything.
pub fn update(entry: SkinEntry, store: SkinStore, cx: &mut App) {
    let current = look(cx);
    let images = |spec: &SkinSpec| {
        (
            SurfaceId::ALL.map(|id| {
                spec.surface(id).and_then(|s| match s.fill {
                    Some(Fill::Image(image)) => Some(image),
                    Some(Fill::Gradient(g)) if !g.is_simple() => Some(ImageFill {
                        image: format!("{g:?}"),
                        fit: Fit::Cover,
                        tint: None,
                        tint_opacity: 0,
                        blur: 0,
                    }),
                    _ => None,
                })
            }),
            spec.fonts.clone(),
            spec.hero.readability,
            spec.hero.scrim,
            glass::sources(spec),
        )
    };
    if current.id != entry.id || images(&current.spec) != images(&entry.spec) {
        return apply_entry(Some(entry), store, None, cx);
    }
    let mut look = Look::plain(&entry.id, entry.spec);
    // the prepared images and fonts carry over
    for (surface, painted) in &current.surfaces {
        if let Some(Paint::Image(image)) = &painted.paint
            && let Some(next) = look.surfaces.get_mut(surface)
        {
            next.paint = Some(Paint::Image(image.clone()));
        }
    }
    look.backdrops = current.backdrops.clone();
    look.hero = current.hero.clone();
    look.fonts = current.fonts.clone();
    look.set_blurred(current.blurred.clone());
    cx.default_global::<SkinState>().look = Arc::new(look);
    cx.refresh_windows();
}

/// What is prepared in the background when a skin is applied.
struct Prepared {
    /// Each image-filled surface's image, at its shape, and its backdrop before the
    /// tint.
    images: Vec<(SurfaceId, RgbaImage, Option<Backdrop>)>,
    /// The hero's image, its shade, and its backdrop.
    hero: Option<(RgbaImage, Scrim, Backdrop)>,
    /// The pack's font files: name, family, bytes.
    fonts: Vec<(String, String, Vec<u8>)>,
    /// Layers' pictures blurred for the materials, by layer and blur.
    blurred: Vec<((SurfaceId, u8), RgbaImage)>,
}

/// How `image` looks behind text: every fourth pixel each way is plenty.
fn measure(image: &RgbaImage) -> Option<Backdrop> {
    Backdrop::measure(
        image
            .enumerate_pixels()
            .filter(|(x, y, _)| x % 4 == 0 && y % 4 == 0)
            .map(|(_, _, p)| Rgb(((p[0] as u32) << 16) | ((p[1] as u32) << 8) | p[2] as u32)),
    )
}

/// The size a surface's image is prepared at: its usual shape at 2×, or less for
/// the big ones, which are soft or patterned anyway.
fn canvas(surface: SurfaceId) -> (u32, u32) {
    match surface {
        SurfaceId::Sidebar => (480, 1800),
        SurfaceId::TopBar | SurfaceId::PlayBar => (2400, 160),
        SurfaceId::Window | SurfaceId::Main => (1600, 1000),
        SurfaceId::PlayButton => (640, 160),
        _ => (1200, 800),
    }
}

/// `image` fitted to `size` as `fill` says: tiled or covering, blurred, then tinted.
/// Returns it and its backdrop before the tint.
fn shape(image: RgbaImage, fill: &ImageFill, (w, h): (u32, u32)) -> (RgbaImage, Option<Backdrop>) {
    // a blurred image is made at a quarter of the size, blurred, and scaled up once
    let scale = if fill.blur > 0 { 4 } else { 1 };
    let mut out = match fill.fit {
        Fit::Tile if fill.blur == 0 => tile(&image, w, h),
        Fit::Tile => image::imageops::resize(
            &tile(&image, w, h),
            w / scale,
            h / scale,
            image::imageops::FilterType::Triangle,
        ),
        Fit::Cover => image::DynamicImage::ImageRgba8(image)
            .resize_to_fill(w / scale, h / scale, image::imageops::FilterType::Triangle)
            .to_rgba8(),
    };
    if fill.blur > 0 {
        out = image::imageops::resize(
            &image::imageops::blur(&out, (fill.blur as f32 / 4.0).max(0.5)),
            w,
            h,
            image::imageops::FilterType::Triangle,
        );
    }
    let backdrop = measure(&out);
    if let Some(tint) = fill.tint.filter(|_| fill.tint_opacity > 0) {
        let t = f32::from(fill.tint_opacity) / 100.0;
        let [r, g, b] =
            [(tint.0 >> 16) & 0xFF, (tint.0 >> 8) & 0xFF, tint.0 & 0xFF].map(|c| c as f32);
        for p in out.pixels_mut() {
            p[0] = (p[0] as f32 + (r - p[0] as f32) * t).round() as u8;
            p[1] = (p[1] as f32 + (g - p[1] as f32) * t).round() as u8;
            p[2] = (p[2] as f32 + (b - p[2] as f32) * t).round() as u8;
        }
    }
    (out, backdrop)
}

/// A gradient GPUI can't draw itself (more stops, or radial), drawn into a small
/// image that is stretched over the surface: gradients are smooth, so 256 × 256 is
/// plenty.
fn rasterize(gradient: &launcher_core::ui_skin::Gradient) -> RgbaImage {
    let size = 256u32;
    let angle = f32::from(gradient.angle).to_radians();
    // CSS: 0deg points up, 90deg to the right
    let (dx, dy) = (angle.sin(), -angle.cos());
    let reach = dx.abs() * 0.5 + dy.abs() * 0.5;
    RgbaImage::from_fn(size, size, |x, y| {
        let (fx, fy) = (
            (x as f32 + 0.5) / size as f32 - 0.5,
            (y as f32 + 0.5) / size as f32 - 0.5,
        );
        let t = match gradient.kind {
            GradientKind::Linear => (fx * dx + fy * dy) / (2.0 * reach.max(1e-3)) + 0.5,
            GradientKind::Radial => (fx * fx + fy * fy).sqrt() / std::f32::consts::FRAC_1_SQRT_2,
        };
        let c = gradient.at(t).0;
        image::Rgba([(c >> 16) as u8, (c >> 8) as u8, c as u8, 255])
    })
}

/// The skin's images and fonts, ready to use: each image-filled surface's image at
/// its shape, the hero's with its shade, gradients GPUI can't draw, and the font
/// files. An image used by several surfaces is read once.
fn prepare(id: &str, spec: &SkinSpec, store: &SkinStore) -> Prepared {
    let mut read: HashMap<String, Option<RgbaImage>> = HashMap::new();
    let mut source = |name: &str| {
        read.entry(name.to_string())
            .or_insert_with(|| load(id, name, store))
            .clone()
    };
    let mut images = Vec::new();
    let mut hero = None;
    for surface in SurfaceId::ALL {
        let Some(own) = spec.surface(surface) else {
            continue;
        };
        match own.fill {
            Some(Fill::Image(fill)) if surface == SurfaceId::Hero => {
                let Some(image) = source(&fill.image) else {
                    continue;
                };
                let image = crate::instances::appearance::fit_for_hero(
                    image::DynamicImage::ImageRgba8(image),
                );
                let Some(backdrop) = measure(&image) else {
                    continue;
                };
                let scrim = match (spec.hero.readability, spec.hero.scrim) {
                    (Readability::Auto, _) => scrim_for(&image),
                    (Readability::Off, Some(n)) => Scrim {
                        text: f32::from(n) / 100.0,
                        dim: 0.0,
                    },
                    (Readability::Off, None) => Scrim {
                        text: 0.0,
                        dim: 0.0,
                    },
                };
                hero = Some((image, scrim, backdrop));
            }
            Some(Fill::Image(fill)) => {
                if let Some(image) = source(&fill.image) {
                    let (image, backdrop) = shape(image, &fill, canvas(surface));
                    images.push((surface, image, backdrop));
                }
            }
            Some(Fill::Gradient(gradient)) if !gradient.is_simple() => {
                images.push((surface, rasterize(&gradient), None));
            }
            _ => {}
        }
    }
    // the layers' pictures as they are drawn, blurred for the materials that show
    // them
    let blurred = glass::sources(spec)
        .into_iter()
        .filter_map(|(layer, blur)| {
            let picture = match images.iter().find(|(s, _, _)| *s == layer) {
                Some((_, image, _)) => image.clone(),
                None => match spec.surface(layer)?.fill? {
                    Fill::Gradient(gradient) => rasterize(&gradient),
                    _ => return None,
                },
            };
            Some(((layer, blur), glass::blur_source(&picture, blur, layer)))
        })
        .collect();
    Prepared {
        images,
        hero,
        fonts: store.fonts(id, spec),
        blurred,
    }
}

/// The sidebar's text colours for `look`: `None` for the launcher's own (nothing
/// painted, no colours of its own). Worked out from the image only where the skin
/// asks for it (`readability: auto`); else its own colours, or the theme's.
pub fn sidebar_palette(look: &Look) -> Option<Palette> {
    let own = look.spec.surface(SurfaceId::Sidebar);
    let tokens = look.spec.tokens != Default::default();
    if own.is_none() && !tokens {
        return None;
    }
    let own = own.unwrap_or_default();
    let backdrop = readable::surface_backdrop(&look.spec, SurfaceId::Sidebar, &|surface| {
        look.backdrops.get(&surface).copied()
    });
    let text = launcher_core::ui_skin::TextSpec {
        primary: own.text,
        muted: own.muted,
    };
    if look.spec.sidebar.readability == Readability::Auto {
        return Some(readable::derive(backdrop, &text, Readability::Auto));
    }
    let mut inks = readable::base_inks_on(&look.spec.tokens, backdrop.mean);
    if let Some(primary) = text.primary {
        inks.primary = primary;
    }
    // the sidebar's quieter colour covers its labels, hints and icons too
    if let Some(muted) = text.muted {
        inks.secondary = muted;
        inks.muted = muted;
        inks.faint = muted;
        inks.icon = muted;
    }
    let dark_text = inks.primary.luminance() < 0.3;
    let contrast = [inks.primary, inks.secondary, inks.muted, inks.faint]
        .into_iter()
        .map(|ink| readable::ratio_on(ink, backdrop))
        .fold(f32::MAX, f32::min);
    Some(Palette {
        inks,
        dark_text,
        backing: 0.0,
        contrast,
    })
}

/// The readability checks for `look`: each surface's text against what it shows,
/// with a suggested fix where it falls short.
pub fn checks(look: &Look) -> Vec<readable::Check> {
    readable::checks(&look.spec, &|surface| look.backdrops.get(&surface).copied())
}

/// Sets up a frame of `look`: the surfaces' paint, the sidebar's colours, the
/// theme's text colours and the fonts. Each window's root view calls it first.
pub fn set_frame(look: &Arc<Look>) {
    lodestone_ui::skin::set_frame(Some(look.clone() as Arc<dyn SkinFrame>));
    let inks = SidebarInks::of(sidebar_palette(look));
    SIDEBAR_INKS.with(|cell| cell.set(inks));
    crate::theme::ink::set(&look.spec.tokens);
    for part in FontPart::ALL {
        crate::theme::fonts::set(part, look.fonts.get(&part).cloned());
    }
}

/// The launcher's own look, for windows that draw themselves without the skin
/// (the skin editor).
pub fn launcher_look() -> Arc<Look> {
    thread_local! {
        static PLAIN: Arc<Look> = Arc::new(Look::default_look());
    }
    PLAIN.with(Arc::clone)
}

/// The sidebar's colours for `look`.
pub fn sidebar_inks_for(look: &Look) -> SidebarInks {
    SidebarInks::of(sidebar_palette(look))
}

/// The sidebar's text colours as the sidebar draws them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SidebarInks {
    pub primary: Color,
    pub secondary: Color,
    pub muted: Color,
    pub faint: Color,
    pub icon: Color,
    /// Behind the selected or hovered row.
    pub fill: Color,
    /// The quiet buttons' labels (New instance…, Import instances…).
    pub ghost: Color,
    /// A backing over the sidebar's image, under its text.
    pub backing: Option<Color>,
}

impl Default for SidebarInks {
    /// The launcher's own.
    fn default() -> Self {
        Self {
            primary: colors::GRAY_300,
            secondary: colors::GRAY_400,
            // a subtitle under a name: 6.5:1 on the sidebar (GRAY_500 was 3.9:1)
            muted: Color::rgb(0x9FA1A8),
            // the section labels (Last played, Instances, a group's name): 4.8:1 (a
            // 30% grey was 1.9:1)
            faint: Color::rgb(0x84868D),
            icon: colors::GRAY_FADED.opacity(50),
            fill: colors::GRAY_800,
            ghost: colors::WHITE.opacity(50),
            backing: None,
        }
    }
}

impl SidebarInks {
    /// Whether the sidebar's text is dark, as drawn (the light theme included): the
    /// sidebar is light.
    pub fn dark_text(&self) -> bool {
        let ink = gpui::Rgba::from(self.primary);
        0.2126 * ink.r + 0.7152 * ink.g + 0.0722 * ink.b < 0.4
    }

    fn of(palette: Option<Palette>) -> Self {
        let Some(palette) = palette else {
            return Self::default();
        };
        let color = |rgb: Rgb| Color::rgb(rgb.0);
        let layer = if palette.dark_text {
            readable::LIGHT
        } else {
            readable::DARK
        };
        Self {
            primary: color(palette.inks.primary),
            secondary: color(palette.inks.secondary),
            muted: color(palette.inks.muted),
            faint: color(palette.inks.faint),
            icon: color(palette.inks.icon),
            // a light row under dark text; the launcher's own under light text
            fill: if palette.dark_text {
                color(readable::LIGHT).opacity(70)
            } else {
                colors::GRAY_800
            },
            ghost: color(palette.inks.muted),
            backing: (palette.backing > 0.0)
                .then(|| color(layer).opacity((palette.backing * 100.0).round() as u32)),
        }
    }
}

thread_local! {
    /// The sidebar's colours for this frame, set by the window's root view.
    static SIDEBAR_INKS: std::cell::Cell<SidebarInks> =
        std::cell::Cell::new(SidebarInks::default());
}

/// The sidebar's colours for this frame.
pub fn sidebar_inks() -> SidebarInks {
    SIDEBAR_INKS.with(|cell| cell.get())
}

/// The backing of a sidebar row over an instance's banner: [`readable::DARK`] (or
/// [`readable::LIGHT`] under dark text) as strong as the row's text needs over
/// `backdrop`, from 55%. With readability off, a fixed 70%.
pub fn row_backing(backdrop: Backdrop, inks: &SidebarInks, readability: Readability) -> Color {
    let rgb = |c: Color| Rgb(c.0 >> 8);
    let dark_text = rgb(inks.primary).luminance() < 0.3;
    let layer = if dark_text {
        readable::LIGHT
    } else {
        readable::DARK
    };
    let alpha = match readability {
        Readability::Off => 0.7,
        Readability::Auto => {
            let texts = [inks.primary, inks.secondary, inks.muted, inks.faint].map(rgb);
            (11..=19)
                .map(|step| step as f32 * 0.05)
                .find(|&alpha| {
                    let seen = backdrop.under(layer, alpha);
                    let end = if dark_text { seen.dark } else { seen.light };
                    texts.iter().all(|t| t.contrast(end) >= readable::BODY)
                })
                .unwrap_or(0.95)
        }
    };
    Color::rgb(layer.0).opacity((alpha * 100.0).round() as u32)
}

/// Image `name` of skin `id`: drawn for a built-in image, read from the pack else.
fn load(id: &str, name: &str, store: &SkinStore) -> Option<RgbaImage> {
    if let Some(builtin) = name.strip_prefix("builtin:") {
        return art::draw(builtin);
    }
    let path = store.image_path(id, name)?;
    let bytes = std::fs::read(path).ok()?;
    crate::images::decode_large(&bytes).or_else(|| {
        image::load_from_memory(&bytes)
            .ok()
            .map(|image| image.to_rgba8())
    })
}

/// `image` repeated over a `width` × `height` canvas.
fn tile(image: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let (w, h) = image.dimensions();
    if w == 0 || h == 0 {
        return image.clone();
    }
    RgbaImage::from_fn(width, height, |x, y| *image.get_pixel(x % w, y % h))
}

/// The colours the window uses for an instance: its own over the skin's.
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    pub accent: Option<Rgb>,
    pub play: PlaySpec,
    pub progress: Option<Rgb>,
    /// Whether the Play button shows the skin's prepared paint (an image, or a
    /// gradient GPUI can't draw) rather than `play`'s colours.
    pub play_layer: bool,
}

/// What the window shows for `instance` with `spec`: the instance's colours win.
pub fn resolve(spec: &SkinSpec, instance: Option<&Instance>) -> Resolved {
    let mut play = spec.play.clone();
    let mut play_layer = false;
    match spec.surface(SurfaceId::PlayButton).and_then(|s| s.fill) {
        Some(Fill::Solid { color, .. }) => {
            play.color = Some(color);
            play.gradient = None;
        }
        Some(Fill::Gradient(gradient)) if gradient.is_simple() => {
            play.color = Some(gradient.stops[0].color);
            play.gradient = Some(gradient.stops[1].color);
        }
        Some(Fill::Gradient(gradient)) => {
            play.color = Some(gradient.at(0.5));
            play.gradient = None;
            play_layer = true;
        }
        Some(Fill::Image(_)) => play_layer = true,
        None => {}
    }
    if let Some(color) = instance.and_then(|i| i.play_color) {
        play.color = Some(color);
        play.gradient = None;
        play_layer = false;
    }
    let progress = match spec
        .surfaces
        .get(&SurfaceId::Progress)
        .and_then(|s| s.fill.as_ref())
    {
        Some(Fill::Solid { color, .. }) => Some(*color),
        Some(Fill::Gradient(gradient)) => Some(gradient.stops[0].color),
        _ => spec.progress,
    };
    Resolved {
        accent: instance.and_then(|i| i.accent).or(spec.accent),
        play,
        progress,
        play_layer,
    }
}

pub use launcher_core::ui_skin::art;

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::ui_skin::built_in;

    #[test]
    fn instances_override_the_skin() {
        let spec = built_in("neon").unwrap();
        let none = resolve(&spec, None);
        assert_eq!(none.accent, spec.accent);
        assert_eq!(none.play.color, spec.play.color);
        let instance = Instance {
            play_color: Some(Rgb(0x123456)),
            accent: Some(Rgb(0xABCDEF)),
            ..Instance::default()
        };
        let resolved = resolve(&spec, Some(&instance));
        assert_eq!(resolved.accent, Some(Rgb(0xABCDEF)));
        assert_eq!(resolved.play.color, Some(Rgb(0x123456)));
        // the instance's colour replaces the skin's gradient
        assert_eq!(resolved.play.gradient, None);
        // the rest of the button stays the skin's
        assert_eq!(resolved.play.glow, spec.play.glow);
        // default look and plain instance: nothing set
        let plain = resolve(&SkinSpec::default(), Some(&Instance::default()));
        assert_eq!(plain.accent, None);
        assert_eq!(plain.play, PlaySpec::default());
    }

    /// `look` with what `prepare` makes for it, without fonts.
    fn prepared(id: &str, spec: SkinSpec) -> Look {
        let store = SkinStore::at(std::path::PathBuf::from("/nonexistent"));
        let mut look = Look::plain(id, spec.clone());
        finish(&mut look, prepare(id, &spec, &store), HashMap::new());
        look
    }

    #[test]
    fn built_in_skins_prepare_their_images() {
        // built-in skins read nothing from the folder
        for id in ["grassblock", "glass", "neon"] {
            let spec = launcher_core::ui_skin::built_in(id).unwrap();
            let start = std::time::Instant::now();
            let look = prepared(id, spec.clone());
            eprintln!("{id}: {:?}", start.elapsed());
            assert!(look.hero.is_some(), "{id}");
            for surface in SurfaceId::ALL {
                if let Some(Fill::Image(_)) = spec.surface(surface).and_then(|s| s.fill) {
                    assert!(
                        surface == SurfaceId::Hero
                            || matches!(look.surfaces[&surface].paint, Some(Paint::Image(_))),
                        "{id} {surface:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn built_in_skins_ship_readable() {
        let mut failing = Vec::new();
        for id in launcher_core::ui_skin::BUILT_IN {
            let look = prepared(id, launcher_core::ui_skin::built_in(id).unwrap());
            for check in checks(&look) {
                eprintln!(
                    "{id}: {} {} {:.2}:1 {}",
                    check.surface.key(),
                    check.role.key(),
                    check.ratio,
                    check.fix.as_ref().map(|f| f.describe()).unwrap_or_default()
                );
                if !check.passes() {
                    failing.push(format!("{id} {} {}", check.surface.key(), check.role.key()));
                }
            }
            // every sidebar text style: primary, secondary, muted, faint
            if let Some(palette) = sidebar_palette(&look) {
                assert!(palette.passes(), "{id} sidebar: {palette:?}");
            }
        }
        assert!(failing.is_empty(), "{failing:#?}");
    }

    #[test]
    fn sidebar_inks_are_the_launchers_own_without_a_skin() {
        let look = Look::default_look();
        assert_eq!(
            SidebarInks::of(sidebar_palette(&look)),
            SidebarInks::default()
        );
        // a bright pack image: its colours as given (the launcher doesn't step in),
        // and the checks say so, with a fix
        let mut spec = SkinSpec::named("Bright");
        spec.sidebar.image = Some("x.png".into());
        let mut look = Look::plain("bright", spec.clone());
        look.backdrops
            .insert(SurfaceId::Sidebar, Backdrop::solid(Rgb(0xF2F2F2)));
        let palette = sidebar_palette(&look).unwrap();
        assert!(!palette.passes());
        assert_eq!(palette.backing, 0.0);
        let failing: Vec<_> = checks(&look).into_iter().filter(|c| !c.passes()).collect();
        assert!(!failing.is_empty());
        let mut fixed = spec.clone();
        readable::apply_all(&mut fixed, &failing);
        let mut look = Look::plain("bright", fixed);
        look.backdrops
            .insert(SurfaceId::Sidebar, Backdrop::solid(Rgb(0xF2F2F2)));
        assert!(
            checks(&look).iter().all(|c| c.passes()),
            "{:?}",
            checks(&look)
        );
        assert!(sidebar_palette(&look).unwrap().passes());
        // dark text sits on light rows, not the launcher's dark ones
        let inks = SidebarInks::of(sidebar_palette(&look));
        assert_ne!(inks.fill, colors::GRAY_800);
        // a pack that opts in gets the launcher's adjustments
        spec.sidebar.readability = Readability::Auto;
        let mut look = Look::plain("bright", spec);
        look.backdrops
            .insert(SurfaceId::Sidebar, Backdrop::solid(Rgb(0xF2F2F2)));
        assert!(sidebar_palette(&look).unwrap().passes());
    }

    #[test]
    fn gradients_are_drawn_or_left_to_gpui() {
        let mut spec = SkinSpec::named("G");
        spec.surfaces.insert(
            SurfaceId::TopBar,
            launcher_core::ui_skin::SurfaceSpec {
                fill: Some(Fill::Gradient(launcher_core::ui_skin::Gradient::vertical(
                    Rgb::BLACK,
                    Rgb::WHITE,
                ))),
                ..Default::default()
            },
        );
        let three = launcher_core::ui_skin::Gradient {
            kind: GradientKind::Radial,
            angle: 180,
            stops: vec![
                launcher_core::ui_skin::Stop {
                    color: Rgb::BLACK,
                    at: 0,
                },
                launcher_core::ui_skin::Stop {
                    color: Rgb(0xFF0000),
                    at: 50,
                },
                launcher_core::ui_skin::Stop {
                    color: Rgb::WHITE,
                    at: 100,
                },
            ],
        };
        spec.surfaces.insert(
            SurfaceId::Card,
            launcher_core::ui_skin::SurfaceSpec {
                fill: Some(Fill::Gradient(three.clone())),
                ..Default::default()
            },
        );
        let look = prepared("g", spec);
        assert!(matches!(
            look.surfaces[&SurfaceId::TopBar].paint,
            Some(Paint::Gradient(_))
        ));
        assert!(matches!(
            look.surfaces[&SurfaceId::Card].paint,
            Some(Paint::Image(_))
        ));
        let image = rasterize(&three);
        // black in the middle, white in the corners
        assert!(image.get_pixel(128, 128)[0] < 20);
        assert!(image.get_pixel(0, 0)[1] > 200);
    }

    #[test]
    fn banner_rows_back_their_text() {
        let inks = SidebarInks::default();
        let bright = Backdrop::solid(Rgb(0xF0F0F0));
        let backing = row_backing(bright, &inks, Readability::Auto);
        // strong enough that the faintest row text reads over a white banner
        assert!(backing.0 & 0xFF >= (0.8 * 255.0) as u32, "{backing:?}");
        let dark = row_backing(Backdrop::solid(Rgb(0x101010)), &inks, Readability::Auto);
        assert!(dark.0 & 0xFF <= backing.0 & 0xFF);
    }

    #[test]
    fn built_in_art_is_drawn() {
        for name in art::NAMES {
            let image = art::draw(name).unwrap();
            assert!(image.width() > 0, "{name}");
        }
        assert!(art::draw("nope").is_none());
        let tiled = tile(&art::draw("dirt").unwrap(), 100, 130);
        assert_eq!(tiled.dimensions(), (100, 130));
    }
}
