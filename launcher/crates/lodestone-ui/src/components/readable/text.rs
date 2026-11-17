//! The text roles: a number with its unit ([`Num`]), an exact value that fits its
//! room by leaving out its middle ([`PathText`]), a short exact value
//! ([`MonoValue`]), the game's and the loaders' phrase ([`ProductPhrase`]), and the
//! small helpers for a name, a time, a key and a caption line.

use std::{ops::Range, panic::Location, rc::Rc, sync::OnceLock};

use gpui::{
    App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Rems, RenderOnce, ShapedLine, SharedString, Size, Stateful,
    Style, StyledText, TextAlign, TextStyle, Window, div, prelude::*,
};

use super::auto_id;
use crate::{
    components::{
        RichText,
        a11y::{Accessible, Role as A11yRole},
        text_tooltip,
    },
    i18n::format::{self, Measure},
    theme::{self, Color, fonts, ink, px, text, weights},
    values::{
        Product, Role, phrase, role_style,
        truncate::{
            ELLIPSIS, MAX_SHOWN, display_path, middle, middle_keeping_extension, path_candidates,
            sanitize,
        },
    },
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Amount {
    Measure(Measure),
    /// Done of a total, both in the total's unit, the unit written once.
    Fraction {
        done: String,
        total: Measure,
    },
}

/// A number with tabular figures and its unit one ink lighter, printed once after
/// the total: "1.2 / 3.4 MB". The number is ink-secondary, or the surrounding ink
/// with [`inherit_color`](Self::inherit_color) (in strips and fact tables); the unit
/// is faint; a fraction's total is muted.
///
/// A value that changes live gets a [`min_w`](Self::min_w) and
/// [`align_right`](Self::align_right), so the text around it doesn't move.
#[derive(Clone, Debug, IntoElement)]
pub struct Num {
    id: ElementId,
    amount: Amount,
    min_w: Option<Pixels>,
    align_right: bool,
    inherit_color: bool,
}

impl Num {
    /// A count: "4,091".
    #[track_caller]
    pub fn count(count: u64) -> Self {
        Self::new(
            Amount::Measure(Measure {
                value: format::number(count),
                before: "",
                after: "",
            }),
            Location::caller(),
        )
    }

    /// A size in decimal units: "4.2 MB".
    #[track_caller]
    pub fn bytes(bytes: u64) -> Self {
        Self::new(
            Amount::Measure(format::bytes_parts(bytes)),
            Location::caller(),
        )
    }

    /// A number already split from its unit.
    #[track_caller]
    pub fn measure(measure: Measure) -> Self {
        Self::new(Amount::Measure(measure), Location::caller())
    }

    /// How much of a size is done: "1.2 / 3.4 MB".
    #[track_caller]
    pub fn fraction_bytes(done: u64, total: u64) -> Self {
        let (done, total) = format::bytes_fraction(done, total);
        Self::new(Amount::Fraction { done, total }, Location::caller())
    }

    /// How many of a count are done: "3 / 5".
    #[track_caller]
    pub fn fraction(done: u64, total: u64) -> Self {
        Self::new(
            Amount::Fraction {
                done: format::number(done.min(total)),
                total: Measure {
                    value: format::number(total),
                    before: "",
                    after: "",
                },
            },
            Location::caller(),
        )
    }

    fn new(amount: Amount, caller: &'static Location<'static>) -> Self {
        let mut this = Self {
            id: ElementId::Integer(0),
            amount,
            min_w: None,
            align_right: false,
            inherit_color: false,
        };
        this.id = auto_id(caller, &this.plain());
        this
    }

    /// At least this wide, for a value that changes while it is shown.
    pub fn min_w(mut self, width: Pixels) -> Self {
        self.min_w = Some(width);
        self
    }

    /// Aligned to the right of its width (in a column, or with a minimum width).
    pub fn align_right(mut self) -> Self {
        self.align_right = true;
        self
    }

    /// The number in the surrounding ink rather than ink-secondary, as a strip's or
    /// a fact table's values are.
    pub fn inherit_color(mut self) -> Self {
        self.inherit_color = true;
        self
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// The number as plain text, for accessible names and tooltips.
    pub fn plain(&self) -> String {
        let (text, _) = self.parts();
        text
    }

    /// The text and its parts.
    fn parts(&self) -> (String, Vec<(Range<usize>, Part)>) {
        let mut text = String::new();
        let mut parts = Vec::new();
        let mut push = |piece: &str, part: Part| {
            if !piece.is_empty() {
                let start = text.len();
                text.push_str(piece);
                parts.push((start..text.len(), part));
            }
        };
        match &self.amount {
            Amount::Measure(measure) => {
                push(measure.before, Part::Unit);
                push(&measure.value, Part::Value);
                push(measure.after, Part::Unit);
            }
            Amount::Fraction { done, total } => {
                push(total.before, Part::Unit);
                push(done, Part::Value);
                push(" / ", Part::Unit);
                push(&total.value, Part::Total);
                push(total.after, Part::Unit);
            }
        }
        (text, parts)
    }
}

#[derive(Clone, Copy, Debug)]
enum Part {
    Value,
    Total,
    Unit,
}

impl RenderOnce for Num {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let base = window.text_style();
        let (text, parts) = self.parts();
        let runs = parts
            .iter()
            .map(|(range, part)| {
                let mut style = match part {
                    Part::Unit => role_style(&base, Role::Unit),
                    Part::Value | Part::Total => role_style(&base, Role::Number),
                };
                match part {
                    Part::Value if self.inherit_color => style.color = base.color,
                    Part::Total => style.color = ink::muted().into(),
                    _ => {}
                }
                style.to_run(range.len())
            })
            .collect();
        let text: SharedString = text.into();
        let styled = StyledText::new(text.clone()).with_runs(runs);
        div()
            .flex_none()
            .whitespace_nowrap()
            .when_some(self.min_w, |this, width| this.min_w(width))
            .when(self.align_right, |this| this.text_right())
            .child(RichText::styled(self.id, text, styled))
    }
}

/// How a [`PathText`] shortens its value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PathKind {
    /// A path: the home folder as `~`, then the last two parts kept.
    Path,
    /// A file name: the extension kept.
    File,
    /// Any other exact value: the middle left out.
    Exact,
}

