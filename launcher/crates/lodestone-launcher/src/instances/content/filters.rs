//! The filters of the Modrinth results: Modrinth's categories for each kind (with
//! features, resolutions and performance impact), where a mod runs, and open source.
//!
//! The categories are Modrinth's (`GET /v2/tag/category`), kept here so the panel
//! works offline and is translated.

use std::collections::BTreeSet;

use launcher_core::content::ContentKind;
use lodestone_minecraft::modrinth::ClientEnvironment;

use crate::i18n::{ContentMsg as M, t};

/// A heading of the panel, and how its choices combine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Categories,
    Features,
    Resolution,
    Performance,
}

impl Group {
    pub fn label(self) -> M {
        match self {
            Group::Categories => M::FilterCategories,
            Group::Features => M::FilterFeatures,
            Group::Resolution => M::FilterResolution,
            Group::Performance => M::FilterPerformance,
        }
    }

    /// Whether a project needs any one of the group's choices (one resolution or
    /// another), rather than all of them (magic and technology).
    pub fn any_of(self) -> bool {
        matches!(self, Group::Resolution | Group::Performance)
    }
}

/// A Modrinth category, by its name in the API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Category {
    pub slug: &'static str,
    label: Label,
    /// The asset path of Modrinth's icon for it, if it has one.
    pub icon: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Label {
    Msg(M),
    /// The same in every language: "16x".
    Raw(&'static str),
}

impl Category {
    const fn msg(slug: &'static str, label: M) -> Self {
        Self {
            slug,
            label: Label::Msg(label),
            icon: None,
        }
    }

    const fn icon(mut self, path: &'static str) -> Self {
        self.icon = Some(path);
        self
    }

    const fn raw(slug: &'static str) -> Self {
        Self {
            slug,
            label: Label::Raw(slug),
            icon: None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self.label {
            Label::Msg(message) => t(message),
            Label::Raw(text) => text,
        }
    }
}

const MOD_CATEGORIES: &[Category] = &[
    Category::msg("adventure", M::CatAdventure).icon("images/categories/adventure.svg"),
    Category::msg("cursed", M::CatCursed).icon("images/categories/cursed.svg"),
    Category::msg("decoration", M::CatDecoration).icon("images/categories/decoration.svg"),
    Category::msg("economy", M::CatEconomy).icon("images/categories/economy.svg"),
    Category::msg("equipment", M::CatEquipment).icon("images/categories/equipment.svg"),
    Category::msg("food", M::CatFood).icon("images/categories/food.svg"),
    Category::msg("game-mechanics", M::CatGameMechanics)
        .icon("images/categories/game-mechanics.svg"),
    Category::msg("library", M::CatLibrary).icon("images/categories/library.svg"),
    Category::msg("magic", M::CatMagic).icon("images/categories/magic.svg"),
    Category::msg("management", M::CatManagement).icon("images/categories/management.svg"),
    Category::msg("minigame", M::CatMinigame).icon("images/categories/minigame.svg"),
    Category::msg("mobs", M::CatMobs).icon("images/categories/mobs.svg"),
    Category::msg("optimization", M::CatOptimization).icon("images/categories/optimization.svg"),
    Category::msg("social", M::CatSocial).icon("images/categories/social.svg"),
    Category::msg("storage", M::CatStorage).icon("images/categories/storage.svg"),
    Category::msg("technology", M::CatTechnology).icon("images/categories/technology.svg"),
    Category::msg("transportation", M::CatTransportation)
        .icon("images/categories/transportation.svg"),
    Category::msg("utility", M::CatUtility).icon("images/categories/utility.svg"),
    Category::msg("worldgen", M::CatWorldGen).icon("images/categories/worldgen.svg"),
];

const PACK_CATEGORIES: &[Category] = &[
    Category::msg("combat", M::CatCombat).icon("images/categories/combat.svg"),
    Category::msg("cursed", M::CatCursed).icon("images/categories/cursed.svg"),
    Category::msg("decoration", M::CatDecoration).icon("images/categories/decoration.svg"),
    Category::msg("modded", M::CatModded).icon("images/categories/modded.svg"),
    Category::msg("realistic", M::CatRealistic).icon("images/categories/realistic.svg"),
    Category::msg("simplistic", M::CatSimplistic).icon("images/categories/simplistic.svg"),
    Category::msg("themed", M::CatThemed).icon("images/categories/themed.svg"),
    Category::msg("tweaks", M::CatTweaks).icon("images/categories/tweaks.svg"),
    Category::msg("utility", M::CatUtility).icon("images/categories/utility.svg"),
    Category::msg("vanilla-like", M::CatVanillaLike).icon("images/categories/vanilla-like.svg"),
];

const PACK_FEATURES: &[Category] = &[
    Category::msg("audio", M::CatAudio).icon("images/categories/audio.svg"),
    Category::msg("blocks", M::CatBlocks).icon("images/categories/blocks.svg"),
    Category::msg("core-shaders", M::CatCoreShaders).icon("images/categories/core-shaders.svg"),
    Category::msg("entities", M::CatEntities).icon("images/categories/entities.svg"),
    Category::msg("environment", M::CatEnvironment).icon("images/categories/environment.svg"),
    Category::msg("equipment", M::CatEquipment).icon("images/categories/equipment-pack.svg"),
    Category::msg("fonts", M::CatFonts).icon("images/categories/fonts.svg"),
    Category::msg("gui", M::CatGui).icon("images/categories/gui.svg"),
    Category::msg("items", M::CatItems).icon("images/categories/items.svg"),
    Category::msg("locale", M::CatLocale).icon("images/categories/locale.svg"),
    Category::msg("models", M::CatModels).icon("images/categories/models.svg"),
];

const PACK_RESOLUTIONS: &[Category] = &[
    Category::msg("8x-", M::CatResolutionLow),
    Category::raw("16x"),
    Category::raw("32x"),
    Category::raw("48x"),
    Category::raw("64x"),
    Category::raw("128x"),
    Category::raw("256x"),
    Category::msg("512x+", M::CatResolutionHigh),
];

const SHADER_CATEGORIES: &[Category] = &[
    Category::msg("cartoon", M::CatCartoon).icon("images/categories/cartoon.svg"),
    Category::msg("cursed", M::CatCursed).icon("images/categories/cursed.svg"),
    Category::msg("fantasy", M::CatFantasy).icon("images/categories/fantasy.svg"),
    Category::msg("realistic", M::CatRealistic).icon("images/categories/realistic-shader.svg"),
    Category::msg("semi-realistic", M::CatSemiRealistic)
        .icon("images/categories/semi-realistic.svg"),
    Category::msg("vanilla-like", M::CatVanillaLike).icon("images/categories/vanilla-like.svg"),
];

const SHADER_FEATURES: &[Category] = &[
    Category::msg("atmosphere", M::CatAtmosphere).icon("images/categories/atmosphere.svg"),
    Category::msg("bloom", M::CatBloom).icon("images/categories/bloom.svg"),
    Category::msg("colored-lighting", M::CatColoredLighting)
        .icon("images/categories/colored-lighting.svg"),
    Category::msg("foliage", M::CatFoliage).icon("images/categories/foliage.svg"),
    Category::msg("path-tracing", M::CatPathTracing).icon("images/categories/path-tracing.svg"),
    Category::msg("pbr", M::CatPbr).icon("images/categories/pbr.svg"),
    Category::msg("reflections", M::CatReflections).icon("images/categories/reflections.svg"),
    Category::msg("shadows", M::CatShadows).icon("images/categories/shadows.svg"),
];

const SHADER_PERFORMANCE: &[Category] = &[
    Category::msg("potato", M::CatPotato).icon("images/categories/potato.svg"),
    Category::msg("low", M::CatLow).icon("images/categories/low.svg"),
    Category::msg("medium", M::CatMedium).icon("images/categories/medium.svg"),
    Category::msg("high", M::CatHigh).icon("images/categories/high.svg"),
    Category::msg("screenshot", M::CatScreenshot).icon("images/categories/screenshot.svg"),
];

/// The groups of categories of `kind`, in the panel's order.
pub fn groups(kind: ContentKind) -> &'static [(Group, &'static [Category])] {
    match kind {
        ContentKind::Mod => &[(Group::Categories, MOD_CATEGORIES)],
        ContentKind::ResourcePack => &[
            (Group::Categories, PACK_CATEGORIES),
            (Group::Features, PACK_FEATURES),
            (Group::Resolution, PACK_RESOLUTIONS),
        ],
        ContentKind::ShaderPack => &[
            (Group::Categories, SHADER_CATEGORIES),
            (Group::Features, SHADER_FEATURES),
            (Group::Performance, SHADER_PERFORMANCE),
        ],
    }
}

