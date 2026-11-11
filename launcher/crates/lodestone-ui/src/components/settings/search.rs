//! Searching a Settings page: the field at its top, and the query its rows are
//! filtered by.
//!
//! A page reads a [`SettingsQuery`] from its [`SettingsSearch`] each time it is drawn
//! and builds its sections through it: [`SettingsQuery::section`] and
//! [`SettingsQuery::card`] leave out the rows that don't match (before they are laid
//! out), drop a section none of whose rows match, and count the rows shown, so
//! [`SettingsQuery::status`] can say how many were found, or that none were.
//!
//! A row matches when every word typed is found in its name, hint, keywords or value
//! (see [`SettingRow::keywords`]), ignoring case and accents, in the language shown
//! and in English: "memory" finds "Mémoire". A section whose own title matches shows
//! all its rows.

use std::{
    cell::Cell,
    collections::HashMap,
    rc::Rc,
    sync::{Mutex, OnceLock},
};

use gpui::{
    AnyElement, App, AppContext as _, Context, Div, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, WeakEntity, Window, actions, div, prelude::FluentBuilder as _,
};

use super::{SettingRow, card, section};
use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, Icon, TextInput,
        a11y::{Accessible, Role},
        focus::Dismiss,
    },
    i18n::{self, CommonMsg, CommonPlural, Locale, t},
    theme::{self, colors, leading, px, text, weights},
};

actions!(
    settings,
    [
        /// ⌘F (Ctrl+F elsewhere): to the search field of the Settings page shown.
        FindSetting
    ]
);

/// Binds ⌘F (Ctrl+F) to [`FindSetting`]. A page with a find of its own (the Host's
/// Files) binds the key in its own context, which comes first.
pub(crate) fn init(cx: &mut App) {
    let key = if cfg!(target_os = "macos") {
        "cmd-f"
    } else {
        "ctrl-f"
    };
    cx.bind_keys([KeyBinding::new(key, FindSetting, None)]);
}

/// Finds the English of a message shown in the current locale; set by each app for
/// its own messages with [`set_english_lookup`].
type EnglishLookup = fn(&str) -> Option<&'static str>;

static ENGLISH: OnceLock<EnglishLookup> = OnceLock::new();

/// Lets a search match a message in English as well as in the language shown: the
/// app's lookup from a message's text in the current locale to its English. Call
/// once at startup.
pub fn set_english_lookup(lookup: EnglishLookup) {
    let _ = ENGLISH.set(lookup);
}

/// An index from each message's text in the current locale to its English, made
/// again when the locale changes, for an app's [`set_english_lookup`].
pub type EnglishIndex = Mutex<Option<(Locale, HashMap<&'static str, &'static str>)>>;

/// The English of `text` in `index`, made from `messages` (each message in a locale
/// and in English) when the locale has changed.
pub fn english_in(
    index: &EnglishIndex,
    messages: impl FnOnce(Locale) -> Vec<(&'static str, &'static str)>,
    text: &str,
) -> Option<&'static str> {
    english_in_locale(index, i18n::locale(), messages, text)
}

fn english_in_locale(
    index: &EnglishIndex,
    locale: Locale,
    messages: impl FnOnce(Locale) -> Vec<(&'static str, &'static str)>,
    text: &str,
) -> Option<&'static str> {
    if locale == Locale::En {
        return None;
    }
    let mut index = index.lock().ok()?;
    if index.as_ref().is_none_or(|(made, _)| *made != locale) {
        let map = messages(locale)
            .into_iter()
            .filter(|(shown, english)| shown != english)
            .collect();
        *index = Some((locale, map));
    }
    index.as_ref()?.1.get(text).copied()
}

/// The English of a message shown in the current locale: through the app's lookup,
/// or this crate's own messages without one.
fn english_of(text: &str) -> Option<&'static str> {
    match ENGLISH.get() {
        Some(lookup) => lookup(text),
        None => {
            static INDEX: EnglishIndex = Mutex::new(None);
            english_in(
                &INDEX,
                |locale| {
                    i18n::Msg::all()
                        .into_iter()
                        .map(|m| (m.text(locale), m.text(Locale::En)))
                        .collect()
                },
                text,
            )
        }
    }
}

