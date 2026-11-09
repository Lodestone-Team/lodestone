//! What a skin can paint: the launcher's surfaces ([`SurfaceId`]), each with a fill
//! (a colour, a gradient or an image), a border, corner radius and text colours; the
//! theme's text and border colours ([`Tokens`]); fonts for each part of the window
//! ([`Fonts`]); and the banners a skin gives new instances ([`SkinBanner`]).

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use super::{Fit, Rgb, SkinError, check_image_name, is_default};

/// The most stops a gradient has.
pub const MAX_STOPS: usize = 8;
/// The thickest border, in pixels.
pub const MAX_BORDER: u8 = 4;
/// The roundest corner, in pixels.
pub const MAX_RADIUS: u8 = 32;
/// The most banners a skin ships.
pub const MAX_BANNERS: usize = 16;
/// The most blur an image fill gets, in pixels.
pub const MAX_FILL_BLUR: u8 = 20;

/// A part of the window a skin can paint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceId {
    /// Behind everything.
    Window,
    /// The bar along the top, with the logo.
    TopBar,
    Sidebar,
    /// The page beside the sidebar.
    Main,
    /// The Play tab's picture, for instances without their own.
    Hero,
    /// The bar along the bottom with the Play button.
    PlayBar,
    /// Panels on a page.
    Card,
    /// The groups of rows in settings.
    SettingCard,
    /// Rows in lists: content, worlds, servers.
    Row,
    /// Dialogs and the Settings window.
    Dialog,
    /// The Settings window's column of pages.
    SettingsNav,
    /// Menus and pop-ups.
    Popover,
    /// Text fields.
    Input,
    ButtonPrimary,
    ButtonSecondary,
    ButtonGhost,
    ButtonDanger,
    /// Switches, while on.
    Toggle,
    /// The instance's tabs (Play, Content, …): the selected one.
    Tab,
    /// Segmented pickers and pills: the selected segment.
    Pill,
    PlayButton,
    /// Progress bars.
    Progress,
    /// Notices at the bottom of the window, such as Undo.
    Toast,
}

impl SurfaceId {
    pub const ALL: [SurfaceId; 23] = [
        SurfaceId::Window,
        SurfaceId::TopBar,
        SurfaceId::Sidebar,
        SurfaceId::Main,
        SurfaceId::Hero,
        SurfaceId::PlayBar,
        SurfaceId::Card,
        SurfaceId::SettingCard,
        SurfaceId::Row,
        SurfaceId::Dialog,
        SurfaceId::SettingsNav,
        SurfaceId::Popover,
        SurfaceId::Input,
        SurfaceId::ButtonPrimary,
        SurfaceId::ButtonSecondary,
        SurfaceId::ButtonGhost,
        SurfaceId::ButtonDanger,
        SurfaceId::Toggle,
        SurfaceId::Tab,
        SurfaceId::Pill,
        SurfaceId::PlayButton,
        SurfaceId::Progress,
        SurfaceId::Toast,
    ];