/// The home folder, read once.
fn home_dir() -> Option<&'static str> {
    static HOME: OnceLock<Option<String>> = OnceLock::new();
    HOME.get_or_init(|| dirs::home_dir().map(|home| home.to_string_lossy().into_owned()))
        .as_deref()
}

/// An exact value in JetBrains Mono that fits its room by leaving out its middle: a
/// path keeps its root and last two parts (`~/…/instances/old-pack`), a file name its
/// extension (`sodium-fab…21.1.jar`). The full value is its tooltip and its
/// accessible name. Paths under the home folder are shown with `~`.
///
/// It takes the room its parent gives it and is never wider than its value; put it
/// in a flex row as a `flex_1` or `min_w_0` child.
#[derive(Clone, Debug, IntoElement)]
pub struct PathText {
    id: ElementId,
    /// The value as given, made safe.
    full: SharedString,
    kind: PathKind,
    /// The home folder to show as `~` (`None`: none); the user's when not set.
    home: Option<Option<String>>,
    size: Option<Rems>,
    color: Option<Color>,
    mono: bool,
    /// An accessible name of its own, such as "Folder: ~/…" (the value otherwise).
    label: Option<SharedString>,
}

impl PathText {
    /// A path on this computer or another.
    #[track_caller]
    pub fn path(path: impl Into<SharedString>) -> Self {
        Self::new(path.into(), PathKind::Path, Location::caller())
    }

    /// A file name.
    #[track_caller]
    pub fn file(name: impl Into<SharedString>) -> Self {
        Self::new(name.into(), PathKind::File, Location::caller())
    }

    /// An address, a URL, an id, a long version.
    #[track_caller]
    pub fn exact(value: impl Into<SharedString>) -> Self {
        Self::new(value.into(), PathKind::Exact, Location::caller())
    }

