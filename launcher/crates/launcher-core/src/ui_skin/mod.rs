//! Launcher skins: the look of the launcher's window (sidebar image, Play button,
//! accent colour, …). Not to be confused with [`crate::skin`], a Minecraft player's
//! skin.
//!
//! A skin is a pack: a folder with a [`SKIN_FILE`] and the images it names, kept in
//! `<data>/skins/<id>/`. Packs are data only: JSON, PNG, JPEG and WebP images, nothing
//! that runs. Importing one checks everything (schema, known keys, names, sizes) and
//! re-encodes every image at a sane size, so what is stored is always well formed.
//! The built-in skins ([`BUILT_IN`]) live in code; their images are drawn by the
//! launcher and named `builtin:<name>`.
//!
//! A skin paints the launcher's surfaces ([`surface::SurfaceId`]) with colours,
//! gradients or images, sets its text and border colours, its fonts, and the banners
//! new instances get. Keeping text readable is the skin's own job: the launcher
//! measures and suggests ([`readable::checks`]), and only adjusts colours itself
//! where a pack asks for it (`readability: "auto"`).
//!
//! The format is described in `launcher/docs/skins.md`.

use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::paths::DataDir;

pub mod art;
pub mod fonts;
pub mod surface;

pub use surface::{
    Fill, FontPart, Fonts, Gradient, GradientKind, ImageFill, Material, MaterialKind, SkinBanner,
    Stop, SurfaceId, SurfaceSpec, Surfaces, Tokens,
};

/// The version of [`SkinSpec`] this build reads and writes.
pub const SCHEMA: u32 = 1;
/// The pack's description.
pub const SKIN_FILE: &str = "skin.json";
/// The most a pack may hold, uncompressed.
pub const MAX_PACK_BYTES: u64 = 20 * 1024 * 1024;
/// The largest image a pack's image is decoded from.
const MAX_DECODE_SIDE: u32 = 8192;
/// The size the sidebar's image is kept at, at most.
pub const SIDEBAR_MAX: (u32, u32) = (1024, 2048);
/// The size the Play hero's image is kept at, at most.
pub const HERO_MAX: (u32, u32) = (2560, 1600);
/// The longest Play button label.
pub const MAX_LABEL: usize = 16;
/// The most blur a sidebar image gets, in pixels.
pub const MAX_BLUR: u8 = 20;
/// The size a surface's image is kept at, at most.
pub const SURFACE_MAX: (u32, u32) = (2048, 2048);

/// The built-in skins' ids, in the gallery's order.
pub const BUILT_IN: [&str; 4] = ["default", "grassblock", "glass", "neon"];
/// The images the launcher draws for the built-in skins.
pub const BUILT_IN_IMAGES: [&str; 8] = [
    "builtin:dirt",
    "builtin:sky",
    "builtin:sunset",
    "builtin:night",
    "builtin:glass",
    "builtin:aurora",
    "builtin:grid",
    "builtin:grid-dawn",
];

/// An sRGB colour, written `"#RRGGBB"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgb(pub u32);

impl Rgb {
    pub const WHITE: Rgb = Rgb(0xFFFFFF);
    pub const BLACK: Rgb = Rgb(0x000000);

    pub fn parse(text: &str) -> Option<Rgb> {
        let hex = text.trim().strip_prefix('#')?;
        if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        u32::from_str_radix(hex, 16).ok().map(Rgb)
    }

    pub fn hex(self) -> String {
        format!("#{:06X}", self.0 & 0xFF_FFFF)
    }

    fn channels(self) -> [f32; 3] {
        [
            ((self.0 >> 16) & 0xFF) as f32 / 255.0,
            ((self.0 >> 8) & 0xFF) as f32 / 255.0,
            (self.0 & 0xFF) as f32 / 255.0,
        ]
    }

    fn from_channels([r, g, b]: [f32; 3]) -> Rgb {
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
        Rgb((byte(r) << 16) | (byte(g) << 8) | byte(b))
    }

    /// `self` mixed with `other`: `amount` 0 is `self`, 1 is `other`.
    pub fn mix(self, other: Rgb, amount: f32) -> Rgb {
        let (a, b) = (self.channels(), other.channels());
        Rgb::from_channels([0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * amount))
    }

    /// The WCAG relative luminance, 0 (black) to 1 (white).
    pub fn luminance(self) -> f32 {
        let linear = |c: f32| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        let [r, g, b] = self.channels().map(linear);
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    /// The WCAG contrast ratio of two colours, 1 to 21.
    pub fn contrast(self, other: Rgb) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        let (light, dark) = if a > b { (a, b) } else { (b, a) };
        (light + 0.05) / (dark + 0.05)
    }

    /// `self`, moved towards white or black (whichever is away from `against`) until
    /// it reaches `min` contrast with `against`. Unchanged when it already does.
    pub fn readable_against(self, against: Rgb, min: f32) -> Rgb {
        if self.contrast(against) >= min {
            return self;
        }
        let target = if against.luminance() < 0.5 {
            Rgb::WHITE
        } else {
            Rgb::BLACK
        };
        (1..=20)
            .map(|step| self.mix(target, step as f32 / 20.0))
            .find(|c| c.contrast(against) >= min)
            .unwrap_or(target)
    }

    /// The text colour for a fill of `self`: white, or near-black on a light fill.
    pub fn ink(self) -> Rgb {
        if Rgb::WHITE.contrast(self) >= 3.0 {
            Rgb::WHITE
        } else {
            Rgb(0x111114)
        }
    }
}

impl Serialize for Rgb {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.hex())
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Rgb::parse(&text)
            .ok_or_else(|| serde::de::Error::custom(format!("{text:?} isn't a #RRGGBB colour")))
    }
}

/// How the sidebar's image fills it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fit {
    /// Scaled to cover the sidebar.
    #[default]
    Cover,
    /// Repeated at its own size, for a pattern.
    Tile,
}

/// Whether the launcher adjusts a surface's text to keep it readable. Readability
/// is the skin's job: by default colours are used as given, and the skin editor
/// suggests fixes. A pack can opt in to the launcher's adjustments with `auto`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Readability {
    /// Opt-in: text colours (and a darker backing if needed) are worked out from the
    /// image, so text keeps [`readable::BODY`] contrast.
    Auto,
    /// The colours are used as given.
    #[default]
    Off,
}

/// A surface's own text colours. With [`Readability::Auto`] they are starting points,
/// made readable if they need to be.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary: Option<Rgb>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muted: Option<Rgb>,
}

/// The Play button's shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    #[default]
    Rounded,
    Pill,
    /// Square corners with a bevel, like a block.
    Pixel,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidebarSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub fit: Fit,
    /// A colour over the image (or the sidebar's own background without one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<Rgb>,
    /// How strong the tint is, 0 to 100.
    #[serde(default = "tint_default", skip_serializing_if = "is_tint_default")]
    pub tint_opacity: u8,
    /// Blur of the image, 0 to [`MAX_BLUR`] pixels, applied once when it loads.
    #[serde(default, skip_serializing_if = "is_default")]
    pub blur: u8,
    #[serde(default, skip_serializing_if = "is_default")]
    pub text: TextSpec,
    #[serde(default, skip_serializing_if = "is_default")]
    pub readability: Readability,
}

impl Default for SidebarSpec {
    fn default() -> Self {
        Self {
            image: None,
            fit: Fit::Cover,
            tint: None,
            tint_opacity: tint_default(),
            blur: 0,
            text: TextSpec::default(),
            readability: Readability::Off,
        }
    }
}

fn tint_default() -> u8 {
    60
}