/// `text` lower-cased, without the accents of Latin letters (so "memoire" finds
/// "Mémoire"), and with typographic apostrophes and non-breaking spaces made plain.
pub fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => out.push('a'),
            'ç' => out.push('c'),
            'è' | 'é' | 'ê' | 'ë' => out.push('e'),
            'ì' | 'í' | 'î' | 'ï' => out.push('i'),
            'ñ' => out.push('n'),
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => out.push('o'),
            'ù' | 'ú' | 'û' | 'ü' => out.push('u'),
            'ý' | 'ÿ' => out.push('y'),
            'œ' => out.push_str("oe"),
            'æ' => out.push_str("ae"),
            '’' | '‘' => out.push('\''),
            '\u{a0}' | '\u{202f}' => out.push(' '),
            other => out.push(other),
        }
    }
    out
}

/// Whether `term` is in `haystack` at the start of a word: "port" finds "Open port"
/// and "portal", not "support". Chinese and Japanese, written without spaces, match
/// anywhere.
fn contains_word_start(haystack: &str, term: &str) -> bool {
    let Some(first) = term.chars().next() else {
        return true;
    };
    if !is_word_char(first) {
        return haystack.contains(term);
    }
    haystack.match_indices(term).any(|(at, _)| {
        haystack[..at]
            .chars()
            .next_back()
            .is_none_or(|before| !is_word_char(before))
    })
}

/// A letter or digit of a language written with spaces between words.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric()
        && !matches!(c,
            '\u{3040}'..='\u{30ff}' // kana
            | '\u{3400}'..='\u{4dbf}' // CJK extension A
            | '\u{4e00}'..='\u{9fff}' // CJK
            | '\u{f900}'..='\u{faff}' // CJK compatibility
            | '\u{ac00}'..='\u{d7af}' // Hangul
        )
}

/// What "Clear search" does.
type ClearSearch = Rc<dyn Fn(&mut Window, &mut App)>;

/// What a Settings page's search field holds, read once each time the page is
/// drawn: the words typed, and a count of the rows shown for them.
#[derive(Clone, Default)]
pub struct SettingsQuery {
    /// As typed, trimmed.
    text: SharedString,
    /// The words, [`fold`]ed.
    terms: Rc<[String]>,
    /// The rows shown so far while the page is drawn.
    found: Rc<Cell<usize>>,
    /// Empties the field ("Clear search").
    clear: Option<ClearSearch>,
}

impl SettingsQuery {
    pub fn new(text: impl Into<SharedString>) -> Self {
        let text: SharedString = text.into();
        let trimmed = text.trim();
        let terms: Vec<String> = fold(trimmed)
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        Self {
            text: if trimmed.len() == text.len() {
                text
            } else {
                trimmed.to_owned().into()
            },
            terms: terms.into(),
            found: Rc::default(),
            clear: None,
        }
    }

    /// What "Clear search" does.
    pub fn on_clear(mut self, clear: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.clear = Some(Rc::new(clear));
        self
    }

    /// Something is typed: the page shows only what matches.
    pub fn is_active(&self) -> bool {
        !self.terms.is_empty()
    }

    /// What was typed, trimmed.
    pub fn text(&self) -> &SharedString {
        &self.text
    }

