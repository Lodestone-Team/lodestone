//! The `options.txt` keys Lodestone shares, by category, with the data versions they
//! exist at and the formats they have had.
//!
//! Sources: minecraft.wiki, "Options.txt (Java Edition)" (the version each key was added
//! or removed in) and "Data version" (the data version of each release); the game's
//! `Options*Fix` data fixers in the 1.21.11 client (the value formats). A key is placed at
//! the data version of the first release that has it. Releases before 1.10 have no data
//! version in `options.txt`; they get the made-up numbers in [`legacy_data_version`]
//! (below 100, where the real ones start), so one number orders every version.

use serde::{Deserialize, Serialize};

/// A group of shared settings, as the launcher's Settings show them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Controls,
    Video,
    Sound,
    /// Accessibility and chat.
    Accessibility,
    Language,
}

impl Category {
    pub const ALL: [Category; 5] = [
        Category::Controls,
        Category::Video,
        Category::Sound,
        Category::Accessibility,
        Category::Language,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Controls => "Controls",
            Category::Video => "Video",
            Category::Sound => "Sound",
            Category::Accessibility => "Accessibility & chat",
            Category::Language => "Language",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Category::Controls => {
                "Mouse sensitivity, invert, raw input, key bindings, toggle crouch and sprint"
            }
            Category::Video => {
                "FOV, render and simulation distance, GUI scale, brightness, fullscreen, VSync, max FPS, particles, graphics"
            }
            Category::Sound => "Every volume slider, subtitles, directional audio",
            Category::Accessibility => "Narrator, text background, chat size and opacity, effects",
            Category::Language => "Game language and font",
        }
    }
}

/// What a setting holds, in the shared (canonical) format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `true` or `false`.
    Bool,
    /// A number, written as the game writes it.
    Number,
    /// `key.keyboard.w`, `key.mouse.left`.
    KeyBinding,
    /// A lowercase language code, `en_us`.
    Language,
    /// `fast`, `fancy`, `fabulous` or `custom`.
    Graphics,
    /// Smooth lighting, `true` or `false`.
    AmbientOcclusion,
    /// Menu background blur, 0–10.
    Blurriness,
}

/// How one key's value is written at a data version. Most keys keep one format;
/// [`Codec::Language`], [`Codec::AmbientOcclusion`], [`Codec::Blurriness`] and
/// [`Codec::KeyBinding`] change with the data version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Plain,
    Bool,
    KeyBinding,
    Language,
    AmbientOcclusion,
    Blurriness,
    /// `fancyGraphics:true|false`, until 1.16.
    FancyGraphics,
    /// `graphicsMode:0|1|2`, 1.16 to 1.21.10.
    GraphicsMode,
    /// `graphicsPreset:"fancy"`, 1.21.11 and later.
    GraphicsPreset,
}

/// A key the game has written a setting under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Form {
    pub key: &'static str,
    /// The first data version with the key.
    pub since: u32,
    /// The first data version without it.
    pub until: u32,
    pub codec: Codec,
}

impl Form {
    pub fn exists_at(&self, dv: u32) -> bool {
        self.since <= dv && dv < self.until
    }
}

/// A shared setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spec {
    /// The key in the shared settings (the current key of the setting).
    pub key: &'static str,
    pub label: &'static str,
    pub category: Category,
    pub kind: Kind,
    pub since: u32,
    pub until: u32,
    /// Older keys of the same setting.
    pub aliases: &'static [Form],
}

impl Spec {
    /// The setting's keys, the current one first.
    pub fn forms(&self) -> impl Iterator<Item = Form> + '_ {
        std::iter::once(Form {
            key: self.key,
            since: self.since,
            until: self.until,
            codec: match self.kind {
                Kind::Bool => Codec::Bool,
                Kind::Number => Codec::Plain,
                Kind::KeyBinding => Codec::KeyBinding,
                Kind::Language => Codec::Language,
                Kind::Graphics => Codec::GraphicsPreset,
                Kind::AmbientOcclusion => Codec::AmbientOcclusion,
                Kind::Blurriness => Codec::Blurriness,
            },
        })
        .chain(self.aliases.iter().copied())
    }

    const fn until(mut self, dv: u32) -> Self {
        self.until = dv;
        self
    }
}

/// No end.
pub const NEVER: u32 = u32::MAX;