/// The icons of the environment's choices and of "Open source only".
pub const ENV_ANY_ICON: &str = "images/categories/any.svg";
pub const ENV_CLIENT_ICON: &str = "images/categories/client.svg";
pub const ENV_SERVER_ICON: &str = "images/categories/server.svg";
pub const OPEN_SOURCE_ICON: &str = "images/categories/open-source.svg";

/// Whether `kind` has the environment filter: only mods run on a server.
pub fn has_environment(kind: ContentKind) -> bool {
    kind == ContentKind::Mod
}

/// What the Modrinth results are filtered by, besides the instance's version and
/// loader.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Filters {
    /// Picked categories, by their Modrinth name (unique within a kind).
    pub categories: BTreeSet<&'static str>,
    pub environment: ClientEnvironment,
    pub open_source: bool,
}

impl Filters {
    /// How many filters are picked, for the Filters button.
    pub fn count(&self) -> usize {
        self.categories.len()
            + usize::from(self.environment != ClientEnvironment::Any)
            + usize::from(self.open_source)
    }

    pub fn toggle_category(&mut self, slug: &'static str) {
        if !self.categories.remove(slug) {
            self.categories.insert(slug);
        }
    }

    /// The picked categories as `ClientSearchParams::with_filters` takes them: each
    /// category of an all-of group on its own, an any-of group's together.
    pub fn category_facets(&self, kind: ContentKind) -> Vec<Vec<String>> {
        let mut facets = Vec::new();
        for (group, categories) in groups(kind) {
            let picked: Vec<String> = categories
                .iter()
                .filter(|c| self.categories.contains(c.slug))
                .map(|c| c.slug.to_string())
                .collect();
            if picked.is_empty() {
                continue;
            }
            if group.any_of() {
                facets.push(picked);
            } else {
                facets.extend(picked.into_iter().map(|slug| vec![slug]));
            }
        }
        facets
    }