    /// The id as written in `skin.json`.
    pub fn key(self) -> &'static str {
        match self {
            SurfaceId::Window => "window",
            SurfaceId::TopBar => "top_bar",
            SurfaceId::Sidebar => "sidebar",
            SurfaceId::Main => "main",
            SurfaceId::Hero => "hero",
            SurfaceId::PlayBar => "play_bar",
            SurfaceId::Card => "card",
            SurfaceId::SettingCard => "setting_card",
            SurfaceId::Row => "row",
            SurfaceId::Dialog => "dialog",
            SurfaceId::SettingsNav => "settings_nav",
            SurfaceId::Popover => "popover",
            SurfaceId::Input => "input",
            SurfaceId::ButtonPrimary => "button_primary",
            SurfaceId::ButtonSecondary => "button_secondary",
            SurfaceId::ButtonGhost => "button_ghost",
            SurfaceId::ButtonDanger => "button_danger",
            SurfaceId::Toggle => "toggle",
            SurfaceId::Tab => "tab",
            SurfaceId::Pill => "pill",
            SurfaceId::PlayButton => "play_button",
            SurfaceId::Progress => "progress",
            SurfaceId::Toast => "toast",
        }
    }

    pub fn from_key(key: &str) -> Option<SurfaceId> {
        SurfaceId::ALL.into_iter().find(|id| id.key() == key)
    }

    /// The launcher's own colour for the surface, without a skin.
    pub fn base(self) -> Rgb {
        match self {
            SurfaceId::Window | SurfaceId::Main | SurfaceId::Row | SurfaceId::Tab => Rgb(0x212327),
            SurfaceId::TopBar | SurfaceId::Popover | SurfaceId::Toast => Rgb(0x2B2D32),
            SurfaceId::Sidebar
            | SurfaceId::PlayBar
            | SurfaceId::Card
            | SurfaceId::SettingCard
            | SurfaceId::Dialog => Rgb(0x26282C),
            SurfaceId::SettingsNav => Rgb(0x212327),
            SurfaceId::Hero | SurfaceId::Input | SurfaceId::Pill => Rgb(0x1D1E21),
            SurfaceId::ButtonSecondary | SurfaceId::ButtonDanger => Rgb(0x2B2D32),
            SurfaceId::ButtonGhost => Rgb(0x212327),
            SurfaceId::ButtonPrimary => Rgb(0x3BA55D),
            SurfaceId::Toggle => Rgb(0x3BA55D),
            SurfaceId::PlayButton => Rgb(0x3BA55D),
            SurfaceId::Progress => Rgb(0x3BA55D),
        }
    }

    /// Whether the surface holds text of its own (a progress bar doesn't).
    pub fn has_text(self) -> bool {
        !matches!(self, SurfaceId::Progress | SurfaceId::Toggle)
    }

    /// Whether its text is a label on a filled control (large and bold), so 3:1 is
    /// enough.
    pub fn is_control(self) -> bool {
        matches!(
            self,
            SurfaceId::ButtonPrimary
                | SurfaceId::ButtonSecondary
                | SurfaceId::ButtonGhost
                | SurfaceId::ButtonDanger
                | SurfaceId::Tab
                | SurfaceId::Pill
                | SurfaceId::PlayButton
        )
    }

    /// What is behind it, which a [`Material`] shows through. Only the panels have
    /// a material: not the window (nothing is behind it), the hero (a picture) or
    /// the controls.
    pub fn behind(self) -> Option<SurfaceId> {
        match self {
            // the window's bars and the page; dialogs, menus and notices float over
            // the window, which is the part of what is behind them a skin paints
            SurfaceId::TopBar
            | SurfaceId::Sidebar
            | SurfaceId::Main
            | SurfaceId::PlayBar
            | SurfaceId::Dialog
            | SurfaceId::Popover
            | SurfaceId::Toast => Some(SurfaceId::Window),
            // inside the Settings window
            SurfaceId::SettingCard | SurfaceId::SettingsNav => Some(SurfaceId::Dialog),
            // on the page
            SurfaceId::Card | SurfaceId::Row | SurfaceId::Input => Some(SurfaceId::Main),
            _ => None,
        }
    }

    /// Whether it takes a [`Material`]: the panels.
    pub fn takes_material(self) -> bool {
        self.behind().is_some()
    }

    /// Whether an image fill makes sense on it: the large surfaces.
    pub fn takes_image(self) -> bool {
        matches!(
            self,
            SurfaceId::Window
                | SurfaceId::TopBar
                | SurfaceId::Sidebar
                | SurfaceId::Main
                | SurfaceId::Hero
                | SurfaceId::PlayBar
                | SurfaceId::Card
                | SurfaceId::SettingCard
                | SurfaceId::Dialog
                | SurfaceId::SettingsNav
                | SurfaceId::Popover
                | SurfaceId::Toast
                | SurfaceId::PlayButton
        )
    }
}

/// Which way a gradient runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GradientKind {
    /// Along a line at `angle`.
    #[default]
    Linear,
    /// Out from the middle.
    Radial,
}

/// A colour along a gradient, at `at` percent of the way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stop {
    pub color: Rgb,
    pub at: u8,
}

/// A gradient: two to [`MAX_STOPS`] colours.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gradient {
    pub kind: GradientKind,
    /// Degrees, as in CSS: 0 runs bottom to top, 90 left to right, 180 (the default)
    /// top to bottom.
    pub angle: i16,
    pub stops: Vec<Stop>,
}

impl Gradient {
    /// Two colours, top to bottom.
    pub fn vertical(from: Rgb, to: Rgb) -> Self {
        Self {
            kind: GradientKind::Linear,
            angle: 180,
            stops: vec![Stop { color: from, at: 0 }, Stop { color: to, at: 100 }],
        }
    }

    /// The colour at `t` (0 to 1) along the stops.
    pub fn at(&self, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0) * 100.0;
        let first = self.stops[0];
        if t <= f32::from(first.at) {
            return first.color;
        }
        for pair in self.stops.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if t <= f32::from(b.at) {
                let span = f32::from(b.at.saturating_sub(a.at)).max(1.0);
                return a.color.mix(b.color, (t - f32::from(a.at)) / span);
            }
        }
        self.stops[self.stops.len() - 1].color
    }

    /// Whether GPUI can draw it as it is: linear, two stops at the ends.
    pub fn is_simple(&self) -> bool {
        self.kind == GradientKind::Linear
            && self.stops.len() == 2
            && self.stops[0].at == 0
            && self.stops[1].at == 100
    }
}

