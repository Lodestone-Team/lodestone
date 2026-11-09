//! Localized presentation of shared game settings. Canonical values remain in core.
use crate::i18n::{SharedOptionsMsg as M, SharedOptionsPlural as P, t, tr, trn};
use launcher_core::shared_options::Row;

/// Translate known options; mod-provided bindings keep their original identifier.
pub fn label(row: &Row) -> String {
    option_message(&row.key).map_or_else(|| row.label.clone(), |message| t(message).to_string())
}

fn option_message(key: &str) -> Option<M> {
    Some(match key {
        "mouseSensitivity" => M::OptionMouseSensitivity,
        "invertYMouse" => M::OptionInvertYMouse,
        "invertXMouse" => M::OptionInvertXMouse,
        "rawMouseInput" => M::OptionRawMouseInput,
        "mouseWheelSensitivity" => M::OptionMouseWheelSensitivity,
        "discrete_mouse_scroll" => M::OptionDiscreteMouseScroll,
        "toggleCrouch" => M::OptionToggleCrouch,
        "toggleSprint" => M::OptionToggleSprint,
        "toggleAttack" => M::OptionToggleAttack,
        "toggleUse" => M::OptionToggleUse,
        "sprintWindow" => M::OptionSprintWindow,
        "autoJump" => M::OptionAutoJump,
        "key_key.attack" => M::OptionKeyKeyAttack,
        "key_key.use" => M::OptionKeyKeyUse,
        "key_key.pickItem" => M::OptionKeyKeyPickItem,
        "key_key.forward" => M::OptionKeyKeyForward,
        "key_key.left" => M::OptionKeyKeyLeft,
        "key_key.back" => M::OptionKeyKeyBack,
        "key_key.right" => M::OptionKeyKeyRight,
        "key_key.jump" => M::OptionKeyKeyJump,
        "key_key.sneak" => M::OptionKeyKeySneak,
        "key_key.sprint" => M::OptionKeyKeySprint,
        "key_key.drop" => M::OptionKeyKeyDrop,
        "key_key.inventory" => M::OptionKeyKeyInventory,
        "key_key.swapOffhand" => M::OptionKeyKeySwapOffhand,
        "key_key.chat" => M::OptionKeyKeyChat,
        "key_key.command" => M::OptionKeyKeyCommand,
        "key_key.playerlist" => M::OptionKeyKeyPlayerlist,
        "key_key.socialInteractions" => M::OptionKeyKeySocialInteractions,
        "key_key.friends" => M::OptionKeyKeyFriends,
        "key_key.advancements" => M::OptionKeyKeyAdvancements,
        "key_key.quickActions" => M::OptionKeyKeyQuickActions,
        "key_key.screenshot" => M::OptionKeyKeyScreenshot,
        "key_key.togglePerspective" => M::OptionKeyKeyTogglePerspective,
        "key_key.smoothCamera" => M::OptionKeyKeySmoothCamera,
        "key_key.fullscreen" => M::OptionKeyKeyFullscreen,
        "key_key.spectatorOutlines" => M::OptionKeyKeySpectatorOutlines,
        "key_key.spectatorHotbar" => M::OptionKeyKeySpectatorHotbar,
        "key_key.toggleGui" => M::OptionKeyKeyToggleGui,
        "key_key.toggleSpectatorShaderEffects" => M::OptionKeyKeyToggleSpectatorShaderEffects,
        "key_key.saveToolbarActivator" => M::OptionKeyKeySaveToolbarActivator,
        "key_key.loadToolbarActivator" => M::OptionKeyKeyLoadToolbarActivator,
        "key_key.hotbar.1" => M::OptionKeyKeyHotbar1,
        "key_key.hotbar.2" => M::OptionKeyKeyHotbar2,
        "key_key.hotbar.3" => M::OptionKeyKeyHotbar3,
        "key_key.hotbar.4" => M::OptionKeyKeyHotbar4,
        "key_key.hotbar.5" => M::OptionKeyKeyHotbar5,
        "key_key.hotbar.6" => M::OptionKeyKeyHotbar6,
        "key_key.hotbar.7" => M::OptionKeyKeyHotbar7,
        "key_key.hotbar.8" => M::OptionKeyKeyHotbar8,
        "key_key.hotbar.9" => M::OptionKeyKeyHotbar9,
        "key_key.streamStartStop" => M::OptionKeyKeyStreamStartStop,
        "key_key.streamPauseUnpause" => M::OptionKeyKeyStreamPauseUnpause,
        "key_key.streamCommercial" => M::OptionKeyKeyStreamCommercial,
        "key_key.streamToggleMic" => M::OptionKeyKeyStreamToggleMic,
        "key_key.debug.overlay" => M::OptionKeyKeyDebugOverlay,
        "key_key.debug.modifier" => M::OptionKeyKeyDebugModifier,
        "fov" => M::OptionFov,
        "renderDistance" => M::OptionRenderDistance,
        "simulationDistance" => M::OptionSimulationDistance,
        "guiScale" => M::OptionGuiScale,
        "gamma" => M::OptionGamma,
        "fullscreen" => M::OptionFullscreen,
        "enableVsync" => M::OptionEnableVsync,
        "maxFps" => M::OptionMaxFps,
        "particles" => M::OptionParticles,
        "graphicsPreset" => M::OptionGraphicsPreset,
        "ao" => M::OptionAo,
        "bobView" => M::OptionBobView,
        "entityShadows" => M::OptionEntityShadows,
        "mipmapLevels" => M::OptionMipmapLevels,
        "biomeBlendRadius" => M::OptionBiomeBlendRadius,
        "entityDistanceScaling" => M::OptionEntityDistanceScaling,
        "cloudRange" => M::OptionCloudRange,
        "improvedTransparency" => M::OptionImprovedTransparency,
        "cutoutLeaves" => M::OptionCutoutLeaves,
        "vignette" => M::OptionVignette,
        "weatherRadius" => M::OptionWeatherRadius,
        "chunkSectionFadeInTime" => M::OptionChunkSectionFadeInTime,
        "textureFiltering" => M::OptionTextureFiltering,
        "maxAnisotropyBit" => M::OptionMaxAnisotropyBit,
        "soundCategory_master" => M::OptionSoundCategoryMaster,
        "soundCategory_music" => M::OptionSoundCategoryMusic,
        "soundCategory_record" => M::OptionSoundCategoryRecord,
        "soundCategory_weather" => M::OptionSoundCategoryWeather,
        "soundCategory_block" => M::OptionSoundCategoryBlock,
        "soundCategory_hostile" => M::OptionSoundCategoryHostile,
        "soundCategory_neutral" => M::OptionSoundCategoryNeutral,
        "soundCategory_player" => M::OptionSoundCategoryPlayer,
        "soundCategory_ambient" => M::OptionSoundCategoryAmbient,
        "soundCategory_voice" => M::OptionSoundCategoryVoice,
        "soundCategory_ui" => M::OptionSoundCategoryUi,
        "showSubtitles" => M::OptionShowSubtitles,
        "directionalAudio" => M::OptionDirectionalAudio,
        "narrator" => M::OptionNarrator,
        "highContrast" => M::OptionHighContrast,
        "highContrastBlockOutline" => M::OptionHighContrastBlockOutline,
        "textBackgroundOpacity" => M::OptionTextBackgroundOpacity,
        "backgroundForChatOnly" => M::OptionBackgroundForChatOnly,
        "chatVisibility" => M::OptionChatVisibility,
        "chatColors" => M::OptionChatColors,
        "chatLinks" => M::OptionChatLinks,
        "chatLinksPrompt" => M::OptionChatLinksPrompt,
        "chatOpacity" => M::OptionChatOpacity,
        "chatScale" => M::OptionChatScale,
        "chatWidth" => M::OptionChatWidth,
        "chatHeightFocused" => M::OptionChatHeightFocused,
        "chatHeightUnfocused" => M::OptionChatHeightUnfocused,
        "chatLineSpacing" => M::OptionChatLineSpacing,
        "chatDelay" => M::OptionChatDelay,
        "autoSuggestions" => M::OptionAutoSuggestions,
        "hideMatchedNames" => M::OptionHideMatchedNames,
        "onlyShowSecureChat" => M::OptionOnlyShowSecureChat,
        "saveChatDrafts" => M::OptionSaveChatDrafts,
        "notificationDisplayTime" => M::OptionNotificationDisplayTime,
        "screenEffectScale" => M::OptionScreenEffectScale,
        "fovEffectScale" => M::OptionFovEffectScale,
        "darknessEffectScale" => M::OptionDarknessEffectScale,
        "damageTiltStrength" => M::OptionDamageTiltStrength,
        "glintSpeed" => M::OptionGlintSpeed,
        "glintStrength" => M::OptionGlintStrength,
        "hideLightningFlashes" => M::OptionHideLightningFlashes,
        "darkMojangStudiosBackground" => M::OptionDarkMojangStudiosBackground,
        "menuBackgroundBlurriness" => M::OptionMenuBackgroundBlurriness,
        "panoramaScrollSpeed" => M::OptionPanoramaScrollSpeed,
        "hideSplashTexts" => M::OptionHideSplashTexts,
        "narratorHotkey" => M::OptionNarratorHotkey,
        "onboardAccessibility" => M::OptionOnboardAccessibility,
        "rotateWithMinecart" => M::OptionRotateWithMinecart,
        "allowCursorChanges" => M::OptionAllowCursorChanges,
        "lang" => M::OptionLang,
        "forceUnicodeFont" => M::OptionForceUnicodeFont,
        "japaneseGlyphVariants" => M::OptionJapaneseGlyphVariants,
        _ => return None,
    })
}

