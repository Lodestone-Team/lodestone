//! An instance's facts as the readable components show them: the game and its
//! loader as plain phrases ("Minecraft 26.3", "Fabric 0.20.1"), keyed Game and
//! Loader in strips and fact tables, and the card that names an instance under a
//! dialog's title.

use gpui::prelude::*;
use launcher_core::instances::Instance;
use lodestone_ui::values::{Product, truncate::sanitize};

use crate::{
    components::{CountBadge, Fact, FactValue, Kind, Num, ObjectCard, ProductPhrase},
    i18n::{FactMsg, t},
    instances::{Ago, play_time},
};

/// "Minecraft 26.3".
pub fn game(version: &str) -> ProductPhrase {
    ProductPhrase::new(Product::Minecraft, version.to_string())
}

/// The instance's loader at its version, "Fabric 0.20.1" (Forge's version as Forge
/// writes it); `None` for Vanilla, whose Loader fact is left out.
pub fn loader(instance: &Instance) -> Option<ProductPhrase> {
    let kind = instance.loader.kind()?;
    let label = instance.loader_label();
    let version = label
        .strip_prefix(kind.label())
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    Some(ProductPhrase::new(Product::from(kind), version))
}

/// The Game fact.
pub fn game_fact(version: &str) -> Fact {
    Fact::new(t(FactMsg::Game), game(version))
}

/// The Loader fact, when the instance has a loader.
pub fn loader_fact(instance: &Instance) -> Option<Fact> {
    loader(instance).map(|phrase| Fact::new(t(FactMsg::Loader), phrase))
}

/// The Game and Loader facts.
pub fn version_facts(instance: &Instance) -> Vec<Fact> {
    std::iter::once(game_fact(&instance.game_version))
        .chain(loader_fact(instance))
        .collect()
}

/// Play time and Last played, for a strip: "Not played yet" waits in the Play time
/// cell (in muted ink, as a time) until the first game.
pub fn played_facts(instance: &Instance) -> Vec<Fact> {
    let mut facts = Vec::new();
    // A game running for the first time has been played, but has no play time yet.
    if instance.play_time > 0 || instance.last_played.is_none() {
        facts.push(Fact::new(
            t(FactMsg::PlayTime),
            if instance.play_time > 0 {
                FactValue::text(play_time(instance.play_time))
            } else {
                FactValue::time(play_time(0))
            },
        ));
    }
    if let Some(when) = instance.last_played {
        facts.push(Fact::new(
            t(FactMsg::LastPlayed),
            FactValue::time(Ago::since(when).label()),
        ));
    }
    facts
}

/// The card naming an instance under a dialog's title, with its game and loader.
pub fn card(instance: &Instance) -> ObjectCard {
    version_facts(instance).into_iter().fold(
        ObjectCard::new(instance.name.clone()).kind(Kind::Instance),
        ObjectCard::fact,
    )
}

/// The card for an instance known only by name (it may be gone).
pub fn named_card(name: &str) -> ObjectCard {
    ObjectCard::new(name.to_string())
        .kind(Kind::Instance)
        .compact(true)
}

/// A loader as a project lists it ("fabric", "neoforge"), as its name is written:
/// "Fabric", "NeoForge"; other ids get a capital letter.
pub fn loader_name(id: &str) -> String {
    match Product::from_name(id) {
        Some(product) => product.name().to_string(),
        None => {
            let mut chars = id.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        }
    }
}

/// Up to `max` values side by side, 6px apart, then "+N" as a badge for the rest:
/// a list with no separators whose end can be seen.
fn short_values(values: &[String], max: usize) -> FactValue {
    let shown: Vec<String> = values.iter().take(max).cloned().collect();
    let more = values.len().saturating_sub(max);
    let plain = lodestone_ui::i18n::format::list(values);
    FactValue::element(
        gpui::div()
            .flex()
            .flex_row()
            .items_center()
            .gap(gpui::px(6.))
            .children(shown.into_iter().map(|value| {
                gpui::div()
                    .flex_none()
                    .whitespace_nowrap()
                    .child(sanitize(&value, 40))
            }))
            .when(more > 0, |this| this.child(CountBadge::new(more as u64))),
        plain,
    )
}

/// A version row's second line, the same in every list of versions (a project's,
/// CurseForge's files, a modpack's): the game versions it is for, its loaders, when
/// it was published and its size.
pub fn release_facts(
    game_versions: &[String],
    loaders: &[String],
    published: Option<String>,
    size: Option<u64>,
    max: usize,
) -> Vec<Fact> {
    let mut facts = Vec::new();
    if !game_versions.is_empty() {
        facts.push(Fact::new(t(FactMsg::Game), short_values(game_versions, max)).priority(0));
    }
    if !loaders.is_empty() {
        let names: Vec<String> = loaders.iter().map(|id| loader_name(id)).collect();
        facts.push(Fact::bare_named(t(FactMsg::Loader), short_values(&names, max)).priority(1));
    }
    if let Some(published) = published.filter(|date| !date.is_empty()) {
        facts.push(
            Fact::icon(
                crate::components::Icon::Clock,
                t(FactMsg::Published),
                FactValue::time(published),
            )
            .priority(2),
        );
    }
    if let Some(size) = size {
        facts.push(Fact::bare_named(t(FactMsg::Size), Num::bytes(size)).priority(3));
    }
    facts
}

/// The author and downloads of a project, as icon facts for a row's second line or
/// a project page: the download count compacted ("45.3M"), exact in the tooltip of
/// the line.
pub fn project_facts(
    author: Option<&str>,
    downloads: Option<u64>,
    followers: Option<u64>,
) -> Vec<Fact> {
    use crate::components::Icon;
    let mut facts = Vec::new();
    if let Some(author) = author.filter(|author| !author.trim().is_empty()) {
        facts.push(
            Fact::icon(
                Icon::User,
                t(FactMsg::Author),
                FactValue::text(sanitize(author, 64)),
            )
            .priority(1),
        );
    }
    let compact = compact_count;
    if let Some(downloads) = downloads {
        facts.push(
            Fact::icon(Icon::Download, t(FactMsg::Downloads), compact(downloads)).priority(0),
        );
    }
    if let Some(followers) = followers {
        facts.push(Fact::icon(Icon::Heart, t(FactMsg::Followers), compact(followers)).priority(2));
    }
    facts
}

/// A count shortened for a glance ("45.3M"), in tabular figures; the exact count is
/// its plain text, which tooltips and assistive technology give.
pub fn compact_count(n: u64) -> FactValue {
    FactValue::element(
        gpui::div()
            .flex_none()
            .whitespace_nowrap()
            .font_features(lodestone_ui::theme::fonts::tabular())
            .child(lodestone_ui::i18n::format::compact(n)),
        lodestone_ui::i18n::format::number(n),
    )
}