/// An image filling a surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageFill {
    /// A file in the pack, or a `builtin:` image.
    pub image: String,
    #[serde(default, skip_serializing_if = "is_default")]
    pub fit: Fit,
    /// A colour over the image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<Rgb>,
    /// How strong the tint is, 0 to 100.
    #[serde(default, skip_serializing_if = "is_default")]
    pub tint_opacity: u8,
    /// 0 to [`MAX_FILL_BLUR`] pixels, applied once when the skin loads.
    #[serde(default, skip_serializing_if = "is_default")]
    pub blur: u8,
}

/// How a surface is painted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fill {
    /// A colour; `opacity` 0 to 100 lets what is behind show through.
    Solid {
        color: Rgb,
        opacity: u8,
    },
    Gradient(Gradient),
    Image(ImageFill),
}

impl Fill {
    pub fn solid(color: Rgb) -> Self {
        Fill::Solid {
            color,
            opacity: 100,
        }
    }

    /// The colours it shows, for checking text against it: a solid fill's colour
    /// over `under`, a gradient's colours along it. `None` for an image (it has to be
    /// measured).
    pub fn colors(&self, under: Rgb) -> Option<Vec<Rgb>> {
        match self {
            Fill::Solid { color, opacity } => {
                Some(vec![under.mix(*color, f32::from(*opacity) / 100.0)])
            }
            Fill::Gradient(gradient) => {
                Some((0..=8).map(|i| gradient.at(i as f32 / 8.0)).collect())
            }
            Fill::Image(_) => None,
        }
    }

    /// The image it names, if it is an image fill.
    pub fn image(&self) -> Option<&str> {
        match self {
            Fill::Image(image) => Some(&image.image),
            _ => None,
        }
    }

    fn validate(&self, at: &str) -> Result<(), SkinError> {
        let invalid = |reason: String| Err(SkinError::Invalid(format!("{at}: {reason}")));
        match self {
            Fill::Solid { opacity, .. } if *opacity > 100 => {
                invalid("opacity must be 0 to 100".into())
            }
            Fill::Solid { .. } => Ok(()),
            Fill::Gradient(gradient) => {
                if !(2..=MAX_STOPS).contains(&gradient.stops.len()) {
                    return invalid(format!("a gradient has 2 to {MAX_STOPS} stops"));
                }
                if gradient.stops.iter().any(|s| s.at > 100)
                    || gradient.stops.windows(2).any(|p| p[1].at < p[0].at)
                {
                    return invalid("stops go from 0 to 100, in order".into());
                }
                if !(-360..=360).contains(&gradient.angle) {
                    return invalid("the angle is -360 to 360 degrees".into());
                }
                Ok(())
            }
            Fill::Image(image) => {
                check_image_name(&image.image)?;
                if image.tint_opacity > 100 {
                    return invalid("tint_opacity must be 0 to 100".into());
                }
                if image.blur > MAX_FILL_BLUR {
                    return invalid(format!("blur must be 0 to {MAX_FILL_BLUR}"));
                }
                Ok(())
            }
        }
    }
}

fn parse_color(value: &Value, at: &str) -> Result<Rgb, String> {
    value
        .as_str()
        .and_then(Rgb::parse)
        .ok_or_else(|| format!("{at} must be a #RRGGBB colour"))
}

fn parse_u8(value: Option<&Value>, at: &str, default: u8) -> Result<u8, String> {
    match value {
        None => Ok(default),
        Some(v) => v
            .as_u64()
            .and_then(|n| u8::try_from(n).ok())
            .ok_or_else(|| format!("{at} must be a whole number 0 to 255")),
    }
}

fn parse_stops(value: &Value) -> Result<Vec<Stop>, String> {
    let list = value
        .as_array()
        .ok_or("gradient.stops must be a list of colours")?;
    let count = list.len().max(2) - 1;
    list.iter()
        .enumerate()
        .map(|(i, stop)| {
            let even = ((i * 100) / count).min(100) as u8;
            match stop {
                Value::String(_) => Ok(Stop {
                    color: parse_color(stop, "a stop")?,
                    at: even,
                }),
                Value::Object(map) => {
                    let known = ["color", "at"];
                    if let Some(key) = map.keys().find(|k| !known.contains(&k.as_str())) {
                        return Err(format!("a stop has no {key:?}"));
                    }
                    Ok(Stop {
                        color: parse_color(
                            map.get("color").ok_or("a stop needs a color")?,
                            "stop.color",
                        )?,
                        at: parse_u8(map.get("at"), "stop.at", even)?,
                    })
                }
                _ => Err("a stop is a colour or {\"color\", \"at\"}".into()),
            }
        })
        .collect()
}