// Data versions of releases (minecraft.wiki "Data version"); before 1.10, made up.
pub const V1_7_2: u32 = 50;
pub const V1_8: u32 = 99;
pub const V1_9: u32 = 169;
pub const V1_10: u32 = 510;
pub const V1_11: u32 = 819;
pub const V1_12: u32 = 1139;
pub const V1_13: u32 = 1519;
pub const V1_14: u32 = 1952;
pub const V1_14_4: u32 = 1976;
pub const V1_15: u32 = 2225;
pub const V1_16: u32 = 2566;
pub const V1_16_2: u32 = 2578;
pub const V1_16_4: u32 = 2584;
pub const V1_17: u32 = 2724;
pub const V1_18: u32 = 2860;
pub const V1_19: u32 = 3105;
pub const V1_19_3: u32 = 3218;
pub const V1_19_4: u32 = 3337;
pub const V1_20_2: u32 = 3578;
pub const V1_20_3: u32 = 3698;
pub const V1_20_5: u32 = 3837;
pub const V1_21_2: u32 = 4080;
pub const V1_21_6: u32 = 4435;
pub const V1_21_9: u32 = 4554;
pub const V1_21_11: u32 = 4671;
pub const V26_1: u32 = 4786;
pub const V26_2: u32 = 4903;
pub const V26_3: u32 = 5023;

/// `OptionsLowerCaseLanguageFix`: older files have `en_US`.
pub const LOWERCASE_LANGUAGE_SINCE: u32 = 816;
/// `OptionsAmbientOcclusionFix`: older files have `ao:0|1|2`.
pub const AO_BOOL_SINCE: u32 = 3214;
/// `OptionsMenuBlurrinessFix`: older files have the blur as 0.0–1.0.
pub const BLUR_INT_SINCE: u32 = 3943;

const fn spec(
    key: &'static str,
    label: &'static str,
    category: Category,
    kind: Kind,
    since: u32,
) -> Spec {
    Spec {
        key,
        label,
        category,
        kind,
        since,
        until: NEVER,
        aliases: &[],
    }
}

const fn bind(key: &'static str, label: &'static str, since: u32) -> Spec {
    spec(key, label, Category::Controls, Kind::KeyBinding, since)
}

const fn controls(key: &'static str, label: &'static str, kind: Kind, since: u32) -> Spec {
    spec(key, label, Category::Controls, kind, since)
}

const fn video(key: &'static str, label: &'static str, kind: Kind, since: u32) -> Spec {
    spec(key, label, Category::Video, kind, since)
}

const fn sound(key: &'static str, label: &'static str, since: u32) -> Spec {
    spec(key, label, Category::Sound, Kind::Number, since)
}

const fn access(key: &'static str, label: &'static str, kind: Kind, since: u32) -> Spec {
    spec(key, label, Category::Accessibility, kind, since)
}

use Kind::{Bool, Number};