    /// Whether every word typed is in one of `texts` (or in its English). Always, with
    /// nothing typed.
    pub fn matches<'a>(&self, texts: impl IntoIterator<Item = &'a str>) -> bool {
        if !self.is_active() {
            return true;
        }
        let mut haystack = String::new();
        for text in texts {
            haystack.push_str(&fold(text));
            haystack.push('\n');
            if let Some(english) = english_of(text) {
                haystack.push_str(&fold(english));
                haystack.push('\n');
            }
        }
        self.terms
            .iter()
            .all(|term| contains_word_start(&haystack, term))
    }

    /// Whether `row` matches.
    pub fn matches_row(&self, row: &SettingRow) -> bool {
        self.matches(row.search_texts())
    }

    /// The rows shown so far while the page is drawn.
    pub fn found(&self) -> usize {
        self.found.get()
    }

    /// Counts `rows` more as shown, for a part of the page drawn on its own (a list
    /// in a view of its own) that did its own matching.
    pub fn add_found(&self, rows: usize) {
        self.found.set(self.found.get() + rows);
    }

    /// Of `rows`, those that match: all of them if `all`.
    fn filter(&self, rows: impl IntoIterator<Item = SearchRow>, all: bool) -> Vec<AnyElement> {
        let all = all || !self.is_active();
        rows.into_iter()
            .filter(|row| all || row.matches(self))
            .map(SearchRow::into_any_element)
            .collect()
    }

    /// A card of the rows that match, counted as found; none if no row does.
    pub fn card(&self, rows: impl IntoIterator<Item = impl Into<SearchRow>>) -> Option<Div> {
        let shown = self.filter(rows.into_iter().map(Into::into), false);
        self.add_found(shown.len());
        (!shown.is_empty()).then(|| card(shown))
    }

    /// A section to fill with cards ([`SearchSection::card`]); its rows are filtered
    /// unless its title matches.
    pub fn section(
        &self,
        title: impl Into<SharedString>,
        lead: Option<SharedString>,
    ) -> SearchSection {
        let title: SharedString = title.into();
        let all = !self.is_active() || self.matches([title.as_ref()]);
        SearchSection {
            query: self.clone(),
            title,
            lead,
            all,
            found: 0,
            parts: Vec::new(),
        }
    }

    /// Empties the search field.
    pub fn clear(&self, window: &mut Window, cx: &mut App) {
        if let Some(clear) = &self.clear {
            clear(window, cx);
        }
    }

    /// While searching, the line at the top of the page that says how many settings
    /// were found, which assistive technology reads out as it changes; or, when none
    /// were, a note saying so with a button that clears the search. Call it after
    /// building the page's sections, which count what they show.
    pub fn status(&self) -> Option<AnyElement> {
        if !self.is_active() {
            return None;
        }
        let found = self.found();
        if found > 0 {
            let message: SharedString =
                i18n::plural(CommonPlural::SettingsFound, found as u64, &[]).into();
            return Some(
                div()
                    .id("settings-search-status")
                    .accessible(Role::Status, "settings-search-status", message.clone())
                    .text_size(theme::text_size(text::SMALL))
                    .line_height(theme::line_height(text::SMALL, leading::NORMAL))
                    .text_color(crate::theme::ink::muted())
                    .child(message)
                    .into_any_element(),
            );
        }
        let message: SharedString =
            i18n::interpolate(t(CommonMsg::NoSettingsMatch), &[("query", &self.text)]).into();
        let clear = self.clear.clone();
        Some(
            div()
                .id("settings-search-empty")
                .accessible(Role::Status, "settings-search-empty", message.clone())
                .flex()
                .flex_col()
                .items_center()
                .gap_3()
                .w_full()
                .py_10()
                .px_4()
                .rounded(theme::radius::XL)
                .border_1()
                .border_color(crate::theme::ink::border())
                .child(Icon::MagnifyingGlass.render(px(20.), colors::GRAY_500))
                .child(
                    div()
                        .text_center()
                        .font_weight(weights::BOLD.into())
                        .text_color(crate::theme::ink::text())
                        .child(message),
                )
                .when_some(clear, |this, clear| {
                    this.child(
                        Button::new("settings-search-clear", t(CommonMsg::ClearSearch))
                            .variant(ButtonVariant::Secondary)
                            .size(ButtonSize::Slim)
                            .on_click(move |_, window, cx| clear(window, cx)),
                    )
                })
                .into_any_element(),
        )
    }
}

