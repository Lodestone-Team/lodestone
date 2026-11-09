//! Readable text on a skin: measuring what a surface shows ([`Backdrop`]), checking
//! its text against it ([`checks`]) with a suggested fix for each shortfall, and,
//! for packs that opt in (`readability: "auto"`), working out text colours and a
//! backing that keep a minimum contrast ([`derive`]).
//!
//! Readability is the skin's responsibility: the launcher suggests, the creator
//! decides.

use super::{Fill, Readability, Rgb, SkinSpec, SurfaceId, TextSpec, Tokens};

/// The contrast body text and subtitles keep: WCAG AA.
pub const BODY: f32 = 4.5;
/// The contrast icons and large text keep.
pub const LARGE: f32 = 3.0;
/// The most backing a surface gets: past it the image would barely show.
pub const MAX_BACKING: f32 = 0.85;
/// The dark backing under light text.
pub const DARK: Rgb = Rgb(0x1D1E21);
/// The light backing under dark text.
pub const LIGHT: Rgb = Rgb(0xF4F6FB);

/// What an image looks like behind text: its average, and its light and dark
/// ends (the 90th and 10th percentiles of luminance), so a busy texture's bright
/// specks count without one stray pixel deciding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Backdrop {
    pub mean: Rgb,
    pub light: Rgb,
    pub dark: Rgb,
}

impl Backdrop {
    /// A plain colour.
    pub fn solid(color: Rgb) -> Self {
        Self {
            mean: color,
            light: color,
            dark: color,
        }
    }

    /// The backdrop of `pixels` (sRGB, `0xRRGGBB`). `None` without pixels.
    pub fn measure(pixels: impl IntoIterator<Item = Rgb>) -> Option<Self> {
        let mut lit: Vec<(f32, Rgb)> = pixels.into_iter().map(|p| (p.luminance(), p)).collect();
        if lit.is_empty() {
            return None;
        }
        let n = lit.len() as f64;
        let sums = lit.iter().fold([0f64; 3], |mut sum, (_, p)| {
            sum[0] += ((p.0 >> 16) & 0xFF) as f64;
            sum[1] += ((p.0 >> 8) & 0xFF) as f64;
            sum[2] += (p.0 & 0xFF) as f64;
            sum
        });
        let mean = Rgb(sums
            .iter()
            .fold(0u32, |acc, s| (acc << 8) | ((s / n).round() as u32 & 0xFF)));
        lit.sort_by(|a, b| a.0.total_cmp(&b.0));
        let at = |q: f64| lit[((lit.len() - 1) as f64 * q).round() as usize].1;
        Some(Self {
            mean,
            light: at(0.9),
            dark: at(0.1),
        })
    }

    /// The backdrop under a layer of `color` at `alpha` (0 to 1).
    pub fn under(self, color: Rgb, alpha: f32) -> Self {
        Self {
            mean: self.mean.mix(color, alpha),
            light: self.light.mix(color, alpha),
            dark: self.dark.mix(color, alpha),
        }
    }
}

/// A surface's text colours: from the brightest to the faintest, and its icons.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inks {
    pub primary: Rgb,
    pub secondary: Rgb,
    pub muted: Rgb,
    pub faint: Rgb,
    pub icon: Rgb,
}

/// The launcher's own text colours on its dark panels.
pub const LIGHT_INKS: Inks = Inks {
    primary: Rgb(0xE3E3E4),
    secondary: Rgb(0xA5A5AC),
    muted: Rgb(0x8A8E96),
    faint: Rgb(0x7A7D84),
    icon: Rgb(0x8E8E95),
};

/// Dark text, for light images.
pub const DARK_INKS: Inks = Inks {
    primary: Rgb(0x111114),
    secondary: Rgb(0x2B2D32),
    muted: Rgb(0x3F4249),
    faint: Rgb(0x4A4D55),
    icon: Rgb(0x44464B),
};

/// The text colours worked out for a surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub inks: Inks,
    /// Dark text on a light backdrop.
    pub dark_text: bool,
    /// A backing over the image, under the text: [`DARK`] (or [`LIGHT`] under
    /// dark text) at this opacity, 0 for none.
    pub backing: f32,
    /// The lowest contrast of the small text (secondary, muted, faint) against the
    /// backdrop's worst end, after the backing.
    pub contrast: f32,
}

impl Palette {
    /// Whether all text reaches [`BODY`].
    pub fn passes(&self) -> bool {
        self.contrast + 0.005 >= BODY
    }
}

/// The lowest contrast of `inks`' text against `backdrop`, for `dark_text`.
fn worst(inks: &Inks, backdrop: Backdrop, dark_text: bool) -> f32 {
    // light text loses on the light end of the image, dark text on the dark end
    let against = if dark_text {
        backdrop.dark
    } else {
        backdrop.light
    };
    [inks.primary, inks.secondary, inks.muted, inks.faint]
        .iter()
        .map(|ink| ink.contrast(against))
        .fold(f32::MAX, f32::min)
}

