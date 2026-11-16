//! The readable components: one small vocabulary, shared by Lodestone Launcher and
//! Lodestone Host, for showing names and values so each value's role reads the same
//! everywhere, in both themes.
//!
//! - Text roles: [`Num`] (tabular figures, a lighter unit), [`PathText`] (an exact
//!   value cut in the middle), [`MonoValue`], [`ProductPhrase`] ("Minecraft 26.3"),
//!   [`CaptionLine`], and [`object_name`], [`time_text`], [`key_text`].
//! - Badges: [`VersionTag`] (a mod's version), [`StatusBadge`], [`CountBadge`],
//!   [`ReasonTag`], [`UntitledName`], [`KindIcon`].
//! - Objects: [`ObjectChip`] inline, [`ObjectCard`] under a dialog's title.
//! - Facts: [`Fact`] (a key and a value), [`FactLine`] (a row's line or a header's
//!   strip), [`FactList`] (a table), [`FactTooltip`].
//! - Values: [`ValueField`] (a path, URL, code or secret with Copy and Reveal),
//!   [`StatTile`] and [`StatRow`], [`ProgressBar`], [`MeterBar`], [`ProgressBlock`],
//!   [`DiffValue`], [`CompareTable`].
//! - Lists: [`NameList`] (names grouped by outcome), [`ListColumns`] and
//!   [`ListHeader`] (right-aligned columns that fold on narrow windows).
//!
//! Sentences with values use [`tr_spans!`](crate::tr_spans). The colours and fonts
//! come from `theme::ink`, `theme::fonts` and `theme::pill`, so launcher skins and
//! the Host's light theme restyle them. `launcher/docs/readable-values.md` says
//! which to use when; the apps' `components` gallery scenes show every one.

mod badges;
mod compare;
mod fact;
mod fact_line;
mod fact_list;
#[cfg(feature = "screenshot")]
mod gallery;
mod kind;
mod list_columns;
mod name_list;
mod object;
mod stat;
mod text;
mod value_field;

use std::{
    hash::{Hash as _, Hasher as _},
    panic::Location,
};

use gpui::{ElementId, SharedString};

pub use badges::{
    CountBadge, ReasonTag, StatusBadge, UntitledName, VERSION_TOOLTIP_OVER, VersionTag, VersionTone,
};
pub use compare::{CompareTable, DiffValue};
use fact::Place;
pub use fact::{Fact, FactKey, FactValue};
pub use fact_line::FactLine;
pub use fact_list::{FactList, FactTooltip};
#[cfg(feature = "screenshot")]
pub use gallery::ComponentGallery;
pub use kind::{Kind, KindIcon};
pub use list_columns::{Column, ListColumns, ListHeader};
pub use name_list::{ItemIcon, NameGroup, NameItem, NameList, NameStyle, Reason};
pub use object::{ObjectCard, ObjectChip};
pub use stat::{MeterBar, ProgressBar, ProgressBlock, StatRow, StatTile};
pub use text::{
    CaptionLine, MonoValue, Num, PathText, ProductPhrase, key_text, object_name, time_text,
};
pub use value_field::{ValueField, ValueKind};

/// An element id from where a component is made and what it shows, as
/// [`RichText`](crate::components::RichText) makes its own, so callers needn't give
/// one: two components at the same place under the same parent differ in what they
/// show.
fn auto_id(caller: &'static Location<'static>, salt: &str) -> ElementId {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    caller.hash(&mut hasher);
    salt.hash(&mut hasher);
    ElementId::Integer(hasher.finish())
}

/// "key: value" in the current locale ("key : value" in French), for accessible
/// names and tooltips.
pub fn key_value(key: &str, value: &str) -> SharedString {
    crate::i18n::interpolate(
        crate::i18n::t(crate::i18n::ValueMsg::KeyValue),
        &[
            ("key", &key as &dyn std::fmt::Display),
            ("value", &value as &dyn std::fmt::Display),
        ],
    )
    .into()
}
