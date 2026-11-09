//! Design tokens carried over from the dashboard's `tailwind.config.js`.
//!
//! Colours, text sizes and font weights keep the dashboard's names and values; the
//! tests at the bottom of this file check them against the Tailwind config. GPUI's
//! spacing helpers (`p_2`, `gap_1p5`, ...) already use Tailwind's 0.25rem scale, and
//! its rem is 16px like the browser's, so spacing needs no tokens of its own.
//!
//! Tailwind's letter spacing (`tracking-*`) has no GPUI equivalent and is not carried over.

use gpui::{AbsoluteLength, Background, Fill, FontWeight, Hsla, Pixels, Rems, Rgba, rems};

pub use zoom::px;

/// An sRGB colour stored as `0xRRGGBBAA`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub u32);

impl Color {
    /// An opaque colour from `0xRRGGBB`.
    pub const fn rgb(hex: u32) -> Self {
        Self((hex << 8) | 0xff)
    }

    /// A colour with alpha from `0xRRGGBBAA`.
    pub const fn rgba(hex: u32) -> Self {
        Self(hex)
    }

    /// The colour `t` of the way from `self` to `other` (0.0 is `self`), channel by
    /// channel, alpha included: for animating between two states.
    pub fn mix(self, other: Color, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let channel = |shift: u32| {
            let a = ((self.0 >> shift) & 0xff) as f32;
            let b = ((other.0 >> shift) & 0xff) as f32;
            ((a + (b - a) * t).round() as u32) << shift
        };
        Self(channel(24) | channel(16) | channel(8) | channel(0))
    }

    /// The colour with its alpha scaled to `percent`, like Tailwind's `bg-gray-800/30`.
    pub const fn opacity(self, percent: u32) -> Self {
        let alpha = self.0 & 0xff;
        // Round to the nearest step instead of truncating.
        let scaled = (alpha * percent + 50) / 100;
        Self((self.0 & 0xffff_ff00) | scaled)
    }

    /// The `#RRGGBB` (or `#RRGGBBAA` when not opaque) form Tailwind uses.
    pub fn to_hex(self) -> String {
        if self.0 & 0xff == 0xff {
            format!("#{:06X}", self.0 >> 8)
        } else {
            format!("#{:08X}", self.0)
        }
    }
}

impl From<Color> for Rgba {
    fn from(color: Color) -> Self {
        gpui::rgba(color.0)
    }
}

impl From<Color> for Hsla {
    fn from(color: Color) -> Self {
        Rgba::from(color).into()
    }
}

impl From<Color> for Fill {
    fn from(color: Color) -> Self {
        Hsla::from(color).into()
    }
}

impl From<Color> for Background {
    fn from(color: Color) -> Self {
        Hsla::from(color).into()
    }
}

/// `theme.colors` in the Tailwind config. `DEFAULT` entries are the bare scale name.
pub mod colors {
    use super::Color;

    pub const TRANSPARENT: Color = Color::rgba(0x0000_0000);

    pub const BLUE_150: Color = Color::rgb(0x69C9FF);
    pub const BLUE_200: Color = Color::rgb(0x59B2F3);
    pub const BLUE_300: Color = Color::rgb(0x1D8EB2);
    pub const BLUE: Color = BLUE_300;
    pub const BLUE_400: Color = Color::rgb(0x037AA0);
    pub const BLUE_500: Color = Color::rgb(0x13668A);
    pub const BLUE_600: Color = Color::rgb(0x334675);
    pub const BLUE_FADED: Color = Color::rgb(0x3C85BA);

    pub const GREEN_200: Color = Color::rgb(0x2AF588);
    pub const GREEN_300: Color = Color::rgb(0x6DD277);
    pub const GREEN: Color = GREEN_300;
    pub const GREEN_400: Color = Color::rgb(0x3DAE5E);
    pub const GREEN_FADED: Color = Color::rgb(0x61AE32);
    pub const GREEN_ENABLED: Color = Color::rgb(0x48F077);