/// `ink` moved towards `extreme` (white for light text, near-black for dark) until
/// it keeps `min` contrast with `against`.
fn push(ink: Rgb, extreme: Rgb, against: Rgb, min: f32) -> Rgb {
    if ink.contrast(against) >= min {
        return ink;
    }
    (1..=20)
        .map(|step| ink.mix(extreme, step as f32 / 20.0))
        .find(|c| c.contrast(against) >= min)
        .unwrap_or(extreme)
}

/// `inks` made readable against `against`: text to [`BODY`], icons to [`LARGE`].
fn adjust(inks: Inks, against: Rgb, dark_text: bool) -> Inks {
    let extreme = if dark_text { Rgb(0x111114) } else { Rgb::WHITE };
    Inks {
        primary: push(inks.primary, extreme, against, BODY),
        secondary: push(inks.secondary, extreme, against, BODY),
        muted: push(inks.muted, extreme, against, BODY),
        faint: push(inks.faint, extreme, against, BODY),
        icon: push(inks.icon, extreme, against, LARGE),
    }
}

/// The text colours for `backdrop`.
///
/// With [`Readability::Auto`]: light text (or dark text, if the image is light
/// enough that dark text needs less help), each colour moved until it reaches its
/// contrast. Where even white (or black) can't, a backing is added, as light as
/// still works, so the image keeps showing. `text` gives starting colours.
///
/// With [`Readability::Off`]: the colours as given, and no backing.
pub fn derive(backdrop: Backdrop, text: &TextSpec, readability: Readability) -> Palette {
    let with = |mut inks: Inks| {
        if let Some(primary) = text.primary {
            inks.primary = primary;
        }
        if let Some(muted) = text.muted {
            inks.muted = muted;
            inks.faint = muted;
        }
        inks
    };
    if readability == Readability::Off {
        let dark_text = text.primary.is_some_and(|p| p.luminance() < 0.3);
        let inks = with(if dark_text { DARK_INKS } else { LIGHT_INKS });
        return Palette {
            inks,
            dark_text,
            backing: 0.0,
            contrast: worst(&inks, backdrop, dark_text),
        };
    }
    // the least backing with which the extreme colour (white or black) leaves
    // room for the greys to reach BODY
    let backing = |dark_text: bool| -> Option<f32> {
        let (end, layer, extreme) = if dark_text {
            (backdrop.dark, LIGHT, Rgb(0x111114))
        } else {
            (backdrop.light, DARK, Rgb::WHITE)
        };
        (0..=17)
            .map(|step| step as f32 * 0.05)
            .find(|&alpha| extreme.contrast(end.mix(layer, alpha)) >= BODY + 1.5)
    };
    let light = backing(false);
    let dark = backing(true);
    // dark text only when it needs clearly less help than light text
    let dark_text = match (light, dark) {
        (Some(l), Some(d)) => d + 0.2 < l,
        (None, Some(_)) => true,
        _ => false,
    };
    let alpha = (if dark_text { dark } else { light }).unwrap_or(MAX_BACKING);
    let layer = if dark_text { LIGHT } else { DARK };
    let seen = backdrop.under(layer, alpha);
    let against = if dark_text { seen.dark } else { seen.light };
    let inks = adjust(
        with(if dark_text { DARK_INKS } else { LIGHT_INKS }),
        against,
        dark_text,
    );
    Palette {
        inks,
        dark_text,
        backing: alpha,
        contrast: worst(&inks, seen, dark_text),
    }
}

/// The launcher's text colours with a skin's [`Tokens`] over them. The launcher's
/// own faint text is translucent; here it is `secondary` half over `under`.
pub fn base_inks(tokens: &Tokens) -> Inks {
    base_inks_on(tokens, DARK)
}

/// [`base_inks`] for text over `under`.
pub fn base_inks_on(tokens: &Tokens, under: Rgb) -> Inks {
    let secondary = tokens.secondary.unwrap_or(Tokens::BASE_SECONDARY);
    Inks {
        primary: tokens.text.unwrap_or(Tokens::BASE_TEXT),
        secondary,
        muted: tokens.muted.unwrap_or(Tokens::BASE_MUTED),
        faint: tokens
            .faint
            .unwrap_or_else(|| under.mix(Tokens::BASE_SECONDARY, 0.5)),
        icon: tokens.faint.unwrap_or_else(|| under.mix(secondary, 0.5)),
    }
}

/// Which text on a surface a check is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextRole {
    /// Names, titles, values.
    Text,
    /// Subtitles, headers, descriptions.
    Muted,
    /// Hints, icons and the footer (3:1).
    Faint,
    /// A control's label: buttons, tabs, the Play button (3:1).
    Label,
}

impl TextRole {
    pub fn key(self) -> &'static str {
        match self {
            TextRole::Text => "text",
            TextRole::Muted => "muted",
            TextRole::Faint => "faint",
            TextRole::Label => "label",
        }
    }

    /// The contrast it needs.
    pub fn target(self) -> f32 {
        match self {
            TextRole::Text | TextRole::Muted => BODY,
            TextRole::Faint | TextRole::Label => LARGE,
        }
    }
}