/// Translate a known binding, or show the canonical identifier if not in vanilla.
fn binding(value: &str) -> String {
    let message = match value {
        "key.keyboard.ac.back" => M::BindingKeyKeyboardAcBack,
        "key.keyboard.ac.bookmarks" => M::BindingKeyKeyboardAcBookmarks,
        "key.keyboard.ac.close" => M::BindingKeyKeyboardAcClose,
        "key.keyboard.ac.exit" => M::BindingKeyKeyboardAcExit,
        "key.keyboard.ac.forward" => M::BindingKeyKeyboardAcForward,
        "key.keyboard.ac.home" => M::BindingKeyKeyboardAcHome,
        "key.keyboard.ac.new" => M::BindingKeyKeyboardAcNew,
        "key.keyboard.ac.open" => M::BindingKeyKeyboardAcOpen,
        "key.keyboard.ac.print" => M::BindingKeyKeyboardAcPrint,
        "key.keyboard.ac.properties" => M::BindingKeyKeyboardAcProperties,
        "key.keyboard.ac.refresh" => M::BindingKeyKeyboardAcRefresh,
        "key.keyboard.ac.save" => M::BindingKeyKeyboardAcSave,
        "key.keyboard.ac.search" => M::BindingKeyKeyboardAcSearch,
        "key.keyboard.ac.stop" => M::BindingKeyKeyboardAcStop,
        "key.keyboard.again" => M::BindingKeyKeyboardAgain,
        "key.keyboard.alternate.erase" => M::BindingKeyKeyboardAlternateErase,
        "key.keyboard.apostrophe" => M::BindingKeyKeyboardApostrophe,
        "key.keyboard.application" => M::BindingKeyKeyboardApplication,
        "key.keyboard.backslash" => M::BindingKeyKeyboardBackslash,
        "key.keyboard.backspace" => M::BindingKeyKeyboardBackspace,
        "key.keyboard.call" => M::BindingKeyKeyboardCall,
        "key.keyboard.cancel" => M::BindingKeyKeyboardCancel,
        "key.keyboard.caps.lock" => M::BindingKeyKeyboardCapsLock,
        "key.keyboard.channel.down" => M::BindingKeyKeyboardChannelDown,
        "key.keyboard.channel.up" => M::BindingKeyKeyboardChannelUp,
        "key.keyboard.clear" => M::BindingKeyKeyboardClear,
        "key.keyboard.clear.again" => M::BindingKeyKeyboardClearAgain,
        "key.keyboard.comma" => M::BindingKeyKeyboardComma,
        "key.keyboard.copy" => M::BindingKeyKeyboardCopy,
        "key.keyboard.crsel" => M::BindingKeyKeyboardCrsel,
        "key.keyboard.currency.subunit" => M::BindingKeyKeyboardCurrencySubunit,
        "key.keyboard.currency.unit" => M::BindingKeyKeyboardCurrencyUnit,
        "key.keyboard.cut" => M::BindingKeyKeyboardCut,
        "key.keyboard.decimal.separator" => M::BindingKeyKeyboardDecimalSeparator,
        "key.keyboard.delete" => M::BindingKeyKeyboardDelete,
        "key.keyboard.down" => M::BindingKeyKeyboardDown,
        "key.keyboard.end" => M::BindingKeyKeyboardEnd,
        "key.keyboard.end.call" => M::BindingKeyKeyboardEndCall,
        "key.keyboard.enter" => M::BindingKeyKeyboardEnter,
        "key.keyboard.enter2" => M::BindingKeyKeyboardEnter2,
        "key.keyboard.equal" => M::BindingKeyKeyboardEqual,
        "key.keyboard.escape" => M::BindingKeyKeyboardEscape,
        "key.keyboard.execute" => M::BindingKeyKeyboardExecute,
        "key.keyboard.exsel" => M::BindingKeyKeyboardExsel,
        "key.keyboard.f1" => M::BindingKeyKeyboardF1,
        "key.keyboard.f10" => M::BindingKeyKeyboardF10,
        "key.keyboard.f11" => M::BindingKeyKeyboardF11,
        "key.keyboard.f12" => M::BindingKeyKeyboardF12,
        "key.keyboard.f13" => M::BindingKeyKeyboardF13,
        "key.keyboard.f14" => M::BindingKeyKeyboardF14,
        "key.keyboard.f15" => M::BindingKeyKeyboardF15,
        "key.keyboard.f16" => M::BindingKeyKeyboardF16,
        "key.keyboard.f17" => M::BindingKeyKeyboardF17,
        "key.keyboard.f18" => M::BindingKeyKeyboardF18,
        "key.keyboard.f19" => M::BindingKeyKeyboardF19,
        "key.keyboard.f2" => M::BindingKeyKeyboardF2,
        "key.keyboard.f20" => M::BindingKeyKeyboardF20,
        "key.keyboard.f21" => M::BindingKeyKeyboardF21,
        "key.keyboard.f22" => M::BindingKeyKeyboardF22,
        "key.keyboard.f23" => M::BindingKeyKeyboardF23,
        "key.keyboard.f24" => M::BindingKeyKeyboardF24,
        "key.keyboard.f25" => M::BindingKeyKeyboardF25,
        "key.keyboard.f3" => M::BindingKeyKeyboardF3,
        "key.keyboard.f4" => M::BindingKeyKeyboardF4,
        "key.keyboard.f5" => M::BindingKeyKeyboardF5,
        "key.keyboard.f6" => M::BindingKeyKeyboardF6,
        "key.keyboard.f7" => M::BindingKeyKeyboardF7,
        "key.keyboard.f8" => M::BindingKeyKeyboardF8,
        "key.keyboard.f9" => M::BindingKeyKeyboardF9,
        "key.keyboard.find" => M::BindingKeyKeyboardFind,
        "key.keyboard.grave.accent" => M::BindingKeyKeyboardGraveAccent,
        "key.keyboard.help" => M::BindingKeyKeyboardHelp,
        "key.keyboard.home" => M::BindingKeyKeyboardHome,
        "key.keyboard.insert" => M::BindingKeyKeyboardInsert,
        "key.keyboard.international1" => M::BindingKeyKeyboardInternational1,
        "key.keyboard.international2" => M::BindingKeyKeyboardInternational2,
        "key.keyboard.international3" => M::BindingKeyKeyboardInternational3,
        "key.keyboard.international4" => M::BindingKeyKeyboardInternational4,
        "key.keyboard.international5" => M::BindingKeyKeyboardInternational5,
        "key.keyboard.international6" => M::BindingKeyKeyboardInternational6,
        "key.keyboard.international7" => M::BindingKeyKeyboardInternational7,
        "key.keyboard.international8" => M::BindingKeyKeyboardInternational8,
        "key.keyboard.international9" => M::BindingKeyKeyboardInternational9,
        "key.keyboard.keypad.0" => M::BindingKeyKeyboardKeypad0,
        "key.keyboard.keypad.00" => M::BindingKeyKeyboardKeypad00,
        "key.keyboard.keypad.000" => M::BindingKeyKeyboardKeypad000,
        "key.keyboard.keypad.1" => M::BindingKeyKeyboardKeypad1,
        "key.keyboard.keypad.2" => M::BindingKeyKeyboardKeypad2,
        "key.keyboard.keypad.3" => M::BindingKeyKeyboardKeypad3,
        "key.keyboard.keypad.4" => M::BindingKeyKeyboardKeypad4,
        "key.keyboard.keypad.5" => M::BindingKeyKeyboardKeypad5,
        "key.keyboard.keypad.6" => M::BindingKeyKeyboardKeypad6,
        "key.keyboard.keypad.7" => M::BindingKeyKeyboardKeypad7,
        "key.keyboard.keypad.8" => M::BindingKeyKeyboardKeypad8,
        "key.keyboard.keypad.9" => M::BindingKeyKeyboardKeypad9,
        "key.keyboard.keypad.a" => M::BindingKeyKeyboardKeypadA,
        "key.keyboard.keypad.add" => M::BindingKeyKeyboardKeypadAdd,
        "key.keyboard.keypad.ampersand" => M::BindingKeyKeyboardKeypadAmpersand,
        "key.keyboard.keypad.at" => M::BindingKeyKeyboardKeypadAt,
        "key.keyboard.keypad.b" => M::BindingKeyKeyboardKeypadB,
        "key.keyboard.keypad.backspace" => M::BindingKeyKeyboardKeypadBackspace,
        "key.keyboard.keypad.binary" => M::BindingKeyKeyboardKeypadBinary,
        "key.keyboard.keypad.c" => M::BindingKeyKeyboardKeypadC,
        "key.keyboard.keypad.clear" => M::BindingKeyKeyboardKeypadClear,
        "key.keyboard.keypad.clear.entry" => M::BindingKeyKeyboardKeypadClearEntry,
        "key.keyboard.keypad.colon" => M::BindingKeyKeyboardKeypadColon,
        "key.keyboard.keypad.comma" => M::BindingKeyKeyboardKeypadComma,
        "key.keyboard.keypad.d" => M::BindingKeyKeyboardKeypadD,
        "key.keyboard.keypad.decimal" => M::BindingKeyKeyboardKeypadDecimal,
        "key.keyboard.keypad.divide" => M::BindingKeyKeyboardKeypadDivide,
        "key.keyboard.keypad.double.ampersand" => M::BindingKeyKeyboardKeypadDoubleAmpersand,
        "key.keyboard.keypad.double.vertical.bar" => M::BindingKeyKeyboardKeypadDoubleVerticalBar,
        "key.keyboard.keypad.e" => M::BindingKeyKeyboardKeypadE,
        "key.keyboard.keypad.enter" => M::BindingKeyKeyboardKeypadEnter,
        "key.keyboard.keypad.equal" => M::BindingKeyKeyboardKeypadEqual,
        "key.keyboard.keypad.equals.as400" => M::BindingKeyKeyboardKeypadEqualsAs400,
        "key.keyboard.keypad.exclamation" => M::BindingKeyKeyboardKeypadExclamation,
        "key.keyboard.keypad.f" => M::BindingKeyKeyboardKeypadF,
        "key.keyboard.keypad.greater" => M::BindingKeyKeyboardKeypadGreater,
        "key.keyboard.keypad.hash" => M::BindingKeyKeyboardKeypadHash,
        "key.keyboard.keypad.hexadecimal" => M::BindingKeyKeyboardKeypadHexadecimal,
        "key.keyboard.keypad.left.brace" => M::BindingKeyKeyboardKeypadLeftBrace,
        "key.keyboard.keypad.left.parenthesis" => M::BindingKeyKeyboardKeypadLeftParenthesis,
        "key.keyboard.keypad.less" => M::BindingKeyKeyboardKeypadLess,
        "key.keyboard.keypad.memory.add" => M::BindingKeyKeyboardKeypadMemoryAdd,
        "key.keyboard.keypad.memory.clear" => M::BindingKeyKeyboardKeypadMemoryClear,
        "key.keyboard.keypad.memory.divide" => M::BindingKeyKeyboardKeypadMemoryDivide,
        "key.keyboard.keypad.memory.multiply" => M::BindingKeyKeyboardKeypadMemoryMultiply,
        "key.keyboard.keypad.memory.recall" => M::BindingKeyKeyboardKeypadMemoryRecall,
        "key.keyboard.keypad.memory.store" => M::BindingKeyKeyboardKeypadMemoryStore,
        "key.keyboard.keypad.memory.subtract" => M::BindingKeyKeyboardKeypadMemorySubtract,
        "key.keyboard.keypad.multiply" => M::BindingKeyKeyboardKeypadMultiply,
        "key.keyboard.keypad.octal" => M::BindingKeyKeyboardKeypadOctal,
        "key.keyboard.keypad.percent" => M::BindingKeyKeyboardKeypadPercent,
        "key.keyboard.keypad.period" => M::BindingKeyKeyboardKeypadPeriod,
        "key.keyboard.keypad.plus.minus" => M::BindingKeyKeyboardKeypadPlusMinus,
        "key.keyboard.keypad.power" => M::BindingKeyKeyboardKeypadPower,
        "key.keyboard.keypad.right.brace" => M::BindingKeyKeyboardKeypadRightBrace,
        "key.keyboard.keypad.right.parenthesis" => M::BindingKeyKeyboardKeypadRightParenthesis,
        "key.keyboard.keypad.space" => M::BindingKeyKeyboardKeypadSpace,
        "key.keyboard.keypad.subtract" => M::BindingKeyKeyboardKeypadSubtract,
        "key.keyboard.keypad.tab" => M::BindingKeyKeyboardKeypadTab,
        "key.keyboard.keypad.vertical.bar" => M::BindingKeyKeyboardKeypadVerticalBar,
        "key.keyboard.keypad.xor" => M::BindingKeyKeyboardKeypadXor,
        "key.keyboard.lang1" => M::BindingKeyKeyboardLang1,
        "key.keyboard.lang2" => M::BindingKeyKeyboardLang2,
        "key.keyboard.lang3" => M::BindingKeyKeyboardLang3,
        "key.keyboard.lang4" => M::BindingKeyKeyboardLang4,
        "key.keyboard.lang5" => M::BindingKeyKeyboardLang5,
        "key.keyboard.lang6" => M::BindingKeyKeyboardLang6,
        "key.keyboard.lang7" => M::BindingKeyKeyboardLang7,
        "key.keyboard.lang8" => M::BindingKeyKeyboardLang8,
        "key.keyboard.lang9" => M::BindingKeyKeyboardLang9,
        "key.keyboard.left" => M::BindingKeyKeyboardLeft,
        "key.keyboard.left.alt" => M::BindingKeyKeyboardLeftAlt,
        "key.keyboard.left.bracket" => M::BindingKeyKeyboardLeftBracket,
        "key.keyboard.left.command" => M::BindingKeyKeyboardLeftCommand,
        "key.keyboard.left.control" => M::BindingKeyKeyboardLeftControl,
        "key.keyboard.left.meta" => M::BindingKeyKeyboardLeftMeta,
        "key.keyboard.left.option" => M::BindingKeyKeyboardLeftOption,
        "key.keyboard.left.shift" => M::BindingKeyKeyboardLeftShift,
        "key.keyboard.left.win" => M::BindingKeyKeyboardLeftWin,
        "key.keyboard.left.windows" => M::BindingKeyKeyboardLeftWindows,
        "key.keyboard.media.eject" => M::BindingKeyKeyboardMediaEject,
        "key.keyboard.media.fast.forward" => M::BindingKeyKeyboardMediaFastForward,
        "key.keyboard.media.next.track" => M::BindingKeyKeyboardMediaNextTrack,
        "key.keyboard.media.pause" => M::BindingKeyKeyboardMediaPause,
        "key.keyboard.media.play" => M::BindingKeyKeyboardMediaPlay,
        "key.keyboard.media.play.pause" => M::BindingKeyKeyboardMediaPlayPause,
        "key.keyboard.media.previous.track" => M::BindingKeyKeyboardMediaPreviousTrack,
        "key.keyboard.media.record" => M::BindingKeyKeyboardMediaRecord,
        "key.keyboard.media.rewind" => M::BindingKeyKeyboardMediaRewind,
        "key.keyboard.media.select" => M::BindingKeyKeyboardMediaSelect,
        "key.keyboard.media.stop" => M::BindingKeyKeyboardMediaStop,
        "key.keyboard.menu" => M::BindingKeyKeyboardMenu,
        "key.keyboard.minus" => M::BindingKeyKeyboardMinus,
        "key.keyboard.mode" => M::BindingKeyKeyboardMode,
        "key.keyboard.mute" => M::BindingKeyKeyboardMute,
        "key.keyboard.new" => M::BindingKeyKeyboardNew,
        "key.keyboard.num.lock" => M::BindingKeyKeyboardNumLock,
        "key.keyboard.oper" => M::BindingKeyKeyboardOper,
        "key.keyboard.out" => M::BindingKeyKeyboardOut,
        "key.keyboard.page.down" => M::BindingKeyKeyboardPageDown,
        "key.keyboard.page.up" => M::BindingKeyKeyboardPageUp,
        "key.keyboard.paste" => M::BindingKeyKeyboardPaste,
        "key.keyboard.pause" => M::BindingKeyKeyboardPause,
        "key.keyboard.period" => M::BindingKeyKeyboardPeriod,
        "key.keyboard.power" => M::BindingKeyKeyboardPower,
        "key.keyboard.print.screen" => M::BindingKeyKeyboardPrintScreen,
        "key.keyboard.prior" => M::BindingKeyKeyboardPrior,
        "key.keyboard.right" => M::BindingKeyKeyboardRight,
        "key.keyboard.right.alt" => M::BindingKeyKeyboardRightAlt,
        "key.keyboard.right.bracket" => M::BindingKeyKeyboardRightBracket,
        "key.keyboard.right.command" => M::BindingKeyKeyboardRightCommand,
        "key.keyboard.right.control" => M::BindingKeyKeyboardRightControl,
        "key.keyboard.right.meta" => M::BindingKeyKeyboardRightMeta,
        "key.keyboard.right.option" => M::BindingKeyKeyboardRightOption,
        "key.keyboard.right.shift" => M::BindingKeyKeyboardRightShift,
        "key.keyboard.right.win" => M::BindingKeyKeyboardRightWin,
        "key.keyboard.right.windows" => M::BindingKeyKeyboardRightWindows,
        "key.keyboard.save" => M::BindingKeyKeyboardSave,
        "key.keyboard.scroll.lock" => M::BindingKeyKeyboardScrollLock,
        "key.keyboard.select" => M::BindingKeyKeyboardSelect,
        "key.keyboard.semicolon" => M::BindingKeyKeyboardSemicolon,
        "key.keyboard.separator" => M::BindingKeyKeyboardSeparator,
        "key.keyboard.slash" => M::BindingKeyKeyboardSlash,
        "key.keyboard.sleep" => M::BindingKeyKeyboardSleep,
        "key.keyboard.soft.left" => M::BindingKeyKeyboardSoftLeft,
        "key.keyboard.soft.right" => M::BindingKeyKeyboardSoftRight,
        "key.keyboard.space" => M::BindingKeyKeyboardSpace,
        "key.keyboard.stop" => M::BindingKeyKeyboardStop,
        "key.keyboard.sys.req" => M::BindingKeyKeyboardSysReq,
        "key.keyboard.tab" => M::BindingKeyKeyboardTab,
        "key.keyboard.thousands.separator" => M::BindingKeyKeyboardThousandsSeparator,
        "key.keyboard.undo" => M::BindingKeyKeyboardUndo,
        "key.keyboard.unknown" => M::BindingKeyKeyboardUnknown,
        "key.keyboard.up" => M::BindingKeyKeyboardUp,
        "key.keyboard.volume.down" => M::BindingKeyKeyboardVolumeDown,
        "key.keyboard.volume.up" => M::BindingKeyKeyboardVolumeUp,
        "key.keyboard.wake" => M::BindingKeyKeyboardWake,
        "key.keyboard.world.1" => M::BindingKeyKeyboardWorld1,
        "key.keyboard.world.2" => M::BindingKeyKeyboardWorld2,
        "key.mouse.left" => M::BindingKeyMouseLeft,
        "key.mouse.right" => M::BindingKeyMouseRight,
        "key.mouse.middle" => M::BindingKeyMouseMiddle,
        _ => {
            if let Some(number) = value.strip_prefix("key.mouse.") {
                return tr!(M::BindingMouse, number = number);
            }
            if let Some(key) = value.strip_prefix("key.keyboard.")
                && key.len() == 1
                && key.bytes().all(|c| c.is_ascii_alphanumeric())
            {
                return key.to_ascii_uppercase();
            }
            return value.to_string();
        }
    };
    t(message).to_string()
}