fn parse_fill(value: &Value) -> Result<Fill, String> {
    if let Value::String(_) = value {
        return Ok(Fill::solid(parse_color(value, "fill")?));
    }
    let map = value
        .as_object()
        .ok_or("fill is a colour, or one of {\"solid\"}, {\"gradient\"}, {\"image\"}")?;
    if map.len() != 1 {
        return Err("fill has exactly one of solid, gradient or image".into());
    }
    let (kind, body) = map.iter().next().expect("one entry");
    let only = |body: &Map<String, Value>, known: &[&str]| -> Result<(), String> {
        match body.keys().find(|k| !known.contains(&k.as_str())) {
            Some(key) => Err(format!("{kind} has no {key:?}")),
            None => Ok(()),
        }
    };
    match kind.as_str() {
        "solid" => match body {
            Value::String(_) => Ok(Fill::solid(parse_color(body, "fill.solid")?)),
            Value::Object(body) => {
                only(body, &["color", "opacity"])?;
                Ok(Fill::Solid {
                    color: parse_color(
                        body.get("color").ok_or("fill.solid needs a color")?,
                        "fill.solid.color",
                    )?,
                    opacity: parse_u8(body.get("opacity"), "fill.solid.opacity", 100)?,
                })
            }
            _ => Err("fill.solid is a colour or {\"color\", \"opacity\"}".into()),
        },
        "gradient" => {
            let body = body.as_object().ok_or("fill.gradient is an object")?;
            only(body, &["kind", "angle", "stops"])?;
            let kind = match body.get("kind").and_then(Value::as_str) {
                None | Some("linear") => GradientKind::Linear,
                Some("radial") => GradientKind::Radial,
                Some(other) => return Err(format!("{other:?} isn't linear or radial")),
            };
            let angle = match body.get("angle") {
                None => 180,
                Some(v) => v
                    .as_i64()
                    .and_then(|n| i16::try_from(n).ok())
                    .ok_or("fill.gradient.angle must be whole degrees")?,
            };
            Ok(Fill::Gradient(Gradient {
                kind,
                angle,
                stops: parse_stops(body.get("stops").ok_or("fill.gradient needs stops")?)?,
            }))
        }
        "image" => match body {
            Value::String(name) => Ok(Fill::Image(ImageFill {
                image: name.clone(),
                fit: Fit::Cover,
                tint: None,
                tint_opacity: 0,
                blur: 0,
            })),
            _ => serde_json::from_value::<ImageFill>(body.clone())
                .map(Fill::Image)
                .map_err(|err| format!("fill.image: {err}")),
        },
        other => Err(format!(
            "{other:?} isn't a kind of fill (solid, gradient or image)"
        )),
    }
}

impl<'de> Deserialize<'de> for Fill {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        parse_fill(&value).map_err(serde::de::Error::custom)
    }
}

impl Serialize for Fill {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let value = match self {
            Fill::Solid {
                color,
                opacity: 100,
            } => Value::String(color.hex()),
            Fill::Solid { color, opacity } => serde_json::json!({
                "solid": { "color": color.hex(), "opacity": opacity }
            }),
            Fill::Gradient(gradient) => {
                let mut body = Map::new();
                if gradient.kind != GradientKind::Linear {
                    body.insert("kind".into(), "radial".into());
                }
                if gradient.angle != 180 {
                    body.insert("angle".into(), gradient.angle.into());
                }
                let count = gradient.stops.len().max(2) - 1;
                let even = gradient
                    .stops
                    .iter()
                    .enumerate()
                    .all(|(i, s)| usize::from(s.at) == ((i * 100) / count).min(100));
                let stops: Vec<Value> = gradient
                    .stops
                    .iter()
                    .map(|s| {
                        if even {
                            Value::String(s.color.hex())
                        } else {
                            serde_json::json!({ "color": s.color.hex(), "at": s.at })
                        }
                    })
                    .collect();
                body.insert("stops".into(), Value::Array(stops));
                serde_json::json!({ "gradient": body })
            }
            Fill::Image(image) => serde_json::json!({
                "image": serde_json::to_value(image).map_err(serde::ser::Error::custom)?
            }),
        };
        value.serialize(serializer)
    }
}

/// The most a material blurs what is behind it, in pixels.
pub const MAX_MATERIAL_BLUR: u8 = 40;

/// What a see-through surface does to what is behind it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialKind {
    /// What is behind shows through as it is.
    #[default]
    Translucent,
    /// What is behind shows through blurred.
    Blur,
    /// Blurred, with a pale veil and fine grain, like frosted glass.
    Frost,
}

impl MaterialKind {
    pub const ALL: [MaterialKind; 3] = [
        MaterialKind::Translucent,
        MaterialKind::Blur,
        MaterialKind::Frost,
    ];