/// A change that makes a check pass.
#[derive(Clone, Debug, PartialEq)]
pub enum Fix {
    /// Set a surface's own `text` (or `muted`) colour.
    SurfaceText {
        surface: SurfaceId,
        role: TextRole,
        color: Rgb,
    },
    /// Set one of the theme's text colours ([`Tokens`]), used where a surface has no
    /// text colour of its own.
    Token { role: TextRole, color: Rgb },
    /// Set the shade under the hero's title, 0 to 100.
    HeroScrim(u8),
    /// Change a surface's colour (keeping its opacity), so the theme's text reads
    /// on it.
    SurfaceFill { surface: SurfaceId, color: Rgb },
    /// Tint a surface's image more strongly.
    Tint {
        surface: SurfaceId,
        tint: Rgb,
        opacity: u8,
    },
    /// Make the see-through window more opaque (the skin's `opacity`, in percent).
    Opacity(u8),
}

impl Fix {
    /// What it changes, for people: `surfaces.sidebar.muted = #A1A4AB`.
    pub fn describe(&self) -> String {
        match self {
            Fix::SurfaceText {
                surface,
                role,
                color,
            } => format!(
                "surfaces.{}.{} = {}",
                surface.key(),
                role_field(*role),
                color.hex()
            ),
            Fix::Token { role, color } => format!("tokens.{} = {}", role.key(), color.hex()),
            Fix::HeroScrim(n) => format!("hero.scrim = {n}"),
            Fix::SurfaceFill { surface, color } => {
                format!("surfaces.{}.fill = {}", surface.key(), color.hex())
            }
            Fix::Tint {
                surface,
                tint,
                opacity,
            } => format!(
                "surfaces.{}.fill.image.tint = {} at {opacity}",
                surface.key(),
                tint.hex()
            ),
            Fix::Opacity(n) => format!("window_opacity = {n}"),
        }
    }
}

/// The text a surface holds, which its checks are about.
pub fn roles(surface: SurfaceId) -> &'static [TextRole] {
    use TextRole::*;
    match surface {
        SurfaceId::Sidebar | SurfaceId::Main => &[Text, Muted, Faint],
        SurfaceId::Card
        | SurfaceId::SettingCard
        | SurfaceId::Dialog
        | SurfaceId::Popover
        | SurfaceId::SettingsNav
        | SurfaceId::Row
        | SurfaceId::PlayBar => &[Text, Muted],
        SurfaceId::TopBar | SurfaceId::Input | SurfaceId::Toast => &[Text],
        id if id.is_control() => &[Label],
        _ => &[],
    }
}

/// Whether a surface's own `text` and `muted` colours are used (elsewhere the
/// theme's [`Tokens`] are).
pub fn own_text(surface: SurfaceId) -> bool {
    matches!(surface, SurfaceId::Sidebar | SurfaceId::Toast) || surface.is_control()
}

fn role_field(role: TextRole) -> &'static str {
    match role {
        TextRole::Muted => "muted",
        _ => "text",
    }
}

/// One text colour on one surface, against what the surface shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    pub surface: SurfaceId,
    pub role: TextRole,
    /// The text's colour.
    pub color: Rgb,
    /// Its contrast against the surface's worst end.
    pub ratio: f32,
    pub target: f32,
    /// What would make it pass; `None` when it passes, or nothing can.
    pub fix: Option<Fix>,
}

impl Check {
    pub fn passes(&self) -> bool {
        self.ratio + 0.005 >= self.target
    }

    /// How far short it falls: 0 when it passes.
    pub fn shortfall(&self) -> f32 {
        (self.target - self.ratio).max(0.0)
    }
}

/// What `fill` shows behind text, over the surface's own colour. `measured` is the
/// image's backdrop for an image fill (before its tint).
pub fn backdrop_of(fill: Option<&Fill>, base: Rgb, measured: Option<Backdrop>) -> Backdrop {
    match fill {
        None => Backdrop::solid(base),
        Some(Fill::Image(image)) => {
            let seen = measured.unwrap_or(Backdrop::solid(base));
            match image.tint {
                Some(tint) => seen.under(tint, f32::from(image.tint_opacity) / 100.0),
                None => seen,
            }
        }
        Some(fill) => {
            let colors = fill.colors(base).unwrap_or_else(|| vec![base]);
            let mut sorted = colors.clone();
            sorted.sort_by(|a, b| a.luminance().total_cmp(&b.luminance()));
            Backdrop {
                mean: Backdrop::measure(colors).map_or(base, |b| b.mean),
                light: sorted[sorted.len() - 1],
                dark: sorted[0],
            }
        }
    }
}

/// The contrast of `color` against the end of `backdrop` it reads worst on.
pub fn ratio_on(color: Rgb, backdrop: Backdrop) -> f32 {
    color
        .contrast(backdrop.light)
        .min(color.contrast(backdrop.dark))
}