    fn new(value: SharedString, kind: PathKind, caller: &'static Location<'static>) -> Self {
        let full: SharedString = sanitize(&value, MAX_SHOWN).into();
        Self {
            id: auto_id(caller, &full),
            full,
            kind,
            home: None,
            size: None,
            color: None,
            mono: true,
            label: None,
        }
    }

    /// The home folder shown as `~` (`None` for none); the user's by default.
    pub fn home(mut self, home: Option<String>) -> Self {
        self.home = Some(home);
        self
    }

    /// The text size (the surrounding size otherwise).
    pub fn size(mut self, size: Rems) -> Self {
        self.size = Some(size);
        self
    }

    /// The text colour (the surrounding ink otherwise).
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// In the surrounding font rather than JetBrains Mono, for a name that isn't
    /// exact text but may be long.
    pub fn proportional(mut self) -> Self {
        self.mono = false;
        self
    }

    /// Its accessible name (the full value by default).
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// The value as given (made safe), for tooltips and accessible names.
    pub fn full(&self) -> &SharedString {
        &self.full
    }

    /// The ways to show the value, from fullest to shortest.
    fn candidates(&self) -> Vec<SharedString> {
        match self.kind {
            PathKind::Path => {
                let home = match &self.home {
                    Some(home) => home.as_deref(),
                    None => home_dir(),
                };
                path_candidates(&display_path(&self.full, home))
                    .into_iter()
                    .map(Into::into)
                    .collect()
            }
            PathKind::File | PathKind::Exact => vec![self.full.clone()],
        }
    }
}

impl RenderOnce for PathText {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let candidates: Rc<[SharedString]> = self.candidates().into();
        let label = self.label.clone().unwrap_or_else(|| self.full.clone());
        div()
            .id(self.id.clone())
            .accessible_id(A11yRole::Label, &self.id, label)
            .tooltip(text_tooltip(self.full.clone()))
            .min_w_0()
            .overflow_hidden()
            .child(FitText {
                fit: Rc::new(Fit {
                    candidates,
                    keep_extension: self.kind != PathKind::Exact,
                }),
                family: self.mono.then(fonts::mono),
                size: self.size,
                color: self.color,
            })
    }
}

/// What a [`FitText`] can show.
struct Fit {
    candidates: Rc<[SharedString]>,
    /// The last resort keeps an extension (paths and file names).
    keep_extension: bool,
}

