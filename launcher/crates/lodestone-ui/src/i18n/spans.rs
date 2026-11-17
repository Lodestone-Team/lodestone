//! Typed sentences: a translated message whose placeholders are filled with values
//! that keep their role, so the value is styled wherever the translator put it.
//!
//! ```ignore
//! tr_spans!(ShellMsg::ChangedSettingsOf, name = Obj(&instance.name))
//! trn_spans!(ContentPlural::FilesAddedTo, count, name = Obj(pack))
//! ```
//!
//! [`tr_spans!`](crate::tr_spans) and [`trn_spans!`](crate::trn_spans) sit next to
//! `tr!` and `trn!`. Each argument is wrapped in its role: [`Obj`] (a name, medium
//! weight in the main ink), [`Num`] (tabular figures), [`Ver`] and [`Mono`]
//! (JetBrains Mono, for versions and exact values), [`Time`] (muted), or a
//! [`Measure`](format::Measure) (a number and its lighter unit). Plain text stays
//! plain. The result, [`Spans`], is an element: a [`RichText`] drawn with one text
//! run per part, so it stays selectable. Runs are needed because a highlight can't
//! change the font family, and a mono value needs JetBrains Mono.
//!
//! Translators keep the whole sentence and its word order, in every language: "Copied
//! {count} values from {name}" and "从 {name} 复制了 {count} 个值" style the same
//! values. [`Spans::plain`] is the sentence as plain text, for accessible names,
//! tooltips and logs.
//!
//! Values are made safe first ([`sanitize`](crate::values::truncate::sanitize)): a
//! name from a server can't break the line or flip the text around it.

use std::{
    hash::{Hash as _, Hasher as _},
    ops::Range,
    panic::Location,
};

use gpui::{App, ElementId, IntoElement, RenderOnce, SharedString, StyledText, Window};

use super::{Locale, format, locale};
use crate::{
    components::RichText,
    values::{Role, role_run, truncate::sanitize},
};

/// The most graphemes a value in a sentence keeps.
const MAX_VALUE: usize = 512;

/// A placeholder's value and its role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Span {
    Plain(SharedString),
    Obj(SharedString),
    Num(SharedString),
    Ver(SharedString),
    Mono(SharedString),
    Time(SharedString),
    /// A number and its unit ("4.2" and " MB"): the unit one ink lighter.
    Measure(format::Measure),
}

/// The thing a sentence is about: an instance's, a server's or a world's name.
pub struct Obj<T>(pub T);
/// A mod's, a plugin's or a modpack's version.
pub struct Ver<T>(pub T);
/// An exact value: a file name, a port, an id, an environment variable.
pub struct Mono<T>(pub T);
/// A time or a duration, already formatted ("2 hours ago").
pub struct Time<T>(pub T);
/// Text with no role, as a plain `&str` is.
pub struct Plain<T>(pub T);
/// A count, grouped as the locale writes numbers.
pub struct Num(pub u64);
/// A number already formatted ("1.5", "1,2 k"), drawn as a number.
pub struct NumText<T>(pub T);

fn clean(text: impl Into<SharedString>) -> SharedString {
    let text = text.into();
    let cleaned = sanitize(&text, MAX_VALUE);
    if cleaned == text.as_ref() {
        text
    } else {
        cleaned.into()
    }
}

macro_rules! wrapper {
    ($($wrapper:ident => $variant:ident),*) => {$(
        impl<T: Into<SharedString>> From<$wrapper<T>> for Span {
            fn from(value: $wrapper<T>) -> Self {
                Span::$variant(clean(value.0))
            }
        }
    )*};
}

wrapper!(Obj => Obj, Ver => Ver, Mono => Mono, Time => Time, Plain => Plain, NumText => Num);

impl From<Num> for Span {
    fn from(value: Num) -> Self {
        Span::Num(format::number(value.0).into())
    }
}

impl From<format::Measure> for Span {
    fn from(measure: format::Measure) -> Self {
        Span::Measure(format::Measure {
            value: sanitize(&measure.value, MAX_VALUE),
            ..measure
        })
    }
}

