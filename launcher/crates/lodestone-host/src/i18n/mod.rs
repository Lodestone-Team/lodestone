//! Lodestone Host's translations, built like the launcher's (see its `i18n` module):
//! each area's messages are defined once in English in [`en`], and each locale has a
//! table per area, so a message without a translation doesn't compile.
//!
//! The locale, [`Text`], [`format`] and the words every app uses ([`CommonMsg`]) come
//! from `lodestone_ui::i18n`. There is one area per work package of the rewrite
//! (`launcher/docs/host-parity.md`), so the people building them each edit their own.
//!
//! - [`t`]: a message as it is, `t(CommonMsg::Cancel)`.
//! - [`tr!`]: with `{name}` placeholders, `tr!(ShellMsg::SignedInAs, name = user)`.
//! - [`trn!`]: with a count, picking the plural form the locale needs.

pub mod en;
mod fr;
mod zh_hans;
mod zh_hant;

#[cfg(test)]
mod tests;

use std::fmt::Display;

#[cfg(test)]
use lodestone_ui::i18n::placeholders;
pub use lodestone_ui::i18n::{
    CommonMsg, CommonPlural, FormatMsg, FormatPlural, Language, Locale, Message, Text, apply,
    cjk_fallbacks, force, format, interpolate, locale, locale_fonts, resolve,
};

pub use en::{
    console_files::{ConsoleFilesMsg, ConsoleFilesPlural},
    core::{CoreMsg, CorePlural},
    instance::{InstanceMsg, InstancePlural},
    instances::{InstancesMsg, InstancesPlural},
    shell::{ShellMsg, ShellPlural},
};

/// The text of a message in the current locale.
pub fn t(message: impl Into<Msg>) -> &'static str {
    message.into().text(locale())
}

/// The text of a plural message for `count`, in the current locale, with `{count}`
/// and the other placeholders filled in. Use it through [`trn!`].
pub fn plural(message: impl Into<Plural>, count: u64, args: &[(&str, &dyn Display)]) -> String {
    lodestone_ui::i18n::plural_of(message.into().forms(locale()), count, args)
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
#[allow(unused_macros)]
macro_rules! trn {
    ($message:expr, $count:expr $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::i18n::plural(
            $message,
            ($count) as u64,
            &[$((stringify!($name), &$value as &dyn ::std::fmt::Display)),*],
        )
    };
}

#[allow(unused_imports)]
pub(crate) use {tr, trn};

pub(crate) use lodestone_ui::messages;

/// Joins the areas into [`Msg`] and [`Plural`], and looks each up in its locale's
/// table.
macro_rules! areas {
    ($( $area:ident => $msg:ident, $plural:ident; )*) => {
        /// Any message of any area, the shared ones (`lodestone_ui`'s) included.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Msg {
            Ui(lodestone_ui::i18n::Msg),
            $( $msg(en::$area::$msg), )*
        }

        /// Any plural message of any area.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Plural {
            Ui(lodestone_ui::i18n::Plural),
            $( $plural(en::$area::$plural), )*
        }

        impl Message for Msg {
            fn current(self) -> &'static str {
                self.text(locale())
            }
        }

        impl From<Msg> for Text {
            fn from(message: Msg) -> Self {
                Text::msg(message)
            }
        }

        impl From<CommonMsg> for Msg {
            fn from(message: CommonMsg) -> Self {
                Msg::Ui(message.into())
            }
        }

        impl From<FormatMsg> for Msg {
            fn from(message: FormatMsg) -> Self {
                Msg::Ui(message.into())
            }
        }

        impl From<CommonPlural> for Plural {
            fn from(message: CommonPlural) -> Self {
                Plural::Ui(message.into())
            }
        }

        impl From<FormatPlural> for Plural {
            fn from(message: FormatPlural) -> Self {
                Plural::Ui(message.into())
            }
        }

        $(
            impl From<en::$area::$msg> for Msg {
                fn from(message: en::$area::$msg) -> Self {
                    Msg::$msg(message)
                }
            }

            impl From<en::$area::$msg> for Text {
                fn from(message: en::$area::$msg) -> Self {
                    Text::msg(Msg::$msg(message))
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
                    Msg::Ui(message) => message.text(locale),
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
                    Msg::Ui(message) => message.name(),
                    $( Msg::$msg(message) => format!("{}::{}", stringify!($msg), message.key()), )*
                }
            }

            /// Every message, for the tests.
            #[cfg(test)]
            fn all() -> Vec<Msg> {
                let mut all: Vec<Msg> =
                    lodestone_ui::i18n::Msg::all().into_iter().map(Msg::Ui).collect();
                $( all.extend(en::$area::$msg::ALL.iter().map(|m| Msg::$msg(*m))); )*
                all
            }
        }

        impl Plural {
            /// The singular and plural in `locale`.
            pub fn forms(self, locale: Locale) -> [&'static str; 2] {
                match self {
                    Plural::Ui(message) => message.forms(locale),
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
                    Plural::Ui(message) => message.name(),
                    $( Plural::$plural(message) => format!("{}::{}", stringify!($plural), message.key()), )*
                }
            }

            #[cfg(test)]
            fn all() -> Vec<Plural> {
                let mut all: Vec<Plural> =
                    lodestone_ui::i18n::Plural::all().into_iter().map(Plural::Ui).collect();
                $( all.extend(en::$area::$plural::ALL.iter().map(|m| Plural::$plural(*m))); )*
                all
            }
        }
    };
}

areas! {
    shell => ShellMsg, ShellPlural;
    instances => InstancesMsg, InstancesPlural;
    console_files => ConsoleFilesMsg, ConsoleFilesPlural;
    instance => InstanceMsg, InstancePlural;
    core => CoreMsg, CorePlural;
}
