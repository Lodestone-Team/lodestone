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
//! The format is described in `launcher/docs/skins.md`.

use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::paths::DataDir;

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

/// The built-in skins' ids, in the gallery's order.
pub const BUILT_IN: [&str; 4] = ["default", "grassblock", "glass", "neon"];
/// The images the launcher draws for the built-in skins.
pub const BUILT_IN_IMAGES: [&str; 4] = [
    "builtin:dirt",
    "builtin:sky",
    "builtin:glass",
    "builtin:grid",
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

/// Whether the launcher keeps a surface's text readable on its own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Readability {
    /// Text colours (and a darker backing if needed) are worked out from the image,
    /// so text keeps [`readable::BODY`] contrast.
    #[default]
    Auto,
    /// The colours are used as given.
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
            readability: Readability::Auto,
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
        }
    }

    /// The images it names, as written (pack files or `builtin:` images).
    pub fn images(&self) -> Vec<&str> {
        [self.sidebar.image.as_deref(), self.hero.image.as_deref()]
            .into_iter()
            .flatten()
            .collect()
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
            spec.play = PlaySpec {
                color: Some(Rgb(0x5DB83B)),
                gradient: Some(Rgb(0x3C8A22)),
                shape: Shape::Pixel,
                label: None,
                glow: false,
            };
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
            spec.play = PlaySpec {
                color: Some(Rgb(0xD61FB4)),
                gradient: Some(Rgb(0x7C1FD6)),
                shape: Shape::Rounded,
                label: None,
                glow: true,
            };
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
        for image in spec.images() {
            if let Some(path) = self.image_path(from, image) {
                fs::copy(path, dir.join(image))?;
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
        let stem = match role {
            ImageRole::Sidebar => "sidebar",
            ImageRole::Hero => "hero",
        };
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
        // only the description and its images: nothing else may come along
        for (name, _) in &files {
            if name != SKIN_FILE && !spec.images().contains(&name.as_str()) {
                return Err(SkinError::Invalid(format!(
                    "{name:?} isn't used by the skin; a pack holds only {SKIN_FILE} and its images"
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
            let json = serde_json::to_string_pretty(&spec).map_err(io::Error::other)?;
            fs::write(staging.join(SKIN_FILE), json)?;
            let id = self.unique_id(&spec.name);
            fs::rename(&staging, self.dir.join(&id))?;
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
        let mut zip = zip::ZipWriter::new(file);
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let zipped = |err: zip::result::ZipError| SkinError::Io(io::Error::other(err));
        zip.start_file(SKIN_FILE, options).map_err(zipped)?;
        let json = serde_json::to_string_pretty(&entry.spec).map_err(io::Error::other)?;
        zip.write_all(json.as_bytes())?;
        for image in entry.spec.images() {
            if let Some(path) = self.image_path(id, image) {
                zip.start_file(image, options).map_err(zipped)?;
                zip.write_all(&fs::read(path)?)?;
            }
        }
        zip.finish().map_err(zipped)?;
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
}

impl ImageRole {
    fn max(self) -> (u32, u32) {
        match self {
            ImageRole::Sidebar => SIDEBAR_MAX,
            ImageRole::Hero => HERO_MAX,
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

/// Keeping text readable on a skin's images: measuring an image, and working out
/// text colours (and a darker or lighter backing when colours can't do it alone) that
/// keep a minimum contrast against it.
pub mod readable {
    use super::{Readability, Rgb, TextSpec};

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
}

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