impl From<&str> for Span {
    fn from(text: &str) -> Self {
        Span::Plain(clean(text.to_string()))
    }
}

impl From<String> for Span {
    fn from(text: String) -> Self {
        Span::Plain(clean(text))
    }
}

impl From<SharedString> for Span {
    fn from(text: SharedString) -> Self {
        Span::Plain(clean(text))
    }
}

impl Span {
    /// Appends the value to `out`, recording its runs.
    fn write(&self, out: &mut String, runs: &mut Vec<(Range<usize>, Role)>) {
        let mut push = |text: &str, role: Role| {
            let start = out.len();
            out.push_str(text);
            if role != Role::Plain && !text.is_empty() {
                runs.push((start..out.len(), role));
            }
        };
        match self {
            Span::Plain(text) => push(text, Role::Plain),
            Span::Obj(text) => push(text, Role::Object),
            Span::Num(text) => push(text, Role::Number),
            Span::Ver(text) => push(text, Role::Version),
            Span::Mono(text) => push(text, Role::Exact),
            Span::Time(text) => push(text, Role::Time),
            Span::Measure(measure) => {
                push(measure.before, Role::Unit);
                push(&measure.value, Role::Number);
                push(measure.after, Role::Unit);
            }
        }
    }
}

/// A sentence with its values' roles; an element that draws it. See the module docs.
#[derive(Clone, Debug, IntoElement)]
pub struct Spans {
    id: ElementId,
    text: SharedString,
    /// The styled parts, in order and apart; the rest is the sentence's own style.
    runs: Vec<(Range<usize>, Role)>,
}

impl Spans {
    /// The sentence as plain text: its accessible name, and what logs show.
    pub fn plain(&self) -> &str {
        &self.text
    }

    /// The styled parts: byte ranges of [`plain`](Self::plain) and their roles.
    pub fn runs(&self) -> &[(Range<usize>, Role)] {
        &self.runs
    }

    /// An id of its own, for a sentence that changes while it may be selected.
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }
}

impl From<Spans> for SharedString {
    fn from(spans: Spans) -> Self {
        spans.text
    }
}

/// Fills `template`'s `{name}` placeholders from `args`, as `interpolate` does: in
/// one pass, so a value is put in as it is and never read for placeholders, and a
/// placeholder with no value stays as written. Use it through
/// [`tr_spans!`](crate::tr_spans).
#[track_caller]
pub fn fill(template: &str, args: &[(&str, Span)]) -> Spans {
    let caller = Location::caller();
    let mut text = String::with_capacity(template.len() + 32);
    let mut runs = Vec::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        text.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            text.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let name = &after[..end];
        match args.iter().find(|(key, _)| *key == name) {
            Some((_, value)) => value.write(&mut text, &mut runs),
            None => {
                text.push('{');
                text.push_str(name);
                text.push('}');
            }
        }
        rest = &after[end + 1..];
    }
    text.push_str(rest);
    // An id from where the sentence is made and what it says, as `RichText` makes
    // its own: two sentences at the same place differ in text.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    caller.hash(&mut hasher);
    text.hash(&mut hasher);
    Spans {
        id: ElementId::Integer(hasher.finish()),
        text: text.into(),
        runs,
    }
}

/// The form of `forms` (the singular and the plural) for `count` in the current
/// locale, with `{count}` a number and the other placeholders from `args`. Use it
/// through [`trn_spans!`](crate::trn_spans).
#[track_caller]
pub fn plural(forms: [&str; 2], count: u64, args: &[(&str, Span)]) -> Spans {
    plural_in(locale(), forms, count, args)
}

#[track_caller]
fn plural_in(locale: Locale, forms: [&str; 2], count: u64, args: &[(&str, Span)]) -> Spans {
    let [one, other] = forms;
    let template = if locale.is_one(count) { one } else { other };
    let mut all = Vec::with_capacity(args.len() + 1);
    all.push(("count", Span::from(Num(count))));
    all.extend(args.iter().cloned());
    fill(template, &all)
}

