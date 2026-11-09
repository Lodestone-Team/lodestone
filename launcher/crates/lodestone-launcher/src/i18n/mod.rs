//! Translations of the launcher's interface.
//!
//! Every message is defined once, in English, as a variant of an area's enum in
//! [`en`]: [`ShellMsg::SettingsTitle`] and the like. Each locale (`fr`, `zh_hans`,
//! `zh_hant`) has a table per area: a `match` over that enum, so a message without a
//! translation does not compile. A unit test checks that every translation has the
//! same `{placeholders}` as the English.
//!
//! Lookups are a `match` on a `Copy` enum and return `&'static str`; nothing is
//! parsed or allocated, except by [`tr!`] and [`trn!`], which fill in placeholders.
//!
//! - [`t`]: a message as it is, `t(CommonMsg::Cancel)`.
//! - [`tr!`]: a message with `{name}` placeholders, `tr!(ShellMsg::Hello, name = user)`.
//! - [`trn!`]: a message with a count, picking the plural form the locale needs:
//!   `trn!(ShellPlural::Instances, count)`. `{count}` is filled in with the count,
//!   grouped as the locale writes numbers.
//! - [`Text`]: a label kept in a component (a text field's placeholder) that follows
//!   the language when it changes.
//! - [`format`]: numbers, sizes, dates and how long ago something was.
//!
//! The current locale is a global, so code on any thread formats in it. Changing
//! the language ([`apply`]) and redrawing the windows updates every view at once.

pub mod format;
mod system;

pub mod en;
mod fr;
mod zh_hans;
mod zh_hant;

#[cfg(test)]
mod tests;

use std::{
    fmt::{Display, Write as _},
    sync::atomic::{AtomicU8, Ordering},
};

use gpui::SharedString;
pub use launcher_core::settings::Language;

pub use en::{
    common::{CommonMsg, CommonPlural},
    content::{ContentMsg, ContentPlural},
    format::{FormatMsg, FormatPlural},
    instance_settings::{InstanceSettingsMsg, InstanceSettingsPlural},
    instances::{InstancesMsg, InstancesPlural},
    servers::{ServersMsg, ServersPlural},
    shared_options::{SharedOptionsMsg, SharedOptionsPlural},
    shell::{ShellMsg, ShellPlural},
    worlds::{WorldsMsg, WorldsPlural},
};

/// A language the launcher is translated into.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Locale {
    #[default]
    En = 0,
    Fr = 1,
    ZhHans = 2,
    ZhHant = 3,
}

impl Locale {
    pub const ALL: [Locale; 4] = [Locale::En, Locale::Fr, Locale::ZhHans, Locale::ZhHant];

    /// The BCP 47 tag: `en`, `fr`, `zh-Hans`, `zh-Hant`.
    pub fn code(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Fr => "fr",
            Locale::ZhHans => "zh-Hans",
            Locale::ZhHant => "zh-Hant",
        }
    }

    /// The language's name in itself, as the Language setting lists it.
    pub fn native_name(self) -> &'static str {
        match self {
            Locale::En => "English",
            Locale::Fr => "Français",
            Locale::ZhHans => "简体中文",
            Locale::ZhHant => "繁體中文",
        }
    }

    /// The locale for a language tag such as `fr-CA`, `zh-Hans-CN`, `zh_TW.UTF-8` or
    /// `en`, if the launcher has it. Chinese without a script is Simplified, except
    /// for Taiwan, Hong Kong and Macao.
    pub fn from_tag(tag: &str) -> Option<Locale> {
        let tag = tag.split(['.', '@']).next().unwrap_or_default();
        let mut parts = tag.split(['-', '_']).map(|part| part.to_ascii_lowercase());
        let language = parts.next()?;
        match language.as_str() {
            "en" => Some(Locale::En),
            "fr" => Some(Locale::Fr),
            "zh" => {
                for part in parts {
                    match part.as_str() {
                        "hans" | "cn" | "sg" | "my" => return Some(Locale::ZhHans),
                        "hant" | "tw" | "hk" | "mo" => return Some(Locale::ZhHant),
                        _ => {}
                    }
                }
                Some(Locale::ZhHans)
            }
            _ => None,
        }
    }

    /// The setting that picks this locale.
    pub fn language(self) -> Language {
        match self {
            Locale::En => Language::English,
            Locale::Fr => Language::French,
            Locale::ZhHans => Language::SimplifiedChinese,
            Locale::ZhHant => Language::TraditionalChinese,
        }
    }

    /// Whether `n` takes the singular ("one") form: 1 in English, 0 and 1 in French.
    /// Chinese has one form, and its tables give the same text for both.
    fn is_one(self, n: u64) -> bool {
        match self {
            Locale::En => n == 1,
            Locale::Fr => n < 2,
            Locale::ZhHans | Locale::ZhHant => false,
        }
    }

    fn from_u8(value: u8) -> Locale {
        match value {
            1 => Locale::Fr,
            2 => Locale::ZhHans,
            3 => Locale::ZhHant,
            _ => Locale::En,
        }
    }
}