/// A row of a card, with the text a search looks for in it: a [`SettingRow`] (its
/// name, hint and keywords), or any element with the words given.
pub struct SearchRow {
    texts: Vec<SharedString>,
    element: RowElement,
}

enum RowElement {
    Row(SettingRow),
    Any(AnyElement),
}

impl SearchRow {
    /// An element that isn't a [`SettingRow`] (a view of its own), found by `texts`.
    pub fn new(
        element: impl IntoElement,
        texts: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        Self {
            texts: texts.into_iter().map(Into::into).collect(),
            element: RowElement::Any(element.into_any_element()),
        }
    }

    fn matches(&self, query: &SettingsQuery) -> bool {
        match &self.element {
            RowElement::Row(row) => query.matches_row(row),
            RowElement::Any(_) => query.matches(self.texts.iter().map(SharedString::as_ref)),
        }
    }

    fn into_any_element(self) -> AnyElement {
        match self.element {
            RowElement::Row(row) => row.into_any_element(),
            RowElement::Any(element) => element,
        }
    }
}

impl From<SettingRow> for SearchRow {
    fn from(row: SettingRow) -> Self {
        Self {
            texts: Vec::new(),
            element: RowElement::Row(row),
        }
    }
}

/// An element with nothing to search: shown only while nothing is typed, or when its
/// section's title matches.
impl From<AnyElement> for SearchRow {
    fn from(element: AnyElement) -> Self {
        Self {
            texts: Vec::new(),
            element: RowElement::Any(element),
        }
    }
}

/// A section of a page being searched: its heading, then cards of the rows that
/// match. Made by [`SettingsQuery::section`]; [`finish`](Self::finish) gives nothing
/// when nothing in it matches.
pub struct SearchSection {
    query: SettingsQuery,
    title: SharedString,
    lead: Option<SharedString>,
    /// Shows every row: nothing is typed, or the title matches.
    all: bool,
    found: usize,
    parts: Vec<Part>,
}

enum Part {
    /// Rows that matched, in a card.
    Found(AnyElement),
    /// Something that isn't a setting (a note, a button under the cards): shown when
    /// anything in the section is.
    Context(AnyElement),
}

impl SearchSection {
    /// A card of the rows that match.
    pub fn card(self, rows: impl IntoIterator<Item = impl Into<SearchRow>>) -> Self {
        self.card_with(rows, |card| card)
    }

    /// A card of the rows that match, styled by `style` (a red border for a danger
    /// zone).
    pub fn card_with(
        mut self,
        rows: impl IntoIterator<Item = impl Into<SearchRow>>,
        style: impl FnOnce(Div) -> Div,
    ) -> Self {
        let shown = self
            .query
            .filter(rows.into_iter().map(Into::into), self.all);
        if !shown.is_empty() {
            self.found += shown.len();
            self.parts
                .push(Part::Found(style(card(shown)).into_any_element()));
        }
        self
    }

    /// A setting outside a card, shown if it matches.
    pub fn row(mut self, row: impl Into<SearchRow>) -> Self {
        let row = row.into();
        if self.all || row.matches(&self.query) {
            self.found += 1;
            self.parts.push(Part::Found(row.into_any_element()));
        }
        self
    }

    /// Something that isn't a setting, such as a note or a button under the cards:
    /// shown when anything else in the section is.
    pub fn child(mut self, element: impl IntoElement) -> Self {
        self.parts.push(Part::Context(element.into_any_element()));
        self
    }

    /// Adds `rows` to the count, for settings drawn by a view of the section's own
    /// that did its own matching, and shows the section for them.
    pub fn add_found(mut self, rows: usize) -> Self {
        self.found += rows;
        self
    }

    /// Whether every row is shown: nothing is typed, or the title matches.
    pub fn shows_all(&self) -> bool {
        self.all
    }