fn is_tint_default(value: &u8) -> bool {
    *value == tint_default()
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BannerSpec {
    /// Each instance's Play background, faintly, behind its row in the sidebar.
    #[serde(default, skip_serializing_if = "is_default")]
    pub in_sidebar: bool,
    #[serde(default, skip_serializing_if = "is_default")]
    pub readability: Readability,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeroSpec {
    /// The Play tab's image for an instance without its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// A shade under the hero's title, 0 to 100, so it reads on a bright image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scrim: Option<u8>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub readability: Readability,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaySpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Rgb>,
    /// A second colour: the button fades from `color` to this one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient: Option<Rgb>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub shape: Shape,
    /// Instead of "PLAY", at most [`MAX_LABEL`] characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// A soft glow in the button's colour.
    #[serde(default, skip_serializing_if = "is_default")]
    pub glow: bool,
}

/// `skin.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkinSpec {
    pub schema: u32,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Links, focus rings, selected tabs, primary buttons. Made readable when used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<Rgb>,
    /// Progress bars, such as the Play button's while the game prepares.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<Rgb>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub sidebar: SidebarSpec,
    #[serde(default, skip_serializing_if = "is_default")]
    pub banner: BannerSpec,
    #[serde(default, skip_serializing_if = "is_default")]
    pub hero: HeroSpec,
    #[serde(default, skip_serializing_if = "is_default")]
    pub play: PlaySpec,
    /// How each surface is painted. Over the older `sidebar`, `hero` and `play`
    /// colours and images where both are given.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub surfaces: Surfaces,
    #[serde(default, skip_serializing_if = "is_default")]
    pub tokens: Tokens,
    #[serde(default, skip_serializing_if = "is_default")]
    pub fonts: Fonts,
    /// Play backgrounds the skin gives instances: a new instance takes one at random.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub banners: Vec<SkinBanner>,
}

impl Default for SkinSpec {
    fn default() -> Self {
        Self::named("Default")
    }
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

impl SkinSpec {
    /// A skin that changes nothing.
    pub fn named(name: &str) -> Self {
        Self {
            schema: SCHEMA,
            name: name.into(),
            author: None,
            accent: None,
            progress: None,
            sidebar: SidebarSpec::default(),
            banner: BannerSpec::default(),
            hero: HeroSpec::default(),
            play: PlaySpec::default(),
            surfaces: Surfaces::new(),
            tokens: Tokens::default(),
            fonts: Fonts::default(),
            banners: Vec::new(),
        }
    }

    /// The images it names, as written (pack files or `builtin:` images), each once.
    pub fn images(&self) -> Vec<&str> {
        let mut images: Vec<&str> = [self.sidebar.image.as_deref(), self.hero.image.as_deref()]
            .into_iter()
            .flatten()
            .chain(
                self.surfaces
                    .values()
                    .filter_map(|s| s.fill.as_ref().and_then(Fill::image)),
            )
            .chain(self.banners.iter().map(|b| b.image.as_str()))
            .collect();
        let mut seen = std::collections::HashSet::new();
        images.retain(|name| seen.insert(*name));
        images
    }

    /// Every file the pack holds besides its description: images and fonts.
    pub fn files(&self) -> Vec<&str> {
        let mut files: Vec<&str> = self
            .images()
            .into_iter()
            .filter(|name| !name.starts_with("builtin:"))
            .collect();
        files.extend(self.fonts.files());
        files
    }

    /// How `id` is painted, from `surfaces` and the older keys that paint it
    /// (`sidebar`, `hero.image`, `play.color`): `None` when the skin leaves it be.
    pub fn surface(&self, id: SurfaceId) -> Option<SurfaceSpec> {
        let mut spec = self.surfaces.get(&id).cloned().unwrap_or_default();
        match id {
            SurfaceId::Sidebar => {
                let legacy = &self.sidebar;
                if spec.fill.is_none() {
                    spec.fill = match (&legacy.image, legacy.tint) {
                        (Some(image), tint) => Some(Fill::Image(ImageFill {
                            image: image.clone(),
                            fit: legacy.fit,
                            tint,
                            tint_opacity: if tint.is_some() {
                                legacy.tint_opacity
                            } else {
                                0
                            },
                            blur: legacy.blur,
                        })),
                        (None, Some(tint)) => Some(Fill::Solid {
                            color: tint,
                            opacity: legacy.tint_opacity,
                        }),
                        (None, None) => None,
                    };
                }
                spec.text = spec.text.or(legacy.text.primary);
                spec.muted = spec.muted.or(legacy.text.muted);
            }
            SurfaceId::Hero if spec.fill.is_none() => {
                spec.fill = self.hero.image.clone().map(|image| {
                    Fill::Image(ImageFill {
                        image,
                        fit: Fit::Cover,
                        tint: None,
                        tint_opacity: 0,
                        blur: 0,
                    })
                });
            }
            SurfaceId::PlayButton if spec.fill.is_none() => {
                spec.fill = match (self.play.color, self.play.gradient) {
                    (Some(from), Some(to)) => Some(Fill::Gradient(Gradient::vertical(from, to))),
                    (Some(color), None) => Some(Fill::solid(color)),
                    _ => None,
                };
            }
            _ => {}
        }
        (!spec.is_empty()).then_some(spec)
    }

    /// The banner called `id`.
    pub fn banner(&self, id: &str) -> Option<&SkinBanner> {
        self.banners.iter().find(|b| b.id == id)
    }

    /// Checks the values. Images are checked by name only here.
    pub fn validate(&self) -> Result<(), SkinError> {
        if self.schema != SCHEMA {
            return Err(SkinError::Invalid(format!(
                "schema {} isn't supported (this launcher reads {SCHEMA})",
                self.schema
            )));
        }
        let name = self.name.trim();
        if name.is_empty() || name.chars().count() > 48 || name.chars().any(char::is_control) {
            return Err(SkinError::Invalid(
                "the name must be 1 to 48 characters".into(),
            ));
        }
        if let Some(author) = &self.author
            && (author.chars().count() > 64 || author.chars().any(char::is_control))
        {
            return Err(SkinError::Invalid("the author is too long".into()));
        }
        if let Some(label) = &self.play.label
            && (label.chars().count() > MAX_LABEL || label.chars().any(char::is_control))
        {
            return Err(SkinError::Invalid(format!(
                "the Play label must be at most {MAX_LABEL} characters"
            )));
        }
        if self.sidebar.tint_opacity > 100 {
            return Err(SkinError::Invalid(
                "sidebar.tint_opacity must be 0 to 100".into(),
            ));
        }
        if self.sidebar.blur > MAX_BLUR {
            return Err(SkinError::Invalid(format!(
                "sidebar.blur must be 0 to {MAX_BLUR}"
            )));
        }
        if self.hero.scrim.is_some_and(|s| s > 100) {
            return Err(SkinError::Invalid("hero.scrim must be 0 to 100".into()));
        }
        for (id, surface) in &self.surfaces {
            surface.validate(*id)?;
        }
        self.fonts.validate()?;
        surface::validate_banners(&self.banners)?;
        for image in self.images() {
            check_image_name(image)?;
        }
        Ok(())
    }
}

/// Whether `name` may name an image: a built-in image, or a plain file name with an
/// image extension, inside the pack.
pub fn check_image_name(name: &str) -> Result<(), SkinError> {
    if name.starts_with("builtin:") {
        return if BUILT_IN_IMAGES.contains(&name) {
            Ok(())
        } else {
            Err(SkinError::Invalid(format!("{name} isn't a built-in image")))
        };
    }
    let plain = !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && !name.contains(['/', '\\', ':'])
        && !name.contains("..")
        && !name.chars().any(char::is_control);
    let image = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "webp"
            )
        });
    if plain && image {
        Ok(())
    } else {
        Err(SkinError::Invalid(format!(
            "{name:?} must be a PNG, JPEG or WebP file in the pack"
        )))
    }
}

/// Why a skin couldn't be read, imported or saved.
#[derive(Debug)]
pub enum SkinError {
    Io(io::Error),
    Invalid(String),
    NotFound(String),
    /// Built-in skins can't be changed or deleted.
    BuiltIn,
}

