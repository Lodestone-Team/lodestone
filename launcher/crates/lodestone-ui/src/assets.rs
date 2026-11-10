//! Images, icons and fonts, embedded in the binary from `launcher/assets`.

use std::borrow::Cow;

use gpui::{App, AssetSource, Result, SharedString};

macro_rules! embed {
    ($($path:literal),* $(,)?) => {
        &[$(($path, include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/", $path)).as_slice())),*]
    };
}

/// Every embedded image and icon, by its path under `launcher/assets`.
static FILES: &[(&str, &[u8])] = embed!(
    "images/hero.svg",
    "images/file-icons/3d.svg",
    "images/file-icons/binary.svg",
    "images/file-icons/console.svg",
    "images/file-icons/document.svg",
    "images/file-icons/exe.svg",
    "images/file-icons/file.svg",
    "images/file-icons/folder-blue.svg",
    "images/file-icons/image.svg",
    "images/file-icons/jar.svg",
    "images/file-icons/java.svg",
    "images/file-icons/javaclass.svg",
    "images/file-icons/javascript.svg",
    "images/file-icons/json.svg",
    "images/file-icons/lock.svg",
    "images/file-icons/lodestone.svg",
    "images/file-icons/log.svg",
    "images/file-icons/minecraft.svg",
    "images/file-icons/pwsh.svg",
    "images/file-icons/readme.svg",
    "images/file-icons/typescript.svg",
    "images/file-icons/yaml.svg",
    "images/file-icons/zip.svg",
    "images/logo.svg",
    "images/host_logo.svg",
    "images/lodestone_mark.svg",
    "images/logo_light.svg",
    "images/instance-icons/amethyst.png",
    "images/instance-icons/apple.png",
    "images/instance-icons/bricks.png",
    "images/instance-icons/cobblestone.png",
    "images/instance-icons/copper.png",
    "images/instance-icons/diamond-ore.png",
    "images/instance-icons/dirt.png",
    "images/instance-icons/emerald-ore.png",
    "images/instance-icons/end-stone.png",
    "images/instance-icons/gem.png",
    "images/instance-icons/glowstone.png",
    "images/instance-icons/gold-ore.png",
    "images/instance-icons/grass.png",
    "images/instance-icons/gravel.png",
    "images/instance-icons/heart.png",
    "images/instance-icons/ice.png",
    "images/instance-icons/leaves.png",
    "images/instance-icons/lodestone.png",
    "images/instance-icons/log.png",
    "images/instance-icons/netherrack.png",
    "images/instance-icons/obsidian.png",
    "images/instance-icons/pickaxe.png",
    "images/instance-icons/planks.png",
    "images/instance-icons/potion.png",
    "images/instance-icons/pumpkin.png",
    "images/instance-icons/redstone-ore.png",
    "images/instance-icons/sand.png",
    "images/instance-icons/snow.png",
    "images/instance-icons/star.png",
    "images/instance-icons/stone.png",
    "images/instance-icons/sword.png",
    "images/instance-icons/torch.png",
    "icons/arrow-down.svg",
    "icons/arrow-left.svg",
    "icons/arrow-up.svg",
    "icons/arrow-up-right-from-square.svg",
    "icons/bars.svg",
    "icons/caret-down.svg",
    "icons/check.svg",
    "icons/circle-up.svg",
    "icons/copy.svg",
    "icons/ellipsis.svg",
    "icons/cube.svg",
    "icons/download.svg",
    "icons/expand.svg",
    "icons/gear.svg",
    "icons/hard-drive.svg",
    "icons/palette.svg",
    "icons/magnifying-glass.svg",
    "icons/plus.svg",
    "icons/right-from-bracket.svg",
    "icons/square-plus.svg",
    "icons/trash-can.svg",
    "icons/triangle-exclamation.svg",
    "icons/upload.svg",
    "icons/user.svg",
    "icons/window-close.svg",
    "icons/window-maximize.svg",
    "icons/window-minimize.svg",
    "icons/window-restore.svg",
    "icons/xmark.svg",
    "icons/bell.svg",
    "icons/link.svg",
    "icons/user-group.svg",
    "icons/chevron-down.svg",
    "icons/chevron-right.svg",
    "icons/chevron-left.svg",
    "icons/pen.svg",
    "icons/floppy-disk.svg",
    "icons/circle-user.svg",
    "images/games/minecraft-vanilla.png",
    "images/games/minecraft-fabric.png",
    "images/games/minecraft-forge.png",
    "images/games/minecraft-paper.png",
    "images/games/minecraft-missing-texture.svg",
    "images/games/generic.svg",
    "icons/circle-info.svg",
    "icons/circle-check.svg",
    "icons/circle-exclamation.svg",
    "icons/server.svg",
    "icons/folder.svg",
    "icons/folder-plus.svg",
    "icons/file.svg",
    "icons/file-pen.svg",
    "icons/file-circle-plus.svg",
    "icons/arrows-rotate.svg",
    "icons/scissors.svg",
    "icons/paste.svg",
    "icons/file-zipper.svg",
    "icons/list-check.svg",
    "icons/clipboard-question.svg",
    "icons/chart-line.svg",
    "icons/puzzle-piece.svg",
    "icons/code-compare.svg",
    "icons/inbox.svg",
    "icons/play.svg",
    "icons/power-off.svg",
    // Modrinth's category icons, for the Content tab's filters.
    "images/categories/adventure.svg",
    "images/categories/any.svg",
    "images/categories/atmosphere.svg",
    "images/categories/audio.svg",
    "images/categories/blocks.svg",
    "images/categories/bloom.svg",
    "images/categories/cartoon.svg",
    "images/categories/client.svg",
    "images/categories/colored-lighting.svg",
    "images/categories/combat.svg",
    "images/categories/core-shaders.svg",
    "images/categories/cursed.svg",
    "images/categories/decoration.svg",
    "images/categories/economy.svg",
    "images/categories/entities.svg",
    "images/categories/environment.svg",
    "images/categories/equipment-pack.svg",
    "images/categories/equipment.svg",
    "images/categories/fantasy.svg",
    "images/categories/foliage.svg",
    "images/categories/fonts.svg",
    "images/categories/food.svg",
    "images/categories/game-mechanics.svg",
    "images/categories/gui.svg",
    "images/categories/high.svg",
    "images/categories/items.svg",
    "images/categories/library.svg",
    "images/categories/locale.svg",
    "images/categories/low.svg",
    "images/categories/magic.svg",
    "images/categories/management.svg",
    "images/categories/medium.svg",
    "images/categories/minigame.svg",
    "images/categories/mobs.svg",
    "images/categories/modded.svg",
    "images/categories/models.svg",
    "images/categories/open-source.svg",
    "images/categories/optimization.svg",
    "images/categories/path-tracing.svg",
    "images/categories/pbr.svg",
    "images/categories/potato.svg",
    "images/categories/realistic-shader.svg",
    "images/categories/realistic.svg",
    "images/categories/reflections.svg",
    "images/categories/screenshot.svg",
    "images/categories/semi-realistic.svg",
    "images/categories/server.svg",
    "images/categories/shadows.svg",
    "images/categories/simplistic.svg",
    "images/categories/social.svg",
    "images/categories/storage.svg",
    "images/categories/technology.svg",
    "images/categories/themed.svg",
    "images/categories/transportation.svg",
    "images/categories/tweaks.svg",
    "images/categories/utility.svg",
    "images/categories/vanilla-like.svg",
    "images/categories/worldgen.svg",
);

/// Every embedded font, by its path under `launcher/assets/fonts` (generated by `build.rs`).
static FONTS: &[(&str, &[u8])] = include!(concat!(env!("OUT_DIR"), "/fonts.rs"));

/// The asset source passed to `Application::with_assets`.
pub struct Assets;

impl Assets {
    /// An embedded file's bytes, without going through GPUI.
    pub fn embedded(path: &str) -> Option<&'static [u8]> {
        FILES
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| *bytes)
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(FILES
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(FILES
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| SharedString::from(*name))
            .collect())
    }
}

/// Registers the embedded fonts with GPUI's text system.
///
/// Fonts are passed as borrowed slices: on macOS GPUI then loads them through Core
/// Graphics, which is also the path Zed uses for its bundled fonts.
pub fn load_fonts(cx: &App) -> Result<()> {
    let fonts = FONTS
        .iter()
        .map(|(_, bytes)| Cow::Borrowed(*bytes))
        .collect();
    cx.text_system().add_fonts(fonts)
}

/// The paths of the embedded fonts, for logging and tests.
pub fn font_files() -> impl Iterator<Item = &'static str> {
    FONTS.iter().map(|(name, _)| *name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_embedded() {
        for icon in crate::components::Icon::ALL {
            let data = Assets.load(icon.path()).unwrap();
            assert!(data.is_some_and(|data| !data.is_empty()), "{icon:?}");
        }
    }

    #[test]
    fn lists_by_prefix() {
        let icons = Assets.list("icons/").unwrap();
        assert_eq!(icons.len(), crate::components::Icon::ALL.len());
        assert!(Assets.load("icons/missing.svg").unwrap().is_none());
    }

    #[test]
    fn embeds_each_font_family() {
        let files: Vec<_> = font_files().collect();
        for family in ["satoshi/", "chillax/", "clash-grotesk/", "jetbrains-mono/"] {
            for face in ["Regular", "Medium", "Bold"] {
                assert!(
                    files
                        .iter()
                        .any(|file| file.starts_with(family) && file.contains(&format!("-{face}."))),
                    "{family} has no {face} face in {files:?}"
                );
            }
        }
    }
}