/// The label colour that reads best on `backdrop`: white or near-black.
pub fn best_ink(backdrop: Backdrop) -> Rgb {
    let dark = Rgb(0x111114);
    if ratio_on(Rgb::WHITE, backdrop) >= ratio_on(dark, backdrop) {
        Rgb::WHITE
    } else {
        dark
    }
}

/// `color` moved to reach `target` on `backdrop`: towards white or black, whichever
/// gets there (or gets closest).
pub fn suggest(color: Rgb, backdrop: Backdrop, target: f32) -> Rgb {
    let toward = |extreme: Rgb| {
        (0..=40)
            .map(|step| color.mix(extreme, step as f32 / 40.0))
            .find(|c| ratio_on(*c, backdrop) >= target)
    };
    let (first, second) = if backdrop.mean.luminance() < 0.18 {
        (Rgb::WHITE, Rgb(0x111114))
    } else {
        (Rgb(0x111114), Rgb::WHITE)
    };
    toward(first).or_else(|| toward(second)).unwrap_or_else(|| {
        if ratio_on(first, backdrop) >= ratio_on(second, backdrop) {
            first
        } else {
            second
        }
    })
}

/// The surface under `surface`, which shows through where it is see-through, or
/// where it paints nothing of its own (the page shows the window).
fn below(surface: SurfaceId) -> Option<SurfaceId> {
    match surface {
        SurfaceId::Main | SurfaceId::TopBar | SurfaceId::Sidebar | SurfaceId::PlayBar => {
            Some(SurfaceId::Window)
        }
        SurfaceId::Card | SurfaceId::Row | SurfaceId::Hero => Some(SurfaceId::Main),
        _ => None,
    }
}

/// The dimmer drawn over the window behind a dialog, and how strong it is.
pub const DIALOG_SCRIM: (Rgb, f32) = (Rgb(0x1D1E21), 0.6);

/// What a surface with `material` shows behind its text: what is behind it (dimmed
/// under a dialog), blurred and frosted as the material says, under its own fill at
/// the material's opacity.
fn material_backdrop(
    spec: &SkinSpec,
    surface: SurfaceId,
    material: super::Material,
    behind: SurfaceId,
    measured: &dyn Fn(SurfaceId) -> Option<Backdrop>,
) -> Backdrop {
    let mut under = surface_backdrop(spec, behind, measured);
    if surface == SurfaceId::Dialog {
        under = under.under(DIALOG_SCRIM.0, DIALOG_SCRIM.1);
    }
    let blur = f32::from(material.blur());
    if blur > 0.0 {
        // a blur pulls the light and dark ends towards the average
        let soften = blur / (blur + 8.0);
        under = Backdrop {
            mean: under.mean,
            light: under.light.mix(under.mean, soften),
            dark: under.dark.mix(under.mean, soften),
        };
    }
    under = under.under(Rgb::WHITE, f32::from(material.frost()) / 100.0);
    let fill = spec.surface(surface).and_then(|s| s.fill);
    let alpha = f32::from(material.opacity(fill.as_ref())) / 100.0;
    let own = match &fill {
        Some(Fill::Solid { color, .. }) => Backdrop::solid(*color),
        fill => backdrop_of(fill.as_ref(), surface.base(), measured(surface)),
    };
    Backdrop {
        mean: under.mean.mix(own.mean, alpha),
        light: under.light.mix(own.light, alpha),
        dark: under.dark.mix(own.dark, alpha),
    }
}

/// What `surface` shows behind its text: its fill over what is below it. `measured`
/// gives an image fill's backdrop (before its tint), when known.
pub fn surface_backdrop(
    spec: &SkinSpec,
    surface: SurfaceId,
    measured: &dyn Fn(SurfaceId) -> Option<Backdrop>,
) -> Backdrop {
    let under = || {
        below(surface)
            .map(|b| surface_backdrop(spec, b, measured))
            .unwrap_or(Backdrop::solid(surface.base()))
    };
    let own = spec.surface(surface);
    if let Some((material, behind)) = own.as_ref().and_then(|s| s.material).zip(surface.behind()) {
        return material_backdrop(spec, surface, material, behind, measured);
    }
    match own.and_then(|s| s.fill) {
        // the page paints nothing of its own: the window shows
        None if surface == SurfaceId::Main => under(),
        None => Backdrop::solid(surface.base()),
        Some(Fill::Solid { color, opacity }) => {
            let under = if opacity < 100 {
                under()
            } else {
                Backdrop::solid(color)
            };
            under.under(color, f32::from(opacity) / 100.0)
        }
        Some(fill) => backdrop_of(Some(&fill), surface.base(), measured(surface)),
    }
}

