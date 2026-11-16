//! An instance's facts as the readable components show them: the game and its
//! loader as plain phrases ("Minecraft 26.3", "Fabric 0.20.1"), keyed Game and
//! Loader in strips and fact tables, and the card that names an instance under a
//! dialog's title.

use launcher_core::instances::Instance;
use lodestone_ui::values::Product;

use crate::{
    components::{Fact, FactValue, Kind, ObjectCard, ProductPhrase},
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