impl Fit {
    /// The fullest way to show the value in `width`: the first candidate that fits,
    /// else the shortest one cut to fit.
    fn choose(&self, width: Pixels, measure: &mut dyn FnMut(&str) -> Pixels) -> SharedString {
        for candidate in self.candidates.iter() {
            if measure(candidate) <= width {
                return candidate.clone();
            }
        }
        let Some(last) = self.candidates.last() else {
            return SharedString::default();
        };
        let shorten = |max: usize| {
            if self.keep_extension {
                middle_keeping_extension(last, max)
            } else {
                middle(last, max)
            }
        };
        // The most graphemes that fit, found by halving.
        let (mut lo, mut hi) = (0usize, last.chars().count());
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            if measure(&shorten(mid)) <= width {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        shorten(lo.max(1)).into()
    }
}

/// One line of text that shows the fullest of its candidates that fits.
struct FitText {
    fit: Rc<Fit>,
    family: Option<SharedString>,
    size: Option<Rems>,
    color: Option<Color>,
}

impl IntoElement for FitText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// The width of `text` in `style`.
fn line_width(text: &str, style: &TextStyle, window: &mut Window) -> Pixels {
    shape(text, style, window).width
}

fn shape(text: &str, style: &TextStyle, window: &mut Window) -> ShapedLine {
    let font_size = style.font_size.to_pixels(window.rem_size());
    let text = SharedString::from(text.to_string());
    let runs = [style.to_run(text.len())];
    window
        .text_system()
        .shape_line(text, font_size, &runs, None)
}

impl Element for FitText {
    type RequestLayoutState = TextStyle;
    type PrepaintState = Option<ShapedLine>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        _cx: &mut App,
    ) -> (LayoutId, TextStyle) {
        let mut style = window.text_style();
        if let Some(family) = self.family.clone() {
            style.font_family = family;
        }
        if let Some(size) = self.size {
            style.font_size = theme::text_size(size);
        }
        if let Some(color) = self.color {
            style.color = color.into();
        }
        let fit = self.fit.clone();
        let measured = style.clone();
        let layout_id = window.request_measured_layout(
            Style::default(),
            move |known, available, window, _cx| {
                let height = measured.line_height_in_pixels(window.rem_size());
                let mut measure = |text: &str| line_width(text, &measured, window);
                let width = known.width.unwrap_or_else(|| match available.width {
                    AvailableSpace::Definite(width) => {
                        let chosen = fit.choose(width, &mut measure);
                        measure(&chosen)
                    }
                    // shrinks to an ellipsis at the least
                    AvailableSpace::MinContent => measure(ELLIPSIS),
                    AvailableSpace::MaxContent => fit
                        .candidates
                        .first()
                        .map_or(Pixels::ZERO, |full| measure(full)),
                });
                Size { width, height }
            },
        );
        (layout_id, style)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        style: &mut TextStyle,
        window: &mut Window,
        _cx: &mut App,
    ) -> Option<ShapedLine> {
        let chosen = self.fit.choose(bounds.size.width + px(0.5), &mut |text| {
            line_width(text, style, window)
        });
        Some(shape(&chosen, style, window))
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        style: &mut TextStyle,
        line: &mut Option<ShapedLine>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(line) = line {
            let line_height = style.line_height_in_pixels(window.rem_size());
            let _ = line.paint(
                bounds.origin,
                line_height,
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
    }
}

/// A short exact value inline: a port, an id, an environment variable. JetBrains
/// Mono 12px in ink-secondary, never cut. Longer or actionable values belong in a
/// [`ValueField`](super::ValueField).
#[derive(IntoElement)]
pub struct MonoValue {
    text: SharedString,
    caller: &'static Location<'static>,
}

impl MonoValue {
    #[track_caller]
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: sanitize(&text.into(), MAX_SHOWN).into(),
            caller: Location::caller(),
        }
    }
}

impl RenderOnce for MonoValue {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let id = auto_id(self.caller, &self.text);
        div()
            .flex_none()
            .whitespace_nowrap()
            .font_family(fonts::mono())
            .text_size(theme::text_size(text::SMALL))
            .text_color(ink::secondary())
            .child(RichText::new(self.text).id(id))
    }
}

/// A name in the object's role: medium weight in the main ink, cut at the end with
/// the whole name as its tooltip.
#[track_caller]
pub fn object_name(name: impl Into<SharedString>) -> Stateful<gpui::Div> {
    let name: SharedString = sanitize(&name.into(), MAX_SHOWN).into();
    div()
        .id(auto_id(Location::caller(), &name))
        .min_w_0()
        .truncate()
        .font_weight(weights::MEDIUMBOLD.into())
        .text_color(ink::text())
        .tooltip(text_tooltip(name.clone()))
        .child(name)
}

/// A time or a duration, in ink-muted.
pub fn time_text(time: impl Into<SharedString>) -> gpui::Div {
    div()
        .flex_none()
        .whitespace_nowrap()
        .text_color(ink::muted())
        .child(time.into())
}

/// A value's key: 12px, weight 450, ink-faint.
pub fn key_text(key: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_size(theme::text_size(text::SMALL))
        .font_weight(weights::MEDIUM.into())
        .text_color(ink::faint())
        .child(key.into())
}

/// The game or a loader at a version, as one plain phrase in the surrounding ink and
/// size: "Minecraft 26.3", "Fabric 0.20.1". It never wraps apart. Mods' versions use
/// a [`VersionTag`](super::VersionTag) instead.
#[derive(Clone, Debug, IntoElement)]
pub struct ProductPhrase {
    product: Product,
    version: SharedString,
}

impl ProductPhrase {
    pub fn new(product: Product, version: impl Into<SharedString>) -> Self {
        Self {
            product,
            version: sanitize(&version.into(), 64).into(),
        }
    }