    /// The section, counting its rows as found; nothing while searching if nothing in
    /// it matched.
    pub fn finish(self) -> Option<Div> {
        if self.query.is_active() && self.found == 0 && !self.all {
            return None;
        }
        self.query.add_found(self.found);
        Some(
            section(self.title, self.lead).children(self.parts.into_iter().map(
                |part| match part {
                    Part::Found(element) | Part::Context(element) => element,
                },
            )),
        )
    }
}

impl gpui::prelude::FluentBuilder for SearchSection {}

/// Says the search changed, for a page that scrolls back to its top.
pub struct SettingsSearchChanged;

/// The search field at the top of a Settings page: a magnifying glass, the text and a
/// × that clears it. Escape clears it too (and with nothing typed, closes what
/// Escape would); [`FindSetting`] (⌘F) comes to it. Pages read
/// [`query`](Self::query) when drawn.
pub struct SettingsSearch {
    input: Entity<TextInput>,
    text: SharedString,
    this: WeakEntity<Self>,
    _observe: Subscription,
}

impl EventEmitter<SettingsSearchChanged> for SettingsSearch {}

impl SettingsSearch {
    /// `automation_id` names the field for automation, such as `settings-search`.
    pub fn new(automation_id: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        let automation_id = automation_id.into();
        let input = cx.new(|cx| {
            TextInput::new(cx)
                .with_placeholder(CommonMsg::SearchSettings)
                .with_label(CommonMsg::SearchSettings)
                .with_leading_icon(Icon::MagnifyingGlass)
                .with_clear_button()
                .with_automation_id(automation_id)
        });
        let _observe = cx.observe(&input, |this, input, cx| {
            let text = input.read(cx).text().clone();
            if text != this.text {
                this.text = text;
                cx.emit(SettingsSearchChanged);
                cx.notify();
            }
        });
        Self {
            input,
            text: SharedString::default(),
            this: cx.entity().downgrade(),
            _observe,
        }
    }

    /// What is typed, for the page being drawn.
    pub fn query(&self) -> SettingsQuery {
        let this = self.this.clone();
        SettingsQuery::new(self.text.clone()).on_clear(move |window, cx| {
            this.update(cx, |this, cx| this.clear(window, cx)).ok();
        })
    }

    /// Whether anything is typed.
    pub fn is_active(&self) -> bool {
        !self.text.trim().is_empty()
    }

    /// Types `text`, for the offscreen screenshots and tests.
    pub fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        let text = text.into();
        self.input.update(cx, |input, cx| input.set_text(text, cx));
    }

    /// Empties the field and puts the focus in it.
    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input
            .update(cx, |input, cx| input.set_text(SharedString::default(), cx));
        self.focus(window, cx);
    }

    /// Puts the focus in the field.
    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.input.focus_handle(cx), cx);
    }
}