    pub fn key(self) -> &'static str {
        match self {
            MaterialKind::Translucent => "translucent",
            MaterialKind::Blur => "blur",
            MaterialKind::Frost => "frost",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        MaterialKind::ALL.into_iter().find(|k| k.key() == key)
    }

    /// How far it blurs, in pixels, unless the material says.
    pub fn default_blur(self) -> u8 {
        match self {
            MaterialKind::Translucent => 0,
            MaterialKind::Blur => 16,
            MaterialKind::Frost => 20,
        }
    }

    /// How much of the surface's fill shows over what is behind, unless the material
    /// or a see-through solid fill says.
    pub fn default_opacity(self) -> u8 {
        match self {
            MaterialKind::Translucent => 60,
            MaterialKind::Blur => 55,
            MaterialKind::Frost => 40,
        }
    }

    pub fn default_frost(self) -> u8 {
        match self {
            MaterialKind::Frost => 14,
            _ => 0,
        }
    }

    pub fn default_grain(self) -> u8 {
        match self {
            MaterialKind::Frost => 30,
            _ => 0,
        }
    }
}

/// How a surface lets what is behind it (the window's background, the page, a
/// dialog) show through: as it is, blurred, or frosted. The surface's own fill is
/// drawn over it at `opacity`.
///
/// In `skin.json` it is a kind (`"frost"`), or an object with the kind and any of
/// the numbers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Material {
    pub kind: MaterialKind,
    /// 0 to [`MAX_MATERIAL_BLUR`] pixels; only for `blur` and `frost`.
    pub blur: Option<u8>,
    /// 0 to 100: how much of the surface's own fill shows.
    pub opacity: Option<u8>,
    /// 0 to 100: a pale veil over what is behind.
    pub frost: Option<u8>,
    /// 0 to 100: fine grain over it.
    pub grain: Option<u8>,
}

impl Material {
    pub fn of(kind: MaterialKind) -> Self {
        Self {
            kind,
            ..Self::default()
        }
    }

    /// The blur in pixels: none for `translucent`.
    pub fn blur(&self) -> u8 {
        match self.kind {
            MaterialKind::Translucent => 0,
            kind => self.blur.unwrap_or(kind.default_blur()),
        }
    }

    /// How much of `fill` shows over what is behind, 0 to 100: the material's
    /// `opacity`, else a see-through solid fill's own, else the kind's.
    pub fn opacity(&self, fill: Option<&Fill>) -> u8 {
        self.opacity.unwrap_or(match fill {
            Some(Fill::Solid { opacity, .. }) if *opacity < 100 => *opacity,
            _ => self.kind.default_opacity(),
        })
    }

    pub fn frost(&self) -> u8 {
        self.frost.unwrap_or(self.kind.default_frost())
    }

    pub fn grain(&self) -> u8 {
        self.grain.unwrap_or(self.kind.default_grain())
    }

    /// Whether it draws what is behind itself (blurred), rather than letting the
    /// window show through.
    pub fn samples_behind(&self) -> bool {
        self.kind != MaterialKind::Translucent
    }

    fn validate(&self, at: &str) -> Result<(), SkinError> {
        let invalid = |reason: String| Err(SkinError::Invalid(format!("{at}: {reason}")));
        if self.blur.is_some_and(|b| b > MAX_MATERIAL_BLUR) {
            return invalid(format!("blur must be 0 to {MAX_MATERIAL_BLUR}"));
        }
        if self.blur.is_some() && self.kind == MaterialKind::Translucent {
            return invalid(
                "a translucent material doesn't blur; use \"blur\" or \"frost\"".into(),
            );
        }
        for (name, value) in [
            ("opacity", self.opacity),
            ("frost", self.frost),
            ("grain", self.grain),
        ] {
            if value.is_some_and(|v| v > 100) {
                return invalid(format!("{name} must be 0 to 100"));
            }
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for Material {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let kind = |key: &str| {
            MaterialKind::from_key(key).ok_or_else(|| {
                D::Error::custom(format!(
                    "{key:?} isn't a material (translucent, blur or frost)"
                ))
            })
        };
        match Value::deserialize(deserializer)? {
            Value::String(key) => Ok(Material::of(kind(&key)?)),
            Value::Object(map) => {
                let known = ["kind", "blur", "opacity", "frost", "grain"];
                if let Some(key) = map.keys().find(|k| !known.contains(&k.as_str())) {
                    return Err(D::Error::custom(format!("material has no {key:?}")));
                }
                let number = |name: &str| -> Result<Option<u8>, D::Error> {
                    match map.get(name) {
                        None => Ok(None),
                        Some(v) => v
                            .as_u64()
                            .and_then(|n| u8::try_from(n).ok())
                            .map(Some)
                            .ok_or_else(|| {
                                D::Error::custom(format!("material.{name} must be a whole number"))
                            }),
                    }
                };
                Ok(Material {
                    kind: kind(
                        map.get("kind")
                            .and_then(Value::as_str)
                            .ok_or_else(|| D::Error::custom("material needs a kind"))?,
                    )?,
                    blur: number("blur")?,
                    opacity: number("opacity")?,
                    frost: number("frost")?,
                    grain: number("grain")?,
                })
            }
            _ => Err(D::Error::custom(
                "material is \"translucent\", \"blur\" or \"frost\", or {\"kind\", …}",
            )),
        }
    }
}

impl Serialize for Material {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if *self == Material::of(self.kind) {
            return self.kind.key().serialize(serializer);
        }
        let mut body = Map::new();
        body.insert("kind".into(), self.kind.key().into());
        for (name, value) in [
            ("blur", self.blur),
            ("opacity", self.opacity),
            ("frost", self.frost),
            ("grain", self.grain),
        ] {
            if let Some(value) = value {
                body.insert(name.into(), value.into());
            }
        }
        Value::Object(body).serialize(serializer)
    }
}

/// How one surface looks. Every part is optional: what is left out stays the
/// launcher's.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurfaceSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Fill>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<Rgb>,
    /// 0 to [`MAX_BORDER`] pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border_width: Option<u8>,
    /// 0 to [`MAX_RADIUS`] pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radius: Option<u8>,
    /// Its main text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<Rgb>,
    /// Its quieter text: subtitles, headers, hints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muted: Option<Rgb>,
    /// How what is behind it shows through: as it is, blurred or frosted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<Material>,
}