static LOCALE: AtomicU8 = AtomicU8::new(Locale::En as u8);
/// A locale set by `--lang` for the offscreen screenshots, which the setting doesn't
/// change; `u8::MAX` while there is none.
static FORCED: AtomicU8 = AtomicU8::new(u8::MAX);

/// The locale the interface is shown in.
pub fn locale() -> Locale {
    Locale::from_u8(LOCALE.load(Ordering::Relaxed))
}

/// The locale for a Language setting: the language itself, or for
/// [`Language::System`] the first of the system's preferred languages the launcher
/// has, else English.
pub fn resolve(language: Language) -> Locale {
    match language {
        Language::English => Locale::En,
        Language::French => Locale::Fr,
        Language::SimplifiedChinese => Locale::ZhHans,
        Language::TraditionalChinese => Locale::ZhHant,
        Language::System => system::preferred_languages()
            .iter()
            .find_map(|tag| Locale::from_tag(tag))
            .unwrap_or_default(),
    }
}

/// Shows the interface in `language` from now on, and returns the locale it resolved
/// to. Redraw the windows afterwards (`cx.refresh_windows()`) to update what is shown.
pub fn apply(language: Language) -> Locale {
    let locale = match FORCED.load(Ordering::Relaxed) {
        u8::MAX => resolve(language),
        forced => Locale::from_u8(forced),
    };
    LOCALE.store(locale as u8, Ordering::Relaxed);
    locale
}

/// Shows the interface in `locale` whatever the setting says, for the offscreen
/// screenshots' `--lang`.
pub fn force(locale: Locale) {
    FORCED.store(locale as u8, Ordering::Relaxed);
    LOCALE.store(locale as u8, Ordering::Relaxed);
}

/// The text of a message in the current locale.
pub fn t(message: impl Into<Msg>) -> &'static str {
    message.into().text(locale())
}

/// The text of a plural message for `count`, in the current locale, with `{count}`
/// and the other placeholders filled in. Use it through [`trn!`].
pub fn plural(message: impl Into<Plural>, count: u64, args: &[(&str, &dyn Display)]) -> String {
    let locale = locale();
    let [one, other] = message.into().forms(locale);
    let template = if locale.is_one(count) { one } else { other };
    let count_text = format::number(count);
    let mut all: Vec<(&str, &dyn Display)> = Vec::with_capacity(args.len() + 1);
    all.push(("count", &count_text));
    all.extend_from_slice(args);
    interpolate(template, &all)
}