impl std::fmt::Display for SkinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkinError::Io(err) => err.fmt(f),
            SkinError::Invalid(reason) => write!(f, "Not a valid skin: {reason}"),
            SkinError::NotFound(id) => write!(f, "No skin {id}"),
            SkinError::BuiltIn => f.write_str("A built-in skin can't be changed"),
        }
    }
}

impl std::error::Error for SkinError {}

impl From<io::Error> for SkinError {
    fn from(err: io::Error) -> Self {
        SkinError::Io(err)
    }
}

/// A skin: built-in, or a pack in the skins folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkinEntry {
    pub id: String,
    pub spec: SkinSpec,
    pub builtin: bool,
}

fn surface(fill: Fill) -> SurfaceSpec {
    SurfaceSpec {
        fill: Some(fill),
        ..SurfaceSpec::default()
    }
}

fn banner(id: &str, name: &str, image: &str) -> SkinBanner {
    SkinBanner {
        id: id.into(),
        name: Some(name.into()),
        image: format!("builtin:{image}"),
    }
}

fn gradient(angle: i16, stops: &[u32]) -> Fill {
    let last = stops.len().max(2) - 1;
    Fill::Gradient(Gradient {
        kind: GradientKind::Linear,
        angle,
        stops: stops
            .iter()
            .enumerate()
            .map(|(i, c)| Stop {
                color: Rgb(*c),
                at: ((i * 100) / last) as u8,
            })
            .collect(),
    })
}

/// The built-in skins' descriptions.
pub fn built_in(id: &str) -> Option<SkinSpec> {
    let mut spec = SkinSpec::named("Default");
    match id {
        "default" => {}
        "grassblock" => {
            spec.name = "Grassblock".into();
            spec.accent = Some(Rgb(0x8BD86A));
            spec.progress = Some(Rgb(0x6FC24A));
            spec.sidebar = SidebarSpec {
                image: Some("builtin:dirt".into()),
                fit: Fit::Tile,
                tint: Some(Rgb(0x140E08)),
                tint_opacity: 55,
                ..SidebarSpec::default()
            };
            spec.hero.image = Some("builtin:sky".into());
            spec.hero.scrim = Some(55);
            spec.play = PlaySpec {
                color: Some(Rgb(0x5DB83B)),
                gradient: Some(Rgb(0x3C8A22)),
                shape: Shape::Pixel,
                label: None,
                glow: false,
            };
            spec.surfaces = Surfaces::from([
                (
                    SurfaceId::Sidebar,
                    SurfaceSpec {
                        text: Some(Rgb(0xF4EEE6)),
                        muted: Some(Rgb(0xD9CDBE)),
                        ..SurfaceSpec::default()
                    },
                ),
                (
                    SurfaceId::TopBar,
                    surface(gradient(180, &[0x3A2A1C, 0x2A1F15])),
                ),
                (
                    SurfaceId::PlayBar,
                    surface(gradient(180, &[0x2A1F15, 0x1E160F])),
                ),
                (
                    SurfaceId::ButtonPrimary,
                    SurfaceSpec {
                        fill: Some(Fill::solid(Rgb(0x3F8F2A))),
                        radius: Some(2),
                        ..SurfaceSpec::default()
                    },
                ),
                (SurfaceId::Toggle, surface(Fill::solid(Rgb(0x5DB83B)))),
                (
                    SurfaceId::Pill,
                    SurfaceSpec {
                        fill: Some(Fill::solid(Rgb(0x3F8F2A))),
                        text: Some(Rgb::WHITE),
                        ..SurfaceSpec::default()
                    },
                ),
            ]);
            spec.tokens.secondary = Some(Rgb(0xCFC8BF));
            spec.tokens.muted = Some(Rgb(0xA29C94));
            spec.tokens.faint = Some(Rgb(0xBDB5AB));
            spec.fonts.heading = Some("Lodestone Pixel".into());
            spec.fonts.play = Some("Lodestone Pixel".into());
            spec.banners = vec![
                banner("day", "Day", "sky"),
                banner("sunset", "Sunset", "sunset"),
                banner("night", "Night", "night"),
            ];
        }
        "glass" => {
            spec.name = "Glass".into();
            spec.accent = Some(Rgb(0xA9C1FF));
            spec.progress = Some(Rgb(0xA9C1FF));
            spec.sidebar = SidebarSpec {
                image: Some("builtin:glass".into()),
                fit: Fit::Cover,
                tint: Some(Rgb(0x10131A)),
                tint_opacity: 45,
                blur: 16,
                ..SidebarSpec::default()
            };
            spec.hero.image = Some("builtin:glass".into());
            spec.banner.in_sidebar = true;
            spec.play = PlaySpec {
                color: Some(Rgb(0xF4F6FB)),
                gradient: None,
                shape: Shape::Pill,
                label: None,
                glow: false,
            };
            let frost = |opacity: u8| SurfaceSpec {
                fill: Some(Fill::Solid {
                    color: Rgb(0xFFFFFF),
                    opacity,
                }),
                border: Some(Rgb(0x343B4F)),
                radius: Some(14),
                ..SurfaceSpec::default()
            };
            spec.surfaces = Surfaces::from([
                (
                    SurfaceId::Sidebar,
                    SurfaceSpec {
                        text: Some(Rgb(0xF4F6FB)),
                        muted: Some(Rgb(0xC9D0E0)),
                        ..SurfaceSpec::default()
                    },
                ),
                (
                    SurfaceId::Window,
                    surface(Fill::Image(ImageFill {
                        image: "builtin:aurora".into(),
                        fit: Fit::Cover,
                        tint: Some(Rgb(0x0B0E16)),
                        tint_opacity: 72,
                        blur: 20,
                    })),
                ),
                (
                    SurfaceId::Main,
                    surface(Fill::Solid {
                        color: Rgb(0x0E111A),
                        opacity: 55,
                    }),
                ),
                (
                    SurfaceId::TopBar,
                    surface(Fill::Solid {
                        color: Rgb(0x0E111A),
                        opacity: 70,
                    }),
                ),
                (
                    SurfaceId::PlayBar,
                    surface(Fill::Solid {
                        color: Rgb(0x0E111A),
                        opacity: 70,
                    }),
                ),
                (SurfaceId::Card, frost(6)),
                (SurfaceId::SettingCard, frost(5)),
                (
                    SurfaceId::Dialog,
                    SurfaceSpec {
                        fill: Some(Fill::Solid {
                            color: Rgb(0x161A26),
                            opacity: 97,
                        }),
                        border: Some(Rgb(0x343B4F)),
                        radius: Some(18),
                        ..SurfaceSpec::default()
                    },
                ),
                (
                    SurfaceId::Popover,
                    SurfaceSpec {
                        fill: Some(Fill::solid(Rgb(0x1A1F2E))),
                        border: Some(Rgb(0x343B4F)),
                        radius: Some(12),
                        ..SurfaceSpec::default()
                    },
                ),
                (
                    SurfaceId::ButtonPrimary,
                    SurfaceSpec {
                        fill: Some(Fill::solid(Rgb(0xA9C1FF))),
                        radius: Some(16),
                        ..SurfaceSpec::default()
                    },
                ),
                (SurfaceId::Toggle, surface(Fill::solid(Rgb(0x7F9CF5)))),
            ]);
            spec.tokens = Tokens {
                secondary: Some(Rgb(0xC2C7D3)),
                muted: Some(Rgb(0xAEB4C2)),
                faint: Some(Rgb(0xA3A9B7)),
                ..Tokens::default()
            };
            spec.fonts.tab = Some("Clash Grotesk".into());
            spec.banners = vec![
                banner("glass", "Glass", "glass"),
                banner("aurora", "Aurora", "aurora"),
            ];
        }
        "neon" => {
            spec.name = "Neon".into();
            spec.accent = Some(Rgb(0x22D3EE));
            spec.progress = Some(Rgb(0xFF2BD6));
            spec.sidebar = SidebarSpec {
                image: None,
                fit: Fit::Cover,
                tint: Some(Rgb(0x07070C)),
                tint_opacity: 92,
                ..SidebarSpec::default()
            };
            spec.hero.image = Some("builtin:grid".into());
            spec.hero.scrim = Some(35);
            spec.play = PlaySpec {
                color: Some(Rgb(0xD61FB4)),
                gradient: Some(Rgb(0x7C1FD6)),
                shape: Shape::Rounded,
                label: None,
                glow: true,
            };
            let panel = |color: u32| SurfaceSpec {
                fill: Some(Fill::solid(Rgb(color))),
                border: Some(Rgb(0x2E1F4A)),
                ..SurfaceSpec::default()
            };
            spec.surfaces = Surfaces::from([
                (SurfaceId::Window, surface(Fill::solid(Rgb(0x0A0912)))),
                (SurfaceId::Main, surface(Fill::solid(Rgb(0x0A0912)))),
                (
                    SurfaceId::TopBar,
                    surface(gradient(90, &[0x0B0716, 0x1A0B2E, 0x0B0716])),
                ),
                (
                    SurfaceId::PlayBar,
                    surface(gradient(90, &[0x0B0716, 0x1A0B2E])),
                ),
                (SurfaceId::Card, panel(0x12101E)),
                (SurfaceId::SettingCard, panel(0x12101E)),
                (SurfaceId::Dialog, panel(0x110F1C)),
                (SurfaceId::SettingsNav, surface(Fill::solid(Rgb(0x0C0A16)))),
                (SurfaceId::Popover, panel(0x151226)),
                (
                    SurfaceId::Input,
                    SurfaceSpec {
                        fill: Some(Fill::solid(Rgb(0x07060D))),
                        border: Some(Rgb(0x22D3EE)),
                        ..SurfaceSpec::default()
                    },
                ),
                (
                    SurfaceId::ButtonPrimary,
                    surface(gradient(90, &[0xC21AA3, 0x6A19C2])),
                ),
                (SurfaceId::Toggle, surface(Fill::solid(Rgb(0x22D3EE)))),
                (
                    SurfaceId::Pill,
                    SurfaceSpec {
                        fill: Some(Fill::solid(Rgb(0x22D3EE))),
                        text: Some(Rgb(0x07060D)),
                        ..SurfaceSpec::default()
                    },
                ),
                (
                    SurfaceId::Progress,
                    surface(gradient(90, &[0xFF2BD6, 0x22D3EE])),
                ),
            ]);
            spec.tokens = Tokens {
                text: Some(Rgb(0xECEAF6)),
                secondary: Some(Rgb(0xB9B4D0)),
                muted: Some(Rgb(0x9A94B8)),
                faint: Some(Rgb(0x857FA6)),
                border: Some(Rgb(0x2E1F4A)),
            };
            spec.fonts.heading = Some("Clash Grotesk".into());
            spec.banners = vec![
                banner("grid", "Grid", "grid"),
                banner("dawn", "Dawn", "grid-dawn"),
            ];
        }
        _ => return None,
    }
    Some(spec)
}