/// The surfaces worth checking: those the skin paints or gives colours, those that
/// show them through, and, when it changes the theme's text colours, the main ones.
fn checked(spec: &SkinSpec) -> Vec<SurfaceId> {
    // a see-through window puts the desktop behind the theme's text everywhere
    let tokens = spec.tokens != Tokens::default()
        || spec
            .transparency
            .is_some_and(super::Transparency::is_see_through);
    let painted = |id: SurfaceId| spec.surface(id).is_some();
    SurfaceId::ALL
        .into_iter()
        .filter(|id| !roles(*id).is_empty() || *id == SurfaceId::Hero)
        .filter(|id| {
            painted(*id)
                || below(*id).is_some_and(|b| painted(b) && *id == SurfaceId::Main)
                || (tokens
                    && matches!(
                        id,
                        SurfaceId::Sidebar
                            | SurfaceId::Main
                            | SurfaceId::Card
                            | SurfaceId::SettingCard
                            | SurfaceId::Dialog
                    ))
        })
        .collect()
}

/// The colour of `role` text on `surface`, and what is behind it.
fn subject(
    spec: &SkinSpec,
    surface: SurfaceId,
    role: TextRole,
    measured: &dyn Fn(SurfaceId) -> Option<Backdrop>,
) -> (Rgb, Backdrop) {
    let backdrop = surface_backdrop(spec, surface, measured);
    let own = spec.surface(surface).unwrap_or_default();
    let own_ok = own_text(surface);
    let base = base_inks_on(&spec.tokens, backdrop.mean);
    let color = match role {
        TextRole::Label => own.text.unwrap_or_else(|| best_ink(backdrop)),
        TextRole::Text => own.text.filter(|_| own_ok).unwrap_or(base.primary),
        TextRole::Muted => own.muted.filter(|_| own_ok).unwrap_or(base.muted),
        // the sidebar's own quieter colour covers its faint text too
        TextRole::Faint => own.muted.filter(|_| own_ok).unwrap_or(base.faint),
    };
    (color, backdrop)
}

/// The smallest change that brings `role` text on `surface` to its contrast: the
/// text's own colour where the surface has one; else the surface's colour, or its
/// image's tint, so the theme's text reads on it; else the theme's colour.
fn fix_for(
    spec: &SkinSpec,
    surface: SurfaceId,
    role: TextRole,
    measured: &dyn Fn(SurfaceId) -> Option<Backdrop>,
) -> Option<Fix> {
    let target = role.target();
    let (color, backdrop) = subject(spec, surface, role, measured);
    let works = |fix: &Fix| {
        let mut tried = spec.clone();
        apply(&mut tried, fix);
        let (color, backdrop) = subject(&tried, surface, role, measured);
        ratio_on(color, backdrop) + 0.005 >= target
    };
    let text_fix = |token: bool| {
        let better = suggest(color, backdrop, target);
        let role = if role == TextRole::Faint && !token {
            TextRole::Muted
        } else {
            role
        };
        let fix = if token {
            Fix::Token {
                role,
                color: better,
            }
        } else {
            Fix::SurfaceText {
                surface,
                role,
                color: better,
            }
        };
        works(&fix).then_some(fix)
    };
    // the surface's own colour, or its image's tint, so the text reads on it
    let light_text = color.luminance() > backdrop.mean.luminance();
    let paint_fix = || match spec.surface(surface).and_then(|s| s.fill) {
        Some(Fill::Solid { color: fill, .. }) => {
            let away = if light_text { Rgb::BLACK } else { Rgb::WHITE };
            (1..=40)
                .map(|step| Fix::SurfaceFill {
                    surface,
                    color: fill.mix(away, step as f32 / 40.0),
                })
                .find(|fix| works(fix))
        }
        Some(Fill::Image(image)) => {
            let tint = image.tint.unwrap_or(if light_text { DARK } else { LIGHT });
            (image.tint_opacity / 5 + 1..=19)
                .map(|step| Fix::Tint {
                    surface,
                    tint,
                    opacity: step * 5,
                })
                .find(|fix| works(fix))
        }
        _ => None,
    };
    if own_text(surface) || role == TextRole::Label {
        return text_fix(false).or_else(paint_fix);
    }
    // the theme's quieter colours are shared by every surface: lighten or darken them
    if role != TextRole::Text {
        return text_fix(true);
    }
    paint_fix().or_else(|| text_fix(true))
}

