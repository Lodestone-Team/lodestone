//! The product phrase rule: the game and its loaders read as one plain phrase,
//! "Minecraft 26.3" or "Fabric 0.20.1", never a name with a version tag after it.
//! Version tags are for mods, plugins and modpacks, whose versions are noisy
//! (`0.116.17+1.21.1`); the game's and the loaders' are short and part of the name
//! people say.
//!
//! In key-over-value strips and fact tables the phrase is the value and the key says
//! what it is: "Game" for Minecraft, "Loader" for the rest ([`Product::fact_key`]).
//! Vanilla has no loader, so it has no product: a row leaves the Loader fact out, and
//! a fact table shows "Vanilla" with no version.

use launcher_core::{
    cores::admin::instances::ServerSoftware, game::loaders::LoaderKind, instances::Loader,
};

use crate::i18n::FactMsg;

/// The game, or a loader or server software that runs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Product {
    Minecraft,
    Fabric,
    Quilt,
    Forge,
    NeoForge,
    Paper,
    Purpur,
}

impl Product {
    pub const ALL: [Product; 7] = [
        Product::Minecraft,
        Product::Fabric,
        Product::Quilt,
        Product::Forge,
        Product::NeoForge,
        Product::Paper,
        Product::Purpur,
    ];

    /// The product a name means, ignoring case: "Fabric", "neoforge", and the keys
    /// `.mrpack` files use (`fabric-loader`, `quilt-loader`). `None` for a mod's name,
    /// and for "Vanilla", which is no loader.
    pub fn from_name(name: &str) -> Option<Product> {
        let name = name.trim();
        let is = |key: &str| name.eq_ignore_ascii_case(key);
        if is("minecraft") {
            Some(Product::Minecraft)
        } else if is("fabric") || is("fabric-loader") {
            Some(Product::Fabric)
        } else if is("quilt") || is("quilt-loader") {
            Some(Product::Quilt)
        } else if is("forge") {
            Some(Product::Forge)
        } else if is("neoforge") || is("neo-forge") {
            Some(Product::NeoForge)
        } else if is("paper") {
            Some(Product::Paper)
        } else if is("purpur") {
            Some(Product::Purpur)
        } else {
            None
        }
    }

    /// The name, as the product spells it. Never translated.
    pub fn name(self) -> &'static str {
        match self {
            Product::Minecraft => "Minecraft",
            Product::Fabric => "Fabric",
            Product::Quilt => "Quilt",
            Product::Forge => "Forge",
            Product::NeoForge => "NeoForge",
            Product::Paper => "Paper",
            Product::Purpur => "Purpur",
        }
    }

    /// Whether this is the game itself rather than what runs it.
    pub fn is_game(self) -> bool {
        self == Product::Minecraft
    }

    /// The key it has in a strip or a fact table: "Game" or "Loader".
    pub fn fact_key(self) -> FactMsg {
        if self.is_game() {
            FactMsg::Game
        } else {
            FactMsg::Loader
        }
    }
}

/// Whether `name` is the game or a loader, so its version is part of a plain phrase
/// rather than a version tag.
pub fn is_game_or_loader(name: &str) -> bool {
    Product::from_name(name).is_some()
}

/// The phrase for `product` at `version`: "Minecraft 26.3". A no-break space joins
/// them, so the phrase never wraps apart.
pub fn phrase(product: Product, version: &str) -> String {
    let version = version.trim();
    if version.is_empty() {
        product.name().to_string()
    } else {
        format!("{}\u{a0}{version}", product.name())
    }
}

impl Product {
    /// An instance's loader; `None` for Vanilla, whose Loader fact is left out.
    pub fn of_loader(loader: Loader) -> Option<Product> {
        match loader {
            Loader::None => None,
            Loader::Fabric => Some(Product::Fabric),
            Loader::Quilt => Some(Product::Quilt),
            Loader::Forge => Some(Product::Forge),
            Loader::NeoForge => Some(Product::NeoForge),
        }
    }

    /// A server's software; `None` for Vanilla and software the Host doesn't know.
    pub fn of_software(software: ServerSoftware) -> Option<Product> {
        match software {
            ServerSoftware::Vanilla | ServerSoftware::Unknown => None,
            ServerSoftware::Paper => Some(Product::Paper),
            ServerSoftware::Fabric => Some(Product::Fabric),
            ServerSoftware::Forge => Some(Product::Forge),
            ServerSoftware::NeoForge => Some(Product::NeoForge),
        }
    }
}

impl From<LoaderKind> for Product {
    fn from(kind: LoaderKind) -> Self {
        match kind {
            LoaderKind::Fabric => Product::Fabric,
            LoaderKind::Quilt => Product::Quilt,
            LoaderKind::Forge => Product::Forge,
            LoaderKind::NeoForge => Product::NeoForge,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_recognised_ignoring_case() {
        for (name, product) in [
            ("Minecraft", Product::Minecraft),
            ("minecraft", Product::Minecraft),
            ("Fabric", Product::Fabric),
            ("fabric-loader", Product::Fabric),
            ("QUILT", Product::Quilt),
            ("quilt-loader", Product::Quilt),
            ("Forge", Product::Forge),
            ("NeoForge", Product::NeoForge),
            ("neoforge", Product::NeoForge),
            ("Paper", Product::Paper),
            ("Purpur", Product::Purpur),
        ] {
            assert_eq!(Product::from_name(name), Some(product), "{name}");
            assert!(is_game_or_loader(name));
        }
        // mods and Vanilla are no product
        for name in ["Sodium", "Fabric API", "Forgero", "Vanilla", ""] {
            assert_eq!(Product::from_name(name), None, "{name}");
            assert!(!is_game_or_loader(name));
        }
    }

    #[test]
    fn a_phrase_is_one_unbreakable_element() {
        assert_eq!(phrase(Product::Minecraft, "26.3"), "Minecraft\u{a0}26.3");
        assert_eq!(phrase(Product::Fabric, "0.20.1"), "Fabric\u{a0}0.20.1");
        assert_eq!(
            phrase(Product::NeoForge, " 21.1.77 "),
            "NeoForge\u{a0}21.1.77"
        );
        assert_eq!(phrase(Product::Paper, ""), "Paper");
        for product in Product::ALL {
            assert!(!phrase(product, "1.0").contains(' '));
        }
    }

    #[test]
    fn the_game_is_keyed_game_and_the_rest_loader() {
        assert!(Product::Minecraft.is_game());
        assert_eq!(Product::Minecraft.fact_key(), FactMsg::Game);
        for product in &Product::ALL[1..] {
            assert!(!product.is_game());
            assert_eq!(product.fact_key(), FactMsg::Loader);
        }
    }

    #[test]
    fn launcher_types_map_to_products() {
        assert_eq!(Product::of_loader(Loader::None), None);
        assert_eq!(Product::of_loader(Loader::Quilt), Some(Product::Quilt));
        assert_eq!(Product::from(LoaderKind::NeoForge), Product::NeoForge);
        assert_eq!(Product::of_software(ServerSoftware::Vanilla), None);
        assert_eq!(Product::of_software(ServerSoftware::Unknown), None);
        assert_eq!(
            Product::of_software(ServerSoftware::Paper),
            Some(Product::Paper)
        );
    }
}