/// The skins folder, `<data>/skins`.
#[derive(Clone, Debug)]
pub struct SkinStore {
    dir: PathBuf,
}

impl SkinStore {
    pub fn new(data: &DataDir) -> Self {
        Self::at(data.root().join("skins"))
    }

    /// A store in `dir`, for tests.
    pub fn at(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The built-in skins, then the packs by name. Packs that can't be read are left
    /// out.
    pub fn list(&self) -> Vec<SkinEntry> {
        let mut entries: Vec<SkinEntry> = BUILT_IN
            .iter()
            .filter_map(|id| {
                built_in(id).map(|spec| SkinEntry {
                    id: (*id).to_string(),
                    spec,
                    builtin: true,
                })
            })
            .collect();
        let mut packs: Vec<SkinEntry> = fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let id = entry.file_name().to_str()?.to_string();
                if id.starts_with('.') {
                    return None;
                }
                self.get(&id).ok()
            })
            .collect();
        packs.sort_by_key(|entry| entry.spec.name.to_lowercase());
        entries.extend(packs);
        entries
    }

    pub fn get(&self, id: &str) -> Result<SkinEntry, SkinError> {
        if let Some(spec) = built_in(id) {
            return Ok(SkinEntry {
                id: id.into(),
                spec,
                builtin: true,
            });
        }
        check_id(id)?;
        let path = self.dir.join(id).join(SKIN_FILE);
        let text = fs::read_to_string(&path).map_err(|err| match err.kind() {
            io::ErrorKind::NotFound => SkinError::NotFound(id.into()),
            _ => SkinError::Io(err),
        })?;
        let spec: SkinSpec =
            serde_json::from_str(&text).map_err(|err| SkinError::Invalid(err.to_string()))?;
        spec.validate()?;
        Ok(SkinEntry {
            id: id.into(),
            spec,
            builtin: false,
        })
    }

    /// The file of image `name` in skin `id`; `None` for a built-in image.
    pub fn image_path(&self, id: &str, name: &str) -> Option<PathBuf> {
        if name.starts_with("builtin:") || check_id(id).is_err() || check_image_name(name).is_err()
        {
            return None;
        }
        Some(self.dir.join(id).join(name))
    }

    /// The file of font `name` in skin `id`.
    pub fn font_path(&self, id: &str, name: &str) -> Option<PathBuf> {
        if check_id(id).is_err() || fonts::check_font_name(name).is_err() {
            return None;
        }
        Some(self.dir.join(id).join(name))
    }

    /// The file of any pack file (image or font) `name` in skin `id`.
    fn file_path(&self, id: &str, name: &str) -> Option<PathBuf> {
        self.image_path(id, name)
            .or_else(|| self.font_path(id, name))
    }

    /// The file of pack `id`'s description.
    pub fn spec_path(&self, id: &str) -> PathBuf {
        self.dir.join(id).join(SKIN_FILE)
    }

    /// Saves a pack's description.
    pub fn save(&self, id: &str, spec: &SkinSpec) -> Result<(), SkinError> {
        if built_in(id).is_some() {
            return Err(SkinError::BuiltIn);
        }
        check_id(id)?;
        spec.validate()?;
        let dir = self.dir.join(id);
        fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(spec).map_err(io::Error::other)?;
        crate::storage::write_atomic(&dir.join(SKIN_FILE), json.as_bytes())?;
        Ok(())
    }

    /// A new pack named `name`, a copy of skin `from` with its images.
    pub fn duplicate(&self, from: &str, name: &str) -> Result<String, SkinError> {
        let source = self.get(from)?;
        let mut spec = source.spec.clone();
        spec.name = name.chars().take(48).collect();
        let id = self.unique_id(&spec.name);
        let dir = self.dir.join(&id);
        fs::create_dir_all(&dir)?;
        for file in spec.files() {
            if let Some(path) = self.file_path(from, file) {
                fs::copy(path, dir.join(file))?;
            }
        }
        self.save(&id, &spec)?;
        Ok(id)
    }

    /// Deletes a pack.
    pub fn delete(&self, id: &str) -> Result<(), SkinError> {
        if built_in(id).is_some() {
            return Err(SkinError::BuiltIn);
        }
        check_id(id)?;
        fs::remove_dir_all(self.dir.join(id))?;
        Ok(())
    }

    /// Copies `source`, an image file, into pack `id` for `role`, re-encoded at the
    /// role's size. Returns the name to put in the description.
    pub fn add_image(&self, id: &str, source: &Path, role: ImageRole) -> Result<String, SkinError> {
        if built_in(id).is_some() {
            return Err(SkinError::BuiltIn);
        }
        check_id(id)?;
        let bytes = read_limited(fs::File::open(source)?, MAX_PACK_BYTES)?;
        let dir = self.dir.join(id);
        fs::create_dir_all(&dir)?;
        let stem = role.stem();
        // a new name each time, so Undo's older description keeps its image
        let name = (1..)
            .map(|n| format!("{stem}-{n}"))
            .find(|stem| {
                !dir.join(format!("{stem}.png")).exists()
                    && !dir.join(format!("{stem}.jpg")).exists()
            })
            .expect("a free name");
        let name = write_image(&bytes, &dir, &name, role.max())?;
        Ok(name)
    }

    /// Copies `source`, a .ttf or .otf font, into pack `id` after checking it. Returns
    /// the name to put in the description, and the font's family.
    pub fn add_font(&self, id: &str, source: &Path) -> Result<(String, String), SkinError> {
        if built_in(id).is_some() {
            return Err(SkinError::BuiltIn);
        }
        check_id(id)?;
        let bytes = read_limited(fs::File::open(source)?, fonts::MAX_FONT_BYTES as u64)
            .map_err(|_| SkinError::Invalid("a font file may be at most 2 MB".into()))?;
        let family = fonts::font_family(&bytes)?;
        let ext = if bytes.starts_with(b"OTTO") {
            "otf"
        } else {
            "ttf"
        };
        let stem: String = source
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("font")
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .take(40)
            .collect();
        let dir = self.dir.join(id);
        fs::create_dir_all(&dir)?;
        let name = format!("{stem}.{ext}");
        fs::write(dir.join(&name), &bytes)?;
        Ok((name, family))
    }

    /// The font files of pack `id`, read and checked, with their families. Fonts that
    /// fail the check are left out.
    pub fn fonts(&self, id: &str, spec: &SkinSpec) -> Vec<(String, String, Vec<u8>)> {
        spec.fonts
            .files()
            .into_iter()
            .filter_map(|name| {
                let path = self.font_path(id, name)?;
                let bytes =
                    read_limited(fs::File::open(path).ok()?, fonts::MAX_FONT_BYTES as u64).ok()?;
                let family = fonts::font_family(&bytes).ok()?;
                Some((name.to_string(), family, bytes))
            })
            .collect()
    }

    /// Image `name` of skin `id`, decoded: drawn for a built-in image.
    pub fn load_image(&self, id: &str, name: &str) -> Option<image::RgbaImage> {
        if let Some(builtin) = name.strip_prefix("builtin:") {
            return art::draw(builtin);
        }
        let path = self.image_path(id, name)?;
        let mut reader = image::ImageReader::open(path)
            .ok()?
            .with_guessed_format()
            .ok()?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MAX_DECODE_SIDE);
        limits.max_image_height = Some(MAX_DECODE_SIDE);
        reader.limits(limits);
        Some(reader.decode().ok()?.to_rgba8())
    }

    /// How each image-filled surface of skin `id` looks behind text, before its tint,
    /// for [`readable::checks`].
    pub fn measure(&self, id: &str, spec: &SkinSpec) -> BTreeMap<SurfaceId, readable::Backdrop> {
        SurfaceId::ALL
            .into_iter()
            .filter_map(|surface| {
                let name = spec.surface(surface)?.fill?.image()?.to_string();
                let image = self.load_image(id, &name)?;
                // a few thousand pixels are plenty
                let step = ((image.width() as u64 * image.height() as u64 / 20_000) as f64)
                    .sqrt()
                    .max(1.0) as u32;
                let backdrop = readable::Backdrop::measure(
                    image
                        .enumerate_pixels()
                        .filter(|(x, y, _)| x % step == 0 && y % step == 0)
                        .map(|(_, _, p)| {
                            Rgb(((p[0] as u32) << 16) | ((p[1] as u32) << 8) | p[2] as u32)
                        }),
                )?;
                Some((surface, backdrop))
            })
            .collect()
    }

    /// Checks a pack (a .zip file or a folder) as importing would, without keeping
    /// it: its description, files, images and fonts. Returns its description, and
    /// its readability checks.
    pub fn check_pack(source: &Path) -> Result<(SkinSpec, Vec<readable::Check>), SkinError> {
        let dir = std::env::temp_dir().join(format!(
            "lodestone-skin-check-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        let store = SkinStore::at(dir.clone());
        let result = store.import(source).and_then(|id| {
            let spec = store.get(&id)?.spec;
            let measured = store.measure(&id, &spec);
            let checks = readable::checks(&spec, &|surface| measured.get(&surface).copied());
            Ok((spec, checks))
        });
        let _ = fs::remove_dir_all(&dir);
        result
    }

    /// Imports a pack from a .zip file or a folder, and returns its id.
    pub fn import(&self, source: &Path) -> Result<String, SkinError> {
        let files = if source.is_dir() {
            read_folder(source)?
        } else {
            read_zip(source)?
        };
        self.import_files(files)
    }

    fn import_files(&self, files: Vec<(String, Vec<u8>)>) -> Result<String, SkinError> {
        let spec_bytes = files
            .iter()
            .find(|(name, _)| name == SKIN_FILE)
            .map(|(_, bytes)| bytes)
            .ok_or_else(|| SkinError::Invalid(format!("there is no {SKIN_FILE}")))?;
        let mut spec: SkinSpec = serde_json::from_slice(spec_bytes)
            .map_err(|err| SkinError::Invalid(err.to_string()))?;
        spec.validate()?;
        // only the description, its images and its fonts: nothing else may come along
        for (name, _) in &files {
            if name != SKIN_FILE && !spec.files().contains(&name.as_str()) {
                return Err(SkinError::Invalid(format!(
                    "{name:?} isn't used by the skin; a pack holds only {SKIN_FILE}, its images and its fonts"
                )));
            }
        }
        fs::create_dir_all(&self.dir)?;
        let staging = tempdir_in(&self.dir)?;
        let result = (|| {
            let rename = |image: &mut Option<String>, role: ImageRole| -> Result<(), SkinError> {
                let Some(name) = image.as_mut() else {
                    return Ok(());
                };
                if name.starts_with("builtin:") {
                    return Ok(());
                }
                let bytes = files
                    .iter()
                    .find(|(file, _)| file == name)
                    .map(|(_, bytes)| bytes)
                    .ok_or_else(|| SkinError::Invalid(format!("{name:?} is missing")))?;
                let stem = Path::new(name.as_str())
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("image")
                    .to_string();
                *name = write_image(bytes, &staging, &stem, role.max())?;
                Ok(())
            };
            rename(&mut spec.sidebar.image, ImageRole::Sidebar)?;
            rename(&mut spec.hero.image, ImageRole::Hero)?;
            // an image used by several surfaces is re-encoded once
            let mut renamed: BTreeMap<String, String> = BTreeMap::new();
            let mut rename_once = |name: &mut String, role: ImageRole| -> Result<(), SkinError> {
                if let Some(done) = renamed.get(name.as_str()) {
                    *name = done.clone();
                    return Ok(());
                }
                let before = name.clone();
                let mut slot = Some(name.clone());
                rename(&mut slot, role)?;
                *name = slot.expect("still named");
                renamed.insert(before, name.clone());
                Ok(())
            };
            for surface in spec.surfaces.values_mut() {
                if let Some(Fill::Image(image)) = surface.fill.as_mut() {
                    rename_once(&mut image.image, ImageRole::Surface)?;
                }
            }
            for banner in &mut spec.banners {
                rename_once(&mut banner.image, ImageRole::Banner)?;
            }
            for name in spec.fonts.files() {
                let bytes = files
                    .iter()
                    .find(|(file, _)| file == name)
                    .map(|(_, bytes)| bytes)
                    .ok_or_else(|| SkinError::Invalid(format!("{name:?} is missing")))?;
                fonts::font_family(bytes)?;
                fs::write(staging.join(name), bytes)?;
            }
            let json = serde_json::to_string_pretty(&spec).map_err(io::Error::other)?;
            fs::write(staging.join(SKIN_FILE), json)?;
            let id = self.unique_id(&spec.name);
            crate::fsx::rename(&staging, self.dir.join(&id))?;
            Ok(id)
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        result
    }

    /// Writes pack (or built-in skin) `id` to a .zip file at `dest`.
    pub fn export(&self, id: &str, dest: &Path) -> Result<(), SkinError> {
        let entry = self.get(id)?;
        let file = fs::File::create(dest)?;
        let mut zip = zip::ZipWriter::new(io::BufWriter::new(file));
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let zipped = |err: zip::result::ZipError| SkinError::Io(io::Error::other(err));
        zip.start_file(SKIN_FILE, options).map_err(zipped)?;
        let json = serde_json::to_string_pretty(&entry.spec).map_err(io::Error::other)?;
        zip.write_all(json.as_bytes())?;
        for file in entry.spec.files() {
            if let Some(path) = self.file_path(id, file) {
                zip.start_file(file, options).map_err(zipped)?;
                zip.write_all(&fs::read(path)?)?;
            }
        }
        zip.finish()
            .map_err(zipped)?
            .into_inner()
            .map_err(|err| err.into_error())?;
        Ok(())
    }

    /// An id for a new pack named `name`, not taken yet.
    fn unique_id(&self, name: &str) -> String {
        let mut slug: String = name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        slug = slug.trim_matches('-').to_string();
        while slug.contains("--") {
            slug = slug.replace("--", "-");
        }
        if slug.is_empty() {
            slug = "skin".into();
        }
        slug.truncate(40);
        let taken = |id: &str| built_in(id).is_some() || self.dir.join(id).exists();
        if !taken(&slug) {
            return slug;
        }
        (2..)
            .map(|n| format!("{slug}-{n}"))
            .find(|id| !taken(id))
            .expect("a free id")
    }
}

/// What an image is for, which decides the size it is kept at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageRole {
    Sidebar,
    Hero,
    /// Any other surface's image.
    Surface,
    /// One of the skin's instance banners.
    Banner,
}

impl ImageRole {
    fn max(self) -> (u32, u32) {
        match self {
            ImageRole::Sidebar => SIDEBAR_MAX,
            ImageRole::Hero | ImageRole::Banner => HERO_MAX,
            ImageRole::Surface => SURFACE_MAX,
        }
    }

    fn stem(self) -> &'static str {
        match self {
            ImageRole::Sidebar => "sidebar",
            ImageRole::Hero => "hero",
            ImageRole::Surface => "surface",
            ImageRole::Banner => "banner",
        }
    }
}

/// A pack's id: lowercase letters, digits and dashes.
fn check_id(id: &str) -> Result<(), SkinError> {
    if !id.is_empty()
        && id.len() <= 48
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        Ok(())
    } else {
        Err(SkinError::NotFound(id.into()))
    }
}