/// Every text colour on the surfaces the skin changes, against what each shows.
/// `measured` gives an image fill's backdrop (before its tint), when known. Packs
/// with `readability: "auto"` on a surface are left to the launcher there.
pub fn checks(spec: &SkinSpec, measured: &dyn Fn(SurfaceId) -> Option<Backdrop>) -> Vec<Check> {
    let mut out = Vec::new();
    for id in checked(spec) {
        let surface = spec.surface(id).unwrap_or_default();
        let auto = match id {
            SurfaceId::Sidebar => spec.sidebar.readability == Readability::Auto,
            SurfaceId::Hero => spec.hero.readability == Readability::Auto,
            _ => false,
        };
        if auto {
            continue;
        }
        let backdrop = surface_backdrop(spec, id, measured);
        if id == SurfaceId::Hero {
            // white title and subtitle over the image, under the shade
            if surface.fill.is_none() {
                continue;
            }
            let scrim = f32::from(spec.hero.scrim.unwrap_or(0)) / 100.0;
            let ratio = ratio_on(Rgb::WHITE, backdrop.under(DARK, scrim));
            let fix = (ratio + 0.005 < BODY)
                .then(|| {
                    (0..=18u8).map(|step| step * 5).find(|n| {
                        ratio_on(Rgb::WHITE, backdrop.under(DARK, f32::from(*n) / 100.0)) >= BODY
                    })
                })
                .flatten()
                .map(Fix::HeroScrim);
            out.push(Check {
                surface: id,
                role: TextRole::Text,
                color: Rgb::WHITE,
                ratio,
                target: BODY,
                fix,
            });
            continue;
        }
        for role in roles(id) {
            let (color, _) = subject(spec, id, *role, measured);
            let target = role.target();
            let opaque = ratio_on(color, backdrop);
            // see-through, the desktop behind shows: at worst white under light text
            // and black under dark text
            let ratio = ratio_on(color, see_through(spec, id, backdrop, None));
            let fix = if ratio + 0.005 >= target {
                None
            } else if opaque + 0.005 >= target {
                let from = spec.window_opacity.unwrap_or(super::DEFAULT_OPACITY);
                (from..=100)
                    .step_by(5)
                    .chain([100])
                    .find(|n| {
                        ratio_on(color, see_through(spec, id, backdrop, Some(*n))) + 0.005 >= target
                    })
                    .map(Fix::Opacity)
            } else {
                fix_for(spec, id, *role, measured)
            };
            out.push(Check {
                surface: id,
                role: *role,
                color,
                ratio,
                target,
                fix,
            });
        }
    }
    out
}

/// What `surface` shows behind its text while the skin's window is see-through
/// (at `opacity` percent, else the skin's): `backdrop` over whatever the desktop
/// shows, from white to black. Unchanged while the skin's window is opaque.
fn see_through(
    spec: &SkinSpec,
    surface: SurfaceId,
    backdrop: Backdrop,
    opacity: Option<u8>,
) -> Backdrop {
    if !spec
        .transparency
        .is_some_and(super::Transparency::is_see_through)
    {
        return backdrop;
    }
    let percent = opacity
        .or(spec.window_opacity)
        .unwrap_or(super::DEFAULT_OPACITY)
        .clamp(super::MIN_OPACITY, 100);
    // a surface with a material keeps as much of its fill as the material says
    let own = spec
        .surface(surface)
        .and_then(|s| s.material.map(|m| m.opacity(s.fill.as_ref())));
    let Some(cover) = super::transparency::coverage(f32::from(percent) / 100.0, surface, own)
    else {
        return backdrop;
    };
    Backdrop {
        mean: Rgb(0x808080).mix(backdrop.mean, cover),
        light: Rgb::WHITE.mix(backdrop.light, cover),
        dark: Rgb(0x000000).mix(backdrop.dark, cover),
    }
}

/// Applies `fix` to `spec`.
pub fn apply(spec: &mut SkinSpec, fix: &Fix) {
    match fix {
        Fix::SurfaceText {
            surface,
            role,
            color,
        } => {
            let entry = spec.surfaces.entry(*surface).or_default();
            match role {
                TextRole::Muted => entry.muted = Some(*color),
                _ => entry.text = Some(*color),
            }
        }
        Fix::Token { role, color } => {
            let slot = match role {
                TextRole::Muted => &mut spec.tokens.muted,
                TextRole::Faint => &mut spec.tokens.faint,
                _ => &mut spec.tokens.text,
            };
            *slot = Some(*color);
        }
        Fix::HeroScrim(n) => spec.hero.scrim = Some(*n),
        Fix::Opacity(n) => spec.window_opacity = Some(*n),
        Fix::SurfaceFill { surface, color } => {
            let current = spec.surface(*surface).and_then(|s| s.fill);
            let opacity = match current {
                Some(Fill::Solid { opacity, .. }) => opacity,
                _ => 100,
            };
            spec.surfaces.entry(*surface).or_default().fill = Some(Fill::Solid {
                color: *color,
                opacity,
            });
        }
        Fix::Tint {
            surface,
            tint,
            opacity,
        } => {
            // the surface's image as it is painted now (from `surfaces`, or the older
            // `sidebar` keys), with the stronger tint
            if let Some(Fill::Image(mut image)) = spec.surface(*surface).and_then(|s| s.fill) {
                image.tint = Some(*tint);
                image.tint_opacity = *opacity;
                spec.surfaces.entry(*surface).or_default().fill = Some(Fill::Image(image));
            }
        }
    }
}