    /// The environment to search with: none for packs.
    pub fn environment_for(&self, kind: ContentKind) -> ClientEnvironment {
        if has_environment(kind) {
            self.environment
        } else {
            ClientEnvironment::Any
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_are_unique_within_a_kind() {
        for kind in ContentKind::ALL {
            let mut seen = BTreeSet::new();
            for (_, categories) in groups(kind) {
                for category in *categories {
                    assert!(seen.insert(category.slug), "{kind:?}: {}", category.slug);
                }
            }
        }
    }

    /// Every icon a category names is embedded, so none shows up blank.
    #[test]
    fn category_icons_are_embedded() {
        for kind in ContentKind::ALL {
            for (_, categories) in groups(kind) {
                for category in *categories {
                    if let Some(path) = category.icon {
                        assert!(
                            lodestone_ui::assets::Assets::embedded(path).is_some(),
                            "{path} is not embedded"
                        );
                    }
                }
            }
        }
        for path in [
            ENV_ANY_ICON,
            ENV_CLIENT_ICON,
            ENV_SERVER_ICON,
            OPEN_SOURCE_ICON,
        ] {
            assert!(
                lodestone_ui::assets::Assets::embedded(path).is_some(),
                "{path}"
            );
        }
    }

    #[test]
    fn facets_combine_by_group() {
        let mut filters = Filters::default();
        filters.toggle_category("audio");
        filters.toggle_category("tweaks");
        filters.toggle_category("16x");
        filters.toggle_category("32x");
        filters.toggle_category("blocks");
        assert_eq!(filters.count(), 5);
        assert_eq!(
            filters.category_facets(ContentKind::ResourcePack),
            vec![
                vec!["tweaks".to_string()],
                vec!["audio".to_string()],
                vec!["blocks".to_string()],
                vec!["16x".to_string(), "32x".to_string()],
            ]
        );
        filters.toggle_category("blocks");
        assert_eq!(filters.count(), 4);
        filters.environment = ClientEnvironment::ServerToo;
        assert_eq!(
            filters.environment_for(ContentKind::ShaderPack),
            ClientEnvironment::Any
        );
        assert_eq!(
            filters.environment_for(ContentKind::Mod),
            ClientEnvironment::ServerToo
        );
    }
}