/// Decodes `bytes` as an image, scales it to fit `max`, and writes it to `dir` as
/// `<stem>.png` (with transparency) or `<stem>.jpg`. Returns the file's name.
fn write_image(bytes: &[u8], dir: &Path, stem: &str, max: (u32, u32)) -> Result<String, SkinError> {
    let mut reader = image::ImageReader::new(io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(SkinError::Io)?;
    let format = reader.format();
    if !matches!(
        format,
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::WebP)
    ) {
        return Err(SkinError::Invalid(format!(
            "{stem} isn't a PNG, JPEG or WebP image"
        )));
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DECODE_SIDE);
    limits.max_image_height = Some(MAX_DECODE_SIDE);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|err| SkinError::Invalid(format!("{stem}: {err}")))?;
    let image = if image.width() > max.0 || image.height() > max.1 {
        image.resize(max.0, max.1, image::imageops::FilterType::Lanczos3)
    } else {
        image
    };
    let stem: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .take(40)
        .collect();
    if image.color().has_alpha() {
        let name = format!("{stem}.png");
        image
            .to_rgba8()
            .save_with_format(dir.join(&name), image::ImageFormat::Png)
            .map_err(|err| SkinError::Io(io::Error::other(err)))?;
        Ok(name)
    } else {
        let name = format!("{stem}.jpg");
        let file = fs::File::create(dir.join(&name))?;
        let mut encoder =
            image::codecs::jpeg::JpegEncoder::new_with_quality(io::BufWriter::new(file), 90);
        encoder
            .encode_image(&image.to_rgb8())
            .map_err(|err| SkinError::Io(io::Error::other(err)))?;
        Ok(name)
    }
}