    pub const RED_200: Color = Color::rgb(0xFF5C5C);
    pub const RED_300: Color = Color::rgb(0xCF4545);
    pub const RED: Color = RED_300;
    pub const RED_400: Color = Color::rgb(0xB63737);
    pub const RED_FADED: Color = Color::rgb(0xAE3232);

    pub const YELLOW_300: Color = Color::rgb(0xEFB440);
    pub const YELLOW: Color = YELLOW_300;
    pub const YELLOW_FADED: Color = Color::rgb(0xAE8B32);

    pub const GRAY_300: Color = Color::rgb(0xE3E3E4);
    pub const GRAY_400: Color = Color::rgb(0xA5A5AC);
    pub const GRAY_500: Color = Color::rgb(0x767A82);
    pub const GRAY_550: Color = Color::rgb(0x67686A);
    pub const GRAY_600: Color = Color::rgb(0x44464B);
    pub const GRAY_700: Color = Color::rgb(0x36393F);
    pub const GRAY_750: Color = Color::rgb(0x303338);
    pub const GRAY_800: Color = Color::rgb(0x2B2D32);
    pub const GRAY_850: Color = Color::rgb(0x26282C);
    pub const GRAY_875: Color = Color::rgb(0x212327);
    pub const GRAY_900: Color = Color::rgb(0x1D1E21);
    pub const GRAY_FADED: Color = Color::rgb(0xA5A5AC);

    pub const FADE_700: Color = Color::rgba(0xD1D1DA19);

    pub const WHITE: Color = Color::rgb(0xFFFFFF);
    pub const VIOLET: Color = Color::rgb(0x8736C7);
    pub const ULTRAMARINE: Color = Color::rgb(0x273EB9);
}

/// `theme.fontFamily`. Each family is embedded from `launcher/assets/fonts`.
pub mod fonts {
    /// `font-sans`, the body font.
    pub const SANS: &str = "Satoshi";
    /// `font-heading`.
    pub const HEADING: &str = "Chillax";
    /// `font-title`.
    pub const TITLE: &str = "Clash Grotesk";
    /// `font-mono`.
    pub const MONO: &str = "JetBrains Mono";
}

/// `theme.fontWeight`, with the dashboard's values.
///
/// The dashboard uses variable fonts, which can render any weight. GPUI only selects
/// among the static faces we embed (Regular 400, Medium 500, Semibold 600 and
/// Bold 700), so [`FontWeightToken::face`] picks the face used to render each token.
pub mod weights {
    use super::FontWeightToken;

    pub const MEDIUM: FontWeightToken = FontWeightToken(450.0);
    pub const MEDIUMBOLD: FontWeightToken = FontWeightToken(500.0);
    pub const BOLD: FontWeightToken = FontWeightToken(550.0);
    pub const EXTRABOLD: FontWeightToken = FontWeightToken(700.0);
}

/// A dashboard font weight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontWeightToken(pub f32);

impl FontWeightToken {
    /// The static face that renders this weight. Weights between two faces round down
    /// to the lighter face, so `medium` (450) is Regular and `bold` (550) is Medium,
    /// which keeps the dashboard's steps distinct. Every embedded family has the
    /// Regular, Medium and Bold faces.
    pub fn face(self) -> FontWeight {
        let face = match self.0 {
            w if w < 500.0 => 400.0,
            w if w < 700.0 => 500.0,
            _ => 700.0,
        };
        FontWeight(face)
    }
}

impl From<FontWeightToken> for FontWeight {
    fn from(token: FontWeightToken) -> Self {
        token.face()
    }
}

/// `theme.fontSize`, in rems.
pub mod text {
    use gpui::{Rems, rems};