impl Focusable for SettingsSearch {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl Render for SettingsSearch {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("settings-search")
            .w_full()
            .on_action(cx.listener(|this, _: &Dismiss, window, cx| {
                if this.text.is_empty() {
                    cx.propagate();
                } else {
                    this.clear(window, cx);
                }
            }))
            .child(self.input.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folding_ignores_case_and_accents() {
        assert_eq!(fold("Mémoire VIVE"), "memoire vive");
        assert_eq!(fold("Arguments de la JVM"), "arguments de la jvm");
        assert_eq!(fold("l’œuvre\u{a0}ici"), "l'oeuvre ici");
        assert_eq!(fold("内存"), "内存");
    }

    #[test]
    fn every_word_must_be_found_in_some_text() {
        let query = SettingsQuery::new("  java   MEM ");
        assert!(query.is_active());
        assert_eq!(query.text().as_ref(), "java   MEM");
        assert!(query.matches(["Java", "Memory and arguments"]));
        assert!(query.matches(["Java memory"]));
        assert!(!query.matches(["Java"]));
        assert!(!query.matches(["Memory"]));
        assert!(query.matches(["Mémoire de JAVA"]));
    }

    #[test]
    fn words_match_from_their_start() {
        let port = SettingsQuery::new("port");
        assert!(port.matches(["Open port"]));
        assert!(port.matches(["Port"]));
        assert!(port.matches(["(port 25565)"]));
        assert!(port.matches(["portal"]));
        assert!(!port.matches(["Plugin support"]));
        assert!(SettingsQuery::new("mem").matches(["Memory"]));
        assert!(SettingsQuery::new("xmx").matches(["-Xmx4G"]));
        assert!(SettingsQuery::new("25565").matches(["port 25565"]));
    }

    #[test]
    fn nothing_typed_matches_everything() {
        let query = SettingsQuery::new("   ");
        assert!(!query.is_active());
        assert!(query.matches(["anything"]));
        assert!(query.matches(std::iter::empty::<&str>()));
    }

    #[test]
    fn the_english_of_a_message_is_found_in_its_locale() {
        let index: EnglishIndex = Mutex::new(None);
        let messages = |locale| {
            i18n::Msg::all()
                .into_iter()
                .map(|m| (m.text(locale), m.text(Locale::En)))
                .collect()
        };
        let french = t_in(CommonMsg::SearchSettings, Locale::Fr);
        assert_eq!(
            english_in_locale(&index, Locale::Fr, messages, french),
            Some("Search settings")
        );
        assert_eq!(
            english_in_locale(&index, Locale::Fr, messages, "nope"),
            None
        );
        // in English there is nothing more to search
        assert_eq!(
            english_in_locale(&index, Locale::En, messages, "Search settings"),
            None
        );
    }

    fn t_in(message: CommonMsg, locale: Locale) -> &'static str {
        i18n::Msg::from(message).text(locale)
    }

    #[test]
    fn chinese_matches_without_spaces() {
        let query = SettingsQuery::new("内存");
        assert!(query.matches(["最大内存"]));
        assert!(!query.matches(["窗口"]));
    }

    #[test]
    fn a_row_matches_by_its_name_hint_and_keywords() {
        let row = SettingRow::new("Memory")
            .hint("How much the game may use")
            .keywords("RAM heap");
        assert!(SettingsQuery::new("memory").matches_row(&row));
        assert!(SettingsQuery::new("game use").matches_row(&row));
        assert!(SettingsQuery::new("ram").matches_row(&row));
        assert!(!SettingsQuery::new("java").matches_row(&row));
        let valued = SettingRow::new("Minecraft").search_text("1.21.4");
        assert!(SettingsQuery::new("1.21").matches_row(&valued));
    }

    #[test]
    fn cards_and_sections_keep_only_what_matches_and_count_it() {
        {
            let query = SettingsQuery::new("ram");
            let rows = || {
                [
                    SettingRow::new("Memory").keywords("RAM"),
                    SettingRow::new("Window size"),
                ]
            };
            assert!(query.card(rows()).is_some());
            assert_eq!(query.found(), 1);
            // a section with no match is left out and counts nothing
            assert!(
                query
                    .section("Display", None)
                    .card([SettingRow::new("Window size")])
                    .child(div())
                    .finish()
                    .is_none()
            );
            assert_eq!(query.found(), 1);
            // a section whose title matches shows every row
            let titled = SettingsQuery::new("display");
            assert!(
                titled
                    .section("Display", None)
                    .card(rows())
                    .finish()
                    .is_some()
            );
            assert_eq!(titled.found(), 2);
            // an element with nothing to search is hidden while searching
            let other = query
                .section("Other", None)
                .card([SearchRow::from(div().into_any_element())])
                .row(SearchRow::new(div(), ["Ram disk"]));
            assert_eq!(other.found, 1);
            // nothing typed: everything, and still counted
            let all = SettingsQuery::default();
            assert!(all.card(rows()).is_some());
            assert_eq!(all.found(), 2);
            assert!(all.status().is_none());
            assert!(query.status().is_some());
        }
    }
}