/// The files of a pack folder (not its subfolders).
fn read_folder(dir: &Path) -> Result<Vec<(String, Vec<u8>)>, SkinError> {
    let mut files = Vec::new();
    let mut total = 0u64;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !kind.is_file() || skipped(&name) {
            continue;
        }
        let bytes = read_limited(fs::File::open(entry.path())?, MAX_PACK_BYTES - total)?;
        total += bytes.len() as u64;
        files.push((name, bytes));
    }
    Ok(files)
}

/// The files of a pack .zip: flat, or all in one folder.
fn read_zip(path: &Path) -> Result<Vec<(String, Vec<u8>)>, SkinError> {
    let file = fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file))
        .map_err(|err| SkinError::Invalid(format!("not a .zip file ({err})")))?;
    let mut names = Vec::new();
    for index in 0..zip.len() {
        let entry = zip
            .by_index(index)
            .map_err(|err| SkinError::Invalid(err.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        // no absolute paths, no "..", nothing outside the pack
        let name = entry
            .enclosed_name()
            .ok_or_else(|| {
                SkinError::Invalid(format!("{:?} points outside the pack", entry.name()))
            })?
            .to_path_buf();
        names.push((index, name));
    }
    // a pack zipped as its folder: strip the one folder all files are in
    let parts: Vec<Vec<String>> = names
        .iter()
        .map(|(_, name)| {
            name.components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect()
        })
        .collect();
    let prefix = match parts.first() {
        Some(first)
            if first.len() == 2 && parts.iter().all(|p| p.len() == 2 && p[0] == first[0]) =>
        {
            1
        }
        _ => 0,
    };
    let mut files = Vec::new();
    let mut total = 0u64;
    for ((index, _), parts) in names.iter().zip(parts) {
        if parts.len() - prefix != 1 {
            if parts.first().is_some_and(|p| p == "__MACOSX") {
                continue;
            }
            return Err(SkinError::Invalid(format!(
                "{:?} is in a subfolder; a pack holds only {SKIN_FILE} and its images",
                parts.join("/")
            )));
        }
        let name = parts[prefix].clone();
        if skipped(&name) {
            continue;
        }
        let entry = zip
            .by_index(*index)
            .map_err(|err| SkinError::Invalid(err.to_string()))?;
        if total + entry.size() > MAX_PACK_BYTES {
            return Err(too_big());
        }
        let bytes = read_limited(entry, MAX_PACK_BYTES - total)?;
        total += bytes.len() as u64;
        files.push((name, bytes));
    }
    Ok(files)
}

/// Files a pack may carry that are ignored: what macOS and Windows leave behind.
fn skipped(name: &str) -> bool {
    matches!(name, ".DS_Store" | "Thumbs.db" | "desktop.ini") || name.starts_with("._")
}

fn too_big() -> SkinError {
    SkinError::Invalid(format!(
        "a pack may hold at most {} MB",
        MAX_PACK_BYTES / 1024 / 1024
    ))
}

/// Reads at most `limit` bytes; more is an error.
fn read_limited(reader: impl Read, limit: u64) -> Result<Vec<u8>, SkinError> {
    let mut bytes = Vec::new();
    reader.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(too_big());
    }
    Ok(bytes)
}