/// Applies every check's fix. Where two fixes set the same colour, the one for the
/// higher contrast wins. Returns how many were applied.
pub fn apply_all(spec: &mut SkinSpec, checks: &[Check]) -> usize {
    let mut ordered: Vec<&Check> = checks.iter().filter(|c| c.fix.is_some()).collect();
    ordered.sort_by(|a, b| b.target.total_cmp(&a.target));
    let mut done: Vec<String> = Vec::new();
    for check in ordered {
        let fix = check.fix.as_ref().expect("filtered");
        let field = fix.describe();
        let field = field.split(" = ").next().unwrap_or_default().to_string();
        if done.contains(&field) {
            continue;
        }
        apply(spec, fix);
        done.push(field);
    }
    done.len()
}

/// Fixes everything that can be, a round at a time: each surface's own colours,
/// fills and tints first, then the theme's shared colours against what the surfaces
/// have become. Returns how many fixes were applied.
pub fn fix_all(spec: &mut SkinSpec, measured: &dyn Fn(SurfaceId) -> Option<Backdrop>) -> usize {
    let mut applied = 0;
    for _ in 0..4 {
        let found = checks(spec, measured);
        let short: Vec<Check> = found.into_iter().filter(|c| !c.passes()).collect();
        let (shared, own): (Vec<Check>, Vec<Check>) = short
            .into_iter()
            .partition(|c| matches!(c.fix, Some(Fix::Token { .. })));
        let round = if own.iter().any(|c| c.fix.is_some()) {
            apply_all(spec, &own)
        } else {
            apply_all(spec, &shared)
        };
        if round == 0 {
            break;
        }
        applied += round;
    }
    applied
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_skin::{Gradient, ImageFill, SurfaceSpec};

    fn painted(fill: Fill) -> SurfaceSpec {
        SurfaceSpec {
            fill: Some(fill),
            ..SurfaceSpec::default()
        }
    }

    #[test]
    fn contrast_follows_wcag() {
        assert!((Rgb::WHITE.contrast(Rgb::BLACK) - 21.0).abs() < 0.01);
        assert!((Rgb(0x767676).contrast(Rgb::WHITE) - 4.54).abs() < 0.02);
        // text reads against the worse end of what is behind it
        let stripes = Backdrop {
            mean: Rgb(0x808080),
            light: Rgb::WHITE,
            dark: Rgb::BLACK,
        };
        assert!(ratio_on(Rgb::WHITE, stripes) < 1.01);
        assert_eq!(best_ink(Backdrop::solid(Rgb(0xF0F0F0))), Rgb(0x111114));
        assert_eq!(best_ink(Backdrop::solid(Rgb(0x202020))), Rgb::WHITE);
    }

    #[test]
    fn surfaces_show_what_is_under_them() {
        let mut spec = SkinSpec::named("Layers");
        spec.surfaces
            .insert(SurfaceId::Window, painted(Fill::solid(Rgb(0xFFFFFF))));
        // a see-through page shows the window
        spec.surfaces.insert(
            SurfaceId::Main,
            painted(Fill::Solid {
                color: Rgb(0x000000),
                opacity: 50,
            }),
        );
        let main = surface_backdrop(&spec, SurfaceId::Main, &|_| None);
        assert_eq!(main.mean, Rgb(0x808080));
        // an opaque card doesn't
        spec.surfaces
            .insert(SurfaceId::Card, painted(Fill::solid(Rgb(0x102030))));
        assert_eq!(
            surface_backdrop(&spec, SurfaceId::Card, &|_| None).mean,
            Rgb(0x102030)
        );
        // a gradient's ends are its lightest and darkest colours
        spec.surfaces.insert(
            SurfaceId::TopBar,
            painted(Fill::Gradient(Gradient::vertical(Rgb::BLACK, Rgb::WHITE))),
        );
        let bar = surface_backdrop(&spec, SurfaceId::TopBar, &|_| None);
        assert_eq!((bar.dark, bar.light), (Rgb::BLACK, Rgb::WHITE));
        // an image under its tint, as measured
        spec.surfaces.insert(
            SurfaceId::Sidebar,
            painted(Fill::Image(ImageFill {
                image: "a.png".into(),
                fit: crate::ui_skin::Fit::Cover,
                tint: Some(Rgb::BLACK),
                tint_opacity: 50,
                blur: 0,
            })),
        );
        let side = surface_backdrop(&spec, SurfaceId::Sidebar, &|_| {
            Some(Backdrop::solid(Rgb::WHITE))
        });
        assert_eq!(side.mean, Rgb(0x808080));
    }

    #[test]
    fn checks_name_shortfalls_and_their_fixes_pass() {
        let mut spec = SkinSpec::named("Pale");
        // a pale sidebar with the launcher's light text, and a pale card
        spec.surfaces
            .insert(SurfaceId::Sidebar, painted(Fill::solid(Rgb(0xE8E4DC))));
        spec.surfaces
            .insert(SurfaceId::Card, painted(Fill::solid(Rgb(0xD0D4DA))));
        spec.surfaces.insert(
            SurfaceId::ButtonPrimary,
            SurfaceSpec {
                fill: Some(Fill::solid(Rgb(0xFFD84D))),
                text: Some(Rgb::WHITE),
                ..SurfaceSpec::default()
            },
        );
        spec.hero.image = Some("builtin:sky".into());
        let bright = Backdrop::solid(Rgb(0xC8E0F8));
        let measured = move |id: SurfaceId| (id == SurfaceId::Hero).then_some(bright);
        let found = checks(&spec, &measured);
        let short: Vec<_> = found.iter().filter(|c| !c.passes()).collect();
        let named = |surface, role| {
            short
                .iter()
                .any(|c| c.surface == surface && c.role == role && c.fix.is_some())
        };
        assert!(named(SurfaceId::Sidebar, TextRole::Text));
        assert!(named(SurfaceId::Sidebar, TextRole::Muted));
        assert!(named(SurfaceId::Card, TextRole::Text));
        assert!(named(SurfaceId::ButtonPrimary, TextRole::Label));
        assert!(named(SurfaceId::Hero, TextRole::Text));
        for check in &short {
            assert!(check.shortfall() > 0.0);
            assert!(!check.fix.as_ref().unwrap().describe().is_empty());
        }
        // the sidebar and the button get their own text colours; the card, whose
        // text is the theme's, a darker colour
        assert!(short.iter().any(|c| matches!(
            c.fix,
            Some(Fix::SurfaceText {
                surface: SurfaceId::Sidebar,
                ..
            })
        )));
        assert!(short.iter().any(|c| matches!(
            c.fix,
            Some(Fix::SurfaceFill {
                surface: SurfaceId::Card,
                ..
            })
        )));
        assert!(
            short
                .iter()
                .any(|c| matches!(c.fix, Some(Fix::HeroScrim(_))))
        );
        // fixing all leaves nothing short
        let mut fixed = spec.clone();
        assert!(fix_all(&mut fixed, &measured) > 0);
        let after = checks(&fixed, &measured);
        assert!(after.iter().all(Check::passes), "{after:#?}");
        fixed.validate().unwrap();
    }

    #[test]
    fn an_image_no_text_colour_reads_on_gets_a_stronger_tint() {
        // a sky over dirt: no one colour reads on both
        let mut spec = SkinSpec::named("Sky");
        spec.sidebar.image = Some("builtin:sky".into());
        let stripes = Backdrop {
            mean: Rgb(0x8098B0),
            light: Rgb(0xD0E8FF),
            dark: Rgb(0x3A2A1C),
        };
        let measured = move |id| (id == SurfaceId::Sidebar).then_some(stripes);
        let text = checks(&spec, &measured)
            .into_iter()
            .find(|c| c.surface == SurfaceId::Sidebar && c.role == TextRole::Text)
            .unwrap();
        assert!(!text.passes());
        let Some(Fix::Tint { opacity, .. }) = text.fix else {
            panic!("{text:?}")
        };
        assert!(opacity > 0);
        fix_all(&mut spec, &measured);
        assert!(checks(&spec, &measured).iter().all(Check::passes));
        // the tint moved into `surfaces`, over the older `sidebar` keys
        assert!(matches!(
            spec.surfaces[&SurfaceId::Sidebar].fill,
            Some(Fill::Image(ImageFill { tint: Some(_), .. }))
        ));
    }

    #[test]
    fn a_see_through_window_suggests_more_opacity() {
        let mut spec = SkinSpec::named("See-through");
        let short = |spec: &SkinSpec| {
            checks(spec, &|_| None)
                .into_iter()
                .filter(|c| !c.passes())
                .collect::<Vec<_>>()
        };
        assert!(short(&spec).is_empty(), "the launcher's own look reads");
        spec.transparency = Some(super::super::Transparency::Clear);
        spec.window_opacity = Some(super::super::MIN_OPACITY);
        let found = short(&spec);
        assert!(
            !found.is_empty(),
            "a faint window over a white desktop doesn't read"
        );
        let fix = found
            .iter()
            .find_map(|c| match c.fix {
                Some(Fix::Opacity(n)) => Some(n),
                _ => None,
            })
            .expect("raising the opacity is suggested");
        assert!(fix > super::super::MIN_OPACITY);
        fix_all(&mut spec, &|_| None);
        assert!(spec.window_opacity.unwrap() >= fix);
        assert!(short(&spec).is_empty());
    }

    #[test]
    fn a_pack_that_opts_in_is_left_to_the_launcher() {
        let mut spec = SkinSpec::named("Auto");
        spec.sidebar.image = Some("x.png".into());
        spec.sidebar.readability = Readability::Auto;
        let measured = |_| Some(Backdrop::solid(Rgb::WHITE));
        assert!(
            checks(&spec, &measured)
                .iter()
                .all(|c| c.surface != SurfaceId::Sidebar)
        );
        // and readability: auto still reads, and writes back
        let json = serde_json::to_string(&spec).unwrap();
        assert!(json.contains("\"readability\":\"auto\""));
        let back: SkinSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, spec);
        // off is the default, and isn't written
        assert_eq!(Readability::default(), Readability::Off);
    }
}