    pub const CAPTION: Rems = rems(0.625);
    pub const SMALL: Rems = rems(0.75);
    pub const MEDIUM: Rems = rems(0.875);
    pub const H3: Rems = rems(1.0);
    pub const H2: Rems = rems(1.25);
    pub const H1: Rems = rems(1.75);
    pub const TITLE: Rems = rems(2.625);
    pub const XLARGE_2: Rems = rems(2.0);
    pub const XLARGE_3: Rems = rems(2.5);
    pub const XLARGE_4: Rems = rems(3.0);
    pub const XLARGE_5: Rems = rems(3.5);
    pub const XLARGE_6: Rems = rems(4.0);
}

/// Tailwind's line heights (`leading-*`), as multiples of the font size.
pub mod leading {
    pub const TIGHT: f32 = 1.25;
    pub const SNUG: f32 = 1.375;
    pub const NORMAL: f32 = 1.5;
}

/// Tailwind's border radii (`rounded-*`), which the dashboard does not override.
pub mod radius {
    use gpui::{Rems, rems};

    /// `rounded-xs`
    pub const XS: Rems = rems(0.125);
    /// `rounded` and `rounded-sm`
    pub const DEFAULT: Rems = rems(0.25);
    /// `rounded-md`
    pub const MD: Rems = rems(0.375);
    /// `rounded-lg`
    pub const LG: Rems = rems(0.5);
    /// `rounded-xl`
    pub const XL: Rems = rems(0.75);
    /// `rounded-2xl`
    pub const XL_2: Rems = rems(1.0);
}

/// `theme.dropShadow`: all of the dashboard's shadows use one colour.
pub mod shadow {
    use super::Color;

    pub const COLOR: Color = Color::rgb(0x111114);
}

/// The accent colour: links, focus rings, selected tabs and primary buttons. It is
/// the dashboard's blue unless the skin or the selected instance sets another (see
/// `crate::skin`), which is made readable first: links keep 4.5:1 against the dark
/// panels, and white text keeps 3:1 on a primary button.
pub mod accent {
    use std::sync::atomic::{AtomicU32, Ordering};

    use launcher_core::ui_skin::Rgb;

    use super::{Color, colors};

    /// The panels links sit on.
    const PANEL: Rgb = Rgb(0x26282C);
    /// No custom accent.
    const NONE: u32 = u32::MAX;

    /// The custom accent as `0xRRGGBB`, or [`NONE`].
    static ACCENT: AtomicU32 = AtomicU32::new(NONE);

    /// Uses `accent`, or the default blue for `None`. Windows need a refresh to show it.
    pub fn set(accent: Option<Rgb>) {
        ACCENT.store(
            accent.map_or(NONE, |rgb| rgb.0 & 0xFF_FFFF),
            Ordering::Relaxed,
        );
    }

    /// The custom accent, if one is set.
    pub fn custom() -> Option<Rgb> {
        match ACCENT.load(Ordering::Relaxed) {
            NONE => None,
            rgb => Some(Rgb(rgb)),
        }
    }

    /// The light accent, for text and lines on the dark panels: links, tab underlines.
    pub fn light() -> Color {
        custom().map_or(colors::BLUE_200, |rgb| {
            Color::rgb(rgb.readable_against(PANEL, 4.5).0)
        })
    }

    /// The fill of primary buttons and selected things, under white text.
    pub fn base() -> Color {
        custom().map_or(colors::BLUE, |rgb| Color::rgb(fill(rgb).0))
    }

    /// A primary button under the pointer.
    pub fn hover() -> Color {
        custom().map_or(colors::BLUE_400, |rgb| {
            Color::rgb(fill(rgb).mix(Rgb::BLACK, 0.15).0)
        })
    }

    /// A primary button being pressed.
    pub fn pressed() -> Color {
        custom().map_or(colors::BLUE_500, |rgb| {
            Color::rgb(fill(rgb).mix(Rgb::BLACK, 0.3).0)
        })
    }