impl SurfaceSpec {
    pub fn is_empty(&self) -> bool {
        *self == SurfaceSpec::default()
    }

    pub(super) fn validate(&self, id: SurfaceId) -> Result<(), SkinError> {
        let at = format!("surfaces.{}", id.key());
        if let Some(fill) = &self.fill {
            if fill.image().is_some() && !id.takes_image() {
                return Err(SkinError::Invalid(format!(
                    "{at}: an image fill is for the large surfaces; use a colour or a gradient"
                )));
            }
            fill.validate(&format!("{at}.fill"))?;
        }
        if (self.text.is_some() || self.muted.is_some()) && !super::readable::own_text(id) {
            return Err(SkinError::Invalid(format!(
                "{at}: text colours are set per surface only for the sidebar, notices and \
                 controls; elsewhere use tokens"
            )));
        }
        if self.muted.is_some() && id != SurfaceId::Sidebar {
            return Err(SkinError::Invalid(format!(
                "{at}.muted is only for the sidebar; use tokens.muted"
            )));
        }
        if self.border_width.is_some_and(|w| w > MAX_BORDER) {
            return Err(SkinError::Invalid(format!(
                "{at}.border_width must be 0 to {MAX_BORDER}"
            )));
        }
        if self.radius.is_some_and(|r| r > MAX_RADIUS) {
            return Err(SkinError::Invalid(format!(
                "{at}.radius must be 0 to {MAX_RADIUS}"
            )));
        }
        if let Some(material) = &self.material {
            if !id.takes_material() {
                return Err(SkinError::Invalid(format!(
                    "{at}.material is for the panels (bars, the page, cards, rows, dialogs, \
                     menus, inputs and notices), not the window, the hero or controls"
                )));
            }
            material.validate(&format!("{at}.material"))?;
        }
        Ok(())
    }
}

/// The surfaces a skin paints, by id.
pub type Surfaces = BTreeMap<SurfaceId, SurfaceSpec>;

/// The theme's colours for text and borders, everywhere a surface doesn't set its
/// own.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tokens {
    /// Names, titles, values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<Rgb>,
    /// Labels and secondary text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary: Option<Rgb>,
    /// Subtitles, descriptions and section headers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muted: Option<Rgb>,
    /// Hints, placeholders and the footer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faint: Option<Rgb>,
    /// Lines between and around things.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<Rgb>,
}

impl Tokens {
    /// The launcher's own text colours.
    pub const BASE_TEXT: Rgb = Rgb(0xE3E3E4);
    pub const BASE_SECONDARY: Rgb = Rgb(0xA5A5AC);
    pub const BASE_MUTED: Rgb = Rgb(0x767A82);
}

/// A part of the window with a font of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontPart {
    Heading,
    Body,
    Sidebar,
    Button,
    Play,
    Tab,
    Mono,
}

impl FontPart {
    pub const ALL: [FontPart; 7] = [
        FontPart::Heading,
        FontPart::Body,
        FontPart::Sidebar,
        FontPart::Button,
        FontPart::Play,
        FontPart::Tab,
        FontPart::Mono,
    ];