/// Every shared setting, in the order the launcher lists them.
pub const SPECS: &[Spec] = &[
    // Controls
    controls("mouseSensitivity", "Mouse sensitivity", Number, 0),
    controls("invertYMouse", "Invert mouse (vertical)", Bool, 0),
    controls("invertXMouse", "Invert mouse (horizontal)", Bool, V1_21_9),
    controls("rawMouseInput", "Raw input", Bool, V1_14_4).until(V26_3),
    controls("mouseWheelSensitivity", "Scroll sensitivity", Number, V1_13),
    controls("discrete_mouse_scroll", "Discrete scrolling", Bool, V1_14),
    controls("toggleCrouch", "Toggle crouch", Bool, V1_15),
    controls("toggleSprint", "Toggle sprint", Bool, V1_15),
    controls("toggleAttack", "Toggle attack", Bool, V1_21_9),
    controls("toggleUse", "Toggle use", Bool, V1_21_9),
    controls("sprintWindow", "Sprint window", Number, V1_21_9),
    controls("autoJump", "Auto-jump", Bool, V1_10),
    bind("key_key.attack", "Attack/Destroy", 0),
    bind("key_key.use", "Use item/Place block", 0),
    bind("key_key.pickItem", "Pick block", 0),
    bind("key_key.forward", "Walk forwards", 0),
    bind("key_key.left", "Strafe left", 0),
    bind("key_key.back", "Walk backwards", 0),
    bind("key_key.right", "Strafe right", 0),
    bind("key_key.jump", "Jump", 0),
    bind("key_key.sneak", "Sneak", 0),
    bind("key_key.sprint", "Sprint", V1_7_2),
    bind("key_key.drop", "Drop selected item", 0),
    bind("key_key.inventory", "Open/Close inventory", 0),
    Spec {
        aliases: &[Form {
            key: "key_key.swapHands",
            since: V1_9,
            until: V1_16,
            codec: Codec::KeyBinding,
        }],
        ..bind("key_key.swapOffhand", "Swap item with offhand", V1_16)
    },
    bind("key_key.chat", "Open chat", 0),
    bind("key_key.command", "Open command", 0),
    bind("key_key.playerlist", "List players", 0),
    bind("key_key.socialInteractions", "Social interactions", V1_16_4),
    bind("key_key.friends", "Friends", V26_2),
    bind("key_key.advancements", "Advancements", V1_12),
    bind("key_key.quickActions", "Quick actions", V1_21_6),
    bind("key_key.screenshot", "Take screenshot", V1_7_2),
    bind("key_key.togglePerspective", "Toggle perspective", V1_7_2),
    bind("key_key.smoothCamera", "Toggle cinematic camera", V1_7_2),
    bind("key_key.fullscreen", "Toggle fullscreen", V1_7_2),
    bind(
        "key_key.spectatorOutlines",
        "Highlight players (spectators)",
        V1_8,
    ),
    bind("key_key.spectatorHotbar", "Spectator hotbar", V1_21_9),
    bind("key_key.toggleGui", "Toggle HUD", V1_21_11),
    bind(
        "key_key.toggleSpectatorShaderEffects",
        "Toggle spectator shader effects",
        V1_21_11,
    ),
    bind(
        "key_key.saveToolbarActivator",
        "Save hotbar activator",
        V1_12,
    ),
    bind(
        "key_key.loadToolbarActivator",
        "Load hotbar activator",
        V1_12,
    ),
    bind("key_key.hotbar.1", "Hotbar slot 1", V1_7_2),
    bind("key_key.hotbar.2", "Hotbar slot 2", V1_7_2),
    bind("key_key.hotbar.3", "Hotbar slot 3", V1_7_2),
    bind("key_key.hotbar.4", "Hotbar slot 4", V1_7_2),
    bind("key_key.hotbar.5", "Hotbar slot 5", V1_7_2),
    bind("key_key.hotbar.6", "Hotbar slot 6", V1_7_2),
    bind("key_key.hotbar.7", "Hotbar slot 7", V1_7_2),
    bind("key_key.hotbar.8", "Hotbar slot 8", V1_7_2),
    bind("key_key.hotbar.9", "Hotbar slot 9", V1_7_2),
    bind("key_key.streamStartStop", "Start/Stop stream", V1_7_2).until(V1_9),
    bind("key_key.streamPauseUnpause", "Pause/Unpause stream", V1_7_2).until(V1_9),
    bind(
        "key_key.streamCommercial",
        "Show stream commercials",
        V1_7_2,
    )
    .until(V1_9),
    bind("key_key.streamToggleMic", "Push to talk/mute", V1_7_2).until(V1_9),
    bind("key_key.debug.overlay", "Debug overlay", V1_21_11),
    bind("key_key.debug.modifier", "Debug modifier", V1_21_11),
    // Video
    video("fov", "FOV", Number, 0),
    video("renderDistance", "Render distance", Number, V1_7_2),
    video("simulationDistance", "Simulation distance", Number, V1_18),
    video("guiScale", "GUI scale", Number, 0),
    video("gamma", "Brightness", Number, 0),
    video("fullscreen", "Fullscreen", Bool, 0),
    video("enableVsync", "VSync", Bool, 0),
    video("maxFps", "Max framerate", Number, V1_7_2),
    video("particles", "Particles", Number, 0),
    Spec {
        aliases: &[
            Form {
                key: "graphicsMode",
                since: V1_16,
                until: V1_21_11,
                codec: Codec::GraphicsMode,
            },
            Form {
                key: "fancyGraphics",
                since: 0,
                until: V1_16,
                codec: Codec::FancyGraphics,
            },
        ],
        ..video("graphicsPreset", "Graphics", Kind::Graphics, V1_21_11)
    },
    video("ao", "Smooth lighting", Kind::AmbientOcclusion, 0),
    video("bobView", "View bobbing", Bool, 0),
    video("entityShadows", "Entity shadows", Bool, V1_8),
    video("mipmapLevels", "Mipmap levels", Number, V1_7_2),
    video("biomeBlendRadius", "Biome blend", Number, V1_13),
    video("entityDistanceScaling", "Entity distance", Number, V1_16),
    video("cloudRange", "Cloud distance", Number, V1_21_6),
    video(
        "improvedTransparency",
        "Improved transparency",
        Bool,
        V1_21_11,
    ),
    video("cutoutLeaves", "Cutout leaves", Bool, V1_21_11),
    video("vignette", "Vignette", Bool, V1_21_11),
    video("weatherRadius", "Weather radius", Number, V1_21_11),
    video(
        "chunkSectionFadeInTime",
        "Chunk fade-in time",
        Number,
        V1_21_11,
    ),
    video("textureFiltering", "Texture filtering", Number, V1_21_11),
    video(
        "maxAnisotropyBit",
        "Anisotropic filtering",
        Number,
        V1_21_11,
    ),
    // Sound
    sound("soundCategory_master", "Master volume", V1_7_2),
    sound("soundCategory_music", "Music", V1_7_2),
    sound("soundCategory_record", "Jukebox/Note blocks", V1_7_2),
    sound("soundCategory_weather", "Weather", V1_7_2),
    sound("soundCategory_block", "Blocks", V1_7_2),
    sound("soundCategory_hostile", "Hostile creatures", V1_7_2),
    sound("soundCategory_neutral", "Friendly creatures", V1_7_2),
    sound("soundCategory_player", "Players", V1_7_2),
    sound("soundCategory_ambient", "Ambient/Environment", V1_7_2),
    sound("soundCategory_voice", "Voice/Speech", V1_9),
    sound("soundCategory_ui", "Interface", V1_21_6),
    spec(
        "showSubtitles",
        "Show subtitles",
        Category::Sound,
        Bool,
        V1_9,
    ),
    spec(
        "directionalAudio",
        "Directional audio",
        Category::Sound,
        Bool,
        V1_19,
    ),
    // Accessibility and chat
    access("narrator", "Narrator", Number, V1_12),
    access("highContrast", "High contrast", Bool, V1_19_4),
    access(
        "highContrastBlockOutline",
        "High contrast block outline",
        Bool,
        V1_21_2,
    ),
    access(
        "textBackgroundOpacity",
        "Text background opacity",
        Number,
        V1_14,
    ),
    access(
        "backgroundForChatOnly",
        "Text background for chat only",
        Bool,
        V1_14,
    ),
    access("chatVisibility", "Chat", Number, 0),
    access("chatColors", "Chat colors", Bool, 0),
    access("chatLinks", "Web links", Bool, 0),
    access("chatLinksPrompt", "Prompt on links", Bool, 0),
    access("chatOpacity", "Chat text opacity", Number, 0),
    access("chatScale", "Chat text size", Number, 0),
    access("chatWidth", "Chat width", Number, 0),
    access("chatHeightFocused", "Chat height (focused)", Number, 0),
    access("chatHeightUnfocused", "Chat height (unfocused)", Number, 0),
    access("chatLineSpacing", "Chat line spacing", Number, V1_16),
    access("chatDelay", "Chat delay", Number, V1_16),
    access("autoSuggestions", "Command suggestions", Bool, V1_13),
    access("hideMatchedNames", "Hide matched names", Bool, V1_16_4),
    access("onlyShowSecureChat", "Only show secure chat", Bool, V1_19),
    access("saveChatDrafts", "Save chat drafts", Bool, V1_21_9),
    access(
        "notificationDisplayTime",
        "Notification time",
        Number,
        V1_19_4,
    ),
    access("screenEffectScale", "Distortion effects", Number, V1_16_2),
    access("fovEffectScale", "FOV effects", Number, V1_16_2),
    access("darknessEffectScale", "Darkness pulsing", Number, V1_19),
    access("damageTiltStrength", "Damage tilt", Number, V1_19_4),
    access("glintSpeed", "Glint speed", Number, V1_19_4),
    access("glintStrength", "Glint strength", Number, V1_19_4),
    access(
        "hideLightningFlashes",
        "Hide lightning flashes",
        Bool,
        V1_18,
    ),
    access(
        "darkMojangStudiosBackground",
        "Monochrome logo",
        Bool,
        V1_17,
    ),
    access(
        "menuBackgroundBlurriness",
        "Menu background blur",
        Kind::Blurriness,
        V1_20_5,
    ),
    access(
        "panoramaScrollSpeed",
        "Panorama scroll speed",
        Number,
        V1_19_3,
    ),
    access("hideSplashTexts", "Hide splash texts", Bool, V1_20_3),
    access("narratorHotkey", "Narrator hotkey", Bool, V1_20_2),
    // Until it is false, the game's first start opens an accessibility screen that the
    // narrator reads aloud.
    access(
        "onboardAccessibility",
        "Accessibility screen on first start",
        Bool,
        V1_19_4,
    ),
    access("rotateWithMinecart", "Rotate with minecarts", Bool, V1_21_2),
    access("allowCursorChanges", "Allow cursor changes", Bool, V1_21_9),
    // Language
    spec("lang", "Language", Category::Language, Kind::Language, 0),
    spec(
        "forceUnicodeFont",
        "Force Unicode font",
        Category::Language,
        Bool,
        V1_7_2,
    ),
    spec(
        "japaneseGlyphVariants",
        "Japanese glyph variants",
        Category::Language,
        Bool,
        V1_20_5,
    ),
];