/// Fills in a template's `{name}` placeholders from `args`. A placeholder without a
/// value is left as it is.
pub fn interpolate(template: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let name = &after[..end];
        match args.iter().find(|(key, _)| *key == name) {
            Some((_, value)) => {
                let _ = write!(out, "{value}");
            }
            None => {
                out.push('{');
                out.push_str(name);
                out.push('}');
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// The names of a template's `{placeholders}`, for the tests.
#[cfg(test)]
fn placeholders(template: &str) -> std::collections::BTreeSet<&str> {
    let mut names = std::collections::BTreeSet::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else { break };
        names.insert(&after[..end]);
        rest = &after[end + 1..];
    }
    names
}

/// A message with `{name}` placeholders filled in, in the current locale:
/// `tr!(ShellMsg::SignedInAs, name = account.username)`. Values are anything that
/// implements `Display`.
macro_rules! tr {
    ($message:expr $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::i18n::interpolate(
            $crate::i18n::t($message),
            &[$((stringify!($name), &$value as &dyn ::std::fmt::Display)),*],
        )
    };
}

/// A plural message for a count, with `{count}` and other placeholders filled in:
/// `trn!(ContentPlural::Mods, mods.len())`, `trn!(ContentPlural::ModsIn, n, name = x)`.
macro_rules! trn {
    ($message:expr, $count:expr $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::i18n::plural(
            $message,
            ($count) as u64,
            &[$((stringify!($name), &$value as &dyn ::std::fmt::Display)),*],
        )
    };
}

pub(crate) use {tr, trn};

/// Text a component keeps and shows later, such as a text field's placeholder: a
/// message, looked up each time it is shown so it follows the language, or text as
/// it is (a name, or text from a server).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Text {
    Msg(Msg),
    Plain(SharedString),
}

impl Text {
    /// The text, in the current locale.
    pub fn get(&self) -> SharedString {
        match self {
            Text::Msg(message) => SharedString::new_static(t(*message)),
            Text::Plain(text) => text.clone(),
        }
    }
}

impl Default for Text {
    fn default() -> Self {
        Text::Plain(SharedString::default())
    }
}

impl From<Msg> for Text {
    fn from(message: Msg) -> Self {
        Text::Msg(message)
    }
}

impl From<&'static str> for Text {
    fn from(text: &'static str) -> Self {
        Text::Plain(text.into())
    }
}

impl From<String> for Text {
    fn from(text: String) -> Self {
        Text::Plain(text.into())
    }
}

impl From<SharedString> for Text {
    fn from(text: SharedString) -> Self {
        Text::Plain(text)
    }
}

/// Defines an area's messages, in English: an enum of plain messages and an enum of
/// plural ones (English and French have a singular and a plural form). Used by the
/// files in [`en`].
macro_rules! messages {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident = $text:expr, )*
        }
        $(#[$pmeta:meta])*
        pub enum $plural:ident {
            $( $(#[$pvmeta:meta])* $pvariant:ident = ($one:expr, $other:expr), )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )*
        }

        impl $name {
            /// Every message of the area, for the tests.
            #[allow(dead_code)]
            pub const ALL: &'static [$name] = &[$($name::$variant,)*];

            /// The English text.
            pub const fn english(self) -> &'static str {
                match self {
                    $( $name::$variant => $text, )*
                }
            }

            /// The variant's name, for the tests' messages.
            #[allow(dead_code)]
            pub const fn key(self) -> &'static str {
                match self {
                    $( $name::$variant => stringify!($variant), )*
                }
            }
        }

        $(#[$pmeta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $plural {
            $( $(#[$pvmeta])* $pvariant, )*
        }

        impl $plural {
            /// Every plural message of the area, for the tests.
            #[allow(dead_code)]
            pub const ALL: &'static [$plural] = &[$($plural::$pvariant,)*];

            /// The English singular and plural.
            pub const fn english(self) -> [&'static str; 2] {
                match self {
                    $( $plural::$pvariant => [$one, $other], )*
                }
            }

            /// The variant's name, for the tests' messages.
            #[allow(dead_code)]
            pub const fn key(self) -> &'static str {
                match self {
                    $( $plural::$pvariant => stringify!($pvariant), )*
                }
            }
        }
    };
}

pub(crate) use messages;