    pub fn key(self) -> &'static str {
        match self {
            FontPart::Heading => "heading",
            FontPart::Body => "body",
            FontPart::Sidebar => "sidebar",
            FontPart::Button => "button",
            FontPart::Play => "play",
            FontPart::Tab => "tab",
            FontPart::Mono => "mono",
        }
    }
}

/// The font families the launcher ships, usable by name in [`Fonts`].
pub const BUNDLED_FONTS: [&str; 5] = [
    "Satoshi",
    "Chillax",
    "Clash Grotesk",
    "JetBrains Mono",
    "Lodestone Pixel",
];

/// Fonts for parts of the window: a bundled family by name ([`BUNDLED_FONTS`]), or
/// a `.ttf` / `.otf` file in the pack.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fonts {
    /// Page titles and section headings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    /// Everything else.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidebar: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub button: Option<String>,
    /// The Play button's label and the hero's title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub play: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab: Option<String>,
    /// Logs and code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mono: Option<String>,
}

impl Fonts {
    pub fn get(&self, part: FontPart) -> Option<&str> {
        match part {
            FontPart::Heading => self.heading.as_deref(),
            FontPart::Body => self.body.as_deref(),
            FontPart::Sidebar => self.sidebar.as_deref(),
            FontPart::Button => self.button.as_deref(),
            FontPart::Play => self.play.as_deref(),
            FontPart::Tab => self.tab.as_deref(),
            FontPart::Mono => self.mono.as_deref(),
        }
    }

    pub fn set(&mut self, part: FontPart, value: Option<String>) {
        let slot = match part {
            FontPart::Heading => &mut self.heading,
            FontPart::Body => &mut self.body,
            FontPart::Sidebar => &mut self.sidebar,
            FontPart::Button => &mut self.button,
            FontPart::Play => &mut self.play,
            FontPart::Tab => &mut self.tab,
            FontPart::Mono => &mut self.mono,
        };
        *slot = value;
    }

    /// The font files it names (not the bundled families).
    pub fn files(&self) -> Vec<&str> {
        let mut files: Vec<&str> = FontPart::ALL
            .iter()
            .filter_map(|part| self.get(*part))
            .filter(|name| is_font_file(name))
            .collect();
        files.sort_unstable();
        files.dedup();
        files
    }

    pub(super) fn validate(&self) -> Result<(), SkinError> {
        for part in FontPart::ALL {
            let Some(name) = self.get(part) else { continue };
            if is_font_file(name) {
                super::fonts::check_font_name(name)?;
            } else if !BUNDLED_FONTS.contains(&name) {
                return Err(SkinError::Invalid(format!(
                    "fonts.{}: {name:?} is neither a bundled font ({}) nor a .ttf or .otf file",
                    part.key(),
                    BUNDLED_FONTS.join(", ")
                )));
            }
        }
        if self.files().len() > super::fonts::MAX_FONT_FILES {
            return Err(SkinError::Invalid(format!(
                "a pack ships at most {} font files",
                super::fonts::MAX_FONT_FILES
            )));
        }
        Ok(())
    }
}

/// Whether `name` names a font file rather than a bundled family.
pub fn is_font_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".ttf") || lower.ends_with(".otf")
}

/// A banner a skin offers its instances: a Play background with a stable id.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkinBanner {
    /// Stays the same when the skin is updated, so instances keep their banner.
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// A file in the pack, or a `builtin:` image.
    pub image: String,
}