impl RenderOnce for Spans {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // The style the sentence is drawn in where it is placed; each value's run
        // changes only what its role changes.
        let base = window.text_style();
        let len = self.text.len();
        let mut runs = Vec::with_capacity(self.runs.len() * 2 + 1);
        let mut at = 0;
        for (range, role) in &self.runs {
            debug_assert!(range.start >= at && range.end <= len);
            debug_assert!(self.text.is_char_boundary(range.start));
            debug_assert!(self.text.is_char_boundary(range.end));
            if range.start > at {
                runs.push(base.to_run(range.start - at));
            }
            runs.push(role_run(&base, *role, range.len()));
            at = range.end;
        }
        if at < len {
            runs.push(base.to_run(len - at));
        }
        let styled = StyledText::new(self.text.clone()).with_runs(runs);
        RichText::styled(self.id, self.text, styled)
    }
}

/// A message with role-tagged values, as a [`Spans`] element:
/// `tr_spans!(ShellMsg::ChangedSettingsOf, name = Obj(&instance.name))`. See
/// [`spans`](crate::i18n::spans).
///
/// The message is looked up through the calling crate's own `crate::i18n::t`, so an
/// app's messages work as well as this crate's: each app's `i18n` module has `t`
/// and wraps this crate's messages in its own.
#[macro_export]
// `crate` is meant: the caller's `i18n`, not this crate's.
#[allow(clippy::crate_in_macro_def)]
macro_rules! tr_spans {
    ($message:expr $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::i18n::spans::fill(
            crate::i18n::t($message),
            &[$((stringify!($name), $crate::i18n::spans::Span::from($value))),*],
        )
    };
}