/// Never shared, whatever else changes.
pub const EXCLUDED: &[&str] = &[
    "version",
    "resourcePacks",
    "incompatibleResourcePacks",
    "lastServer",
];

/// The shared setting `key` (its current key or an older one).
pub fn find(key: &str) -> Option<&'static Spec> {
    SPECS
        .iter()
        .find(|spec| spec.key == key || spec.aliases.iter().any(|alias| alias.key == key))
}

/// A key binding Lodestone doesn't list (one a mod adds): shared only with instances
/// whose `options.txt` already has it.
pub fn is_other_binding(key: &str) -> bool {
    key.starts_with("key_") && find(key).is_none()
}

/// The made-up data version of a release before 1.10, which has none in
/// `options.txt`: 1.9.x → 169 (its real one), 1.8.x → 99, 1.7.x → 50, older → 0.
/// `None` for 1.10 and later, snapshots and anything else.
pub fn legacy_data_version(game_version: &str) -> Option<u32> {
    let mut parts = game_version.split('.');
    let (Some("1"), Some(minor)) = (parts.next(), parts.next()) else {
        return None;
    };
    match minor.parse::<u32>().ok()? {
        0..=6 => Some(0),
        7 => Some(V1_7_2),
        8 => Some(V1_8),
        9 => Some(V1_9),
        _ => None,
    }
}