/// Format values in the current locale without changing what is stored.
pub fn show(row: &Row, value: &str) -> String {
    let value = value.trim();
    let choice = match (row.key.as_str(), value) {
        (_, "true") => Some(M::ValueOn),
        (_, "false") => Some(M::ValueOff),
        ("graphicsPreset", "fast") => Some(M::ValueFast),
        ("graphicsPreset", "fancy") => Some(M::ValueFancy),
        ("graphicsPreset", "fabulous") => Some(M::ValueFabulous),
        ("graphicsPreset", "custom") => Some(M::ValueCustom),
        ("particles", "0") => Some(M::ValueAll),
        ("particles", "1") => Some(M::ValueDecreased),
        ("particles", "2") => Some(M::ValueMinimal),
        ("narrator", "0") => Some(M::ValueOff),
        ("narrator", "1") => Some(M::ValueNarratorAll),
        ("narrator", "2") => Some(M::ValueNarratorChat),
        ("narrator", "3") => Some(M::ValueNarratorSystem),
        ("chatVisibility", "0") => Some(M::ValueShown),
        ("chatVisibility", "1") => Some(M::ValueCommands),
        ("chatVisibility", "2") => Some(M::ValueHidden),
        ("textureFiltering", "0") => Some(M::ValueNone),
        ("textureFiltering", "1") => Some(M::ValueRgss),
        ("textureFiltering", "2") => Some(M::ValueAnisotropic),
        _ => None,
    };
    if let Some(message) = choice {
        return t(message).to_string();
    }
    if row.key.starts_with("key_") {
        return binding(value);
    }
    if let Ok(n) = value.parse::<f64>() {
        // Older files may spell an integer particle choice as e.g. "1.0".
        if row.key == "particles" {
            let message = match n as i64 {
                0 => Some(M::ValueAll),
                1 => Some(M::ValueDecreased),
                2 => Some(M::ValueMinimal),
                _ => None,
            };
            if let Some(message) = message {
                return t(message).to_string();
            }
        }
        if row.key == "guiScale" && n == 0.0 {
            return t(M::ValueAuto).to_string();
        }
        if row.key == "maxFps" && n >= 260.0 {
            return t(M::ValueUnlimited).to_string();
        }
        if matches!(row.key.as_str(), "renderDistance" | "simulationDistance")
            && n >= 0.0
            && n.is_finite()
        {
            return trn!(P::Chunks, n as u64);
        }
    }
    row.show(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::shared_options::keys::SPECS;
    #[test]
    fn every_known_option_has_a_translation() {
        for spec in SPECS {
            assert!(option_message(spec.key).is_some(), "{}", spec.key);
        }
        assert!(option_message("key_mod.custom").is_none());
    }

    #[test]
    fn decimal_particle_choices_use_the_translated_value() {
        let row = Row {
            key: "particles".into(),
            label: "Particles".into(),
            category: launcher_core::shared_options::Category::Video,
            shared: None,
            local: None,
            overridden: false,
        };
        assert_eq!(show(&row, "1.0"), t(M::ValueDecreased));
    }

    #[test]
    fn vanilla_binding_names_and_plain_letters() {
        assert_eq!(binding("key.keyboard.w"), "W");
        assert_eq!(binding("key.keyboard.4"), "4");
        assert_eq!(binding("scancode.42"), "scancode.42");
        assert!(option_message("renderDistance").is_some());
        assert_eq!(
            crate::i18n::Msg::from(M::OptionRenderDistance).text(crate::i18n::Locale::ZhHans),
            "渲染距离"
        );
    }
}