/// A new empty folder in `dir`, for a pack being imported.
fn tempdir_in(dir: &Path) -> io::Result<PathBuf> {
    for n in 0..1000u32 {
        let path = dir.join(format!(".import-{}-{n}", std::process::id()));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    Err(io::Error::other("no free folder to import into"))
}

pub mod readable;

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        image::RgbImage::from_pixel(width, height, image::Rgb([40, 120, 60]))
            .write_to(&mut io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn zip_of(files: &[(&str, &[u8])]) -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut zip = zip::ZipWriter::new(file.reopen().unwrap());
        for (name, bytes) in files {
            zip.start_file(*name, zip::write::FileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        file
    }

    #[test]
    fn packs_carry_surface_images_fonts_and_banners() {
        let dir = tempfile::tempdir().unwrap();
        let store = SkinStore::at(dir.path().join("skins"));
        let json = r##"{
            "schema": 1,
            "name": "Rich",
            "surfaces": {
                "top_bar": {"fill": {"gradient": {"angle": 90, "stops": ["#101010", "#303030", "#505050"]}}},
                "card": {"fill": {"image": {"image": "paper.png", "fit": "tile"}}, "radius": 12, "border": "#444444"},
                "button_primary": {"fill": "#2266CC", "text": "#FFFFFF"}
            },
            "tokens": {"muted": "#A0A4AC"},
            "fonts": {"heading": "serif.ttf", "play": "Lodestone Pixel"},
            "banners": [
                {"id": "dawn", "name": "Dawn", "image": "dawn.png"},
                {"id": "night", "image": "builtin:night"}
            ]
        }"##;
        let font = super::fonts::tests::tiny_font("Pack Serif");
        let pack = zip_of(&[
            (SKIN_FILE, json.as_bytes()),
            ("paper.png", &png(32, 32)),
            ("dawn.png", &png(64, 40)),
            ("serif.ttf", &font),
        ]);
        let id = store.import(pack.path()).unwrap();
        let entry = store.get(&id).unwrap();
        let spec = &entry.spec;
        assert_eq!(spec.banner("dawn").unwrap().name.as_deref(), Some("Dawn"));
        for file in spec.files() {
            assert!(
                dir.path().join("skins").join(&id).join(file).is_file(),
                "{file}"
            );
        }
        assert_eq!(spec.fonts.files(), ["serif.ttf"]);
        let fonts = store.fonts(&id, spec);
        assert_eq!(fonts[0].1, "Pack Serif");
        assert!(matches!(
            spec.surface(SurfaceId::Card).unwrap().fill,
            Some(Fill::Image(_))
        ));
        // out and back in, the same pack
        let out = dir.path().join("rich.zip");
        store.export(&id, &out).unwrap();
        let again = store.import(&out).unwrap();
        assert_eq!(store.get(&again).unwrap().spec.surfaces, spec.surfaces);
        // the copy keeps its font
        let copy = store.duplicate(&id, "Rich copy").unwrap();
        assert_eq!(store.fonts(&copy, &store.get(&copy).unwrap().spec).len(), 1);
    }

    #[test]
    fn packs_are_checked_without_being_kept() {
        let dir = tempfile::tempdir().unwrap();
        let pack = dir.path().join("pale");
        fs::create_dir_all(&pack).unwrap();
        fs::write(
            pack.join(SKIN_FILE),
            r##"{"schema": 1, "name": "Pale", "surfaces": {"sidebar": {"fill": {"image": "white.png"}}}}"##,
        )
        .unwrap();
        let mut bytes = Vec::new();
        image::RgbImage::from_pixel(8, 8, image::Rgb([250, 250, 250]))
            .write_to(&mut io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        fs::write(pack.join("white.png"), bytes).unwrap();
        let (spec, checks) = SkinStore::check_pack(&pack).unwrap();
        assert_eq!(spec.name, "Pale");
        // the launcher's light text on a white image: short, with a fix
        let text = checks
            .iter()
            .find(|c| c.surface == SurfaceId::Sidebar && c.role == readable::TextRole::Text)
            .unwrap();
        assert!(!text.passes() && text.fix.is_some(), "{text:?}");
        fs::write(pack.join("extra.sh"), "echo").unwrap();
        assert!(SkinStore::check_pack(&pack).is_err());
    }

    #[test]
    fn packs_refuse_what_they_cant_hold() {
        let dir = tempfile::tempdir().unwrap();
        let store = SkinStore::at(dir.path().join("skins"));
        let import = |json: &str, files: &[(&str, &[u8])]| {
            let mut all = vec![(SKIN_FILE, json.as_bytes())];
            all.extend_from_slice(files);
            store.import(zip_of(&all).path())
        };
        // a font that isn't one
        let err = import(
            r#"{"schema": 1, "name": "X", "fonts": {"body": "x.ttf"}}"#,
            &[("x.ttf", b"#!/bin/sh\necho not a font")],
        )
        .unwrap_err();
        assert!(err.to_string().contains("font"), "{err}");
        // an unknown surface, an unknown font, a font by path
        for json in [
            r##"{"schema": 1, "name": "X", "surfaces": {"titlebar": {"fill": "#000000"}}}"##,
            r#"{"schema": 1, "name": "X", "fonts": {"body": "Comic Sans"}}"#,
            r#"{"schema": 1, "name": "X", "fonts": {"body": "../evil.ttf"}}"#,
            r##"{"schema": 1, "name": "X", "surfaces": {"card": {"fill": "#000000", "text": "#FFFFFF"}}}"##,
            r##"{"schema": 1, "name": "X", "surfaces": {"toggle": {"fill": {"image": "a.png"}}}}"##,
            r#"{"schema": 1, "name": "X", "banners": [{"id": "Bad Id", "image": "builtin:sky"}]}"#,
        ] {
            assert!(import(json, &[]).is_err(), "{json}");
        }
    }

    /// The examples in the docs and the create-skin skill are valid skins.
    #[test]
    fn documented_examples_are_valid() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for doc in ["docs/skins.md", "skills/create-skin/SKILL.md"] {
            let text = fs::read_to_string(root.join(doc)).unwrap();
            let mut found = 0;
            for block in text.split("```json").skip(1) {
                let json = block.split("```").next().unwrap();
                if !json.contains("\"schema\"") {
                    continue;
                }
                let spec: SkinSpec =
                    serde_json::from_str(json).unwrap_or_else(|e| panic!("{doc}: {e}\n{json}"));
                spec.validate().unwrap_or_else(|e| panic!("{doc}: {e}"));
                found += 1;
            }
            assert!(found > 0, "{doc} has an example");
        }
    }

    #[test]
    fn colours_parse_and_print() {
        assert_eq!(Rgb::parse("#1d8eb2"), Some(Rgb(0x1D8EB2)));
        assert_eq!(Rgb(0x1D8EB2).hex(), "#1D8EB2");
        assert_eq!(Rgb::parse("1D8EB2"), None);
        assert_eq!(Rgb::parse("#12345"), None);
        assert_eq!(Rgb::parse("#GGGGGG"), None);
    }

    #[test]
    fn contrast_is_checked_and_fixed() {
        assert!((Rgb::WHITE.contrast(Rgb::BLACK) - 21.0).abs() < 0.01);
        let bg = Rgb(0x26282C);
        // a dark blue link on a dark background is lightened until it reads
        let fixed = Rgb(0x1F2F6F).readable_against(bg, 4.5);
        assert!(fixed.contrast(bg) >= 4.5);
        // a readable colour is left alone
        assert_eq!(Rgb(0x59B2F3).readable_against(bg, 4.5), Rgb(0x59B2F3));
        // text on a light button is dark
        assert_eq!(Rgb(0xF4F6FB).ink(), Rgb(0x111114));
        assert_eq!(Rgb(0x1D8EB2).ink(), Rgb::WHITE);
    }

    #[test]
    fn specs_parse_strictly() {
        let spec: SkinSpec = serde_json::from_str(
            r##"{"schema":1,"name":"Mine","accent":"#7BD65D","play":{"shape":"pill","label":"GO"}}"##,
        )
        .unwrap();
        assert_eq!(spec.play.shape, Shape::Pill);
        assert_eq!(spec.sidebar.tint_opacity, 60);
        spec.validate().unwrap();
        // unknown keys are refused
        assert!(
            serde_json::from_str::<SkinSpec>(r#"{"schema":1,"name":"x","script":"y"}"#).is_err()
        );
        assert!(
            serde_json::from_str::<SkinSpec>(r#"{"schema":1,"name":"x","play":{"onclick":"y"}}"#)
                .is_err()
        );
        let mut bad = spec.clone();
        bad.schema = 2;
        assert!(bad.validate().is_err());
        let mut bad = spec.clone();
        bad.play.label = Some("x".repeat(17));
        assert!(bad.validate().is_err());
        let mut bad = spec.clone();
        bad.sidebar.blur = 21;
        assert!(bad.validate().is_err());
    }

    #[test]
    fn backdrops_are_measured_by_their_ends() {
        use readable::Backdrop;
        // mostly dark, with 20% bright specks: the light end sees the specks
        let pixels = (0..100).map(|i| {
            if i % 5 == 0 {
                Rgb(0xE0E0E0)
            } else {
                Rgb(0x202020)
            }
        });
        let backdrop = Backdrop::measure(pixels).unwrap();
        assert_eq!(backdrop.light, Rgb(0xE0E0E0));
        assert_eq!(backdrop.dark, Rgb(0x202020));
        assert!(backdrop.mean.luminance() > Rgb(0x202020).luminance());
        assert!(Backdrop::measure(std::iter::empty()).is_none());
        // a dark layer darkens every end
        let under = backdrop.under(Rgb::BLACK, 0.5);
        assert!(under.light.luminance() < backdrop.light.luminance());
    }

    #[test]
    fn text_is_made_readable_on_any_backdrop() {
        use readable::{BODY, Backdrop, derive};
        let none = TextSpec::default();
        for color in [
            0x000000, 0x26282C, 0x6B4A2F, 0x8A8E96, 0xC8C8C8, 0xFFFFFF, 0x5FA8F0,
        ] {
            let palette = derive(Backdrop::solid(Rgb(color)), &none, Readability::Auto);
            assert!(palette.passes(), "#{color:06X}: {palette:?}");
        }
        // a busy dirt texture: light text, greys lifted, little or no backing
        let dirt = Backdrop {
            mean: Rgb(0x6F4C31),
            light: Rgb(0x8B623F),
            dark: Rgb(0x5C3F28),
        };
        let palette = derive(dirt, &none, Readability::Auto);
        assert!(!palette.dark_text);
        assert!(palette.contrast >= BODY);
        // a white image takes dark text rather than a heavy backing
        let paper = derive(Backdrop::solid(Rgb(0xF8F8F8)), &none, Readability::Auto);
        assert!(paper.dark_text, "{paper:?}");
        assert!(paper.backing < 0.3);
        // the launcher's own panel needs no backing
        let panel = derive(Backdrop::solid(Rgb(0x26282C)), &none, Readability::Auto);
        assert_eq!(panel.backing, 0.0);
        // given colours are starting points, made readable
        let given = TextSpec {
            primary: Some(Rgb(0x404040)),
            muted: Some(Rgb(0x303030)),
        };
        let fixed = derive(dirt, &given, Readability::Auto);
        assert!(fixed.passes());
        // off: as given, and it says it falls short
        let off = derive(dirt, &given, Readability::Off);
        assert_eq!(off.inks.muted, Rgb(0x303030));
        assert_eq!(off.backing, 0.0);
        assert!(!off.passes());
    }

    #[test]
    fn image_names_stay_in_the_pack() {
        for good in ["sidebar.png", "Hero 1.JPG", "x.webp", "builtin:dirt"] {
            assert!(check_image_name(good).is_ok(), "{good}");
        }
        for bad in [
            "../x.png",
            "a/b.png",
            "a\\b.png",
            "/etc/x.png",
            "C:x.png",
            ".hidden.png",
            "x.svg",
            "x.html",
            "x.png.exe",
            "builtin:nope",
            "",
        ] {
            assert!(check_image_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn built_ins_are_valid_and_listed_first() {
        for id in BUILT_IN {
            built_in(id).unwrap().validate().unwrap();
        }
        let dir = tempfile::tempdir().unwrap();
        let store = SkinStore::at(dir.path().join("skins"));
        let ids: Vec<String> = store.list().into_iter().map(|e| e.id).collect();
        assert_eq!(ids, BUILT_IN);
        assert!(store.save("neon", &SkinSpec::named("x")).is_err());
        assert!(store.delete("default").is_err());
    }

    #[test]
    fn import_export_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = SkinStore::at(dir.path().join("skins"));
        let spec = br##"{"schema":1,"name":"My Look","accent":"#7BD65D",
            "sidebar":{"image":"side.png","fit":"tile","blur":4},
            "hero":{"image":"builtin:sky"}}"##;
        let big = png(3000, 1000);
        let zip = zip_of(&[("My Look/skin.json", spec), ("My Look/side.png", &big)]);
        let id = store.import(zip.path()).unwrap();
        assert_eq!(id, "my-look");
        let entry = store.get(&id).unwrap();
        assert_eq!(entry.spec.accent, Some(Rgb(0x7BD65D)));
        // re-encoded at the sidebar's size
        let side = entry.spec.sidebar.image.clone().unwrap();
        let (w, h) = image::image_dimensions(store.image_path(&id, &side).unwrap()).unwrap();
        assert!(w <= SIDEBAR_MAX.0 && h <= SIDEBAR_MAX.1);
        // export and import again: the same skin under a new id
        let out = dir.path().join("out.zip");
        store.export(&id, &out).unwrap();
        let again = store.import(&out).unwrap();
        assert_eq!(again, "my-look-2");
        assert_eq!(store.get(&again).unwrap().spec, entry.spec);
        // a copy keeps the images
        let copy = store.duplicate(&id, "Copy").unwrap();
        assert!(store.image_path(&copy, &side).unwrap().exists());
        store.delete(&copy).unwrap();
        assert!(store.get(&copy).is_err());
    }

    #[test]
    fn bad_packs_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let store = SkinStore::at(dir.path().join("skins"));
        let spec = br#"{"schema":1,"name":"x"}"#;
        // extra files, scripts, traversal, subfolders, missing images, no description
        let cases: Vec<Vec<(&str, &[u8])>> = vec![
            vec![("skin.json", spec), ("run.sh", b"#!/bin/sh")],
            vec![("skin.json", spec), ("index.html", b"<script>")],
            vec![("skin.json", spec), ("../evil.png", b"x")],
            vec![("a/skin.json", spec), ("b/x.png", b"x")],
            vec![(
                "skin.json",
                br#"{"schema":1,"name":"x","hero":{"image":"gone.png"}}"#,
            )],
            vec![("readme.txt", b"hi")],
        ];
        for files in cases {
            let zip = zip_of(&files);
            assert!(
                store.import(zip.path()).is_err(),
                "{:?}",
                files.iter().map(|f| f.0).collect::<Vec<_>>()
            );
        }
        // nothing was left behind
        let left: Vec<_> = fs::read_dir(store.dir()).unwrap().collect();
        assert!(left.is_empty(), "{left:?}");
        // a fake image is refused
        let fake = zip_of(&[
            (
                "skin.json",
                br#"{"schema":1,"name":"x","hero":{"image":"h.png"}}"#,
            ),
            ("h.png", b"<html></html>"),
        ]);
        assert!(store.import(fake.path()).is_err());
    }

    #[test]
    fn packs_over_the_limit_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let store = SkinStore::at(dir.path().join("skins"));
        let huge = vec![0u8; (MAX_PACK_BYTES + 1) as usize];
        let zip = zip_of(&[
            (
                "skin.json",
                br#"{"schema":1,"name":"x","hero":{"image":"h.png"}}"#,
            ),
            ("h.png", &huge),
        ]);
        assert!(matches!(
            store.import(zip.path()),
            Err(SkinError::Invalid(_))
        ));
    }
}