/// The data versions of the releases from 1.10 to 1.13.2, whose client jars have no
/// `version.json` (minecraft.wiki "Data version").
pub fn release_data_version(game_version: &str) -> Option<u32> {
    Some(match game_version {
        "1.10" => 510,
        "1.10.1" => 511,
        "1.10.2" => 512,
        "1.11" => 819,
        "1.11.1" => 921,
        "1.11.2" => 922,
        "1.12" => 1139,
        "1.12.1" => 1241,
        "1.12.2" => 1343,
        "1.13" => 1519,
        "1.13.1" => 1628,
        "1.13.2" => 1631,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_none_is_excluded() {
        for (i, spec) in SPECS.iter().enumerate() {
            for form in spec.forms() {
                assert!(!EXCLUDED.contains(&form.key), "{}", form.key);
                for other in &SPECS[i + 1..] {
                    assert!(other.forms().all(|f| f.key != form.key), "{}", form.key);
                }
                assert!(form.since < form.until, "{}", form.key);
            }
        }
    }

    #[test]
    fn graphics_has_one_key_at_each_version() {
        let graphics = find("graphicsMode").unwrap();
        assert_eq!(graphics.key, "graphicsPreset");
        for dv in [0, V1_8, 1343, 2230, V1_16, 3465, 4554, V1_21_11, V26_3] {
            assert_eq!(
                graphics.forms().filter(|f| f.exists_at(dv)).count(),
                1,
                "{dv}"
            );
        }
    }

    #[test]
    fn legacy_versions() {
        assert_eq!(legacy_data_version("1.8.9"), Some(V1_8));
        assert_eq!(legacy_data_version("1.7.10"), Some(V1_7_2));
        assert_eq!(legacy_data_version("1.9.4"), Some(V1_9));
        assert_eq!(legacy_data_version("1.10"), None);
        assert_eq!(legacy_data_version("1.20.1"), None);
        assert_eq!(legacy_data_version("26.1"), None);
        assert_eq!(legacy_data_version("15w32a"), None);
        assert_eq!(release_data_version("1.12.2"), Some(1343));
        assert_eq!(release_data_version("1.14"), None);
    }
}