    /// The phrase for a name the game or a loader goes by ("Fabric", "neoforge");
    /// `None` for anything else, such as a mod.
    pub fn from_name(name: &str, version: impl Into<SharedString>) -> Option<Self> {
        Product::from_name(name).map(|product| Self::new(product, version))
    }

    /// The key it has in a strip or a fact table: "Game" or "Loader", translated.
    pub fn key(&self) -> &'static str {
        crate::i18n::t(self.product.fact_key())
    }

    /// The phrase as text.
    pub fn plain(&self) -> String {
        phrase(self.product, &self.version)
    }
}

impl RenderOnce for ProductPhrase {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div().flex_none().whitespace_nowrap().child(self.plain())
    }
}

/// A 12px ink-faint line under a heading: where a list comes from, or the folder a
/// page shows (a path, in mono, shortened in the middle).
#[derive(IntoElement)]
pub struct CaptionLine {
    content: Caption,
}

enum Caption {
    Text(SharedString),
    Path(PathText),
}

impl CaptionLine {
    pub fn text(text: impl Into<SharedString>) -> Self {
        Self {
            content: Caption::Text(text.into()),
        }
    }

    #[track_caller]
    pub fn path(path: impl Into<SharedString>) -> Self {
        Self {
            content: Caption::Path(PathText::path(path)),
        }
    }
}

impl RenderOnce for CaptionLine {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .min_w_0()
            .text_size(theme::text_size(text::SMALL))
            .text_color(ink::faint())
            .child(match self.content {
                Caption::Text(text) => div().truncate().child(text).into_any_element(),
                Caption::Path(path) => path.into_any_element(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One pixel a character.
    fn chars() -> impl FnMut(&str) -> Pixels {
        |text: &str| px(text.chars().count() as f32)
    }

    #[test]
    fn a_path_shows_the_fullest_candidate_that_fits() {
        let fit = Fit {
            candidates: path_candidates("~/Library/Lodestone/instances/old-pack")
                .into_iter()
                .map(SharedString::from)
                .collect::<Vec<_>>()
                .into(),
            keep_extension: true,
        };
        let mut measure = chars();
        assert_eq!(
            fit.choose(px(100.), &mut measure),
            "~/Library/Lodestone/instances/old-pack"
        );
        assert_eq!(fit.choose(px(24.), &mut measure), "~/…/instances/old-pack");
        assert_eq!(fit.choose(px(12.), &mut measure), "…/old-pack");
        // shorter than every candidate: the last one, cut to fit
        assert_eq!(fit.choose(px(6.), &mut measure), "…/o…ck");
        assert_eq!(fit.choose(px(0.), &mut measure), "…");
    }

    #[test]
    fn a_file_name_keeps_its_extension_when_cut() {
        let fit = Fit {
            candidates: vec![SharedString::from("sodium-fabric-0.6.0+mc1.21.1.jar")].into(),
            keep_extension: true,
        };
        let chosen = fit.choose(px(16.), &mut chars());
        assert!(chosen.ends_with(".jar"), "{chosen}");
        assert_eq!(chosen.chars().count(), 16);
    }

    #[test]
    fn nums_put_the_unit_once_after_the_total() {
        let num = Num::fraction(3, 5);
        assert_eq!(num.plain(), "3 / 5");
        let num = Num::measure(Measure {
            value: "1.2".into(),
            before: "",
            after: " MB",
        });
        assert_eq!(num.plain(), "1.2 MB");
        let (text, parts) = Num::fraction(7, 5).parts();
        // never more done than the total
        assert_eq!(text, "5 / 5");
        assert_eq!(parts.len(), 3);
    }

    #[test]
    fn the_product_phrase_is_keyed_game_or_loader() {
        let phrase = ProductPhrase::from_name("fabric-loader", "0.20.1").unwrap();
        assert_eq!(phrase.plain(), "Fabric\u{a0}0.20.1");
        assert!(ProductPhrase::from_name("Sodium", "0.6.0").is_none());
        let game = ProductPhrase::new(Product::Minecraft, "26.3");
        assert_eq!(game.plain(), "Minecraft\u{a0}26.3");
    }
}