    /// Focus rings and soft highlights.
    pub fn faded() -> Color {
        custom().map_or(colors::BLUE_FADED, |rgb| {
            Color::rgb(fill(rgb).mix(Rgb(0x7F8AA0), 0.2).0)
        })
    }

    /// `rgb`, darkened until white text on it keeps 3:1.
    pub fn fill(rgb: Rgb) -> Rgb {
        rgb.readable_against(Rgb::WHITE, 3.0)
    }

    /// Whether `rgb` had to change to be readable, for the Skin page's check.
    pub fn adjusted(rgb: Rgb) -> bool {
        rgb.readable_against(PANEL, 4.5) != rgb || fill(rgb) != rgb
    }
}

/// The interface's zoom (Ctrl/⌘ + and −).
///
/// Every size in the launcher goes through [`px`], which multiplies by the zoom, and
/// the window's rem follows it too (see [`zoom::rem_size`]), so text, spacing and
/// fixed sizes grow together, as a browser zooms a page. Sizes the platform owns,
/// such as the window's own size and the title bar's height under the traffic
/// lights, use `gpui::px` and stay as they are.
pub mod zoom {
    use std::sync::atomic::{AtomicU32, Ordering};

    use gpui::Pixels;

    /// The levels + and − step through, as in browsers.
    pub const LEVELS: [f32; 10] = [0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];

    /// The zoom as f32 bits: 1.0 to start with.
    static ZOOM: AtomicU32 = AtomicU32::new(0x3f80_0000);

    /// The current zoom, 1.0 for 100%.
    pub fn get() -> f32 {
        f32::from_bits(ZOOM.load(Ordering::Relaxed))
    }

    /// Sets the zoom, kept within [`LEVELS`], and returns it. Windows need a refresh
    /// to show it.
    pub fn set(zoom: f32) -> f32 {
        let zoom = clamp(zoom);
        ZOOM.store(zoom.to_bits(), Ordering::Relaxed);
        zoom
    }

    /// `zoom` within the first and last levels (100% if it isn't a number).
    pub fn clamp(zoom: f32) -> f32 {
        if zoom.is_finite() {
            zoom.clamp(LEVELS[0], LEVELS[LEVELS.len() - 1])
        } else {
            1.0
        }
    }

    /// The next level up from `current` (`up`), or down.
    pub fn step(current: f32, up: bool) -> f32 {
        if up {
            LEVELS
                .into_iter()
                .find(|level| *level > current + 1e-3)
                .unwrap_or(current)
        } else {
            LEVELS
                .into_iter()
                .rev()
                .find(|level| *level < current - 1e-3)
                .unwrap_or(current)
        }
    }

    /// `value` logical pixels at the current zoom. Use this, not `gpui::px`, for
    /// sizes in the interface.
    pub fn px(value: f32) -> Pixels {
        gpui::px(value * get())
    }

    /// The window's rem at the current zoom: 16px at 100%, as in browsers.
    pub fn rem_size() -> Pixels {
        gpui::px(16.0 * get())
    }
}

/// Sizes of the dashboard's layout that are not on the spacing scale.
pub mod layout {
    use gpui::Pixels;

    use super::px;

    /// The left sidebar's default width (`leftNavSize`).
    pub fn sidebar_width() -> Pixels {
        px(SIDEBAR_DEFAULT)
    }

    /// The sidebar's widths, in logical pixels at 100% zoom: the default, how
    /// narrow and wide it can be dragged, the folded rail of icons, and the width
    /// below which a drag folds it into the rail.
    pub const SIDEBAR_DEFAULT: f32 = 220.0;
    pub const SIDEBAR_MIN: f32 = 180.0;
    pub const SIDEBAR_MAX: f32 = 360.0;
    pub const SIDEBAR_RAIL: f32 = 64.0;
    pub const SIDEBAR_FOLD_BELOW: f32 = 140.0;

