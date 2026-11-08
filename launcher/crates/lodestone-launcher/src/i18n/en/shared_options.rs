//! Minecraft shared-option labels and values.
use crate::i18n::messages;

messages! {
    /// Shared settings labels and value names.
    pub enum SharedOptionsMsg {
        /// options.sensitivity.
        OptionMouseSensitivity = "Mouse sensitivity",
        /// options.invertMouseY.
        OptionInvertYMouse = "Invert mouse (vertical)",
        /// options.invertMouseX.
        OptionInvertXMouse = "Invert mouse (horizontal)",
        /// options.rawMouseInput.
        OptionRawMouseInput = "Raw input",
        /// options.mouseWheelSensitivity.
        OptionMouseWheelSensitivity = "Scroll sensitivity",
        /// options.discrete_mouse_scroll.
        OptionDiscreteMouseScroll = "Discrete scrolling",
        /// Launcher-specific wording.
        OptionToggleCrouch = "Toggle crouch",
        /// Launcher-specific wording.
        OptionToggleSprint = "Toggle sprint",
        /// Launcher-specific wording.
        OptionToggleAttack = "Toggle attack",
        /// Launcher-specific wording.
        OptionToggleUse = "Toggle use",
        /// options.sprintWindow.
        OptionSprintWindow = "Sprint window",
        /// options.autoJump.
        OptionAutoJump = "Auto-jump",
        /// key.attack.
        OptionKeyKeyAttack = "Attack/Destroy",
        /// key.use.
        OptionKeyKeyUse = "Use item/Place block",
        /// key.pickItem.
        OptionKeyKeyPickItem = "Pick block",
        /// key.forward.
        OptionKeyKeyForward = "Walk forwards",
        /// key.left.
        OptionKeyKeyLeft = "Strafe left",
        /// key.back.
        OptionKeyKeyBack = "Walk backwards",
        /// key.right.
        OptionKeyKeyRight = "Strafe right",
        /// key.jump.
        OptionKeyKeyJump = "Jump",
        /// key.sneak.
        OptionKeyKeySneak = "Sneak",
        /// key.sprint.
        OptionKeyKeySprint = "Sprint",
        /// key.drop.
        OptionKeyKeyDrop = "Drop selected item",
        /// key.inventory.
        OptionKeyKeyInventory = "Open/Close inventory",
        /// key.swapOffhand.
        OptionKeyKeySwapOffhand = "Swap item with offhand",
        /// key.chat.
        OptionKeyKeyChat = "Open chat",
        /// key.command.
        OptionKeyKeyCommand = "Open command",
        /// key.playerlist.
        OptionKeyKeyPlayerlist = "List players",
        /// key.socialInteractions.
        OptionKeyKeySocialInteractions = "Social interactions",
        /// key.friends.
        OptionKeyKeyFriends = "Friends",
        /// key.advancements.
        OptionKeyKeyAdvancements = "Advancements",
        /// key.quickActions.
        OptionKeyKeyQuickActions = "Quick actions",
        /// key.screenshot.
        OptionKeyKeyScreenshot = "Take screenshot",
        /// key.togglePerspective.
        OptionKeyKeyTogglePerspective = "Toggle perspective",
        /// key.smoothCamera.
        OptionKeyKeySmoothCamera = "Toggle cinematic camera",
        /// key.fullscreen.
        OptionKeyKeyFullscreen = "Toggle fullscreen",
        /// key.spectatorOutlines.
        OptionKeyKeySpectatorOutlines = "Highlight players (spectators)",
        /// key.spectatorHotbar.
        OptionKeyKeySpectatorHotbar = "Spectator hotbar",
        /// key.toggleGui.
        OptionKeyKeyToggleGui = "Toggle HUD",
        /// key.toggleSpectatorShaderEffects.
        OptionKeyKeyToggleSpectatorShaderEffects = "Toggle spectator shader effects",
        /// key.saveToolbarActivator.
        OptionKeyKeySaveToolbarActivator = "Save hotbar activator",
        /// key.loadToolbarActivator.
        OptionKeyKeyLoadToolbarActivator = "Load hotbar activator",
        /// key.hotbar.1.
        OptionKeyKeyHotbar1 = "Hotbar slot 1",
        /// key.hotbar.2.
        OptionKeyKeyHotbar2 = "Hotbar slot 2",
        /// key.hotbar.3.
        OptionKeyKeyHotbar3 = "Hotbar slot 3",
        /// key.hotbar.4.
        OptionKeyKeyHotbar4 = "Hotbar slot 4",
        /// key.hotbar.5.
        OptionKeyKeyHotbar5 = "Hotbar slot 5",
        /// key.hotbar.6.
        OptionKeyKeyHotbar6 = "Hotbar slot 6",
        /// key.hotbar.7.
        OptionKeyKeyHotbar7 = "Hotbar slot 7",
        /// key.hotbar.8.
        OptionKeyKeyHotbar8 = "Hotbar slot 8",
        /// key.hotbar.9.
        OptionKeyKeyHotbar9 = "Hotbar slot 9",
        /// Launcher-specific wording.
        OptionKeyKeyStreamStartStop = "Start/Stop stream",
        /// Launcher-specific wording.
        OptionKeyKeyStreamPauseUnpause = "Pause/Unpause stream",
        /// Launcher-specific wording.
        OptionKeyKeyStreamCommercial = "Show stream commercials",
        /// Launcher-specific wording.
        OptionKeyKeyStreamToggleMic = "Push to talk/mute",
        /// key.debug.overlay.
        OptionKeyKeyDebugOverlay = "Debug overlay",
        /// key.debug.modifier.
        OptionKeyKeyDebugModifier = "Debug modifier",
        /// options.fov.
        OptionFov = "FOV",
        /// options.renderDistance.
        OptionRenderDistance = "Render distance",
        /// options.simulationDistance.
        OptionSimulationDistance = "Simulation distance",
        /// options.guiScale.
        OptionGuiScale = "GUI scale",
        /// options.gamma.
        OptionGamma = "Brightness",
        /// options.fullscreen.
        OptionFullscreen = "Fullscreen",
        /// options.vsync.
        OptionEnableVsync = "VSync",
        /// options.framerateLimit.
        OptionMaxFps = "Max framerate",
        /// options.particles.
        OptionParticles = "Particles",
        /// options.graphics.
        OptionGraphicsPreset = "Graphics",
        /// options.ao.
        OptionAo = "Smooth lighting",
        /// options.viewBobbing.
        OptionBobView = "View bobbing",
        /// options.entityShadows.
        OptionEntityShadows = "Entity shadows",
        /// options.mipmapLevels.
        OptionMipmapLevels = "Mipmap levels",
        /// options.biomeBlendRadius.
        OptionBiomeBlendRadius = "Biome blend",
        /// options.entityDistanceScaling.
        OptionEntityDistanceScaling = "Entity distance",
        /// options.renderCloudsDistance.
        OptionCloudRange = "Cloud distance",
        /// options.improvedTransparency.
        OptionImprovedTransparency = "Improved transparency",
        /// options.cutoutLeaves.
        OptionCutoutLeaves = "Cutout leaves",
        /// options.vignette.
        OptionVignette = "Vignette",
        /// options.weatherRadius.
        OptionWeatherRadius = "Weather radius",
        /// options.chunkFade.
        OptionChunkSectionFadeInTime = "Chunk fade-in time",
        /// options.textureFiltering.
        OptionTextureFiltering = "Texture filtering",
        /// options.maxAnisotropy.
        OptionMaxAnisotropyBit = "Anisotropic filtering",
        /// soundCategory.master.
        OptionSoundCategoryMaster = "Master volume",
        /// soundCategory.music.
        OptionSoundCategoryMusic = "Music",
        /// soundCategory.record.
        OptionSoundCategoryRecord = "Jukebox/Note blocks",
        /// soundCategory.weather.
        OptionSoundCategoryWeather = "Weather",
        /// soundCategory.block.
        OptionSoundCategoryBlock = "Blocks",
        /// soundCategory.hostile.
        OptionSoundCategoryHostile = "Hostile creatures",
        /// soundCategory.neutral.
        OptionSoundCategoryNeutral = "Friendly creatures",
        /// soundCategory.player.
        OptionSoundCategoryPlayer = "Players",
        /// soundCategory.ambient.
        OptionSoundCategoryAmbient = "Ambient/Environment",
        /// soundCategory.voice.
        OptionSoundCategoryVoice = "Voice/Speech",
        /// soundCategory.ui.
        OptionSoundCategoryUi = "Interface",
        /// options.showSubtitles.
        OptionShowSubtitles = "Show subtitles",
        /// options.directionalAudio.
        OptionDirectionalAudio = "Directional audio",
        /// options.narrator.
        OptionNarrator = "Narrator",
        /// options.accessibility.high_contrast.
        OptionHighContrast = "High contrast",
        /// options.accessibility.high_contrast_block_outline.
        OptionHighContrastBlockOutline = "High contrast block outline",
        /// options.accessibility.text_background_opacity.
        OptionTextBackgroundOpacity = "Text background opacity",
        /// Launcher-specific wording.
        OptionBackgroundForChatOnly = "Text background for chat only",
        /// options.chat.visibility.
        OptionChatVisibility = "Chat",
        /// options.chat.color.
        OptionChatColors = "Chat colors",
        /// options.chat.links.
        OptionChatLinks = "Web links",
        /// options.chat.links.prompt.
        OptionChatLinksPrompt = "Prompt on links",
        /// options.chat.opacity.
        OptionChatOpacity = "Chat text opacity",
        /// options.chat.scale.
        OptionChatScale = "Chat text size",
        /// options.chat.width.
        OptionChatWidth = "Chat width",
        /// options.chat.height.focused.
        OptionChatHeightFocused = "Chat height (focused)",
        /// options.chat.height.unfocused.
        OptionChatHeightUnfocused = "Chat height (unfocused)",
        /// options.chat.line_spacing.
        OptionChatLineSpacing = "Chat line spacing",
        /// Launcher-specific wording.
        OptionChatDelay = "Chat delay",
        /// options.autoSuggestCommands.
        OptionAutoSuggestions = "Command suggestions",
        /// options.hideMatchedNames.
        OptionHideMatchedNames = "Hide matched names",
        /// options.onlyShowSecureChat.
        OptionOnlyShowSecureChat = "Only show secure chat",
        /// options.chat.drafts.
        OptionSaveChatDrafts = "Save chat drafts",
        /// options.notifications.display_time.
        OptionNotificationDisplayTime = "Notification time",
        /// options.screenEffectScale.
        OptionScreenEffectScale = "Distortion effects",
        /// options.fovEffectScale.
        OptionFovEffectScale = "FOV effects",
        /// options.darknessEffectScale.
        OptionDarknessEffectScale = "Darkness pulsing",
        /// options.damageTiltStrength.
        OptionDamageTiltStrength = "Damage tilt",
        /// options.glintSpeed.
        OptionGlintSpeed = "Glint speed",
        /// options.glintStrength.
        OptionGlintStrength = "Glint strength",
        /// options.hideLightningFlashes.
        OptionHideLightningFlashes = "Hide lightning flashes",
        /// options.darkMojangStudiosBackgroundColor.
        OptionDarkMojangStudiosBackground = "Monochrome logo",
        /// options.accessibility.menu_background_blurriness.
        OptionMenuBackgroundBlurriness = "Menu background blur",
        /// options.accessibility.panorama_speed.
        OptionPanoramaScrollSpeed = "Panorama scroll speed",
        /// options.hideSplashTexts.
        OptionHideSplashTexts = "Hide splash texts",
        /// options.accessibility.narrator_hotkey.
        OptionNarratorHotkey = "Narrator hotkey",
        /// Launcher-specific wording.
        OptionOnboardAccessibility = "Accessibility screen on first start",
        /// options.rotateWithMinecart.
        OptionRotateWithMinecart = "Rotate with minecarts",
        /// options.allowCursorChanges.
        OptionAllowCursorChanges = "Allow cursor changes",
        /// options.language.
        OptionLang = "Language",
        /// options.forceUnicodeFont.
        OptionForceUnicodeFont = "Force Unicode font",
        /// options.japaneseGlyphVariants.
        OptionJapaneseGlyphVariants = "Japanese glyph variants",
        /// key.keyboard.ac.back.
        BindingKeyKeyboardAcBack = "AC Back",
        /// key.keyboard.ac.bookmarks.
        BindingKeyKeyboardAcBookmarks = "AC Favorites",
        /// key.keyboard.ac.close.
        BindingKeyKeyboardAcClose = "AC Close",
        /// key.keyboard.ac.exit.
        BindingKeyKeyboardAcExit = "AC Exit",
        /// key.keyboard.ac.forward.
        BindingKeyKeyboardAcForward = "AC Forward",
        /// key.keyboard.ac.home.
        BindingKeyKeyboardAcHome = "AC Home",
        /// key.keyboard.ac.new.
        BindingKeyKeyboardAcNew = "AC New",
        /// key.keyboard.ac.open.
        BindingKeyKeyboardAcOpen = "AC Open",
        /// key.keyboard.ac.print.
        BindingKeyKeyboardAcPrint = "AC Print",
        /// key.keyboard.ac.properties.
        BindingKeyKeyboardAcProperties = "AC Properties",
        /// key.keyboard.ac.refresh.
        BindingKeyKeyboardAcRefresh = "AC Refresh",
        /// key.keyboard.ac.save.
        BindingKeyKeyboardAcSave = "AC Save",
        /// key.keyboard.ac.search.
        BindingKeyKeyboardAcSearch = "AC Search",
        /// key.keyboard.ac.stop.
        BindingKeyKeyboardAcStop = "AC Stop",
        /// key.keyboard.again.
        BindingKeyKeyboardAgain = "Again",
        /// key.keyboard.alternate.erase.
        BindingKeyKeyboardAlternateErase = "Alternate Erase",
        /// key.keyboard.apostrophe.
        BindingKeyKeyboardApostrophe = "'",
        /// key.keyboard.application.
        BindingKeyKeyboardApplication = "Application",
        /// key.keyboard.backslash.
        BindingKeyKeyboardBackslash = "\\",
        /// key.keyboard.backspace.
        BindingKeyKeyboardBackspace = "Backspace",
        /// key.keyboard.call.
        BindingKeyKeyboardCall = "Call",
        /// key.keyboard.cancel.
        BindingKeyKeyboardCancel = "Cancel",
        /// key.keyboard.caps.lock.
        BindingKeyKeyboardCapsLock = "Caps Lock",
        /// key.keyboard.channel.down.
        BindingKeyKeyboardChannelDown = "Channel Down",
        /// key.keyboard.channel.up.
        BindingKeyKeyboardChannelUp = "Channel Up",
        /// key.keyboard.clear.
        BindingKeyKeyboardClear = "Clear",
        /// key.keyboard.clear.again.
        BindingKeyKeyboardClearAgain = "Clear Again",
        /// key.keyboard.comma.
        BindingKeyKeyboardComma = ",",
        /// key.keyboard.copy.
        BindingKeyKeyboardCopy = "Copy",
        /// key.keyboard.crsel.
        BindingKeyKeyboardCrsel = "CrSel",
        /// key.keyboard.currency.subunit.
        BindingKeyKeyboardCurrencySubunit = "Currency Subunit",
        /// key.keyboard.currency.unit.
        BindingKeyKeyboardCurrencyUnit = "Currency Unit",
        /// key.keyboard.cut.
        BindingKeyKeyboardCut = "Cut",
        /// key.keyboard.decimal.separator.
        BindingKeyKeyboardDecimalSeparator = "Decimal Separator",
        /// key.keyboard.delete.
        BindingKeyKeyboardDelete = "Delete",
        /// key.keyboard.down.
        BindingKeyKeyboardDown = "Down Arrow",
        /// key.keyboard.end.
        BindingKeyKeyboardEnd = "End",
        /// key.keyboard.end.call.
        BindingKeyKeyboardEndCall = "End Call",
        /// key.keyboard.enter.
        BindingKeyKeyboardEnter = "Enter",
        /// key.keyboard.enter2.
        BindingKeyKeyboardEnter2 = "Enter 2",
        /// key.keyboard.equal.
        BindingKeyKeyboardEqual = "=",
        /// key.keyboard.escape.
        BindingKeyKeyboardEscape = "Escape",
        /// key.keyboard.execute.
        BindingKeyKeyboardExecute = "Execute",
        /// key.keyboard.exsel.
        BindingKeyKeyboardExsel = "ExSel",
        /// key.keyboard.f1.
        BindingKeyKeyboardF1 = "F1",
        /// key.keyboard.f10.
        BindingKeyKeyboardF10 = "F10",
        /// key.keyboard.f11.
        BindingKeyKeyboardF11 = "F11",
        /// key.keyboard.f12.
        BindingKeyKeyboardF12 = "F12",
        /// key.keyboard.f13.
        BindingKeyKeyboardF13 = "F13",
        /// key.keyboard.f14.
        BindingKeyKeyboardF14 = "F14",
        /// key.keyboard.f15.
        BindingKeyKeyboardF15 = "F15",
        /// key.keyboard.f16.
        BindingKeyKeyboardF16 = "F16",
        /// key.keyboard.f17.
        BindingKeyKeyboardF17 = "F17",
        /// key.keyboard.f18.
        BindingKeyKeyboardF18 = "F18",
        /// key.keyboard.f19.
        BindingKeyKeyboardF19 = "F19",
        /// key.keyboard.f2.
        BindingKeyKeyboardF2 = "F2",
        /// key.keyboard.f20.
        BindingKeyKeyboardF20 = "F20",
        /// key.keyboard.f21.
        BindingKeyKeyboardF21 = "F21",
        /// key.keyboard.f22.
        BindingKeyKeyboardF22 = "F22",
        /// key.keyboard.f23.
        BindingKeyKeyboardF23 = "F23",
        /// key.keyboard.f24.
        BindingKeyKeyboardF24 = "F24",
        /// key.keyboard.f25.
        BindingKeyKeyboardF25 = "F25",
        /// key.keyboard.f3.
        BindingKeyKeyboardF3 = "F3",
        /// key.keyboard.f4.
        BindingKeyKeyboardF4 = "F4",
        /// key.keyboard.f5.
        BindingKeyKeyboardF5 = "F5",
        /// key.keyboard.f6.
        BindingKeyKeyboardF6 = "F6",
        /// key.keyboard.f7.
        BindingKeyKeyboardF7 = "F7",
        /// key.keyboard.f8.
        BindingKeyKeyboardF8 = "F8",
        /// key.keyboard.f9.
        BindingKeyKeyboardF9 = "F9",
        /// key.keyboard.find.
        BindingKeyKeyboardFind = "Find",
        /// key.keyboard.grave.accent.
        BindingKeyKeyboardGraveAccent = "`",
        /// key.keyboard.help.
        BindingKeyKeyboardHelp = "Help",
        /// key.keyboard.home.
        BindingKeyKeyboardHome = "Home",
        /// key.keyboard.insert.
        BindingKeyKeyboardInsert = "Insert",
        /// key.keyboard.international1.
        BindingKeyKeyboardInternational1 = "International 1",
        /// key.keyboard.international2.
        BindingKeyKeyboardInternational2 = "International 2",
        /// key.keyboard.international3.
        BindingKeyKeyboardInternational3 = "International 3",
        /// key.keyboard.international4.
        BindingKeyKeyboardInternational4 = "International 4",
        /// key.keyboard.international5.
        BindingKeyKeyboardInternational5 = "International 5",
        /// key.keyboard.international6.
        BindingKeyKeyboardInternational6 = "International 6",
        /// key.keyboard.international7.
        BindingKeyKeyboardInternational7 = "International 7",
        /// key.keyboard.international8.
        BindingKeyKeyboardInternational8 = "International 8",
        /// key.keyboard.international9.
        BindingKeyKeyboardInternational9 = "International 9",
        /// key.keyboard.keypad.0.
        BindingKeyKeyboardKeypad0 = "Keypad 0",
        /// key.keyboard.keypad.00.
        BindingKeyKeyboardKeypad00 = "Keypad 00",
        /// key.keyboard.keypad.000.
        BindingKeyKeyboardKeypad000 = "Keypad 000",
        /// key.keyboard.keypad.1.
        BindingKeyKeyboardKeypad1 = "Keypad 1",
        /// key.keyboard.keypad.2.
        BindingKeyKeyboardKeypad2 = "Keypad 2",
        /// key.keyboard.keypad.3.
        BindingKeyKeyboardKeypad3 = "Keypad 3",
        /// key.keyboard.keypad.4.
        BindingKeyKeyboardKeypad4 = "Keypad 4",
        /// key.keyboard.keypad.5.
        BindingKeyKeyboardKeypad5 = "Keypad 5",
        /// key.keyboard.keypad.6.
        BindingKeyKeyboardKeypad6 = "Keypad 6",
        /// key.keyboard.keypad.7.
        BindingKeyKeyboardKeypad7 = "Keypad 7",
        /// key.keyboard.keypad.8.
        BindingKeyKeyboardKeypad8 = "Keypad 8",
        /// key.keyboard.keypad.9.
        BindingKeyKeyboardKeypad9 = "Keypad 9",
        /// key.keyboard.keypad.a.
        BindingKeyKeyboardKeypadA = "Keypad A",
        /// key.keyboard.keypad.add.
        BindingKeyKeyboardKeypadAdd = "Keypad +",
        /// key.keyboard.keypad.ampersand.
        BindingKeyKeyboardKeypadAmpersand = "Keypad &",
        /// key.keyboard.keypad.at.
        BindingKeyKeyboardKeypadAt = "Keypad @",
        /// key.keyboard.keypad.b.
        BindingKeyKeyboardKeypadB = "Keypad B",
        /// key.keyboard.keypad.backspace.
        BindingKeyKeyboardKeypadBackspace = "Keypad Backspace",
        /// key.keyboard.keypad.binary.
        BindingKeyKeyboardKeypadBinary = "Keypad Binary",
        /// key.keyboard.keypad.c.
        BindingKeyKeyboardKeypadC = "Keypad C",
        /// key.keyboard.keypad.clear.
        BindingKeyKeyboardKeypadClear = "Keypad Clear",
        /// key.keyboard.keypad.clear.entry.
        BindingKeyKeyboardKeypadClearEntry = "Keypad Clear Entry",
        /// key.keyboard.keypad.colon.
        BindingKeyKeyboardKeypadColon = "Keypad :",
        /// key.keyboard.keypad.comma.
        BindingKeyKeyboardKeypadComma = "Keypad ,",
        /// key.keyboard.keypad.d.
        BindingKeyKeyboardKeypadD = "Keypad D",
        /// key.keyboard.keypad.decimal.
        BindingKeyKeyboardKeypadDecimal = "Keypad Decimal",
        /// key.keyboard.keypad.divide.
        BindingKeyKeyboardKeypadDivide = "Keypad /",
        /// key.keyboard.keypad.double.ampersand.
        BindingKeyKeyboardKeypadDoubleAmpersand = "Keypad &&",
        /// key.keyboard.keypad.double.vertical.bar.
        BindingKeyKeyboardKeypadDoubleVerticalBar = "Keypad ||",
        /// key.keyboard.keypad.e.
        BindingKeyKeyboardKeypadE = "Keypad E",
        /// key.keyboard.keypad.enter.
        BindingKeyKeyboardKeypadEnter = "Keypad Enter",
        /// key.keyboard.keypad.equal.
        BindingKeyKeyboardKeypadEqual = "Keypad =",
        /// key.keyboard.keypad.equals.as400.
        BindingKeyKeyboardKeypadEqualsAs400 = "Keypad = (AS400)",
        /// key.keyboard.keypad.exclamation.
        BindingKeyKeyboardKeypadExclamation = "Keypad !",
        /// key.keyboard.keypad.f.
        BindingKeyKeyboardKeypadF = "Keypad F",
        /// key.keyboard.keypad.greater.
        BindingKeyKeyboardKeypadGreater = "Keypad >",
        /// key.keyboard.keypad.hash.
        BindingKeyKeyboardKeypadHash = "Keypad #",
        /// key.keyboard.keypad.hexadecimal.
        BindingKeyKeyboardKeypadHexadecimal = "Keypad Hexadecimal",
        /// key.keyboard.keypad.left.brace.
        BindingKeyKeyboardKeypadLeftBrace = "Keypad {",
        /// key.keyboard.keypad.left.parenthesis.
        BindingKeyKeyboardKeypadLeftParenthesis = "Keypad (",
        /// key.keyboard.keypad.less.
        BindingKeyKeyboardKeypadLess = "Keypad <",
        /// key.keyboard.keypad.memory.add.
        BindingKeyKeyboardKeypadMemoryAdd = "Keypad M+",
        /// key.keyboard.keypad.memory.clear.
        BindingKeyKeyboardKeypadMemoryClear = "Keypad MC",
        /// key.keyboard.keypad.memory.divide.
        BindingKeyKeyboardKeypadMemoryDivide = "Keypad M/",
        /// key.keyboard.keypad.memory.multiply.
        BindingKeyKeyboardKeypadMemoryMultiply = "Keypad M*",
        /// key.keyboard.keypad.memory.recall.
        BindingKeyKeyboardKeypadMemoryRecall = "Keypad MR",
        /// key.keyboard.keypad.memory.store.
        BindingKeyKeyboardKeypadMemoryStore = "Keypad MS",
        /// key.keyboard.keypad.memory.subtract.
        BindingKeyKeyboardKeypadMemorySubtract = "Keypad M-",
        /// key.keyboard.keypad.multiply.
        BindingKeyKeyboardKeypadMultiply = "Keypad *",
        /// key.keyboard.keypad.octal.
        BindingKeyKeyboardKeypadOctal = "Keypad Octal",
        /// key.keyboard.keypad.percent.
        BindingKeyKeyboardKeypadPercent = "Keypad %",
        /// key.keyboard.keypad.period.
        BindingKeyKeyboardKeypadPeriod = "Keypad .",
        /// key.keyboard.keypad.plus.minus.
        BindingKeyKeyboardKeypadPlusMinus = "Keypad ±",
        /// key.keyboard.keypad.power.
        BindingKeyKeyboardKeypadPower = "Keypad ^",
        /// key.keyboard.keypad.right.brace.
        BindingKeyKeyboardKeypadRightBrace = "Keypad }",
        /// key.keyboard.keypad.right.parenthesis.
        BindingKeyKeyboardKeypadRightParenthesis = "Keypad )",
        /// key.keyboard.keypad.space.
        BindingKeyKeyboardKeypadSpace = "Keypad Space",
        /// key.keyboard.keypad.subtract.
        BindingKeyKeyboardKeypadSubtract = "Keypad -",
        /// key.keyboard.keypad.tab.
        BindingKeyKeyboardKeypadTab = "Keypad Tab",
        /// key.keyboard.keypad.vertical.bar.
        BindingKeyKeyboardKeypadVerticalBar = "Keypad |",
        /// key.keyboard.keypad.xor.
        BindingKeyKeyboardKeypadXor = "Keypad XOR",
        /// key.keyboard.lang1.
        BindingKeyKeyboardLang1 = "Language 1",
        /// key.keyboard.lang2.
        BindingKeyKeyboardLang2 = "Language 2",
        /// key.keyboard.lang3.
        BindingKeyKeyboardLang3 = "Language 3",
        /// key.keyboard.lang4.
        BindingKeyKeyboardLang4 = "Language 4",
        /// key.keyboard.lang5.
        BindingKeyKeyboardLang5 = "Language 5",
        /// key.keyboard.lang6.
        BindingKeyKeyboardLang6 = "Language 6",
        /// key.keyboard.lang7.
        BindingKeyKeyboardLang7 = "Language 7",
        /// key.keyboard.lang8.
        BindingKeyKeyboardLang8 = "Language 8",
        /// key.keyboard.lang9.
        BindingKeyKeyboardLang9 = "Language 9",
        /// key.keyboard.left.
        BindingKeyKeyboardLeft = "Left Arrow",
        /// key.keyboard.left.alt.
        BindingKeyKeyboardLeftAlt = "Left Alt",
        /// key.keyboard.left.bracket.
        BindingKeyKeyboardLeftBracket = "[",
        /// key.keyboard.left.command.
        BindingKeyKeyboardLeftCommand = "Left Command",
        /// key.keyboard.left.control.
        BindingKeyKeyboardLeftControl = "Left Control",
        /// key.keyboard.left.meta.
        BindingKeyKeyboardLeftMeta = "Left Meta",
        /// key.keyboard.left.option.
        BindingKeyKeyboardLeftOption = "Left Option",
        /// key.keyboard.left.shift.
        BindingKeyKeyboardLeftShift = "Left Shift",
        /// key.keyboard.left.win.
        BindingKeyKeyboardLeftWin = "Left Win",
        /// key.keyboard.left.windows.
        BindingKeyKeyboardLeftWindows = "Left Windows",
        /// key.keyboard.media.eject.
        BindingKeyKeyboardMediaEject = "Eject",
        /// key.keyboard.media.fast.forward.
        BindingKeyKeyboardMediaFastForward = "Fast Forward",
        /// key.keyboard.media.next.track.
        BindingKeyKeyboardMediaNextTrack = "Next Track",
        /// key.keyboard.media.pause.
        BindingKeyKeyboardMediaPause = "Media Pause",
        /// key.keyboard.media.play.
        BindingKeyKeyboardMediaPlay = "Media Play",
        /// key.keyboard.media.play.pause.
        BindingKeyKeyboardMediaPlayPause = "Play/Pause",
        /// key.keyboard.media.previous.track.
        BindingKeyKeyboardMediaPreviousTrack = "Previous Track",
        /// key.keyboard.media.record.
        BindingKeyKeyboardMediaRecord = "Record",
        /// key.keyboard.media.rewind.
        BindingKeyKeyboardMediaRewind = "Rewind",
        /// key.keyboard.media.select.
        BindingKeyKeyboardMediaSelect = "Media Select",
        /// key.keyboard.media.stop.
        BindingKeyKeyboardMediaStop = "Media Stop",
        /// key.keyboard.menu.
        BindingKeyKeyboardMenu = "Menu",
        /// key.keyboard.minus.
        BindingKeyKeyboardMinus = "-",
        /// key.keyboard.mode.
        BindingKeyKeyboardMode = "Mode",
        /// key.keyboard.mute.
        BindingKeyKeyboardMute = "Mute",
        /// key.keyboard.new.
        BindingKeyKeyboardNew = "New",
        /// key.keyboard.num.lock.
        BindingKeyKeyboardNumLock = "Num Lock",
        /// key.keyboard.oper.
        BindingKeyKeyboardOper = "Oper",
        /// key.keyboard.out.
        BindingKeyKeyboardOut = "Out",
        /// key.keyboard.page.down.
        BindingKeyKeyboardPageDown = "Page Down",
        /// key.keyboard.page.up.
        BindingKeyKeyboardPageUp = "Page Up",
        /// key.keyboard.paste.
        BindingKeyKeyboardPaste = "Paste",
        /// key.keyboard.pause.
        BindingKeyKeyboardPause = "Pause",
        /// key.keyboard.period.
        BindingKeyKeyboardPeriod = ".",
        /// key.keyboard.power.
        BindingKeyKeyboardPower = "Power",
        /// key.keyboard.print.screen.
        BindingKeyKeyboardPrintScreen = "Print Screen",
        /// key.keyboard.prior.
        BindingKeyKeyboardPrior = "Prior",
        /// key.keyboard.right.
        BindingKeyKeyboardRight = "Right Arrow",
        /// key.keyboard.right.alt.
        BindingKeyKeyboardRightAlt = "Right Alt",
        /// key.keyboard.right.bracket.
        BindingKeyKeyboardRightBracket = "]",
        /// key.keyboard.right.command.
        BindingKeyKeyboardRightCommand = "Right Command",
        /// key.keyboard.right.control.
        BindingKeyKeyboardRightControl = "Right Control",
        /// key.keyboard.right.meta.
        BindingKeyKeyboardRightMeta = "Right Meta",
        /// key.keyboard.right.option.
        BindingKeyKeyboardRightOption = "Right Option",
        /// key.keyboard.right.shift.
        BindingKeyKeyboardRightShift = "Right Shift",
        /// key.keyboard.right.win.
        BindingKeyKeyboardRightWin = "Right Win",
        /// key.keyboard.right.windows.
        BindingKeyKeyboardRightWindows = "Right Windows",
        /// key.keyboard.save.
        BindingKeyKeyboardSave = "Save",
        /// key.keyboard.scroll.lock.
        BindingKeyKeyboardScrollLock = "Scroll Lock",
        /// key.keyboard.select.
        BindingKeyKeyboardSelect = "Select",
        /// key.keyboard.semicolon.
        BindingKeyKeyboardSemicolon = ";",
        /// key.keyboard.separator.
        BindingKeyKeyboardSeparator = "Separator",
        /// key.keyboard.slash.
        BindingKeyKeyboardSlash = "/",
        /// key.keyboard.sleep.
        BindingKeyKeyboardSleep = "Sleep",
        /// key.keyboard.soft.left.
        BindingKeyKeyboardSoftLeft = "Soft Left",
        /// key.keyboard.soft.right.
        BindingKeyKeyboardSoftRight = "Soft Right",
        /// key.keyboard.space.
        BindingKeyKeyboardSpace = "Space",
        /// key.keyboard.stop.
        BindingKeyKeyboardStop = "Stop",
        /// key.keyboard.sys.req.
        BindingKeyKeyboardSysReq = "SysRq",
        /// key.keyboard.tab.
        BindingKeyKeyboardTab = "Tab",
        /// key.keyboard.thousands.separator.
        BindingKeyKeyboardThousandsSeparator = "Thousands Separator",
        /// key.keyboard.undo.
        BindingKeyKeyboardUndo = "Undo",
        /// key.keyboard.unknown.
        BindingKeyKeyboardUnknown = "Not Bound",
        /// key.keyboard.up.
        BindingKeyKeyboardUp = "Up Arrow",
        /// key.keyboard.volume.down.
        BindingKeyKeyboardVolumeDown = "Volume Down",
        /// key.keyboard.volume.up.
        BindingKeyKeyboardVolumeUp = "Volume Up",
        /// key.keyboard.wake.
        BindingKeyKeyboardWake = "Wake",
        /// key.keyboard.world.1.
        BindingKeyKeyboardWorld1 = "World 1",
        /// key.keyboard.world.2.
        BindingKeyKeyboardWorld2 = "World 2",
        /// key.mouse.left.
        BindingKeyMouseLeft = "Left Button",
        /// key.mouse.right.
        BindingKeyMouseRight = "Right Button",
        /// key.mouse.middle.
        BindingKeyMouseMiddle = "Middle Button",
        /// key.mouse.
        BindingMouse = "Button {number}",
        /// options.on.
        ValueOn = "ON",
        /// options.off.
        ValueOff = "OFF",
        /// options.graphics.fast.
        ValueFast = "Fast",
        /// options.graphics.fancy.
        ValueFancy = "Fancy",
        /// options.graphics.fabulous.
        ValueFabulous = "Fabulous!",
        /// options.graphics.custom.
        ValueCustom = "Custom",
        /// options.guiScale.auto.
        ValueAuto = "Auto",
        /// options.framerateLimit.max.
        ValueUnlimited = "Unlimited",
        /// options.particles.all.
        ValueAll = "All",
        /// options.particles.decreased.
        ValueDecreased = "Decreased",
        /// options.particles.minimal.
        ValueMinimal = "Minimal",
        /// options.narrator.all.
        ValueNarratorAll = "Narrates All",
        /// options.narrator.chat.
        ValueNarratorChat = "Narrates Chat",
        /// options.narrator.system.
        ValueNarratorSystem = "Narrates System",
        /// options.chat.visibility.full.
        ValueShown = "Shown",
        /// options.chat.visibility.system.
        ValueCommands = "Commands Only",
        /// options.chat.visibility.hidden.
        ValueHidden = "Hidden",
        /// options.textureFiltering.none.
        ValueNone = "None",
        /// options.textureFiltering.rgss.
        ValueRgss = "RGSS",
        /// options.textureFiltering.anisotropic.
        ValueAnisotropic = "Anisotropic",
    }
    /// Numeric shared-setting values.
    pub enum SharedOptionsPlural {
        /// Render or simulation distance.
        Chunks = ("{count} chunk", "{count} chunks"),
    }
}