/// Joins the areas into [`Msg`] and [`Plural`], and looks each up in its locale's
/// table.
macro_rules! areas {
    ($( $area:ident => $msg:ident, $plural:ident; )*) => {
        /// Any message of any area.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Msg {
            $( $msg(en::$area::$msg), )*
        }

        /// Any plural message of any area.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Plural {
            $( $plural(en::$area::$plural), )*
        }

        $(
            impl From<en::$area::$msg> for Msg {
                fn from(message: en::$area::$msg) -> Self {
                    Msg::$msg(message)
                }
            }

            impl From<en::$area::$msg> for Text {
                fn from(message: en::$area::$msg) -> Self {
                    Text::Msg(Msg::$msg(message))
                }
            }

            impl From<en::$area::$plural> for Plural {
                fn from(message: en::$area::$plural) -> Self {
                    Plural::$plural(message)
                }
            }
        )*

        impl Msg {
            /// The text in `locale`.
            pub fn text(self, locale: Locale) -> &'static str {
                match self {
                    $( Msg::$msg(message) => match locale {
                        Locale::En => message.english(),
                        Locale::Fr => fr::$area::text(message),
                        Locale::ZhHans => zh_hans::$area::text(message),
                        Locale::ZhHant => zh_hant::$area::text(message),
                    }, )*
                }
            }

            /// "area::Variant", for the tests' messages.
            #[cfg(test)]
            fn name(self) -> String {
                match self {
                    $( Msg::$msg(message) => format!("{}::{}", stringify!($msg), message.key()), )*
                }
            }

            /// Every message, for the tests.
            #[cfg(test)]
            fn all() -> Vec<Msg> {
                let mut all = Vec::new();
                $( all.extend(en::$area::$msg::ALL.iter().map(|m| Msg::$msg(*m))); )*
                all
            }
        }

        impl Plural {
            /// The singular and plural in `locale`.
            pub fn forms(self, locale: Locale) -> [&'static str; 2] {
                match self {
                    $( Plural::$plural(message) => match locale {
                        Locale::En => message.english(),
                        Locale::Fr => fr::$area::plural(message),
                        Locale::ZhHans => zh_hans::$area::plural(message),
                        Locale::ZhHant => zh_hant::$area::plural(message),
                    }, )*
                }
            }

            #[cfg(test)]
            fn name(self) -> String {
                match self {
                    $( Plural::$plural(message) => format!("{}::{}", stringify!($plural), message.key()), )*
                }
            }

            #[cfg(test)]
            fn all() -> Vec<Plural> {
                let mut all = Vec::new();
                $( all.extend(en::$area::$plural::ALL.iter().map(|m| Plural::$plural(*m))); )*
                all
            }
        }
    };
}

areas! {
    common => CommonMsg, CommonPlural;
    format => FormatMsg, FormatPlural;
    shell => ShellMsg, ShellPlural;
    instances => InstancesMsg, InstancesPlural;
    instance_settings => InstanceSettingsMsg, InstanceSettingsPlural;
    content => ContentMsg, ContentPlural;
    worlds => WorldsMsg, WorldsPlural;
    servers => ServersMsg, ServersPlural;
    shared_options => SharedOptionsMsg, SharedOptionsPlural;
}

/// Gives `element` and its children the current locale's Chinese fonts, tried for
/// characters the bundled fonts lack. Set on the window's root.
pub fn locale_fonts<E: gpui::Styled>(mut element: E) -> E {
    use std::sync::OnceLock;
    static LISTS: [OnceLock<gpui::FontFallbacks>; 4] = [
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
    ];
    let locale = locale();
    if let Some(fonts) = cjk_fallbacks(locale) {
        let list = LISTS[locale as usize].get_or_init(|| {
            gpui::FontFallbacks::from_fonts(fonts.iter().map(|font| font.to_string()).collect())
        });
        element.text_style().font_fallbacks = Some(list.clone());
    }
    element
}

/// The fonts that draw Chinese text in `locale`, tried before the system's own
/// fallback. The bundled fonts have no Chinese glyphs; without a list, macOS picks a
/// Chinese font by the system's languages, which may draw Simplified glyph shapes for
/// Traditional text. `None` for English and French.
pub fn cjk_fallbacks(locale: Locale) -> Option<&'static [&'static str]> {
    match locale {
        Locale::En | Locale::Fr => None,
        Locale::ZhHans => Some(if cfg!(target_os = "macos") {
            &["PingFang SC", "Hiragino Sans GB"]
        } else if cfg!(windows) {
            &["Microsoft YaHei UI", "Microsoft YaHei"]
        } else {
            &[
                "Noto Sans CJK SC",
                "Source Han Sans SC",
                "WenQuanYi Micro Hei",
            ]
        }),
        Locale::ZhHant => Some(if cfg!(target_os = "macos") {
            &["PingFang TC", "PingFang HK"]
        } else if cfg!(windows) {
            &["Microsoft JhengHei UI", "Microsoft JhengHei"]
        } else {
            &["Noto Sans CJK TC", "Source Han Sans TC"]
        }),
    }
}