/// A plural message for a count with role-tagged values, as a [`Spans`] element:
/// `trn_spans!(ContentPlural::FilesAdded, n, name = Obj(pack))`. `{count}` is a
/// number. Like [`tr_spans!`](crate::tr_spans), it looks the message up through the
/// calling crate's `crate::i18n::Plural`.
#[macro_export]
// `crate` is meant: the caller's `i18n`, not this crate's.
#[allow(clippy::crate_in_macro_def)]
macro_rules! trn_spans {
    ($message:expr, $count:expr $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::i18n::spans::plural(
            crate::i18n::Plural::from($message).forms($crate::i18n::locale()),
            ($count) as u64,
            &[$((stringify!($name), $crate::i18n::spans::Span::from($value))),*],
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{Msg, ValueMsg, ValuePlural};

    /// Each run's text and role.
    fn parts(spans: &Spans) -> Vec<(&str, Role)> {
        spans
            .runs()
            .iter()
            .map(|(range, role)| (&spans.plain()[range.clone()], *role))
            .collect()
    }

    /// The runs are in order, apart, and on character boundaries, as `with_runs`
    /// needs.
    fn assert_well_formed(spans: &Spans) {
        let text = spans.plain();
        let mut at = 0;
        for (range, _) in spans.runs() {
            assert!(range.start >= at && range.start < range.end && range.end <= text.len());
            assert!(text.is_char_boundary(range.start) && text.is_char_boundary(range.end));
            at = range.end;
        }
    }

    #[test]
    fn values_keep_their_role_whatever_the_word_order() {
        let args = || {
            [
                ("count", Span::from(NumText("12"))),
                ("name", Span::from(Obj("Survival"))),
            ]
        };
        for (template, plain) in [
            (
                "Copied {count} values from {name}",
                "Copied 12 values from Survival",
            ),
            (
                "{count} valeurs copiées depuis {name}",
                "12 valeurs copiées depuis Survival",
            ),
            (
                "从 {name} 复制了 {count} 个值",
                "从 Survival 复制了 12 个值",
            ),
            (
                "從 {name} 複製了 {count} 個值",
                "從 Survival 複製了 12 個值",
            ),
        ] {
            let spans = fill(template, &args());
            assert_eq!(spans.plain(), plain);
            assert_well_formed(&spans);
            let mut found = parts(&spans);
            found.sort_by_key(|(_, role)| format!("{role:?}"));
            assert_eq!(
                found,
                [("12", Role::Number), ("Survival", Role::Object)],
                "{template}"
            );
        }
    }

    #[test]
    fn real_messages_in_every_locale() {
        for locale in Locale::ALL {
            let spans = fill(
                Msg::from(ValueMsg::FromTo).text(locale),
                &[("from", Ver("6.1.0").into()), ("to", Ver("6.2.0").into())],
            );
            assert_eq!(
                parts(&spans),
                [("6.1.0", Role::Version), ("6.2.0", Role::Version)],
                "{}",
                locale.code()
            );
            assert_well_formed(&spans);
        }
    }

    #[test]
    fn multi_byte_values_and_measures_are_covered_exactly() {
        let spans = fill(
            "{name}：{size}，{time}",
            &[
                ("name", Obj("世界 é").into()),
                (
                    "size",
                    format::Measure {
                        value: "1,2".into(),
                        before: "",
                        after: "\u{a0}Go",
                    }
                    .into(),
                ),
                ("time", Time("今天 14:30").into()),
            ],
        );
        assert_eq!(spans.plain(), "世界 é：1,2\u{a0}Go，今天 14:30");
        assert_well_formed(&spans);
        assert_eq!(
            parts(&spans),
            [
                ("世界 é", Role::Object),
                ("1,2", Role::Number),
                ("\u{a0}Go", Role::Unit),
                ("今天 14:30", Role::Time),
            ]
        );
    }

    #[test]
    fn values_are_never_read_for_placeholders_and_unknown_ones_stay() {
        let spans = fill(
            "{a} and {b} and {missing}",
            &[("a", Obj("{b}").into()), ("b", Mono("x").into())],
        );
        assert_eq!(spans.plain(), "{b} and x and {missing}");
        assert_eq!(parts(&spans), [("{b}", Role::Object), ("x", Role::Exact)]);
        assert_eq!(fill("open { only", &[]).plain(), "open { only");
        // an empty value adds no run
        assert!(fill("{a}", &[("a", Obj("").into())]).runs().is_empty());
    }

    #[test]
    fn hostile_values_are_made_safe() {
        let spans = fill(
            "Removed {name}",
            &[("name", Mono("evil\u{202E}gpj\nx.jar").into())],
        );
        assert_eq!(spans.plain(), "Removed evilgpj x.jar");
        assert_well_formed(&spans);
        let long = "y".repeat(5000);
        let spans = fill("{name}", &[("name", Obj(long).into())]);
        assert_eq!(spans.plain().chars().count(), MAX_VALUE);
    }

    #[test]
    fn plurals_pick_the_locale_form_and_count_is_a_number() {
        let forms = ["{count} file in {name}", "{count} files in {name}"];
        let args = [("name", Span::from(Obj("Pack")))];
        assert_eq!(
            plural_in(Locale::En, forms, 1, &args).plain(),
            "1 file in Pack"
        );
        assert_eq!(
            plural_in(Locale::En, forms, 0, &args).plain(),
            "0 files in Pack"
        );
        let fr = ["{count} fichier", "{count} fichiers"];
        assert_eq!(plural_in(Locale::Fr, fr, 0, &[]).plain(), "0 fichier");
        assert_eq!(plural_in(Locale::Fr, fr, 2, &[]).plain(), "2 fichiers");
        let spans = plural_in(Locale::En, forms, 3, &args);
        assert_eq!(parts(&spans), [("3", Role::Number), ("Pack", Role::Object)]);
    }

    #[test]
    fn the_macros_resolve_this_crates_messages() {
        let spans = crate::tr_spans!(ValueMsg::KeyValue, key = "Size", value = Num(4));
        assert_eq!(spans.plain(), "Size: 4");
        assert_eq!(parts(&spans), [("4", Role::Number)]);
        let spans = crate::trn_spans!(ValuePlural::ShowMore, 3);
        assert_eq!(spans.plain(), "Show 3 more");
        assert_eq!(parts(&spans), [("3", Role::Number)]);
        // the same call place and text give the same id, so a selection survives
        let make = || crate::tr_spans!(ValueMsg::Hidden);
        assert_eq!(make().id, make().id);
    }
}