pub(super) fn validate_banners(banners: &[SkinBanner]) -> Result<(), SkinError> {
    if banners.len() > MAX_BANNERS {
        return Err(SkinError::Invalid(format!(
            "a skin has at most {MAX_BANNERS} banners"
        )));
    }
    for (i, banner) in banners.iter().enumerate() {
        let ok_id = !banner.id.is_empty()
            && banner.id.len() <= 32
            && banner
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !ok_id {
            return Err(SkinError::Invalid(format!(
                "banners[{i}].id must be 1 to 32 lowercase letters, digits or dashes"
            )));
        }
        if banners[..i].iter().any(|b| b.id == banner.id) {
            return Err(SkinError::Invalid(format!(
                "two banners are called {:?}",
                banner.id
            )));
        }
        if banner
            .name
            .as_ref()
            .is_some_and(|n| n.chars().count() > 48 || n.chars().any(char::is_control))
        {
            return Err(SkinError::Invalid(format!("banners[{i}].name is too long")));
        }
        check_image_name(&banner.image)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill(json: &str) -> Result<Fill, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }

    fn material(json: &str) -> Result<Material, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }

    #[test]
    fn materials_read_short_and_long_and_write_back_short() {
        assert_eq!(
            material(r#""frost""#).unwrap(),
            Material::of(MaterialKind::Frost)
        );
        let long = material(r#"{"kind": "blur", "blur": 24, "opacity": 30}"#).unwrap();
        assert_eq!(long.blur(), 24);
        assert_eq!(long.opacity(None), 30);
        assert_eq!(long.frost(), 0);
        for m in [Material::of(MaterialKind::Translucent), long] {
            let json = serde_json::to_string(&m).unwrap();
            assert_eq!(material(&json).unwrap(), m, "{json}");
        }
        assert_eq!(
            serde_json::to_string(&Material::of(MaterialKind::Blur)).unwrap(),
            r#""blur""#
        );
        assert!(material(r#""glass""#).is_err());
        assert!(material(r#"{"kind": "frost", "shine": 3}"#).is_err());
        assert!(material(r#"{"blur": 3}"#).is_err());
    }

    #[test]
    fn a_see_through_solid_sets_the_opacity_unless_the_material_does() {
        let solid = Fill::Solid {
            color: Rgb(0x112233),
            opacity: 35,
        };
        let frost = Material::of(MaterialKind::Frost);
        assert_eq!(frost.opacity(Some(&solid)), 35);
        assert_eq!(frost.opacity(Some(&Fill::solid(Rgb(0x112233)))), 40);
        let own = Material {
            opacity: Some(70),
            ..frost
        };
        assert_eq!(own.opacity(Some(&solid)), 70);
        // translucent never blurs
        assert_eq!(Material::of(MaterialKind::Translucent).blur(), 0);
    }

    #[test]
    fn materials_are_checked() {
        let with = |material: Material| SurfaceSpec {
            material: Some(material),
            ..SurfaceSpec::default()
        };
        let frost = Material::of(MaterialKind::Frost);
        assert!(with(frost).validate(SurfaceId::Card).is_ok());
        assert!(with(frost).validate(SurfaceId::Window).is_err());
        assert!(with(frost).validate(SurfaceId::Hero).is_err());
        assert!(with(frost).validate(SurfaceId::ButtonPrimary).is_err());
        let too = |m: Material| with(m).validate(SurfaceId::TopBar).is_err();
        assert!(too(Material {
            blur: Some(MAX_MATERIAL_BLUR + 1),
            ..frost
        }));
        assert!(too(Material {
            grain: Some(101),
            ..frost
        }));
        assert!(too(Material {
            blur: Some(4),
            ..Material::of(MaterialKind::Translucent)
        }));
    }

    #[test]
    fn fills_read_in_every_form_and_write_back_short() {
        assert_eq!(fill(r##""#112233""##).unwrap(), Fill::solid(Rgb(0x112233)));
        let half = fill(r##"{"solid": {"color": "#112233", "opacity": 50}}"##).unwrap();
        assert_eq!(
            half,
            Fill::Solid {
                color: Rgb(0x112233),
                opacity: 50
            }
        );
        let gradient =
            fill(r##"{"gradient": {"angle": 90, "stops": ["#000000", "#808080", "#FFFFFF"]}}"##)
                .unwrap();
        let Fill::Gradient(g) = &gradient else {
            panic!()
        };
        assert_eq!(
            g.stops.iter().map(|s| s.at).collect::<Vec<_>>(),
            [0, 50, 100]
        );
        assert_eq!(g.at(0.5), Rgb(0x808080));
        assert!(!g.is_simple());
        let image = fill(r#"{"image": {"image": "dirt.png", "fit": "tile", "blur": 2}}"#).unwrap();
        assert_eq!(image.image(), Some("dirt.png"));
        for f in [Fill::solid(Rgb(0x112233)), half, gradient, image] {
            let json = serde_json::to_string(&f).unwrap();
            assert_eq!(fill(&json).unwrap(), f, "{json}");
        }
        assert_eq!(
            serde_json::to_string(&Fill::solid(Rgb(0x112233))).unwrap(),
            r##""#112233""##
        );
    }

    #[test]
    fn bad_fills_say_what_is_wrong() {
        assert!(
            fill(r#"{"paint": "red"}"#)
                .unwrap_err()
                .contains("isn't a kind of fill")
        );
        assert!(
            fill(r##"{"gradient": {"stops": ["#000000"], "spin": 1}}"##)
                .unwrap_err()
                .contains("spin")
        );
        assert!(fill(r#""red""#).unwrap_err().contains("#RRGGBB"));
        let one = Fill::Gradient(Gradient {
            kind: GradientKind::Linear,
            angle: 0,
            stops: vec![Stop {
                color: Rgb::BLACK,
                at: 0,
            }],
        });
        assert!(one.validate("x").is_err());
    }

    #[test]
    fn surface_ids_are_written_as_their_keys() {
        for id in SurfaceId::ALL {
            let json = serde_json::to_string(&id).unwrap();
            assert_eq!(json, format!("\"{}\"", id.key()));
            assert_eq!(SurfaceId::from_key(id.key()), Some(id));
        }
    }
}