    /// The sidebar for its edge dragged to `x` (logical pixels at 100% zoom, from
    /// the window's left edge), in a window `window_width` wide: its width, and
    /// whether it is folded into the rail. It never takes more than 45% of the
    /// window.
    pub fn sidebar_drag(x: f32, window_width: f32) -> (f32, bool) {
        if x < SIDEBAR_FOLD_BELOW {
            return (SIDEBAR_DEFAULT, true);
        }
        let max = SIDEBAR_MAX.min(window_width * 0.45).max(SIDEBAR_MIN);
        (x.clamp(SIDEBAR_MIN, max).round(), false)
    }

    /// The logo in the top bar (`w-28`). It doesn't zoom, like the bar it sits in.
    pub fn logo_width() -> Pixels {
        gpui::px(112.0)
    }

    /// Dialog panels (`w-[500px]`).
    pub fn dialog_width() -> Pixels {
        px(500.0)
    }
}

/// Width of the focus ring (`focus-visible:ring-4`).
pub fn focus_ring_width() -> Pixels {
    px(4.0)
}

/// A text size as the absolute length GPUI's `text_size` takes.
pub fn text_size(size: Rems) -> AbsoluteLength {
    AbsoluteLength::Rems(size)
}

/// A line height for `size` at the given `leading` multiple.
pub fn line_height(size: Rems, leading: f32) -> Rems {
    rems(size.0 * leading)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sidebar_folds_below_its_threshold_and_stays_in_bounds() {
        use layout::*;
        assert_eq!(sidebar_drag(100.0, 1200.0), (SIDEBAR_DEFAULT, true));
        assert_eq!(sidebar_drag(150.0, 1200.0), (SIDEBAR_MIN, false));
        assert_eq!(sidebar_drag(260.4, 1200.0), (260.0, false));
        assert_eq!(sidebar_drag(900.0, 1200.0), (SIDEBAR_MAX, false));
        // never more than 45% of a narrow window, but never under the minimum
        assert_eq!(sidebar_drag(350.0, 720.0), (324.0, false));
        assert_eq!(sidebar_drag(350.0, 300.0), (SIDEBAR_MIN, false));
    }

    #[test]
    fn zoom_steps_through_the_levels_and_stops_at_the_ends() {
        assert_eq!(zoom::step(1.0, true), 1.1);
        assert_eq!(zoom::step(1.0, false), 0.9);
        assert_eq!(zoom::step(2.0, true), 2.0);
        assert_eq!(zoom::step(0.67, false), 0.67);
        assert_eq!(zoom::step(1.05, true), 1.1);
        assert_eq!(zoom::clamp(5.0), 2.0);
        assert_eq!(zoom::clamp(f32::NAN), 1.0);
    }
    use std::{collections::BTreeMap, fs, path::Path};

    /// The dashboard's Tailwind config, read from the monorepo checkout.
    fn tailwind_config() -> String {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../dashboard/tailwind.config.js");
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("reading {}: {err}", path.display()))
    }

    /// The body of the object literal that follows `key:`, without its braces.
    fn object<'a>(source: &'a str, key: &str) -> &'a str {
        let start = source
            .find(&format!("{key}: {{"))
            .unwrap_or_else(|| panic!("`{key}` not found"));
        let open = start + source[start..].find('{').unwrap();
        let mut depth = 0;
        for (offset, ch) in source[open..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &source[open + 1..open + offset];
                    }
                }
                _ => {}
            }
        }
        panic!("`{key}` is not closed");
    }

    /// Flattens `name: { key: 'value' }` and `name: 'value'` entries into
    /// `name-key` / `name` keys, ignoring comments.
    fn flatten(body: &str) -> BTreeMap<String, String> {
        let mut values = BTreeMap::new();
        let mut scope: Option<String> = None;
        for line in body.lines() {
            let line = line.split("//").next().unwrap().trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('}') {
                scope = None;
                continue;
            }
            let (key, value) = line.split_once(':').unwrap();
            let key = key.trim().trim_matches('\'');
            let value = value.trim().trim_end_matches(',');
            if value == "{" {
                scope = Some(key.to_string());
                continue;
            }
            let value = value.trim_matches('\'').to_string();
            let name = match &scope {
                Some(scope) if key == "DEFAULT" => scope.clone(),
                Some(scope) => format!("{scope}-{key}"),
                None => key.to_string(),
            };
            values.insert(name, value);
        }
        values
    }

    fn token_colors() -> BTreeMap<&'static str, Color> {
        use colors::*;
        BTreeMap::from([
            ("transparent", TRANSPARENT),
            ("blue-150", BLUE_150),
            ("blue-200", BLUE_200),
            ("blue-300", BLUE_300),
            ("blue", BLUE),
            ("blue-400", BLUE_400),
            ("blue-500", BLUE_500),
            ("blue-600", BLUE_600),
            ("blue-faded", BLUE_FADED),
            ("green-200", GREEN_200),
            ("green-300", GREEN_300),
            ("green", GREEN),
            ("green-400", GREEN_400),
            ("green-faded", GREEN_FADED),
            ("green-enabled", GREEN_ENABLED),
            ("red-200", RED_200),
            ("red-300", RED_300),
            ("red", RED),
            ("red-400", RED_400),
            ("red-faded", RED_FADED),
            ("yellow-300", YELLOW_300),
            ("yellow", YELLOW),
            ("yellow-faded", YELLOW_FADED),
            ("gray-300", GRAY_300),
            ("gray-400", GRAY_400),
            ("gray-500", GRAY_500),
            ("gray-550", GRAY_550),
            ("gray-600", GRAY_600),
            ("gray-700", GRAY_700),
            ("gray-750", GRAY_750),
            ("gray-800", GRAY_800),
            ("gray-850", GRAY_850),
            ("gray-875", GRAY_875),
            ("gray-900", GRAY_900),
            ("gray-faded", GRAY_FADED),
            ("fade-700", FADE_700),
            ("white", WHITE),
            ("violet", VIOLET),
            ("ultramarine", ULTRAMARINE),
        ])
    }

    /// The dark theme's CSS variables (`:root, [data-theme='dark'] { --name: value; }`)
    /// from the dashboard's `globals.css`. The dashboard's light theme came later;
    /// the launcher has only the dark one.
    fn dark_css_variables() -> BTreeMap<String, String> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../dashboard/src/globals.css");
        let css = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("reading {}: {err}", path.display()));
        let start = css
            .find("[data-theme='dark'] {")
            .expect("globals.css has a dark theme block");
        let body = &css[start..start + css[start..].find('}').unwrap()];
        body.lines()
            .filter_map(|line| {
                let line = line.trim().strip_prefix("--")?;
                let (name, value) = line.split_once(':')?;
                Some((
                    name.trim().to_string(),
                    value.trim().trim_end_matches(';').to_string(),
                ))
            })
            .collect()
    }

    #[test]
    fn colors_match_tailwind() {
        let config = tailwind_config();
        let variables = dark_css_variables();
        let mut tailwind = flatten(object(&config, "colors"));
        // `current` is CSS's currentColor, which has no fixed value.
        assert_eq!(tailwind.remove("current").as_deref(), Some("currentColor"));
        let resolve = |value: &str| -> String {
            if value == "transparent" {
                return "#00000000".to_string();
            }
            match value
                .strip_prefix("var(--")
                .and_then(|v| v.strip_suffix(')'))
            {
                Some(name) => variables
                    .get(name)
                    .unwrap_or_else(|| panic!("--{name} is not in the dark theme"))
                    .to_uppercase(),
                None => value.to_uppercase(),
            }
        };

        // Every launcher token has the dashboard's (dark theme) value. The dashboard
        // also has semantic colours for its light theme that the launcher doesn't use.
        for (name, color) in token_colors() {
            let value = tailwind
                .get(name)
                .unwrap_or_else(|| panic!("{name} is not in the dashboard's colours"));
            assert_eq!(color.to_hex(), resolve(value), "{name}");
        }
    }

    #[test]
    fn text_sizes_match_tailwind() {
        let config = tailwind_config();
        let tailwind = flatten(object(&config, "fontSize"));
        let tokens = BTreeMap::from([
            ("caption", text::CAPTION),
            ("small", text::SMALL),
            ("medium", text::MEDIUM),
            ("h3", text::H3),
            ("h2", text::H2),
            ("h1", text::H1),
            ("title", text::TITLE),
            ("2xlarge", text::XLARGE_2),
            ("3xlarge", text::XLARGE_3),
            ("4xlarge", text::XLARGE_4),
            ("5xlarge", text::XLARGE_5),
            ("6xlarge", text::XLARGE_6),
        ]);
        let tokens: BTreeMap<String, String> = tokens
            .into_iter()
            .map(|(name, size)| (name.to_string(), format!("{}rem", size.0)))
            .collect();
        assert_eq!(tokens, tailwind);
    }

    #[test]
    fn font_weights_match_tailwind() {
        let config = tailwind_config();
        let tailwind = flatten(object(&config, "fontWeight"));
        let tokens = BTreeMap::from([
            ("medium".to_string(), weights::MEDIUM.0.to_string()),
            ("mediumbold".to_string(), weights::MEDIUMBOLD.0.to_string()),
            ("bold".to_string(), weights::BOLD.0.to_string()),
            ("extrabold".to_string(), weights::EXTRABOLD.0.to_string()),
        ]);
        assert_eq!(tokens, tailwind);
    }

    #[test]
    fn font_families_match_tailwind() {
        let config = tailwind_config();
        let families = object(&config, "fontFamily");
        for (key, family) in [
            ("sans", fonts::SANS),
            ("heading", fonts::HEADING),
            ("title", fonts::TITLE),
            ("mono", fonts::MONO),
        ] {
            let line = families
                .lines()
                .find(|line| line.trim_start().starts_with(&format!("{key}:")))
                .unwrap_or_else(|| panic!("fontFamily.{key} not found"));
            assert!(
                line.contains(&format!("['{family}'")),
                "fontFamily.{key} is `{}`, expected {family} first",
                line.trim()
            );
        }
    }

    #[test]
    fn weights_render_with_distinct_faces() {
        assert_eq!(weights::MEDIUM.face(), FontWeight::NORMAL);
        assert_eq!(weights::MEDIUMBOLD.face(), FontWeight::MEDIUM);
        assert_eq!(weights::BOLD.face(), FontWeight::MEDIUM);
        assert_eq!(weights::EXTRABOLD.face(), FontWeight::BOLD);
    }

    #[test]
    fn opacity_scales_alpha() {
        assert_eq!(colors::GRAY_FADED.opacity(30), Color::rgba(0xA5A5AC4D));
        assert_eq!(colors::WHITE.opacity(50), Color::rgba(0xFFFFFF80));
        assert_eq!(colors::WHITE.opacity(100), colors::WHITE);
        assert_eq!(colors::BLUE.opacity(0), Color::rgba(0x1D8EB200));
        // An alpha already below 1 is scaled, as Tailwind's color-mix does.
        assert_eq!(colors::FADE_700.opacity(10), Color::rgba(0xD1D1DA03));
    }

    #[test]
    fn color_converts_to_gpui() {
        let rgba: Rgba = colors::BLUE_300.into();
        assert_eq!(rgba, gpui::rgb(0x1D8EB2));
        let hsla: Hsla = colors::FADE_700.into();
        assert!((hsla.a - 0x19 as f32 / 255.0).abs() < 1e-6);
    }
}
