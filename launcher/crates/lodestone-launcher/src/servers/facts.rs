//! A server's facts as the readable components show them: the game and its loader as
//! plain phrases ("Minecraft 1.21.1", "Fabric 0.16.14"), its state as a badge, and
//! its players as a count with a key.

use launcher_core::cores::{ServerInstance, ServerLoader, ServerState};
use lodestone_ui::{i18n::format::Measure, values::Product};

use super::{loader_label, state_label};
use crate::{
    components::{Fact, FactValue, Icon, Num, ProductPhrase, StatusBadge},
    i18n::{FactMsg, ServersMsg as M, format, t},
    theme::Tone,
};

/// "Minecraft 1.21.1".
pub fn game(version: &str) -> ProductPhrase {
    ProductPhrase::new(Product::Minecraft, version.to_string())
}

/// A server's loader at its version, as one phrase: "Fabric 0.16.14", or "Spigot"
/// for one that is no product of its own. `None` for Vanilla, whose Loader fact is
/// left out.
pub fn loader(loader: ServerLoader, version: Option<&str>) -> Option<FactValue> {
    if loader == ServerLoader::Vanilla {
        return None;
    }
    let version = version.unwrap_or_default().trim();
    Some(match Product::from_name(loader.label()) {
        Some(product) if loader != ServerLoader::Other => {
            ProductPhrase::new(product, version.to_string()).into()
        }
        _ => {
            let label = loader_label(loader);
            if version.is_empty() {
                FactValue::text(label)
            } else {
                FactValue::text(format!("{label}\u{a0}{version}"))
            }
        }
    })
}

/// The game and the loader of a server, for a row's second line: phrases that need no
/// key on screen (it is still spoken).
pub fn versions(server: &ServerInstance) -> Vec<Fact> {
    let mut facts =
        vec![Fact::bare_named(t(FactMsg::Game), game(&server.game_version)).priority(1)];
    if let Some(value) = loader(server.loader, server.loader_version.as_deref()) {
        facts.push(Fact::bare_named(t(FactMsg::Loader), value).priority(2));
    }
    facts
}

/// The same, keyed Game and Loader, for a header's strip or a table.
pub fn keyed_versions(server: &ServerInstance) -> Vec<Fact> {
    let mut facts = vec![Fact::new(t(FactMsg::Game), game(&server.game_version)).priority(1)];
    if let Some(value) = loader(server.loader, server.loader_version.as_deref()) {
        facts.push(Fact::new(t(FactMsg::Loader), value).priority(2));
    }
    facts
}

/// The tone of a server's state: green while it runs, yellow while it changes, red
/// when it crashed.
pub fn state_tone(state: ServerState) -> Tone {
    match state {
        ServerState::Running => Tone::Success,
        ServerState::Starting | ServerState::Stopping => Tone::Warning,
        ServerState::Error => Tone::Danger,
        ServerState::Stopped | ServerState::Unknown | ServerState::Hidden => Tone::Neutral,
    }
}

/// The server's state as a badge.
pub fn state_badge(state: ServerState) -> StatusBadge {
    StatusBadge::new(state_label(state), state_tone(state))
}

/// The players online while the server runs, "3 / 20", when the core says.
pub fn players_value(state: ServerState, count: Option<u32>, max: Option<u32>) -> Option<Num> {
    if state != ServerState::Running {
        return None;
    }
    let count = u64::from(count?);
    Some(match max {
        Some(max) => Num::fraction(count, u64::from(max)),
        None => Num::count(count),
    })
}

/// [`players_value`] behind a person icon, keyed Players.
pub fn players(state: ServerState, count: Option<u32>, max: Option<u32>) -> Option<Fact> {
    players_value(state, count, max)
        .map(|value| Fact::icon(Icon::User, t(FactMsg::Players), value).priority(0))
}

/// How this computer reaches a server ("Local network"), keyed Connection.
pub fn connection(label: &'static str) -> Fact {
    Fact::new(t(FactMsg::Connection), label).priority(4)
}

/// A round trip in milliseconds, its unit one ink lighter: "12 ms".
pub fn ping_value(ms: u64) -> Num {
    let template = t(M::PingMs);
    let (before, after) = template.split_once("{ms}").unwrap_or(("", template));
    Num::measure(Measure {
        value: format::number(ms),
        before,
        after,
    })
}

/// [`ping_value`], keyed Ping.
pub fn ping(ms: u64) -> Fact {
    Fact::new(t(FactMsg::Ping), ping_value(ms)).priority(5)
}

/// [`players`] for a server of a core's list.
pub fn server_players(server: &ServerInstance) -> Option<Fact> {
    players(server.state, server.player_count, server.max_player_count)
}

/// A game or loader written as one string ("Fabric 0.16.11", "Vanilla"), as a phrase
/// when it names a product, and as it is otherwise.
pub fn phrase(text: &str) -> FactValue {
    let text = text.trim();
    let (name, version) = text.split_once(' ').unwrap_or((text, ""));
    match ProductPhrase::from_name(name, version.trim().to_string()) {
        Some(phrase) => phrase.into(),
        None => FactValue::text(text.to_string()),
    }
}

/// The card naming a server under a dialog's title, with its game and loader (or the
/// host it is on) as facts.
pub fn server_card(name: &str) -> crate::components::ObjectCard {
    crate::components::ObjectCard::new(lodestone_ui::values::truncate::sanitize(name, 120))
        .kind(crate::components::Kind::Server)
}

/// The host a server is on, as a muted chip keyed Host.
pub fn host_fact(name: &str) -> Fact {
    Fact::new(
        t(FactMsg::Host),
        crate::components::ObjectChip::new(lodestone_ui::values::truncate::sanitize(name, 64))
            .kind(crate::components::Kind::Computer)
            .muted(),
    )
}

/// The card naming a host, with its address as an exact value.
pub fn host_card(name: &str, address: &str) -> crate::components::ObjectCard {
    crate::components::ObjectCard::new(lodestone_ui::values::truncate::sanitize(name, 64))
        .kind(crate::components::Kind::Computer)
        .fact(Fact::new(
            t(FactMsg::Address),
            FactValue::mono(address.to_string()),
        ))
}

/// [`server_card`] with the server's game and loader as phrases.
pub fn server_versions_card(server: &ServerInstance) -> crate::components::ObjectCard {
    versions(server)
        .into_iter()
        .fold(server_card(&server.name), |card, fact| card.fact(fact))
}
